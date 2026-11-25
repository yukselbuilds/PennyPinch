import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { after, before, test } from "node:test";
import { PGlite } from "@electric-sql/pglite";
import { prepareRestoredReports } from "./restore.mjs";

const now = Date.parse("2026-11-25T15:00:00Z");
const revoked = "39c2a18b-4bc9-4a1d-a7a7-b7a5f6bc638a";
const other = "8c590959-8e75-48ad-ad20-9f9cd339de55";
const db = new PGlite();

before(async () => {
  const sql = await readFile(new URL("./migrations/0001_reports.sql", import.meta.url), "utf8");
  await db.exec(sql);
  await db.query("INSERT INTO reports.usage_days VALUES ('2026-11-24', $1, '{}'), ('2026-10-25', $2, '{}'), ('2026-10-26', $2, '{}')", [revoked, other]);
  const minute = Math.floor(now / 60_000);
  await db.query("INSERT INTO reports.usage_rate_limits VALUES ($1, $3, 2), ($2, $3, 3)", [revoked, other, minute]);
  await db.exec("INSERT INTO reports.usage_intake_quota VALUES ('2026-11-24', 2), ('2026-10-25', 1)");
});
after(() => db.close());

test("requires explicit verified erasure inputs before a restore can write", async () => {
  const mustNotWrite = { query: () => assert.fail("Unexpected database query") };
  for (const ids of [null, revoked, [revoked, "bad-id"], new Array(1)]) {
    await assert.rejects(prepareRestoredReports(mustNotWrite, ids, now), { message: "invalid_restore_input" });
  }
  await assert.rejects(prepareRestoredReports(mustNotWrite, [revoked], NaN), { message: "invalid_restore_input" });
});

test("preserves both tables on an erasure failure and safely retries the restore", async () => {
  await db.exec(`
    CREATE FUNCTION reports.fixture_reject_delete() RETURNS trigger
    LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'fixture deletion refused'; END; $$;
    CREATE TRIGGER fixture_delete BEFORE DELETE ON reports.usage_rate_limits
    FOR EACH ROW EXECUTE FUNCTION reports.fixture_reject_delete();
  `);
  await assert.rejects(prepareRestoredReports(db, [revoked], now), { code: "P0001" });
  assert.equal((await db.query("SELECT count(*)::int AS count FROM reports.usage_days")).rows[0].count, 3);
  assert.equal((await db.query("SELECT count(*)::int AS count FROM reports.usage_rate_limits")).rows[0].count, 2);
  await db.exec("DROP TRIGGER fixture_delete ON reports.usage_rate_limits");

  assert.equal(await prepareRestoredReports(db, [revoked, revoked], now), true);
  assert.equal(await prepareRestoredReports(db, [revoked], now), true);
  assert.deepEqual((await db.query("SELECT report_day::text AS day, installation_id FROM reports.usage_days")).rows,
    [{ day: "2026-10-26", installation_id: other }]);
  assert.deepEqual((await db.query("SELECT installation_id, request_count FROM reports.usage_rate_limits")).rows,
    [{ installation_id: other, request_count: 3 }]);
  assert.deepEqual((await db.query("SELECT received_day::text AS day, accepted_count FROM reports.usage_intake_quota")).rows,
    [{ day: "2026-11-24", accepted_count: 2 }]);
});
