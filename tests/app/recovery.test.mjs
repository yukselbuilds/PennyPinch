import test from "node:test";
import assert from "node:assert/strict";
import { createRecoveryClient } from "../../src/lib/recovery.ts";

const requestId = "7132bced-3ce0-4c1b-8f48-d93d7b96f7ce";

function client(invoke, overrides = {}) {
  return createRecoveryClient({
    isAvailable: () => true,
    requestId: () => requestId,
    invoke,
    ...overrides,
  });
}

function snapshot(overrides = {}) {
  return {
    schema_version: 1,
    request_id: requestId,
    status: "snapshot",
    observed_at_utc: "2026-11-08T10:15:30Z",
    runtime: { engine_status: "ready", ingress_status: "ready", error_code: null },
    traffic: { status: "none", period: "seven_days", last_observed_utc: null, outcome: "unknown", error_layer: "unknown" },
    clients: [
      { client_kind: "codex", detected_count: 1, installation_status: "current", compatibility: "config_supported", config_state: "matches_owned_endpoint", config_scope: "user_config", endpoint_binding: "unknown" },
      { client_kind: "claude_code", detected_count: 0, installation_status: "not_found", compatibility: "not_verified", config_state: "not_owned", config_scope: "unknown", endpoint_binding: "not_owned" },
      { client_kind: "opencode", detected_count: 2, installation_status: "stale", compatibility: "not_verified", config_state: "conflict", config_scope: "project_local_config", endpoint_binding: "different" },
    ],
    ...overrides,
  };
}

test("sends one exact request and returns a valid empty-traffic snapshot", async () => {
  const calls = [];
  const api = client(async (...args) => { calls.push(args); return snapshot(); });
  assert.deepEqual(await api.getSnapshot(), { kind: "snapshot", snapshot: snapshot() });
  assert.deepEqual(calls, [["get_recovery_snapshot", { request: { schema_version: 1, request_id: requestId } }]]);
});

test("accepts observed traffic and redacted client states without implying success", async () => {
  const value = snapshot({
    traffic: { status: "observed", period: "seven_days", last_observed_utc: "2026-11-08T10:10:00.125Z", outcome: "error_reported", error_layer: "unknown" },
  });
  assert.deepEqual(await client(async () => value).getSnapshot(), { kind: "snapshot", snapshot: value });
});

test("rejects unknown fields, bad correlation, malformed enums/counts and inconsistent traffic timestamps", async () => {
  const invalid = [
    snapshot({ private_path: "C:\\secret" }),
    snapshot({ request_id: "a87abcea-c90c-4dbb-bede-3f3de05f161f" }),
    snapshot({ schema_version: 2 }),
    snapshot({ observed_at_utc: "yesterday" }),
    snapshot({ runtime: { ...snapshot().runtime, error_code: "auth_error" } }),
    snapshot({ runtime: { ...snapshot().runtime, private: "token" } }),
    snapshot({ traffic: { ...snapshot().traffic, last_observed_utc: "2026-11-08T10:10:00Z" } }),
    snapshot({ traffic: { ...snapshot().traffic, status: "observed", last_observed_utc: null } }),
    snapshot({ traffic: { ...snapshot().traffic, status: "unavailable", outcome: "no_error_reported" } }),
    snapshot({ traffic: { ...snapshot().traffic, error_layer: "provider" } }),
    snapshot({ clients: snapshot().clients.slice(0, 2) }),
    snapshot({ clients: [snapshot().clients[1], snapshot().clients[0], snapshot().clients[2]] }),
    snapshot({ clients: snapshot().clients.map((row, i) => i ? row : { ...row, detected_count: 513 }) }),
    snapshot({ clients: snapshot().clients.map((row, i) => i ? row : { ...row, installation_status: "not_found", detected_count: 1 }) }),
    snapshot({ clients: snapshot().clients.map((row, i) => i ? row : { ...row, config_scope: "C:\\private" }) }),
    snapshot({ clients: snapshot().clients.map((row, i) => i ? row : { ...row, config_state: "matches_owned_endpoint", config_scope: "unknown", endpoint_binding: "same" }) }),
    snapshot({ clients: snapshot().clients.map((row, i) => i ? row : { ...row, config_state: "matches_owned_endpoint", compatibility: "not_verified" }) }),
    snapshot({ clients: snapshot().clients.map((row, i) => i ? row : { ...row, installation_status: "stale", detected_count: 0 }) }),
    snapshot({ clients: snapshot().clients.map((row, i) => i ? row : { ...row, compatibility: "config_supported", installation_status: "unknown" }) }),
    snapshot({ clients: snapshot().clients.map((row, i) => i ? row : { ...row, config_state: "not_owned", endpoint_binding: "different" }) }),
  ];
  for (const reply of invalid) {
    assert.deepEqual(await client(async () => reply).getSnapshot(), { kind: "invalid_response" });
  }
});

test("validates fixed native errors and suppresses unknown error fields", async () => {
  for (const [code, retryable] of [["invalid_input", false], ["busy", true], ["unavailable", false]]) {
    assert.deepEqual(await client(async () => ({
      schema_version: 1, request_id: requestId, status: "error",
      error: { code, retryable, message_key: `recovery.${code}`, diagnostic_id: null },
    })).getSnapshot(), { kind: "recovery_error", code, retryable });
  }
  const invalid = [
    { code: "busy", retryable: false, message_key: "recovery.busy", diagnostic_id: null },
    { code: "busy", retryable: true, message_key: "recovery.busy", diagnostic_id: "private" },
    { code: "future", retryable: false, message_key: "recovery.future", diagnostic_id: null },
    { code: "unavailable", retryable: false, message_key: "recovery.unavailable", diagnostic_id: null, path: "C:\\secret" },
  ];
  for (const error of invalid) {
    const result = await client(async () => ({ schema_version: 1, request_id: requestId, status: "error", error })).getSnapshot();
    assert.deepEqual(result, { kind: "invalid_response" });
    assert.equal(JSON.stringify(result).includes("secret"), false);
  }
});

test("does not invoke when unavailable or request ID is invalid", async () => {
  let calls = 0;
  const invoke = async () => { calls++; return snapshot(); };
  assert.deepEqual(await client(invoke, { isAvailable: () => false }).getSnapshot(), { kind: "desktop_unavailable" });
  assert.deepEqual(await client(invoke, { requestId: () => "not-a-uuid" }).getSnapshot(), { kind: "request_failed" });
  assert.equal(calls, 0);
});

test("masks thrown native errors and releases the request guard", async () => {
  let calls = 0;
  const api = client(async () => {
    calls++;
    if (calls === 1) throw new Error("C:\\secret\\config with token");
    return snapshot();
  });
  assert.deepEqual(await api.getSnapshot(), { kind: "request_failed" });
  assert.deepEqual(await api.getSnapshot(), { kind: "snapshot", snapshot: snapshot() });
  assert.equal(calls, 2);
});

test("allows one in-flight request and does not invoke a duplicate", async () => {
  let resolve;
  let calls = 0;
  const api = client(() => { calls++; return new Promise(done => { resolve = done; }); });
  const first = api.getSnapshot();
  assert.deepEqual(await api.getSnapshot(), { kind: "client_busy" });
  assert.equal(calls, 1);
  resolve(snapshot());
  assert.deepEqual(await first, { kind: "snapshot", snapshot: snapshot() });
});
