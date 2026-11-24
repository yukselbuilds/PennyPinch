import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import { after, test } from "node:test";
import { PGlite } from "@electric-sql/pglite";
import neonFunction, { createReportPool } from "./function.mjs";
import { handleReportRequest } from "./service.mjs";
import { pruneExpiredUsageDays } from "./usage.mjs";

const url = "https://pennypinch.test/v1/usage";
const adminUrl = "https://pennypinch.test/v1/admin/summary";
const panelUrl = "https://pennypinch.test/admin";
const ids = ["125d94ba-1e98-4d64-ac76-eb56f665a527", "55676b9c-87d3-479d-ab92-fa0fc07a9d32"];
const tokens = ["A".repeat(43), "B".repeat(43)];
const digest = (text) => createHash("sha256").update(text).digest("hex");
const credentials = JSON.stringify(Object.fromEntries(tokens.map((token, i) => [digest(token), ids[i]])));
const now = Date.parse("2026-11-24T15:00:00Z");
const day = (offset) => new Date(now + offset * 86_400_000).toISOString().slice(0, 10);

function summary(id) {
  return {
    schema_version: 1, notice_version: 1, installation_id: id, report_day: day(-1),
    os_family: "windows", app_version: "0.1.1-preview.4", engine_version: "0.45.0",
    incomparable_requests: 1, failed_requests: 0, groups: [],
  };
}

const headers = (token) => ({
  "content-type": "application/json",
  ...(token ? { authorization: "Bearer " + token } : {}),
});
const request = (body, token) => new Request(url, { method: "POST", headers: headers(token), body: JSON.stringify(body) });
const db = new PGlite();
const migration = await readFile(new URL("./migrations/0001_reports.sql", import.meta.url), "utf8");
after(() => db.close());

async function firstValue(sql, params = []) {
  const { rows } = await db.query(sql, params);
  return Object.values(rows[0])[0];
}

async function start({ settings = {}, migrate = true } = {}) {
  await db.exec("DROP SCHEMA IF EXISTS reports CASCADE");
  if (migrate) await db.exec(migration);
  const options = { db, intakeEnabled: true, credentialDigests: credentials, ...settings };
  return {
    send: (body, token, time = now) => handleReportRequest(request(body, token), options, time),
    fetch: (address, init) => handleReportRequest(new Request(address, init), options, now),
  };
}

async function expectResponse(response, code, status) {
  assert.equal(response.status, code);
  assert.equal(response.headers.get("cache-control"), "no-store");
  assert.deepEqual(await response.json(), { status });
}

test("keeps the Neon pilot closed and refuses an owner connection URL", async () => {
  assert.equal(createReportPool(), undefined);
  assert.equal(createReportPool("postgresql://neondb_owner:fixture@localhost/neondb"), undefined);
  assert.equal(createReportPool("postgresql://pennypinch_reports_runtime:%@localhost/neondb"), undefined);
  await expectResponse(await neonFunction.fetch(request(summary(ids[0]), tokens[0])), 404, "not_found");
  await expectResponse(await neonFunction.fetch(new Request(adminUrl)), 404, "not_found");
  await expectResponse(await neonFunction.fetch(new Request(panelUrl)), 404, "not_found");
});

test("disables intake by default and reports missing storage", async () => {
  const sample = summary(ids[0]);
  await expectResponse(await handleReportRequest(request(sample, tokens[0]), {}), 404, "not_found");
  await expectResponse(await handleReportRequest(request(sample, tokens[0]), { intakeEnabled: true }), 503, "unavailable");
});

test("enforces the route, method, and the installation bound to each token", async () => {
  const { send, fetch } = await start();
  const sample = summary(ids[0]);
  const wrongMethod = await fetch(url);
  assert.equal(wrongMethod.headers.get("allow"), "POST");
  await expectResponse(wrongMethod, 405, "method_not_allowed");
  await expectResponse(await fetch(url + "?unexpected=1", { method: "POST" }), 404, "not_found");
  await expectResponse(await fetch(url.replace("https:", "http:"), { method: "POST" }), 404, "not_found");
  await expectResponse(await send(sample), 401, "unauthorized");
  await expectResponse(await send(sample, "C".repeat(43)), 401, "unauthorized");
  await expectResponse(await send(sample, tokens[1]), 403, "forbidden");
  assert.equal(await firstValue("SELECT count(*)::int FROM reports.usage_days"), 0);
  assert.equal(await firstValue("SELECT count(*)::int FROM reports.usage_intake_quota"), 0);
});

