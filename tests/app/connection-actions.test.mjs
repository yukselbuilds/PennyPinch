import assert from "node:assert/strict";
import test from "node:test";

const api = await import("../../src/lib/connectionActions.ts").catch(() => null);
const planId = "019a0000-0000-7000-8000-000000000003";
const requestId = "019a0000-0000-7000-8000-000000000001";
const ownerId = "019a0000-0000-7000-8000-000000000005";
const endpointId = "019a0000-0000-7000-8000-000000000004";
const fingerprint = "a".repeat(64);
const recordFingerprint = "b".repeat(64);
const clientKind = "codex";

function makePlan(overrides = {}) {
  return {
    schema_version: 1,
    request_id: "019a0000-0000-7000-8000-000000000010",
    plan_id: planId,
    expires_at_unix_seconds: Math.floor(Date.now() / 1000) + 300,
    installation_id: "019a0000-0000-7000-8000-000000000002",
    client_kind: clientKind,
    scope: "user_config",
    scan_generation: 7,
    config_fingerprint: fingerprint,
    config_presence: "present",
    config_source: "codex_toml",
    endpoint_id: endpointId,
    endpoint_host: "127.0.0.1",
    endpoint_port: 41731,
    endpoint_field: {
      field: "codex_openai_base_url",
      before: "missing",
      proposed_value: "http://127.0.0.1:41731/v1",
      after_headroom: true,
    },
    status: "preview",
    model: "present",
    reasoning: "present",
    model_unchanged: true,
    provider_unchanged: true,
    reasoning_unchanged: true,
    auth_unchanged: true,
    agent_count_unchanged: true,
    runtime_effective: "unknown",
    auth: "unknown",
    hook_shell: "absent",
    restart_required: true,
    blockers: [],
    ...overrides,
  };
}

function makeOwnership(overrides = {}) {
  return {
    state: "active",
    owner_id: ownerId,
    revision: 1,
    record_fingerprint: recordFingerprint,
    config_source: "codex_toml",
    endpoint_id: endpointId,
    endpoint_port: 41731,
    ...overrides,
  };
}

function success(status, ownership, overrides = {}) {
  return {
    schema_version: 1,
    request_id: requestId,
    status,
    client_kind: clientKind,
    ownership,
    ...overrides,
  };
}

function error(code = "config_conflict", overrides = {}) {
  const contracts = {
    validation_failed: [false, "connection.validation_failed"],
    unsupported_schema: [false, "connection.unsupported_schema"],
    busy: [true, "connection.busy"],
    not_found: [false, "connection.not_found"],
    plan_expired: [true, "connection.plan_expired"],
    config_conflict: [true, "connection.config_conflict"],
    config_invalid: [false, "connection.config_invalid"],
    config_unsafe: [false, "connection.config_unsafe"],
    config_unavailable: [true, "connection.config_unavailable"],
    unsupported_filesystem: [false, "connection.unsupported_filesystem"],
    commit_unknown: [true, "connection.commit_unknown"],
  };
  const [retryable, message_key] = contracts[code];
  return {
    schema_version: 1,
    request_id: requestId,
    status: "error",
    error: { code, retryable, message_key, diagnostic_id: null, ...overrides },
  };
}

function createClient(invoke, native = true) {
  return api.createConnectionActionClient({
    isNative: () => native,
    createRequestId: () => requestId,
    invoke,
  });
}

test("the typed connection action client is exported", () => {
  assert.equal(typeof api?.createConnectionActionClient, "function");
});

test("apply sends only its correlated narrow request and reconstructs an active result", async () => {
  let calls = 0;
  const client = createClient(async (command, args) => {
    calls += 1;
    assert.equal(command, "apply_connection_plan");
    assert.deepEqual(args, { request: {
      schema_version: 1,
      request_id: requestId,
      plan_id: planId,
      expected_config_fingerprint: fingerprint,
    } });
    return success("applied", makeOwnership());
  });
  const result = await client.apply(makePlan({ authorization: "fixture-secret", full_config: "private" }));
  assert.equal(calls, 1);
  assert.equal(result.kind, "success");
  assert.equal(result.reply.status, "applied");
  assert.equal(result.reply.ownership.endpoint_port, 41731);
});

test("apply accepts an unchanged result with no ownership and rejects blocked or expired plans before invoke", async () => {
  let calls = 0;
  const client = createClient(async () => {
    calls += 1;
    return success("unchanged", null);
  });
  assert.equal((await client.apply(makePlan())).kind, "success");
  const activeNoop = await createClient(async () => success("unchanged", makeOwnership())).apply(makePlan());
  assert.equal(activeNoop.kind, "success");
  assert.equal((await client.apply(makePlan({ status: "blocked", blockers: ["custom_gateway"] }))).kind, "invalid_request");
  assert.equal((await client.apply(makePlan({ expires_at_unix_seconds: 1 }))).kind, "invalid_request");
  assert.equal(calls, 1);
});

test("apply rejects mismatched identity, client, source, endpoint, status, and secret-shaped extra fields", async () => {
  const invalidReplies = [
    success("applied", makeOwnership(), { request_id: "019a0000-0000-7000-8000-000000000099" }),
    success("applied", makeOwnership(), { schema_version: 2 }),
    success("applied", makeOwnership(), { client_kind: "claude_code" }),
    success("applied", makeOwnership({ config_source: "claude_json" })),
    success("applied", makeOwnership({ endpoint_port: 0 })),
    success("applied", makeOwnership({ endpoint_id: "019a0000-0000-7000-8000-000000000099" })),
    success("applied", makeOwnership({ state: "released" })),
    { ...success("applied", makeOwnership()), authorization: "sentinel-secret" },
    success("success", makeOwnership()),
  ];
  for (const reply of invalidReplies) {
    const result = await createClient(async () => reply).apply(makePlan());
    assert.equal(result.kind, "invalid_response");
    assert.doesNotMatch(JSON.stringify(result), /sentinel-secret|authorization/i);
  }
});

