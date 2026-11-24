import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { after, before, beforeEach, test } from "node:test";
import { PGlite } from "@electric-sql/pglite";
import retention, { createCleanupPool, handleScheduledCleanup } from "./retention-function.mjs";

const base = "https://fixture.invalid/internal/expire-usage";
const id = "trigger-retention-fixture";
const invocation = "retention-delivery-1";
const now = Date.parse("2026-11-24T15:00:00Z");
const event = {
  version: 1, invocation_id: invocation, trigger: { type: "schedule", id },
  data: { scheduled_at: "2026-11-23T04:00:00Z" },
};
const headers = { "content-type": "application/json", "x-neon-trigger-invocation-id": invocation };
const request = (init = {}, address = base) =>
  new Request(address, { method: "POST", headers, body: JSON.stringify(event), ...init });
const db = new PGlite();

before(async () => {
  for (const file of ["0001_reports.sql", "0003_reports_cleanup.sql"]) {
    await db.exec(await readFile(new URL("./migrations/" + file, import.meta.url), "utf8"));
  }
});
beforeEach(() => db.exec("TRUNCATE reports.usage_days, reports.usage_intake_quota, reports.usage_rate_limits"));
after(() => db.close());

test("keeps the deployed entrypoint closed without a trigger ID", async () => {
  const original = process.env.REPORTS_CLEANUP_TRIGGER_ID;
  delete process.env.REPORTS_CLEANUP_TRIGGER_ID;
  try {
    const { default: withoutTrigger } = await import("./retention-function.mjs?disabled");
    const response = await withoutTrigger.fetch(request());
    assert.equal(response.status, 404);
    assert.equal(response.headers.get("cache-control"), "no-store");
  } finally {
    if (original !== undefined) process.env.REPORTS_CLEANUP_TRIGGER_ID = original;
  }
  assert.equal((await retention.fetch(request({ body: undefined, method: "GET" }))).status, 404);
});

test("rejects unauthenticated and misrouted requests before storage access", async () => {
  const settings = { triggerId: id, db: { query: () => assert.fail("unexpected query") } };
  for (const req of [
    request({ headers: { "content-type": "application/json" } }),
    request({ body: undefined, method: "GET" }),
    request({}, base.replace("https:", "http:")),
    request({}, base + "?trigger=fixture"),
    request({}, "https://fixture.invalid/admin"),
  ]) {
    assert.equal((await handleScheduledCleanup(req, settings)).status, 404);
  }
});

test("requires a matching schedule envelope", async () => {
  const settings = { triggerId: id, db: { query: () => assert.fail("unexpected query") } };
  for (const value of [
    null,
    { ...event, version: 2 },
    { ...event, invocation_id: "another-delivery" },
    { ...event, trigger: { id: "another-trigger", type: "schedule" } },
    { ...event, trigger: { id, type: "storage_object_created" } },
    { ...event, data: { scheduled_at: "invalid" } },
  ]) {
    const response = await handleScheduledCleanup(request({ body: JSON.stringify(value) }), settings);
    assert.equal(response.status, 400);
  }
  assert.equal((await handleScheduledCleanup(request({ body: "}" }), settings)).status, 400);
  assert.equal((await handleScheduledCleanup(request({ headers: { ...headers, "content-type": "text/plain" } }), settings)).status, 400);
});

test("uses processing time and preserves current records under the cleanup role", async () => {
  await db.exec(`INSERT INTO reports.usage_days VALUES
    ('2026-10-24','1b93c822-ac37-4360-aa79-000000000001','{}'),
    ('2026-10-25','1b93c822-ac37-4360-aa79-000000000001','{}');
    INSERT INTO reports.usage_intake_quota VALUES ('2026-10-24',1), ('2026-10-25',1);`);
  const minute = Math.floor(now / 60_000);
  await db.query(`INSERT INTO reports.usage_rate_limits VALUES
    ('1b93c822-ac37-4360-aa79-000000000001',$1,1), ('1b93c822-ac37-4360-aa79-000000000002',$2,1)`,
  [minute - 1441, minute - 1440]);

  await db.exec("SET ROLE pennypinch_reports_cleanup");
  try {
    await assert.rejects(db.query("SELECT summary FROM reports.usage_days"), { code: "42501" });
    await assert.rejects(db.query("UPDATE reports.usage_intake_quota SET accepted_count = 2"), { code: "42501" });
    await assert.rejects(db.query("INSERT INTO reports.usage_intake_quota VALUES (current_date,1)"), { code: "42501" });
    await assert.rejects(db.query("TRUNCATE reports.usage_days"), { code: "42501" });
    assert.equal((await handleScheduledCleanup(request(), { db, triggerId: id }, now)).status, 204);
    assert.equal((await handleScheduledCleanup(request(), { db, triggerId: id }, now)).status, 204);
  } finally {
    await db.exec("RESET ROLE");
  }
  assert.deepEqual((await db.query("SELECT report_day::text AS day FROM reports.usage_days")).rows, [{ day: "2026-10-25" }]);
  assert.deepEqual((await db.query("SELECT received_day::text AS day FROM reports.usage_intake_quota")).rows, [{ day: "2026-10-25" }]);
  assert.deepEqual((await db.query("SELECT window_minute::int AS minute FROM reports.usage_rate_limits")).rows,
    [{ minute: minute - 1440 }]);
});

test("retries bounded backlog and storage failures without returning details", async () => {
  let batches = 0;
  const busy = { query: async () => { batches++; return { rows: [{ count: 500 }] }; } };
  const failure = { query: async () => { throw new Error("fixture private data"); } };
  for (const storage of [busy, failure, undefined]) {
    const response = await handleScheduledCleanup(request(), { db: storage, triggerId: id }, now);
    assert.equal(response.status, 503);
    assert.equal(response.headers.get("retry-after"), "60");
    assert.equal(await response.text(), "");
  }
  assert.equal(batches, 8);
});

test("an owner URL is rejected and URI options cannot turn off TLS verification", async () => {
  assert.equal(createCleanupPool("postgresql://neondb_owner:fixture@localhost/neondb"), undefined);
  const pool = createCleanupPool("postgresql://pennypinch_reports_cleanup:fixture@localhost/neondb?sslmode=disable");
  try {
    assert.equal(pool.options.connectionString, "postgresql://pennypinch_reports_cleanup:fixture@localhost/neondb");
    assert.deepEqual(pool.options.ssl, { rejectUnauthorized: true });
  } finally {
    await pool.end();
  }
});
