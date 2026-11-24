import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { before, after, test } from "node:test";
import { Miniflare } from "miniflare";
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

const fixture = new Miniflare({
  host: "127.0.0.1",
  port: 0,
  modules: true,
  script: "export default { fetch() { return new Response(null, { status: 404 }); } };",
  d1Databases: { DB: "1f8b7278-aa59-4f4a-a2b4-9b017585eb43" },
  cf: false,
  telemetry: { enabled: false },
});
let db;
before(async () => {
  db = await fixture.getD1Database("DB");
  for (const name of ["0001_usage.sql", "0002_intake.sql"]) {
    const sql = await readFile(new URL("./migrations/" + name, import.meta.url), "utf8");
    await db.batch(sql.trim().split(/\r?\n\r?\n/).map((statement) => db.prepare(statement)));
  }
});
after(async () => fixture.dispose());

test("stores negative savings, observed zeroes, and separate bases and coverage", async () => {
  assert.equal(await saveUsageDay(db, JSON.stringify(report), report.installation_id, now), "stored");
  const { results } = await db.prepare("SELECT summary FROM usage_days").all();
  assert.equal(results.length, 1);
  const stored = JSON.parse(results[0].summary);
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
  const stored = await db.prepare("SELECT summary FROM usage_days WHERE report_day = ?").bind(next.report_day).first("summary");
  assert.equal(JSON.parse(stored).failed_requests, next.failed_requests);
});

test("leaves the day empty when the storage transaction fails", async () => {
  const day = { ...report, report_day: "2026-11-21" };
  const failedBatch = {
    prepare: (sql) => db.prepare(sql),
    batch: (statements) => db.batch([
      ...statements,
      db.prepare("INSERT INTO usage_days (report_day, installation_id, summary) VALUES ('2026-11-21', 'invalid-id', '{}')"),
    ]),
  };
  await assert.rejects(() => saveUsageDay(failedBatch, JSON.stringify(day), report.installation_id, now), /CHECK constraint failed/);
  const stored = await db.prepare("SELECT COUNT(*) AS rows FROM usage_days WHERE report_day = ?")
    .bind(day.report_day).first("rows");
  assert.equal(stored, 0);
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
  const stored = await db.prepare("SELECT summary FROM usage_days WHERE report_day = ?").bind(day.report_day).first("summary");
  assert.deepEqual(JSON.parse(stored).groups, []);
});

test("expires reports older than 30 UTC days in bounded batches", async () => {
  const cutoff = { ...report, report_day: "2026-10-25" };
  assert.equal(await saveUsageDay(db, JSON.stringify(cutoff), report.installation_id, now), "stored");
  await db.prepare(
    "WITH RECURSIVE n(i) AS (VALUES(1) UNION ALL SELECT i+1 FROM n WHERE i<501) " +
    "INSERT INTO usage_days (report_day, installation_id, summary) SELECT '2026-10-24', printf('%036d', i), '{}' FROM n",
  ).run();
  await db.prepare("INSERT INTO usage_intake_quota (received_day, accepted_count) VALUES (?, 1)")
    .bind("2026-10-24").run();
  assert.equal(await pruneUsageDays(db, now), 500);
  assert.equal(await pruneUsageDays(db, now), 1);
  assert.equal(await pruneUsageDays(db, now), 0);
  assert.equal(await db.prepare("SELECT COUNT(*) AS rows FROM usage_days WHERE report_day = ?")
    .bind(cutoff.report_day).first("rows"), 1);
  const quota = await db.prepare("SELECT received_day FROM usage_intake_quota ORDER BY received_day").all();
  assert.deepEqual(quota.results.map((row) => row.received_day), ["2026-11-24"]);
});
