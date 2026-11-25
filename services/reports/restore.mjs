import { isInstallationId, pruneExpiredUsageDays } from "./usage.mjs";

export async function prepareRestoredReports(db, erasedInstallationIds, now = Date.now()) {
  if (!Array.isArray(erasedInstallationIds) || !Number.isSafeInteger(now) || now < 0 || now >= Date.UTC(10000, 0, 1)) {
    throw new TypeError("invalid_restore_input");
  }
  const ids = [...new Set(erasedInstallationIds)];
  if (!ids.every(isInstallationId)) throw new TypeError("invalid_restore_input");

  if (ids.length) {
    // Remove reports and their identifying rate rows in one statement.
    await db.query(
      "WITH erased AS (DELETE FROM reports.usage_days WHERE installation_id = ANY($1::uuid[])) " +
      "DELETE FROM reports.usage_rate_limits WHERE installation_id = ANY($1::uuid[])",
      [ids],
    );
  }
  return pruneExpiredUsageDays(db, now);
}