test("stores one report and preserves the original on a conflicting retry", async () => {
  const { send } = await start();
  const sample = summary(ids[0]);
  await expectResponse(await send(sample, tokens[0]), 201, "stored");
  await expectResponse(await send(sample, tokens[0]), 200, "duplicate");
  await expectResponse(await send({ ...sample, failed_requests: 1 }, tokens[0]), 409, "conflict");
  const row = await firstValue("SELECT summary FROM reports.usage_days");
  assert.equal(JSON.parse(row).failed_requests, 0);
  for (const value of [...ids, ...tokens, ...tokens.map(digest)]) assert.ok(!row.includes(value));
  assert.equal(await firstValue("SELECT accepted_count FROM reports.usage_intake_quota"), 1);
});

test("checks media type, UTF-8, and the received body size", async () => {
  const { fetch } = await start();
  const send = (body, changes = {}) => fetch(url, {
    method: "POST", headers: headers(tokens[0]), body, ...changes,
  });
  await expectResponse(await send("{}", { headers: { ...headers(tokens[0]), "content-type": "text/plain" } }),
    415, "unsupported_media_type");
  await expectResponse(await send(new Uint8Array([0xff, 0x7b, 0x7d])), 400, "invalid");
  await expectResponse(await send("x".repeat(4097)), 413, "too_large");
  const chunked = new ReadableStream({ start(controller) {
    controller.enqueue(new Uint8Array(4096).fill(32));
    controller.enqueue(new Uint8Array([32]));
    controller.close();
  } });
  await expectResponse(await send(chunked, { duplex: "half" }), 413, "too_large");
  assert.equal(await firstValue("SELECT count(*)::int FROM reports.usage_days"), 0);
});

test("limits bursts per authenticated installation", async () => {
  const { send } = await start();
  const sample = summary(ids[0]);
  await expectResponse(await send(sample, tokens[0]), 201, "stored");
  for (let i = 0; i < 5; i++) await expectResponse(await send(sample, tokens[0]), 200, "duplicate");
  const limited = await send(sample, tokens[0]);
  assert.equal(limited.headers.get("retry-after"), "60");
  await expectResponse(limited, 429, "rate_limited");
  await expectResponse(await send(summary(ids[1]), tokens[1]), 201, "stored");
  await expectResponse(await send(sample, tokens[0], now + 60_000), 200, "duplicate");
  await expectResponse(await send(sample, tokens[0], now), 429, "rate_limited");
});

test("enforces the UTC write budget across concurrent installations and preserves it on erasure", async () => {
  const { send } = await start();
  const today = day(0);
  await db.query("INSERT INTO reports.usage_intake_quota (received_day, accepted_count) VALUES ($1, 127)", [today]);
  const samples = ids.map(summary);
  const responses = await Promise.all(samples.map((sample, i) => send(sample, tokens[i])));
  assert.deepEqual(responses.map((response) => response.status).sort(), [201, 429]);
  const accepted = responses.findIndex((response) => response.status === 201);
  const rejected = 1 - accepted;
  const seconds = Number(responses[rejected].headers.get("retry-after"));
  assert.ok(seconds > 0 && seconds <= 86_400);
  await expectResponse(responses[rejected], 429, "daily_quota");
  await expectResponse(await send(samples[accepted], tokens[accepted]), 200, "duplicate");
  await db.query("DELETE FROM reports.usage_days WHERE installation_id = $1", [ids[accepted]]);
  await expectResponse(await send(samples[rejected], tokens[rejected]), 429, "daily_quota");
  assert.equal(await firstValue("SELECT accepted_count FROM reports.usage_intake_quota"), 128);
});

test("maps a PostgreSQL failure to a generic response", async () => {
  const { send } = await start({ migrate: false });
  await expectResponse(await send(summary(ids[0]), tokens[0]), 503, "unavailable");
});

const adminToken = "fixture-admin-access";
const adminOptions = { headers: { authorization: "Bearer " + adminToken } };

function startAdmin({ settings, ...options } = {}) {
  return start({ ...options, settings: {
    verifyAdmin: (request) => request.headers.get("authorization") === "Bearer " + adminToken,
    ...settings,
  } });
}

