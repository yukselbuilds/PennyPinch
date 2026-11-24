import { authorizeAdmin } from "./admin-access.mjs";
import { renderAdminPanel } from "./admin-panel.mjs";
import { isInstallationId, maxUsageReportBytes, saveUsageDay, summarizeUsageDays } from "./usage.mjs";

const maxPilotCredentials = 32;
const secondsPerDay = 86400;
const reportStatuses = { stored: 201, duplicate: 200, invalid: 400, forbidden: 403, conflict: 409 };

function reply(status, code, headers, details) {
  return Response.json({ status, ...details }, {
    status: code,
    headers: { "cache-control": "no-store", "x-content-type-options": "nosniff", ...headers },
  });
}

function pilotCredentials(secret) {
  if (typeof secret !== "string" || secret.length > 4096) throw new Error("credentials_unavailable");
  const records = JSON.parse(secret);
  if (!records || typeof records !== "object" || Array.isArray(records)) throw new Error("credentials_unavailable");
  const entries = Object.entries(records);
  if (entries.length === 0 || entries.length > maxPilotCredentials ||
      entries.some(([hash, id]) => !/^[a-f0-9]{64}$/.test(hash) || !isInstallationId(id))) {
    throw new Error("credentials_unavailable");
  }
  return records;
}

async function tokenDigest(token) {
  const bytes = await crypto.subtle.digest("SHA-256", new TextEncoder().encode(token));
  return Array.from(new Uint8Array(bytes), (byte) => byte.toString(16).padStart(2, "0")).join("");
}

async function readReport(request) {
  if (!request.body) return "";
  const reader = request.body.getReader();
  const bytes = new Uint8Array(maxUsageReportBytes);
  let length = 0;
  try {
    for (;;) {
      const { done, value } = await reader.read();
      if (done) break;
      if (length + value.byteLength > bytes.length) {
        void reader.cancel().catch(() => {});
        return null;
      }
      bytes.set(value, length);
      length += value.byteLength;
    }
    return new TextDecoder("utf-8", { fatal: true }).decode(bytes.subarray(0, length));
  } finally {
    reader.releaseLock();
  }
}

async function adminReport(request, env, url) {
  const access = await authorizeAdmin(request, env);
  if (access === "not_found") return reply(access, 404);
  if (access === "unavailable") return reply(access, 503);
  if (access === "unauthorized") return reply(access, 401);
  if (request.method !== "GET") return reply("method_not_allowed", 405, { allow: "GET" });
  const range = /^\?days=(1|7|30)$/.exec(url.search);
  if (url.search && !range) return reply("invalid", 400);
  const days = Number(range?.[1] ?? 7);
  let summary = null;
  try {
    if (env.REPORTS_DB) summary = await summarizeUsageDays(env.REPORTS_DB, days);
  } catch {
    // Keep database errors out of both responses.
  }
  if (url.pathname === "/admin") return renderAdminPanel(days, summary);
  return summary ? reply("ready", 200, undefined, { summary }) : reply("unavailable", 503);
}

export default {
  async fetch(request, env) {
    const url = new URL(request.url);
    if (url.protocol !== "https:") return reply("not_found", 404);
    if (url.pathname === "/v1/admin/summary" || url.pathname === "/admin") return adminReport(request, env, url);
    if (env.REPORTS_ENABLED !== "true" || url.pathname !== "/v1/usage" || url.search) {
      return reply("not_found", 404);
    }
    if (request.method !== "POST") return reply("method_not_allowed", 405, { allow: "POST" });
    if (!env.REPORTS_DB || !env.USAGE_RATE) return reply("unavailable", 503);

    try {
      const credentials = pilotCredentials(env.PILOT_USAGE_CREDENTIALS);
      const bearer = /^Bearer ([A-Za-z0-9_-]{43})$/i.exec(request.headers.get("authorization") ?? "");
      const id = bearer ? credentials[await tokenDigest(bearer[1])] : undefined;
      if (!id) return reply("unauthorized", 401, { "www-authenticate": "Bearer" });

      const { success } = await env.USAGE_RATE.limit({ key: id });
      if (!success) return reply("rate_limited", 429, { "retry-after": "60" });
      const mediaType = request.headers.get("content-type")?.split(";", 1)[0].trim().toLowerCase();
      if (mediaType !== "application/json") return reply("unsupported_media_type", 415);
      const length = request.headers.get("content-length");
      if (length !== null && !/^\d+$/.test(length)) return reply("invalid", 400);
      if (length !== null && Number(length) > maxUsageReportBytes) return reply("too_large", 413);

      let body;
      try {
        body = await readReport(request);
      } catch {
        return reply("invalid", 400);
      }
      if (body === null) return reply("too_large", 413);

      const now = Date.now();
      const status = await saveUsageDay(env.REPORTS_DB, body, id, now);
      if (status === "daily_quota") {
        const seconds = secondsPerDay - Math.floor(now / 1000) % secondsPerDay;
        return reply(status, 429, { "retry-after": String(seconds) });
      }
      return reply(status, reportStatuses[status]);
    } catch {
      return reply("unavailable", 503);
    }
  },
};