test("unchanged active ownership must match the preview route", async () => {
  const reply = success("unchanged", makeOwnership({ endpoint_port: 41732 }));
  assert.equal((await createClient(async () => reply).apply(makePlan())).kind, "invalid_response");
});

test("undo sends only owner metadata and accepts exactly the next released revision", async () => {
  let calls = 0;
  const owned = makeOwnership();
  const client = createClient(async (command, args) => {
    calls += 1;
    assert.equal(command, "undo_connection");
    assert.deepEqual(args, { request: {
      schema_version: 1,
      request_id: requestId,
      client_kind: clientKind,
      owner_id: ownerId,
      expected_revision: 1,
      expected_record_fingerprint: recordFingerprint,
    } });
    return success("released", makeOwnership({ state: "released", revision: 2, record_fingerprint: "c".repeat(64) }));
  });
  const result = await client.undo(clientKind, owned);
  assert.equal(calls, 1);
  assert.equal(result.kind, "success");
  assert.equal(result.reply.status, "released");
});

test("undo rejects a wrong owner, revision, source, route, or result state", async () => {
  const cases = [
    makeOwnership({ revision: Number.MAX_SAFE_INTEGER }),
    makeOwnership({ config_source: "claude_json" }),
  ];
  for (const owned of cases) {
    assert.equal((await createClient(async () => success("released", makeOwnership({ state: "released", revision: 2 }))).undo(clientKind, owned)).kind, "invalid_request");
  }
  const badReplies = [
    success("released", makeOwnership({ state: "released", revision: 3 })),
    success("released", makeOwnership({ state: "released", revision: 2, owner_id: "019a0000-0000-7000-8000-000000000099" })),
    success("released", makeOwnership({ state: "released", revision: 2, owner_id: "019a0000-0000-7000-8000-000000000099" })),
    success("released", makeOwnership({ state: "released", revision: 2, config_source: "claude_json" })),
    success("released", makeOwnership({ state: "released", revision: 2, endpoint_port: 41732 })),
    success("released", makeOwnership({ state: "active", revision: 2 })),
  ];
  for (const reply of badReplies) {
    assert.equal((await createClient(async () => reply).undo(clientKind, makeOwnership())).kind, "invalid_response");
  }
});

test("ownership snapshot accepts only null or valid active/released records", async () => {
  assert.equal((await createClient(async () => success("snapshot", null)).snapshot(clientKind)).kind, "success");
  assert.equal((await createClient(async () => success("snapshot", makeOwnership({ state: "released" }))).snapshot(clientKind)).kind, "success");
  assert.equal((await createClient(async () => success("snapshot", makeOwnership({ state: "unknown" }))).snapshot(clientKind)).kind, "invalid_response");
});

test("fixed action error tuples are reconstructed and mismatched or raw errors are rejected", async () => {
  const result = await createClient(async () => error("config_unsafe")).snapshot(clientKind);
  assert.equal(result.kind, "action_error");
  assert.equal(result.error.code, "config_unsafe");
  assert.equal(result.error.retryable, false);
  assert.equal(result.error.message_key, "connection.config_unsafe");
  assert.equal(result.error.diagnostic_id, null);

  const mismatch = await createClient(async () => error("config_unsafe", { retryable: true })).snapshot(clientKind);
  assert.equal(mismatch.kind, "invalid_response");
  const thrown = await createClient(async () => { throw new Error("D:\\secret\\config.toml sentinel-secret"); }).snapshot(clientKind);
  assert.equal(thrown.kind, "request_failed");
  assert.doesNotMatch(JSON.stringify(thrown), /config\.toml|sentinel-secret/);

  for (const code of [
    "validation_failed", "unsupported_schema", "busy", "not_found", "plan_expired",
    "config_conflict", "config_invalid", "config_unsafe", "config_unavailable",
    "unsupported_filesystem", "commit_unknown",
  ]) {
    const decoded = await createClient(async () => error(code)).snapshot(clientKind);
    assert.equal(decoded.kind, "action_error", code);
    assert.equal(decoded.error.code, code);
    assert.equal(decoded.error.message_key, `connection.${code}`);
  }
});

test("browser calls stay unavailable and native failures are never retried", async () => {
  let calls = 0;
  const browser = createClient(async () => { calls += 1; return success("snapshot", null); }, false);
  assert.equal((await browser.snapshot(clientKind)).kind, "desktop_unavailable");
  const failing = createClient(async () => { calls += 1; throw new Error("bounded"); });
  assert.equal((await failing.snapshot(clientKind)).kind, "request_failed");
  assert.equal(calls, 1);
});

test("invalid client kinds and ownership records never reach native IPC", async () => {
  let calls = 0;
  const client = createClient(async () => { calls += 1; return success("snapshot", null); });
  assert.equal((await client.snapshot("unknown")).kind, "invalid_request");
  assert.equal((await client.undo(clientKind, { ...makeOwnership(), record_fingerprint: "NOT-A-HASH" })).kind, "invalid_request");
  assert.equal((await client.apply(makePlan({ endpoint_field: null }))).kind, "invalid_request");
  assert.equal(calls, 0);
});