test("uses only the runtime grants to store a report and read the summary", async () => {
  const { send, fetch } = await startAdmin();
  await db.exec(await readFile(new URL("./migrations/0002_reports_runtime.sql", import.meta.url), "utf8"));
  await db.exec("SET ROLE pennypinch_reports_runtime");
  try {
    await expectResponse(await send(summary(ids[0]), tokens[0]), 201, "stored");
    await expectResponse(await send(summary(ids[0]), tokens[0]), 200, "duplicate");
    const response = await fetch(adminUrl, adminOptions);
    assert.equal(response.status, 200);
    assert.equal((await response.json()).summary.coverage.installations, 1);
    await assert.rejects(db.query("DELETE FROM reports.usage_days"), { code: "42501" });
    await assert.rejects(db.query("UPDATE reports.usage_intake_quota SET received_day = current_date"), { code: "42501" });
    await assert.rejects(db.query("CREATE TABLE reports.extra (id int)"), { code: "42501" });
  } finally {
    await db.exec("RESET ROLE");
  }
});

test("keeps admin reads closed without a verifier", async () => {
  const { fetch } = await start();
  await expectResponse(await fetch(adminUrl), 404, "not_found");
  await expectResponse(await fetch(panelUrl, adminOptions), 404, "not_found");
});

test("requires the verifier to approve administrator requests before reading", async () => {
  const { fetch } = await startAdmin({ migrate: false });
  await expectResponse(await fetch(adminUrl), 401, "unauthorized");
  await expectResponse(await fetch(adminUrl, { headers: headers(tokens[0]) }), 401, "unauthorized");
  await expectResponse(await fetch(adminUrl, adminOptions), 503, "unavailable");
  await expectResponse(await handleReportRequest(new Request(adminUrl), {
    verifyAdmin: () => "truthy-but-not-authorized",
  }, now), 401, "unauthorized");
});

const mainBasis = "headroom-0.45.0-ws-json-v1:o200k_base";
const otherBasis = "headroom-0.45.0-ws-json-v1:cl100k_base";
const pair = (basis, before, after) => ({
  basis, comparable_requests: 1, before_tokens: before, after_tokens: after,
});

test("groups weighted totals and the per-installation median by basis", async () => {
  const { send, fetch } = await startAdmin();
  await expectResponse(await send({ ...summary(ids[0]), report_day: day(-2), failed_requests: 1,
    groups: [pair(mainBasis, 100, 60), pair(otherBasis, 0, 10)] }, tokens[0]), 201, "stored");
  await expectResponse(await send({ ...summary(ids[0]), groups: [pair(mainBasis, 0, 5)] }, tokens[0]), 201, "stored");
  await expectResponse(await send({ ...summary(ids[1]), groups: [pair(mainBasis, 50, 60), pair(otherBasis, 100, 90)] },
    tokens[1]), 201, "stored");
  const result = await fetch(adminUrl, adminOptions);
  assert.equal(result.status, 200);
  assert.equal(result.headers.get("cache-control"), "no-store");
  const body = await result.text();
  for (const privateValue of [...ids, ...tokens, adminToken]) {
    assert.ok(!body.includes(privateValue));
  }
  const { status, summary: stats } = JSON.parse(body);
  assert.equal(status, "ready");
  assert.deepEqual(stats.period, { from_day: day(-7), to_day_exclusive: day(0) });
  assert.deepEqual(stats.coverage, { installations: 2, report_days: 3, comparable_requests: "5",
    incomparable_requests: "3", failed_requests: "1" });
  const main = stats.groups.find((group) => group.basis === mainBasis);
  assert.deepEqual(main, {
    basis: mainBasis, installations: 2, installations_with_percentage: 2, comparable_requests: "3",
    before_tokens: "150", after_tokens: "125", saved_tokens: "25",
    weighted_saved_percent: 100 * (25 / 150), median_installation_saved_percent: 7.5,
  });
  const other = stats.groups.find((group) => group.basis === otherBasis);
  assert.equal(other.weighted_saved_percent, 0);
  assert.equal(other.installations_with_percentage, 1);
  assert.equal(other.median_installation_saved_percent, 10);
  const one = await fetch(adminUrl + "?days=1", adminOptions);
  assert.equal(one.status, 200);
  assert.equal((await one.json()).summary.coverage.report_days, 2);
  await expectResponse(await fetch(adminUrl + "?days=7&days=1", adminOptions), 400, "invalid");
  await expectResponse(await fetch(adminUrl + "?days=31", adminOptions), 400, "invalid");
  const wrongMethod = await fetch(adminUrl, { ...adminOptions, method: "POST" });
  assert.equal(wrongMethod.headers.get("allow"), "GET");
  await expectResponse(wrongMethod, 405, "method_not_allowed");
  assert.equal(await firstValue("SELECT count(*)::int FROM reports.usage_days"), 3);
});

