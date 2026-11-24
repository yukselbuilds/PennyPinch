import { createRemoteJWKSet, jwtVerify } from "jose";

const teamOrigin = /^https:\/\/[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?\.cloudflareaccess\.com$/;
const adminAudience = /^[a-f0-9]{64}$/;
let cachedKeys;

function accessKeys(issuer) {
  if (cachedKeys?.issuer !== issuer) {
    const jwks = new URL("/cdn-cgi/access/certs", issuer);
    cachedKeys = { issuer, verify: createRemoteJWKSet(jwks, { timeoutDuration: 3000 }) };
  }
  return cachedKeys.verify;
}

export async function authorizeAdmin(request, env) {
  const { ADMIN_ACCESS_ISSUER: issuer, ADMIN_ACCESS_AUDIENCE: audience } = env;
  if (!issuer || !audience) return "not_found";
  if (!teamOrigin.test(issuer) || !adminAudience.test(audience)) return "unavailable";

  const assertion = request.headers.get("cf-access-jwt-assertion");
  if (!assertion || assertion.length > 8192) return "unauthorized";
  try {
    const { payload } = await jwtVerify(assertion, accessKeys(issuer), {
      algorithms: ["RS256"],
      issuer,
      audience,
      requiredClaims: ["iss", "aud", "exp", "iat", "nbf", "sub", "type", "email"],
    });
    if (payload.type !== "app" || typeof payload.sub !== "string" || !payload.sub ||
        typeof payload.email !== "string" || !payload.email) return "unauthorized";
    return "authorized";
  } catch (error) {
    if (error instanceof TypeError ||
        ["ERR_JWKS_TIMEOUT", "ERR_JWKS_INVALID", "ERR_JOSE_GENERIC"].includes(error?.code)) {
      return "unavailable";
    }
    return "unauthorized";
  }
}
