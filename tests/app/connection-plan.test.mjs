import assert from "node:assert/strict";
import test from "node:test";
import {
  createConnectionPlanClient,
  decodeConnectionPlanReply,
} from "../../src/lib/connectionPlan.ts";

const requestId = "019a0000-0000-7000-8000-000000000001";
const installationId = "019a0000-0000-7000-8000-000000000002";
const planId = "019a0000-0000-7000-8000-000000000003";
const fingerprint = "a".repeat(64);

function makePlan(overrides = {}) {
  return {
    schema_version: 1,
    request_id: requestId,
    plan_id: planId,
    expires_at_unix_seconds: 1_800_000_300,
    installation_id: installationId,
    client_kind: "codex",
    scope: "user_config",
    scan_generation: 7,
    config_fingerprint: fingerprint,
    config_presence: "present",
    config_source: "codex_toml",
    endpoint_id: "019a0000-0000-7000-8000-000000000004",
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

function makePreview(plan = makePlan()) {
  return {
    schema_version: 1,
    request_id: requestId,
    status: "preview",
    plan,
  };
}

function makeError(overrides = {}) {
  return {
    schema_version: 1,
    request_id: requestId,
    status: "error",
    error: {
      code: "config_conflict",
      retryable: true,
      message_key: "connection.config_conflict",
      diagnostic_id: null,
      ...overrides,
    },
  };
}

function decode(value, expectedRequestId = requestId, clientKind = "codex") {
  return decodeConnectionPlanReply(value, {
    requestId: expectedRequestId,
    installationId,
    scanGeneration: 7,
    clientKind,
  });
}

test("strictly decodes a correlated preview and reconstructs only typed plan fields", () => {
  const result = decode(makePreview());
  assert.equal(result.kind, "preview");
  assert.equal(result.plan.config_source, "codex_toml");
  assert.equal(result.plan.endpoint_field.proposed_value, "http://127.0.0.1:41731/v1");

  const extended = makePreview();
  extended.plan.private_path = "D:\\private\\config.toml";
  assert.equal(decode(extended), null);

  const nestedExtended = makePreview();
  nestedExtended.plan.endpoint_field.authorization = "secret";
  assert.equal(decode(nestedExtended), null);
});

test("rejects request, generation, source, endpoint, and status inconsistencies", () => {
  assert.equal(decode(makePreview(), "019a0000-0000-7000-8000-000000000099"), null);
  assert.equal(decode(makePreview(makePlan({ scan_generation: Number.MAX_SAFE_INTEGER + 1 }))), null);
  assert.equal(decode(makePreview(makePlan({ config_source: "opencode_json" }))), null);
  assert.equal(decode(makePreview(makePlan({ endpoint_port: 41732 }))), null);
  assert.equal(decode(makePreview(makePlan({ expires_at_unix_seconds: 8_640_000_000_001 }))), null);
  assert.equal(decode(makePreview(makePlan({ restart_required: false }))), null);
  assert.equal(decode(makePreview(makePlan({ model_unchanged: false }))), null);
  assert.equal(decode(makePreview(makePlan({ endpoint_field: {
    ...makePlan().endpoint_field,
    proposed_value: "https://example.invalid/v1",
  } }))), null);
  assert.equal(decode(makePreview(makePlan({ status: "blocked", blockers: [] }))), null);
  assert.equal(decode(makePreview(makePlan({ status: "preview", blockers: ["custom_gateway"] }))), null);
});

test("accepts only exact fixed error envelopes and sanitizes their contents", () => {
  const result = decode(makeError());
  assert.deepEqual(result, {
    kind: "plan_error",
    error: {
      code: "config_conflict",
      retryable: true,
      message_key: "connection.config_conflict",
      diagnostic_id: null,
    },
  });
  assert.equal(decode(makeError({ code: "custom", message_key: "secret path" })), null);
  assert.equal(decode(makeError({ retryable: false })), null);
  const extra = makeError();
  extra.error.raw = "D:\\private";
  assert.equal(decode(extra), null);
});

test("validates client-specific endpoint and config source pairs", () => {
  const claude = makePlan({
    client_kind: "claude_code",
    config_source: "claude_json",
    endpoint_field: {
      field: "claude_anthropic_base_url",
      before: "same_stable_endpoint",
      proposed_value: "http://127.0.0.1:41731",
      after_headroom: true,
    },
  });
  assert.equal(decode(makePreview(claude), requestId, "claude_code").kind, "preview");
  assert.equal(decode(makePreview(claude), requestId, "codex"), null);

  const opencode = makePlan({
    client_kind: "opencode_v1",
    config_source: "opencode_jsonc",
    config_presence: "present",
    endpoint_field: {
      field: "opencode_v1_openai_base_url",
      before: "missing",
      proposed_value: "http://127.0.0.1:41731/v1",
      after_headroom: true,
    },
  });
  assert.equal(decode(makePreview(opencode), requestId, "opencode").kind, "preview");
  assert.equal(decode(makePreview({ ...opencode, config_presence: "missing" }), requestId, "opencode"), null);
});

test("missing config plans carry only the default-preserving declarations", () => {
  const missingCodex = makePlan({
    config_presence: "missing",
    model: "absent",
    reasoning: "absent",
    auth: "unknown",
    hook_shell: "absent",
  });
  assert.equal(decode(makePreview(missingCodex)).kind, "preview");
  assert.equal(decode(makePreview({ ...missingCodex, model: "present" })), null);
  assert.equal(decode(makePreview({ ...missingCodex, auth: "secret_configured" })), null);
  assert.equal(decode(makePreview({
    ...missingCodex,
    endpoint_field: { ...missingCodex.endpoint_field, before: "same_stable_endpoint" },
  })), null);
  assert.equal(decode(makePreview({ ...missingCodex, hook_shell: "present_unchanged" })), null);

  const missingOpenCode = makePlan({
    client_kind: "opencode_v1",
    config_presence: "missing",
    config_source: "opencode_missing",
    model: "absent",
    reasoning: "unknown",
    auth: "unknown",
    hook_shell: "absent",
    endpoint_field: {
      field: "opencode_v1_openai_base_url",
      before: "missing",
      proposed_value: "http://127.0.0.1:41731/v1",
      after_headroom: false,
    },
    status: "blocked",
    blockers: ["client_version_unknown"],
  });
  assert.equal(decode(makePreview(missingOpenCode), requestId, "opencode").kind, "preview");
  assert.equal(decode(makePreview({ ...missingOpenCode, reasoning: "absent" }), requestId, "opencode"), null);
});

test("native client sends only the typed preview request and returns fixed failure results", async () => {
  let invoked = null;
  const native = createConnectionPlanClient({
    isNative: () => true,
    createRequestId: () => requestId,
    invoke: async (command, args) => {
      invoked = { command, args };
      return makePreview();
    },
  });
  const result = await native.preview({ installationId, scanGeneration: 7, clientKind: "codex" });
  assert.equal(result.kind, "preview");
  assert.deepEqual(invoked, {
    command: "preview_connection_plan",
    args: {
      request: {
        schema_version: 1,
        request_id: requestId,
        installation_id: installationId,
        scan_generation: 7,
        scope: "user_config",
      },
    },
  });

  let bridgeCalls = 0;
  const unavailable = createConnectionPlanClient({
    isNative: () => false,
    createRequestId: () => requestId,
    invoke: async () => { bridgeCalls += 1; return makePreview(); },
  });
  assert.deepEqual(await unavailable.preview({ installationId, scanGeneration: 7, clientKind: "codex" }), {
    kind: "desktop_unavailable",
  });
  assert.equal(bridgeCalls, 0);

  const failed = createConnectionPlanClient({
    isNative: () => true,
    createRequestId: () => requestId,
    invoke: async () => { throw new Error("D:\\private\\credential"); },
  });
  assert.deepEqual(await failed.preview({ installationId, scanGeneration: 7, clientKind: "codex" }), {
    kind: "request_failed",
  });
});