test("preserves exact token totals and leaves a zero-base percentage undefined", async () => {
  const { send, fetch } = await startAdmin();
  const max = Number.MAX_SAFE_INTEGER;
  for (let i = 0; i < ids.length; i++) {
    await expectResponse(await send({
      ...summary(ids[i]), groups: [pair(mainBasis, max, max - 1), pair(otherBasis, 0, 1)],
    }, tokens[i]), 201, "stored");
  }
  const result = await fetch(adminUrl + "?days=30", adminOptions);
  assert.equal(result.status, 200);
  const { groups } = (await result.json()).summary;
  const main = groups.find((group) => group.basis === mainBasis);
  assert.equal(main.before_tokens, String(BigInt(max) * 2n));
  assert.equal(main.saved_tokens, "2");
  const other = groups.find((group) => group.basis === otherBasis);
  assert.equal(other.saved_tokens, "-2");
  assert.equal(other.weighted_saved_percent, null);
  assert.equal(other.median_installation_saved_percent, null);
});

test("shows coverage without a savings percentage when usage has no comparable pair", async () => {
  const { send, fetch } = await startAdmin();
  await expectResponse(await send(summary(ids[0]), tokens[0]), 201, "stored");
  const result = await fetch(adminUrl, adminOptions);
  assert.equal(result.status, 200);
  const stats = (await result.json()).summary;
  assert.deepEqual(stats.coverage, { installations: 1, report_days: 1, comparable_requests: "0",
    incomparable_requests: "1", failed_requests: "0" });
  assert.deepEqual(stats.groups, []);
});

test("refuses a query above the report-day bound", async () => {
  const { fetch } = await startAdmin();
  await seedReports(day(-1), 4097);
  await expectResponse(await fetch(adminUrl, adminOptions), 503, "unavailable");
});

test("keeps reports private when the identity verifier fails", async () => {
  const { fetch } = await startAdmin({ settings: { verifyAdmin: () => { throw new Error("private_fixture"); } } });
  await expectResponse(await fetch(adminUrl, adminOptions), 503, "unavailable");
});

test("renders the private panel only after administrator approval", async () => {
  const { fetch } = await startAdmin({ settings: { intakeEnabled: false } });
  await expectResponse(await fetch(panelUrl), 401, "unauthorized");
  await expectResponse(await fetch(panelUrl, { headers: headers(tokens[0]) }), 401, "unauthorized");
  await expectResponse(await fetch(panelUrl + "?days=7&days=1", adminOptions), 400, "invalid");
  await expectResponse(await fetch(panelUrl.replace("https:", "http:"), adminOptions), 404, "not_found");
  const nonGet = await fetch(panelUrl, { ...adminOptions, method: "POST" });
  assert.equal(nonGet.headers.get("allow"), "GET");
  await expectResponse(nonGet, 405, "method_not_allowed");
  const response = await fetch(panelUrl, adminOptions);
  assert.equal(response.status, 200);
  assert.equal(response.headers.get("content-type"), "text/html; charset=utf-8");
  assert.equal(response.headers.get("cache-control"), "no-store");
  assert.equal(response.headers.get("x-content-type-options"), "nosniff");
  assert.equal(response.headers.get("referrer-policy"), "no-referrer");
  const policy = response.headers.get("content-security-policy");
  assert.match(policy, /default-src 'none'.*frame-ancestors 'none'.*form-action 'none'/);
  const nonce = /style-src 'nonce-([^']+)'/.exec(policy)[1];
  assert.equal(atob(nonce).length, 16);
  const html = await response.text();
  assert.ok(html.includes(`<style nonce="${nonce}">`));
  assert.match(html, /No reports in this period/);
  assert.match(html, /href="\/admin\?days=7" aria-current="page"/);
  assert.doesNotMatch(html, /<script\b/);
});

