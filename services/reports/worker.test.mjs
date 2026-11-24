import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import { Miniflare } from "miniflare";
import worker from "./worker.mjs";

const url = "https://pennypinch.test/v1/usage";
const ids = ["125d94ba-1e98-4d64-ac76-eb56f665a527", "55676b9c-87d3-479d-ab92-fa0fc07a9d32"];
const tokens = ["A".repeat(43), "B".repeat(43)];
const digest = (text) => createHash("sha256").update(text).digest("hex");
const credentials = JSON.stringify(Object.fromEntries(tokens.map((token, i) => [digest(token), ids[i]])));
const day = (offset) => new Date(Date.now() + offset * 86_400_000).toISOString().slice(0, 10);

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

async function applyMigration(db, name) {
  const sql = await readFile(new URL("./migrations/" + name, import.meta.url), "utf8");
  await db.batch(sql.trim().split(/\r?\n\r?\n/).map((statement) => db.prepare(statement)));
}

async function start(t, { rate = 100, bindings = {}, migrate = true } = {}) {
  const fixture = new Miniflare({
    host: "127.0.0.1", port: 0, modules: true,
    scriptPath: fileURLToPath(new URL("./worker.mjs", import.meta.url)),
    bindings: { REPORTS_ENABLED: "true", PILOT_USAGE_CREDENTIALS: credentials, ...bindings },
    d1Databases: { REPORTS_DB: "70edd725-b13f-490a-8ead-ab8862fa50f1" },
    ratelimits: { USAGE_RATE: { namespace_id: "1", simple: { limit: rate, period: 60 } } },
    cf: false, telemetry: { enabled: false },
  });
  t.after(() => fixture.dispose());
  const db = await fixture.getD1Database("REPORTS_DB");
  if (migrate) {
    await applyMigration(db, "0001_usage.sql");
    await applyMigration(db, "0002_intake.sql");
  }
  return { db, send: (body, token) => fixture.dispatchFetch(url, {
    method: "POST", headers: headers(token), body: JSON.stringify(body),
  }),
    fetch: (address, options) => fixture.dispatchFetch(address, options) };
}

async function expectResponse(response, code, status) {
  assert.equal(response.status, code);
  assert.equal(response.headers.get("cache-control"), "no-store");
  assert.deepEqual(await response.json(), { status });
}

test("the Worker disables intake by default and rejects missing bindings", async () => {
  const sample = summary(ids[0]);
  await expectResponse(await worker.fetch(request(sample, tokens[0]), {}), 404, "not_found");
  await expectResponse(await worker.fetch(request(sample, tokens[0]), { REPORTS_ENABLED: "true" }), 503, "unavailable");
});

test("enforces the route, method, and the installation bound to each token", async (t) => {
  const { db, send, fetch } = await start(t);
  const sample = summary(ids[0]);
  const wrongMethod = await fetch(url);
  assert.equal(wrongMethod.headers.get("allow"), "POST");
  await expectResponse(wrongMethod, 405, "method_not_allowed");
  await expectResponse(await fetch(url + "?unexpected=1", { method: "POST" }), 404, "not_found");
  await expectResponse(await fetch(url.replace("https:", "http:"), { method: "POST" }), 404, "not_found");
  await expectResponse(await send(sample), 401, "unauthorized");
  await expectResponse(await send(sample, "C".repeat(43)), 401, "unauthorized");
  await expectResponse(await send(sample, tokens[1]), 403, "forbidden");
  assert.equal(await db.prepare("SELECT COUNT(*) AS n FROM usage_days").first("n"), 0);
  assert.equal(await db.prepare("SELECT COUNT(*) AS n FROM usage_intake_quota").first("n"), 0);
});

test("stores one report and preserves the original on a conflicting retry", async (t) => {
  const { db, send } = await start(t);
  const sample = summary(ids[0]);
  await expectResponse(await send(sample, tokens[0]), 201, "stored");
  await expectResponse(await send(sample, tokens[0]), 200, "duplicate");
  await expectResponse(await send({ ...sample, failed_requests: 1 }, tokens[0]), 409, "conflict");
  const row = await db.prepare("SELECT summary FROM usage_days").first("summary");
  assert.equal(JSON.parse(row).failed_requests, 0);
  for (const value of [...ids, ...tokens, ...tokens.map(digest)]) assert.ok(!row.includes(value));
  assert.equal(await db.prepare("SELECT accepted_count FROM usage_intake_quota").first("accepted_count"), 1);
});

test("checks media type, UTF-8, and the received body size", async (t) => {
  const { db, fetch } = await start(t);
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
  assert.equal(await db.prepare("SELECT COUNT(*) AS n FROM usage_days").first("n"), 0);
});

test("limits bursts per authenticated installation", async (t) => {
  const { send } = await start(t, { rate: 6 });
  const sample = summary(ids[0]);
  await expectResponse(await send(sample, tokens[0]), 201, "stored");
  for (let i = 0; i < 5; i++) await expectResponse(await send(sample, tokens[0]), 200, "duplicate");
  const limited = await send(sample, tokens[0]);
  assert.equal(limited.headers.get("retry-after"), "60");
  await expectResponse(limited, 429, "rate_limited");
  await expectResponse(await send(summary(ids[1]), tokens[1]), 201, "stored");
});

test("enforces the UTC write budget across concurrent installations and preserves it on erasure", async (t) => {
  const { db, send } = await start(t);
  const today = day(0);
  await db.prepare("INSERT INTO usage_intake_quota (received_day, accepted_count) VALUES (?, 127)").bind(today).run();
  const samples = ids.map(summary);
  const responses = await Promise.all(samples.map((sample, i) => send(sample, tokens[i])));
  assert.deepEqual(responses.map((response) => response.status).sort(), [201, 429]);
  const accepted = responses.findIndex((response) => response.status === 201);
  const rejected = 1 - accepted;
  const seconds = Number(responses[rejected].headers.get("retry-after"));
  assert.ok(seconds > 0 && seconds <= 86_400);
  await expectResponse(responses[rejected], 429, "daily_quota");
  await expectResponse(await send(samples[accepted], tokens[accepted]), 200, "duplicate");
  await db.prepare("DELETE FROM usage_days WHERE installation_id = ?").bind(ids[accepted]).run();
  await expectResponse(await send(samples[rejected], tokens[rejected]), 429, "daily_quota");
  assert.equal(await db.prepare("SELECT accepted_count FROM usage_intake_quota").first("accepted_count"), 128);
});

test("maps a D1 failure to a generic response", async (t) => {
  const { send } = await start(t, { migrate: false });
  await expectResponse(await send(summary(ids[0]), tokens[0]), 503, "unavailable");
});

test("migrates saved summaries and keeps their retries outside the new write budget", async (t) => {
  const { db, send } = await start(t, { migrate: false });
  await applyMigration(db, "0001_usage.sql");
  const sample = summary(ids[0]);
  const { installation_id, report_day, ...stored } = sample;
  await db.prepare("INSERT INTO usage_days (report_day, installation_id, summary) VALUES (?, ?, ?)")
    .bind(report_day, installation_id, JSON.stringify(stored)).run();
  await applyMigration(db, "0002_intake.sql");
  await expectResponse(await send(sample, tokens[0]), 200, "duplicate");
  const row = await db.prepare("SELECT received_day, summary FROM usage_days").first();
  assert.equal(row.received_day, null);
  assert.equal(row.summary, JSON.stringify(stored));
  assert.equal(await db.prepare("SELECT COUNT(*) AS n FROM usage_intake_quota").first("n"), 0);
});
