const dayMilliseconds = 86_400_000;
const retentionDays = 30;
const dailyWriteLimit = 128;
export const maxUsageReportBytes = 4096;

const installationId = /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;
const appVersion = /^(?:0|[1-9]\d{0,5})\.(?:0|[1-9]\d{0,5})\.(?:0|[1-9]\d{0,5})(?:-preview\.[1-9]\d{0,5})?$/;
const osFamilies = new Set(["windows", "macos", "linux"]);
const basisIds = new Set(
  ["cl100k_base", "gpt2", "headroom_estimator", "o200k_base", "p50k_base", "r50k_base"]
    .map((counter) => "headroom-0.45.0-ws-json-v1:" + counter),
);
const reportKeys = [
  "schema_version", "notice_version", "installation_id", "report_day", "os_family", "app_version",
  "engine_version", "incomparable_requests", "failed_requests", "groups",
];
const groupKeys = ["basis", "comparable_requests", "before_tokens", "after_tokens"];

function hasExactKeys(value, keys) {
  return value !== null && typeof value === "object" && !Array.isArray(value) &&
    Object.keys(value).length === keys.length && keys.every((key) => Object.hasOwn(value, key));
}

function counter(value) {
  return Number.isSafeInteger(value) && value >= 0;
}

export function isInstallationId(value) {
  return typeof value === "string" && installationId.test(value);
}

function utcDay(now) {
  return Math.floor(now / dayMilliseconds) * dayMilliseconds;
}

function dateLabel(timestamp) {
  return new Date(timestamp).toISOString().slice(0, 10);
}

function parseReport(raw, now) {
  if (typeof raw !== "string" || raw.length > maxUsageReportBytes ||
      new TextEncoder().encode(raw).length > maxUsageReportBytes) {
    return null;
  }
  let value;
  try {
    value = JSON.parse(raw);
  } catch {
    return null;
  }
  if (!hasExactKeys(value, reportKeys)) return null;
  const { schema_version, notice_version, installation_id, report_day, os_family, app_version,
    engine_version, incomparable_requests, failed_requests, groups } = value;
  if (schema_version !== 1 || notice_version !== 1 || engine_version !== "0.45.0" ||
      !isInstallationId(installation_id) ||
      typeof app_version !== "string" || !appVersion.test(app_version) || !osFamilies.has(os_family) ||
      !counter(incomparable_requests) || !counter(failed_requests) ||
      !Array.isArray(groups) || groups.length > basisIds.size) {
    return null;
  }

  if (typeof report_day !== "string" || !/^\d{4}-\d{2}-\d{2}$/.test(report_day)) return null;
  const observedDay = Date.parse(report_day + "T00:00:00.000Z");
  const today = utcDay(now);
  if (!Number.isFinite(observedDay) || dateLabel(observedDay) !== report_day ||
      observedDay < today - retentionDays * dayMilliseconds || observedDay >= today) {
    return null;
  }

  const uniqueBases = new Set();
  const orderedGroups = [];
  let totalRequests = incomparable_requests + failed_requests;
  for (const group of groups) {
    if (!hasExactKeys(group, groupKeys)) return null;
    const { basis, comparable_requests, before_tokens, after_tokens } = group;
    if (!basisIds.has(basis) || uniqueBases.has(basis) ||
        !counter(comparable_requests) || comparable_requests === 0 ||
        !counter(before_tokens) || !counter(after_tokens)) {
      return null;
    }
    uniqueBases.add(basis);
    totalRequests += comparable_requests;
    orderedGroups.push({ basis, comparable_requests, before_tokens, after_tokens });
  }
  if (!Number.isSafeInteger(totalRequests) || totalRequests === 0) return null;
  orderedGroups.sort((left, right) => left.basis.localeCompare(right.basis, "en"));
  return {
    installationId: installation_id, reportDay: report_day,
    summary: JSON.stringify({
      schema_version, notice_version, os_family, app_version, engine_version,
      incomparable_requests, failed_requests, groups: orderedGroups,
    }),
  };
}

export async function saveUsageDay(db, raw, authenticatedId, now = Date.now()) {
  const report = parseReport(raw, now);
  if (!report) return "invalid";
  if (report.installationId !== authenticatedId) return "forbidden";

  // The insert, the quota trigger, and the retry lookup share one D1 transaction.
  const receivedDay = dateLabel(utcDay(now));
  const [inserted, current] = await db.batch([
    db.prepare(
      "INSERT INTO usage_days (report_day, installation_id, summary, received_day) " +
      "SELECT ?, ?, ?, ? WHERE COALESCE((SELECT accepted_count FROM usage_intake_quota WHERE received_day = ?), 0) < ? " +
      "ON CONFLICT(report_day, installation_id) DO NOTHING RETURNING report_day",
    ).bind(report.reportDay, report.installationId, report.summary, receivedDay, receivedDay, dailyWriteLimit),
    db.prepare("SELECT summary FROM usage_days WHERE report_day = ? AND installation_id = ?")
      .bind(report.reportDay, report.installationId),
  ]);
  if (current.results.length === 0) return "daily_quota";
  if (current.results[0].summary !== report.summary) return "conflict";
  return inserted.results.length === 1 ? "stored" : "duplicate";
}

export async function pruneUsageDays(db, now = Date.now()) {
  const cutoff = dateLabel(utcDay(now) - retentionDays * dayMilliseconds);
  const [deleted] = await db.batch([
    db.prepare(
      "DELETE FROM usage_days WHERE (report_day, installation_id) IN " +
      "(SELECT report_day, installation_id FROM usage_days WHERE report_day < ? " +
      "ORDER BY report_day, installation_id LIMIT 500)",
    ).bind(cutoff),
    db.prepare("DELETE FROM usage_intake_quota WHERE received_day < ?").bind(cutoff),
  ]);
  return deleted.meta.changes;
}