test("renders date ranges, signed savings and exact numbers without exposing private values", async () => {
  const { send, fetch } = await startAdmin();
  await expectResponse(await send({ ...summary(ids[0]), report_day: day(-2), failed_requests: 1,
    groups: [pair(mainBasis, 100, 60), pair(otherBasis, 0, 2)] }, tokens[0]), 201, "stored");
  const oneDay = await fetch(panelUrl + "?days=1", adminOptions);
  assert.match(await oneDay.text(), /No reports in this period/);
  await expectResponse(await send({ ...summary(ids[0]), groups: [pair(mainBasis, 100, 105)] }, tokens[0]),
    201, "stored");
  await expectResponse(await send({ ...summary(ids[1]), groups: [pair(mainBasis, 300, 315)] }, tokens[1]),
    201, "stored");
  const oneDayHtml = await (await fetch(panelUrl + "?days=1", adminOptions)).text();
  assert.match(oneDayHtml, /href="\/admin\?days=1" aria-current="page"/);
  assert.match(oneDayHtml, /-5%/);
  assert.ok(oneDayHtml.includes(`datetime="${day(-1)}"`));

  const maximum = Number.MAX_SAFE_INTEGER;
  for (let i = 0; i < ids.length; i++) {
    await expectResponse(await send({ ...summary(ids[i]), report_day: day(-3),
      groups: [pair(mainBasis, maximum, maximum), pair("headroom-0.45.0-ws-json-v1:gpt2", 1, 1)] },
    tokens[i]), 201, "stored");
  }
  const response = await fetch(panelUrl + "?days=30", adminOptions);
  assert.equal(response.status, 200);
  const html = await response.text();
  assert.match(html, /href="\/admin\?days=30" aria-current="page"/);
  assert.ok(html.includes(`datetime="${day(-30)}"`));
  assert.match(html, /18,014,398,509,482,482/);
  assert.match(html, /<dd>&lt;0.01% saved<\/dd>/);
  assert.match(html, /<dd>0%<\/dd>/);
  assert.ok(html.includes(otherBasis));
  assert.match(html, /Unavailable/);
  for (const privateValue of [...ids, ...tokens, adminToken]) {
    assert.ok(!html.includes(privateValue));
  }
});

test("renders coverage with no comparable input and gives a distinct database error", async () => {
  const { send, fetch } = await startAdmin();
  await expectResponse(await send(summary(ids[0]), tokens[0]), 201, "stored");
  const html = await (await fetch(panelUrl, adminOptions)).text();
  assert.match(html, /No comparable requests in this period/);
  assert.match(html, /Incomparable requests\s*<\/dt>\s*<dd>1<\/dd>/);
  assert.doesNotMatch(html, /Weighted savings/);
  await db.exec("DROP TABLE reports.usage_days");
  const unavailable = await fetch(panelUrl, adminOptions);
  assert.equal(unavailable.status, 503);
  assert.equal(unavailable.headers.get("cache-control"), "no-store");
  const fallback = await unavailable.text();
  assert.match(fallback, /Usage reports are unavailable/);
  assert.doesNotMatch(fallback, /PostgreSQL|usage_days/);
});

async function seedReports(reportDay, count) {
  await db.query(
    "INSERT INTO reports.usage_days (report_day, installation_id, summary) " +
    "SELECT $1::date, ('1b93c822-ac37-4360-aa79-' || lpad(i::text, 12, '0'))::uuid, '{}' " +
    "FROM generate_series(1, $2::int) AS g(i)", [reportDay, count],
  );
}

test("retention clears the expired reports, quotas, and rate windows while intake is closed", async () => {
  const { fetch } = await start({ settings: { intakeEnabled: false } });
  await seedReports("2026-10-24", 501);
  await seedReports("2026-10-25", 1);
  await db.exec(
    "INSERT INTO reports.usage_intake_quota (received_day, accepted_count) VALUES ('2026-10-24', 1), ('2026-10-25', 1);",
  );
  const minute = Math.floor(now / 60_000);
  await db.query(
    "INSERT INTO reports.usage_rate_limits (installation_id, window_minute, request_count) VALUES ($1, $2, 4), ($3, $4, 1)",
    [ids[0], minute - 1441, ids[1], minute - 1440],
  );
  await expectResponse(await fetch(url, { method: "POST" }), 404, "not_found");
  assert.equal(await pruneExpiredUsageDays(db, now), true);
  assert.equal(await firstValue("SELECT report_day::text FROM reports.usage_days"), "2026-10-25");
  assert.equal(await firstValue("SELECT received_day::text FROM reports.usage_intake_quota"), "2026-10-25");
  assert.equal(await firstValue("SELECT installation_id FROM reports.usage_rate_limits"), ids[1]);
});

test("retention bounds the backlog work and completes it on the next call", async () => {
  await start();
  await seedReports("2026-10-24", 4_001);
  assert.equal(await pruneExpiredUsageDays(db, now), false);
  assert.equal(await firstValue("SELECT count(*)::int FROM reports.usage_days"), 1);
  assert.equal(await pruneExpiredUsageDays(db, now), true);
  assert.equal(await firstValue("SELECT count(*)::int FROM reports.usage_days"), 0);
});

test("retention reports database failure to the scheduler", async () => {
  await start({ migrate: false });
  await assert.rejects(() => pruneExpiredUsageDays(db, now), { code: "42P01" });
});
