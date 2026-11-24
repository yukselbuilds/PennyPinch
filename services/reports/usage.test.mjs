import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { before, after, test } from "node:test";
import { PGlite } from "@electric-sql/pglite";
import { pruneUsageDays, saveUsageDay } from "./usage.mjs";

const now = Date.parse("2026-11-24T15:00:00Z");
const report = {
  schema_version: 1,
  notice_version: 1,
  installation_id: "125d94ba-1e98-4d64-ac76-eb56f665a527",
  report_day: "2026-11-23",
  os_family: "windows",
  app_version: "0.1.1-preview.4",
  engine_version: "0.45.0",
  incomparable_requests: 2,
  failed_requests: 1,
  groups: [
    { basis: "headroom-0.45.0-ws-json-v1:o200k_base", comparable_requests: 3, before_tokens: 10, after_tokens: 14 },
    { basis: "headroom-0.45.0-ws-json-v1:cl100k_base", comparable_requests: 1, before_tokens: 0, after_tokens: 0 },
  ],
};

const db = new PGlite();
before(async () => {
  await db.exec(await readFile(new URL("./migrations/0001_reports.sql", import.meta.url), "utf8"));
});
after(() => db.close());

test("stores negative savings, observed zeroes, and separate bases and coverage", async () => {
  assert.equal(await saveUsageDay(db, JSON.stringify(report), report.installation_id, now), "stored");
  const { rows } = await db.query("SELECT summary FROM reports.usage_days");
  assert.equal(rows.length, 1);
  const stored = JSON.parse(rows[0].summary);
  assert.equal(stored.groups.length, 2);
  assert.equal(stored.groups[0].basis, report.groups[1].basis);
  assert.equal(stored.groups[0].before_tokens, 0);
  assert.equal(stored.groups[1].before_tokens - stored.groups[1].after_tokens, -4);
  assert.equal(stored.incomparable_requests, 2);
  assert.equal(stored.failed_requests, 1);
});

test("coalesces concurrent retries and refuses a changed summary for the same day", async () => {
  const next = { ...report, report_day: "2026-11-22" };
  const reordered = { ...next, groups: [...next.groups].reverse().map(({ basis, ...counts }) => ({ ...counts, basis })) };
  const outcomes = await Promise.all([
    saveUsageDay(db, JSON.stringify(next), report.installation_id, now),
    saveUsageDay(db, JSON.stringify(reordered), report.installation_id, now),
  ]);
  assert.deepEqual(outcomes.sort(), ["duplicate", "stored"]);
  assert.equal(await saveUsageDay(db, JSON.stringify({ ...next, failed_requests: 7 }), report.installation_id, now), "conflict");
  const { rows } = await db.query("SELECT summary FROM reports.usage_days WHERE report_day = $1", [next.report_day]);
  assert.equal(JSON.parse(rows[0].summary).failed_requests, next.failed_requests);
});

test("rolls back the report when the quota write fails", async () => {
  const day = { ...report, report_day: "2026-11-21" };
  await db.exec("ALTER TABLE reports.usage_intake_quota ADD CHECK (received_day <> DATE '2026-11-25')");
  await assert.rejects(
    () => saveUsageDay(db, JSON.stringify(day), report.installation_id, Date.parse("2026-11-25T00:00:00Z")),
    { code: "23514" },
  );
  const { rows } = await db.query("SELECT count(*)::int AS count FROM reports.usage_days WHERE report_day = $1", [day.report_day]);
  assert.equal(rows[0].count, 0);
});

test("rejects unsupported fields and invalid counters before any database call", async () => {
  const unsupported = [
    { model: "private-project-model" },
    { installation_id: "other-account" },
    { app_version: "0.1.0-private-project" },
    { schema_version: 2 },
    { notice_version: 0 },
    { engine_version: "0.46.0" },
    { failed_requests: -1 },
    { incomparable_requests: 2 ** 53 },
    { report_day: "2026-11-31" },
    { report_day: "2026-11-24" },
    { report_day: "2026-10-24" },
    { groups: [report.groups[0], report.groups[0]] },
    { groups: [{ ...report.groups[0], basis: "provider_usage" }] },
    { groups: [{ ...report.groups[0], after_tokens: 1.5 }] },
    { groups: [{ ...report.groups[0], comparable_requests: 0 }] },
    { groups: [{ ...report.groups[0], tool_output: "PRIVATE_FIXTURE_CANARY" }] },
    { groups: [], incomparable_requests: 0, failed_requests: 0 },
  ];
  const input = unsupported.map((fields) => JSON.stringify({ ...report, ...fields }));
  input.push("null", "[]", "{", " ".repeat(4097));
  for (const value of input) {
    assert.equal(await saveUsageDay(undefined, value, report.installation_id, now), "invalid");
  }
});

test("stores coverage with no comparable requests", async () => {
  const day = { ...report, report_day: "2026-11-20", groups: [] };
  assert.equal(await saveUsageDay(db, JSON.stringify(day), report.installation_id, now), "stored");
  const { rows } = await db.query("SELECT summary FROM reports.usage_days WHERE report_day = $1", [day.report_day]);
  assert.deepEqual(JSON.parse(rows[0].summary).groups, []);
});

test("expires reports older than 30 UTC days in bounded batches", async () => {
  const cutoff = { ...report, report_day: "2026-10-25" };
  assert.equal(await saveUsageDay(db, JSON.stringify(cutoff), report.installation_id, now), "stored");
  await db.exec(
    "INSERT INTO reports.usage_days (report_day, installation_id, summary) " +
    "SELECT '2026-10-24', ('1b93c822-ac37-4360-aa79-' || lpad(i::text, 12, '0'))::uuid, '{}' " +
    "FROM generate_series(1, 501) AS g(i); " +
    "INSERT INTO reports.usage_intake_quota (received_day, accepted_count) VALUES ('2026-10-24', 1);",
  );
  assert.equal(await pruneUsageDays(db, now), 500);
  assert.equal(await pruneUsageDays(db, now), 1);
  assert.equal(await pruneUsageDays(db, now), 0);
  const { rows } = await db.query(
    "SELECT report_day::text AS report_day FROM reports.usage_days WHERE report_day <= $1", [cutoff.report_day],
  );
  assert.deepEqual(rows, [{ report_day: cutoff.report_day }]);
  const quota = await db.query("SELECT received_day::text AS received_day FROM reports.usage_intake_quota ORDER BY received_day");
  assert.deepEqual(quota.rows.map((row) => row.received_day), ["2026-11-24"]);
});
