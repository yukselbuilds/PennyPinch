import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { after, before, test } from "node:test";
import { PGlite } from "@electric-sql/pglite";
import { createLocalJWKSet, exportJWK, generateKeyPair, SignJWT } from "jose";
import { createAdminHandler } from "./admin-function.mjs";

const origin = "https://admin.fixture.invalid";
const issuer = "https://auth.fixture.invalid";
const jwksUrl = issuer + "/pilot/auth/jwks";
const subject = "pilot-admin-fixture";
const audience = "pilot-reports-fixture";
const protectedPath = "/v1/admin/summary";
const sessionPath = "/v1/admin/session";
const clockStart = Date.parse("2026-11-24T15:00:00Z");
const now = clockStart / 1000;
const env = {
  NEON_AUTH_BASE_URL: issuer + "/pilot/auth", NEON_AUTH_JWKS_URL: jwksUrl,
  REPORTS_ADMIN_SUBJECT: subject, REPORTS_ADMIN_AUDIENCE: audience,
};
const db = new PGlite();
const keys = await generateKeyPair("EdDSA", { extractable: true });
const otherKeys = await generateKeyPair("EdDSA");
const fixtureJwks = createLocalJWKSet({ keys: [{ ...await exportJWK(keys.publicKey), kid: "fixture-key" }] });
const tokenFor = (claims = {}, signer = keys.privateKey, alg = "EdDSA") =>
  new SignJWT({ iss: issuer, aud: audience, sub: subject, iat: now - 15, exp: now + 180, ...claims })
    .setProtectedHeader({ alg, kid: "fixture-key" }).sign(signer);
const request = (path, options) => new Request(origin + path, options);
const authorization = (token) => ({ authorization: "Bearer " + token });
const admin = (settings) => createAdminHandler({
  env, clock: () => clockStart, db,
  createKeySet: (url) => { assert.equal(url.href, jwksUrl); return fixtureJwks; },
  ...settings,
});

before(async () => {
  await db.exec(await readFile(new URL("../migrations/0001_reports.sql", import.meta.url), "utf8"));
  await db.query("INSERT INTO reports.usage_days VALUES ('2026-11-23', '1b93c822-ac37-4360-aa79-000000000001', $1)", [
    JSON.stringify({
      schema_version: 1, incomparable_requests: 0, failed_requests: 0,
      groups: [{ basis: "headroom-0.45.0-ws-json-v1:o200k_base", comparable_requests: 1, before_tokens: 100, after_tokens: 75 }],
    }),
  ]);
});
after(() => db.close());

test("keeps the panel, summary, and session route closed if the operator configuration is incomplete", async () => {
  for (const configuration of [
    {},
    { ...env, REPORTS_ADMIN_SUBJECT: "" },
    { ...env, REPORTS_ADMIN_AUDIENCE: "" },
    { ...env, NEON_AUTH_JWKS_URL: "http://auth.fixture.invalid/pilot/auth/jwks" },
  ]) {
    const handler = admin({ env: configuration, createKeySet: () => assert.fail("key resolver should stay off") });
    for (const path of ["/admin", protectedPath, sessionPath]) {
      const result = await handler(request(path));
      assert.equal(result.status, 404);
      assert.equal(result.headers.get("cache-control"), "no-store");
    }
  }
});

test("rejects malformed or unauthorised tokens before a database read", async () => {
  const handler = admin({ db: { query: () => assert.fail("unexpected database query") } });
  const refused = [
    "malformed",
    await tokenFor({ sub: "another-user" }),
    await tokenFor({ iss: "https://wrong.fixture.invalid" }),
    await tokenFor({ aud: "different-application" }),
    await tokenFor({ exp: now }),
    await tokenFor({ iat: now - 1000 }),
    await tokenFor({ iat: now + 10 }),
    await tokenFor({ exp: now + 3600 }),
    await tokenFor({ iat: undefined }),
    await tokenFor({ exp: undefined }),
    await tokenFor({}, otherKeys.privateKey),
    await tokenFor({}, new Uint8Array(32).fill(7), "HS256"),
  ];
  for (const token of refused) {
    assert.equal((await handler(request(protectedPath, { headers: authorization(token) }))).status, 401);
  }
  assert.equal((await handler(request(protectedPath))).status, 401);
});

test("accepts the allowed subject and opens the panel and summary with the same short browser session", async () => {
  const handler = admin();
  const token = await tokenFor();
  const bearer = await handler(request(protectedPath, { headers: authorization(token) }));
  assert.equal(bearer.status, 200);
  assert.equal((await bearer.json()).summary.groups[0].saved_tokens, "25");
  const session = await handler(request(sessionPath, {
    method: "POST", headers: { origin, ...authorization(token) },
  }));
  assert.equal(session.status, 204);
  assert.equal(session.headers.get("cache-control"), "no-store");
  const cookie = session.headers.get("set-cookie");
  assert.match(cookie, /^__Host-pennypinch-reports-admin=/);
  assert.match(cookie, /; Path=\/; HttpOnly; Secure; SameSite=Strict; Max-Age=180$/);
  const sessionHeaders = { cookie: cookie.split(";", 1)[0] };
  for (const days of [1, 7, 30]) {
    const panel = await handler(request("/admin?days=" + days, { headers: sessionHeaders }));
    assert.equal(panel.status, 200);
    assert.equal(panel.headers.get("cache-control"), "no-store");
    assert.match(await panel.text(), new RegExp('href="/admin\\?days=' + days + '" aria-current="page"'));
  }
  const summary = await handler(request(protectedPath, { headers: sessionHeaders }));
  assert.equal(summary.status, 200);
  assert.equal((await summary.json()).summary.coverage.installations, 1);
  assert.equal((await handler(request("/v1/usage", { method: "POST", headers: sessionHeaders }))).status, 404);
});

test("rejects a cross-origin session change and expires and clears the session cookie", async () => {
  let time = clockStart;
  const handler = admin({ clock: () => time });
  const token = await tokenFor();
  const session = (method, headers = {}) => handler(request(sessionPath, { method, headers }));
  for (const method of ["POST", "DELETE"]) {
    const wrongOrigin = await session(method, { origin: "https://other.fixture.invalid", ...authorization(token) });
    assert.equal(wrongOrigin.status, 403);
    assert.equal(wrongOrigin.headers.has("set-cookie"), false);
  }
  assert.equal((await session("POST", { origin })).status, 401);
  const signedIn = await session("POST", { origin, ...authorization(token) });
  const cookie = signedIn.headers.get("set-cookie").split(";", 1)[0];
  assert.equal((await handler(request(protectedPath, {
    headers: { cookie, authorization: "malformed" },
  }))).status, 401);
  assert.equal((await handler(request(protectedPath, { headers: { cookie: cookie + "; " + cookie } }))).status, 401);
  const signedOut = await session("DELETE", { origin });
  assert.equal(signedOut.status, 204);
  assert.match(signedOut.headers.get("set-cookie"), /^__Host-pennypinch-reports-admin=; .*; Max-Age=0$/);
  time += 180 * 1000;
  assert.equal((await handler(request(protectedPath, { headers: { cookie } }))).status, 401);
});
