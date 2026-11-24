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
const loginPath = "/admin/login";
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
    for (const path of ["/admin", protectedPath, sessionPath, loginPath, "/admin/logout"]) {
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

const loginCredentials = { email: "admin@fixture.invalid", password: "test-password-0123" };
const loginRequest = (fields = loginCredentials, from = origin) => request(loginPath, {
  method: "POST", headers: { origin: from }, body: new URLSearchParams(fields),
});
const browserHeaders = { accept: "text/html" };
const providerCookie = "__Secure-neon-auth.session_token=fixture-provider-session";

function authFixture(token, { loginStatus = 200, logoutStatus = 200 } = {}) {
  const calls = [];
  return {
    calls,
    async fetch(address, init) {
      assert.equal(init.redirect, "error");
      assert.equal(init.cache, "no-store");
      assert.equal(init.headers.origin, origin);
      assert.equal(init.headers.referer, origin + "/");
      const url = new URL(address);
      assert.equal(url.origin, issuer);
      calls.push(url.pathname);
      if (url.pathname === "/pilot/auth/sign-in/email") {
        assert.equal(init.method, "POST");
        assert.deepEqual(JSON.parse(init.body), { ...loginCredentials, rememberMe: false });
        const headers = new Headers();
        headers.append("set-cookie", providerCookie + "; HttpOnly; Secure; Path=/");
        headers.append("set-cookie", "unrelated=fixture; Secure");
        return new Response("private provider details", { status: loginStatus, headers });
      }
      assert.equal(init.headers.cookie, providerCookie);
      if (url.pathname === "/pilot/auth/token") {
        assert.equal(init.method, "GET");
        return Response.json({ token });
      }
      assert.equal(url.pathname, "/pilot/auth/sign-out");
      assert.equal(init.method, "POST");
      return new Response(null, { status: logoutStatus });
    },
  };
}

test("offers a same-origin login form and refuses cross-origin or oversized submissions", async () => {
  const handler = admin({ requestUpstream: () => assert.fail("unexpected provider request") });
  const start = await handler(request("/admin", { headers: browserHeaders }));
  assert.equal(start.status, 303);
  assert.equal(start.headers.get("location"), loginPath);
  const page = await handler(request(loginPath));
  assert.equal(page.status, 200);
  assert.equal(page.headers.get("cache-control"), "no-store");
  assert.match(page.headers.get("content-security-policy"), /form-action 'self';.*frame-ancestors 'none'/);
  assert.match(await page.text(), /method="post" action="\/admin\/login"/);
  assert.equal((await handler(loginRequest(loginCredentials, "https://other.fixture.invalid"))).status, 403);
  for (const invalid of [
    { ...loginCredentials, password: "x".repeat(4096) },
    { ...loginCredentials, extra: "field" },
    { ...loginCredentials, password: "" },
  ]) {
    assert.equal((await handler(loginRequest(invalid))).status, 400);
  }
});

test("exchanges the short provider session for a signed panel session and then clears it", async () => {
  const provider = authFixture(await tokenFor());
  const handler = admin({ requestUpstream: provider.fetch });
  const response = await handler(loginRequest());
  assert.equal(response.status, 303);
  assert.equal(response.headers.get("location"), loginPath);
  assert.deepEqual(provider.calls, ["/pilot/auth/sign-in/email", "/pilot/auth/token", "/pilot/auth/sign-out"]);
  const setCookie = response.headers.get("set-cookie");
  assert.doesNotMatch(setCookie, /fixture-provider-session|unrelated=/);
  const cookie = setCookie.split(";", 1)[0];
  const account = await handler(request(loginPath, { headers: { cookie } }));
  assert.match(await account.text(), /View reports[\s\S]*action="\/admin\/logout"/);
  assert.equal((await handler(request("/admin", { headers: { cookie, ...browserHeaders } }))).status, 200);
  const denied = await handler(request("/admin/logout", { method: "POST", headers: { origin: "https://other.fixture.invalid", cookie } }));
  assert.equal(denied.status, 403);
  assert.equal(denied.headers.has("set-cookie"), false);
  const logout = await handler(request("/admin/logout", { method: "POST", headers: { origin, cookie } }));
  assert.equal(logout.status, 303);
  assert.match(logout.headers.get("set-cookie"), /; Max-Age=0$/);
});

test("leaves the panel signed out when credentials, subject, or provider sign-out fail", async () => {
  for (const [token, settings, expected, calls] of [
    [await tokenFor(), { loginStatus: 401 }, 401, 1],
    [await tokenFor({ sub: "another-user" }), {}, 401, 3],
    [await tokenFor(), { logoutStatus: 503 }, 503, 3],
  ]) {
    const provider = authFixture(token, settings);
    const response = await admin({ requestUpstream: provider.fetch })(loginRequest());
    assert.equal(response.status, expected);
    assert.equal(response.headers.has("set-cookie"), false);
    const html = await response.text();
    assert.doesNotMatch(html, /private provider details|test-password-0123|admin@fixture.invalid/);
    assert.equal(provider.calls.length, calls);
  }
});
