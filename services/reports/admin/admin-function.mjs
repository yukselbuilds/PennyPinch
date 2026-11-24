import { createRemoteJWKSet, jwtVerify } from "jose";
import { createReportPool } from "../function.mjs";
import { handleReportRequest } from "../service.mjs";

const cookieName = "__Host-pennypinch-reports-admin";
const sessionPath = "/v1/admin/session";
const maxSessionSeconds = 900;
const signedToken = /^[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+$/;

function secureUrl(value) {
  const url = URL.parse(value ?? "");
  return url?.protocol === "https:" && !url.username && !url.password && !url.search && !url.hash ? url : null;
}

function bearerToken(request) {
  return /^Bearer ([^ ]+)$/i.exec(request.headers.get("authorization") ?? "")?.[1];
}

function sessionToken(request) {
  if (request.headers.has("authorization")) return bearerToken(request);
  const prefix = cookieName + "=";
  const matches = request.headers.get("cookie")?.split(";")
    .map((cookie) => cookie.trim()).filter((cookie) => cookie.startsWith(prefix));
  return matches?.length === 1 ? matches[0].slice(prefix.length) : undefined;
}

function sessionResponse(status, cookie) {
  const headers = { "cache-control": "no-store", "x-content-type-options": "nosniff" };
  if (cookie) headers["set-cookie"] = cookie;
  return new Response(null, { status, headers });
}

function adminCookie(token, maxAge) {
  return cookieName + "=" + token + "; Path=/; HttpOnly; Secure; SameSite=Strict; Max-Age=" + maxAge;
}

export function createAdminHandler({ db, env = process.env, createKeySet = createRemoteJWKSet, clock = Date.now } = {}) {
  const authUrl = secureUrl(env.NEON_AUTH_BASE_URL);
  const jwksUrl = secureUrl(env.NEON_AUTH_JWKS_URL);
  const audience = env.REPORTS_ADMIN_AUDIENCE;
  const subject = env.REPORTS_ADMIN_SUBJECT;
  if (!authUrl || !jwksUrl || !audience || !subject) {
    return (request) => handleReportRequest(request);
  }
  const keys = createKeySet(jwksUrl, { timeoutDuration: 5_000 });

  async function expiresAt(token, now) {
    if (!token || token.length > 3600 || !signedToken.test(token)) return 0;
    try {
      const { payload } = await jwtVerify(token, keys, {
        issuer: authUrl.origin, audience, algorithms: ["EdDSA"],
        requiredClaims: ["sub", "iat", "exp"],
        maxTokenAge: maxSessionSeconds, currentDate: new Date(now * 1000),
      });
      return payload.sub === subject && Number.isSafeInteger(payload.iat) && Number.isSafeInteger(payload.exp) &&
        payload.exp > now && payload.exp - payload.iat <= maxSessionSeconds ? payload.exp : 0;
    } catch {
      return 0;
    }
  }

  return async function fetchAdmin(request) {
    const url = new URL(request.url);
    const time = clock();
    const now = Math.floor(time / 1000);
    if (url.pathname === sessionPath) {
      if (url.protocol !== "https:" || url.search) return sessionResponse(404);
      if (request.headers.get("origin") !== url.origin) return sessionResponse(403);
      if (request.method === "DELETE") return sessionResponse(204, adminCookie("", 0));
      if (request.method !== "POST") return sessionResponse(405);
      const token = bearerToken(request);
      const expiry = await expiresAt(token, now);
      return expiry ? sessionResponse(204, adminCookie(token, expiry - now)) : sessionResponse(401);
    }
    return handleReportRequest(request, {
      db, verifyAdmin: async (req) => await expiresAt(sessionToken(req), now) > now,
    }, time);
  };
}

const db = createReportPool(process.env.REPORTS_ADMIN_DATABASE_URL);
export default { fetch: createAdminHandler({ db }) };
