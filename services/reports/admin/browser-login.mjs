const formBytes = 4096;
const sessionCookie = /^__Secure-neon-auth\.session_token=[A-Za-z0-9._~%+\/=:-]{1,1024}$/;

export async function readLoginForm(request) {
  const type = request.headers.get("content-type")?.split(";", 1)[0].trim().toLowerCase();
  if (type !== "application/x-www-form-urlencoded" || !request.body) return null;
  const reader = request.body.getReader();
  const bytes = new Uint8Array(formBytes);
  let length = 0;
  try {
    for (;;) {
      const { done, value } = await reader.read();
      if (done) break;
      if (length + value.byteLength > bytes.length) {
        void reader.cancel().catch(() => {});
        return null;
      }
      bytes.set(value, length);
      length += value.byteLength;
    }
    const form = new URLSearchParams(new TextDecoder("utf-8", { fatal: true }).decode(bytes.subarray(0, length)));
    const email = form.get("email")?.trim();
    const password = form.get("password");
    return form.size === 2 && email && email.length <= 254 && password && password.length <= 128
      ? { email, password }
      : null;
  } catch {
    return null;
  } finally {
    reader.releaseLock();
  }
}

export async function signInWithNeon(credentials, authUrl, origin, requestUpstream) {
  const root = authUrl.href.replace(/\/$/, "");
  function send(path, init, headers = {}) {
    return requestUpstream(root + path, {
      ...init,
      headers: { accept: "application/json", origin, referer: origin + "/", ...headers },
      cache: "no-store", redirect: "error", signal: AbortSignal.timeout(5000),
    });
  }
  const login = await send("/sign-in/email", {
    method: "POST",
    body: JSON.stringify({ ...credentials, rememberMe: false }),
  }, { "content-type": "application/json" });
  cancelBody(login);
  if (login.status === 400 || login.status === 401 || login.status === 403) return null;
  if (!login.ok) throw new Error("auth_service_unavailable");
  const cookies = login.headers.getSetCookie().map((cookie) => cookie.split(";", 1)[0])
    .filter((cookie) => sessionCookie.test(cookie));
  if (cookies.length !== 1) throw new Error("auth_session_unavailable");
  const cookie = cookies[0];
  try {
    const access = await send("/token", { method: "GET" }, { cookie });
    if (!access.ok) {
      cancelBody(access);
      throw new Error("auth_token_unavailable");
    }
    const { token } = await access.json();
    if (typeof token !== "string") throw new Error("auth_token_unavailable");
    return token;
  } finally {
    const signOut = await send("/sign-out", { method: "POST" }, { cookie });
    cancelBody(signOut);
    if (!signOut.ok) throw new Error("auth_sign_out_unavailable");
  }
}

function cancelBody(response) {
  void response.body?.cancel().catch(() => {});
}

const messages = {
  400: "Enter an email address and password.",
  401: "Sign-in failed. Check your account details.",
  503: "Sign-in is temporarily unavailable. Try again.",
};

export function renderAdminLogin({ status = 200, signedIn = false } = {}) {
  const nonce = btoa(String.fromCharCode(...crypto.getRandomValues(new Uint8Array(16))));
  return new Response(`<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Reports account · PennyPinch</title>
  <style nonce="${nonce}">
    :root { color-scheme: light dark; --bg: #f3f5f0; --surface: #fff; --text: #19241d; --line: #a6b4a9; --action: #145433; --on-action: #fff; --error: #9c2417; }
    @media (prefers-color-scheme: dark) {
      :root { --bg: #101613; --surface: #19221d; --text: #e8f2eb; --line: #54695b; --action: #9aebba; --on-action: #11241a; --error: #ffad9f; }
    }
    * { box-sizing: border-box; }
    body { margin: 0; min-height: 100vh; display: grid; place-items: center; padding: 20px; font: 1rem/1.5 "Segoe UI", system-ui, sans-serif; background: var(--bg); color: var(--text); }
    main { width: min(100%, 26rem); padding: clamp(20px, 5vw, 32px); background: var(--surface); border: 1px solid var(--line); border-radius: 16px; }
    p { margin: 0 0 16px; }
    h1 { margin: 0 0 20px; font-size: 1.6rem; line-height: 1.25; }
    form { display: grid; gap: 16px; }
    label { display: grid; gap: 6px; font-weight: 600; }
    input, button, a.action { font: inherit; border-radius: 8px; min-height: 44px; padding: 8px 12px; }
    input { width: 100%; color: var(--text); background: var(--bg); border: 1px solid var(--line); }
    button, a.action { color: var(--on-action); background: var(--action); border: 1px solid var(--action); font-weight: 600; cursor: pointer; text-align: center; text-decoration: none; }
    .account { display: grid; gap: 12px; }
    .secondary { background: transparent; color: var(--text); border-color: var(--line); }
    [role="alert"] { color: var(--error); }
    :focus-visible { outline: 3px solid var(--action); outline-offset: 3px; }
  </style>
</head>
<body>
  <main>
    <p>PennyPinch</p>
    <h1>${signedIn ? "Reports account" : "Sign in to usage reports"}</h1>
    ${messages[status] ? '<p role="alert">' + messages[status] + "</p>" : ""}
    ${signedIn ? `<div class="account">
      <a class="action" href="/admin">View reports</a>
      <form method="post" action="/admin/logout"><button class="secondary">Sign out</button></form>
    </div>` : `<form method="post" action="/admin/login">
      <label>Email <input type="email" name="email" autocomplete="username" maxlength="254" required></label>
      <label>Password <input type="password" name="password" autocomplete="current-password" maxlength="128" required></label>
      <button>Sign in</button>
    </form>`}
  </main>
</body>
</html>`, { status, headers: {
    "content-type": "text/html; charset=utf-8",
    "cache-control": "no-store",
    "content-security-policy": "default-src 'none'; style-src 'nonce-" + nonce + "'; form-action 'self'; base-uri 'none'; frame-ancestors 'none'",
    "referrer-policy": "same-origin",
    "x-content-type-options": "nosniff",
  } });
}
