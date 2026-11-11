import assert from "node:assert/strict";
import test from "node:test";
import { createRetainedVersionsClient } from "../../src/lib/retainedVersions.ts";

const requestId = "123e4567-e89b-42d3-a456-426614174000";
const otherRequestId = "123e4567-e89b-42d3-a456-426614174001";
const fingerprint = "a".repeat(64);

function snapshot(overrides = {}) {
  return {
    schema_version: 1,
    request_id: requestId,
    status: "snapshot",
    client_kind: "claude_code",
    scope: "project_local_config",
    supported: true,
    review_fingerprint: fingerprint,
    active_records: 3,
    selected_records: 2,
    unresolved_records: 0,
    changed_versions: 1,
    completed_archives: 0,
    record_limit: 64,
    archive_limit: 64,
    can_archive: true,
    ...overrides,
  };
}

function backendError(code, retryable) {
  return {
    schema_version: 1,
    request_id: requestId,
    status: "error",
    error: { code, retryable, message_key: `retained.${code}`, diagnostic_id: null },
  };
}

function clientWith(reply, overrides = {}) {
  const calls = [];
  const client = createRetainedVersionsClient({
    isNative: () => true,
    createRequestId: () => requestId,
    invoke: async (command, args) => {
      calls.push({ command, args });
      return typeof reply === "function" ? reply(command, args) : reply;
    },
    ...overrides,
  });
  return { client, calls };
}

test("read sends the exact scoped request and accepts a correlated snapshot", async () => {
  const { client, calls } = clientWith((_command, args) => snapshot({ request_id: args.request.request_id }));
  const result = await client.read("claude_code", "project_local_config");
  assert.deepEqual(calls, [{
    command: "get_retained_config_versions",
    args: { request: {
      schema_version: 1,
      request_id: requestId,
      client_kind: "claude_code",
      scope: "project_local_config",
    } },
  }]);
  assert.equal(result.kind, "snapshot");
  assert.equal(result.data.status, "snapshot");
  assert.equal(result.data.changed_versions, 1);
});

test("archive is explicit, carries the reviewed fingerprint, and accepts only archived status", async () => {
  const { client, calls } = clientWith((_command, args) => snapshot({
    request_id: args.request.request_id,
    status: "archived",
    selected_records: 0,
    unresolved_records: 0,
    changed_versions: 0,
    completed_archives: 1,
    can_archive: false,
  }));
  assert.equal(calls.length, 0, "constructing the client must not archive anything");
  const result = await client.archive("claude_code", "project_local_config", fingerprint);
  assert.equal(calls.length, 1);
  assert.deepEqual(calls[0], {
    command: "archive_retained_config_versions",
    args: { request: {
      schema_version: 1,
      request_id: requestId,
      client_kind: "claude_code",
      scope: "project_local_config",
      expected_review_fingerprint: fingerprint,
    } },
  });
  assert.equal(result.kind, "snapshot");
  assert.equal(result.data.status, "archived");
});

test("unknown keys and request/client/scope/status mismatches fail closed", async (t) => {
  const cases = [
    ["unknown snapshot key", snapshot({ private_path: "C:\\private" })],
    ["wrong request id", snapshot({ request_id: otherRequestId })],
    ["wrong client", snapshot({ client_kind: "codex" })],
    ["wrong scope", snapshot({ scope: "user_config" })],
    ["wrong operation status", snapshot({ status: "archived" })],
    ["unknown error key", backendError("busy", true)],
  ];
  cases[cases.length - 1][1].error.raw = "do not expose";
  for (const [name, reply] of cases) {
    await t.test(name, async () => {
      const { client } = clientWith(reply);
      const result = await client.read("claude_code", "project_local_config");
      assert.deepEqual(result, {
        kind: "error",
        data: { code: "invalid_response", retryable: false, message_key: "retained.invalid_response", diagnostic_id: null },
      });
    });
  }
});

test("snapshot invariants, bounds, and unsupported shape are validated", async (t) => {
  const invalid = [
    snapshot({ active_records: 65 }),
    snapshot({ selected_records: 4 }),
    snapshot({ unresolved_records: 1 }),
    snapshot({ changed_versions: 3 }),
    snapshot({ can_archive: false }),
    snapshot({ review_fingerprint: null }),
    snapshot({ supported: false }),
  ];
  for (let index = 0; index < invalid.length; index += 1) {
    await t.test(`invalid snapshot ${index + 1}`, async () => {
      const { client } = clientWith(invalid[index]);
      assert.equal((await client.read("claude_code", "project_local_config")).data.code, "invalid_response");
    });
  }

  const unsupported = snapshot({
    supported: false,
    review_fingerprint: null,
    active_records: 0,
    selected_records: 0,
    unresolved_records: 0,
    changed_versions: 0,
    completed_archives: 0,
    can_archive: false,
  });
  const { client } = clientWith(unsupported);
  const result = await client.read("claude_code", "project_local_config");
  assert.equal(result.kind, "snapshot");
  assert.equal(result.data.supported, false);
});

test("scope restrictions and malformed archive fingerprints invoke nothing", async () => {
  const { client, calls } = clientWith(snapshot());
  assert.equal((await client.read("codex", "project_local_config")).data.code, "invalid_input");
  assert.equal((await client.archive("claude_code", "user_config", "not-a-hash")).data.code, "invalid_input");
  assert.equal(calls.length, 0);
});

test("backend errors are exact and use fixed retryability without exposing raw fields", async (t) => {
  const cases = [
    ["invalid_input", false], ["scope_changed", true], ["busy", true], ["conflict", true],
    ["unavailable", true], ["archive_limit", false], ["commit_unknown", true],
  ];
  for (const [code, retryable] of cases) {
    await t.test(code, async () => {
      const { client } = clientWith((_command, args) => ({
        ...backendError(code, retryable), request_id: args.request.request_id,
      }));
      const result = await client.read("claude_code", "project_local_config");
      assert.deepEqual(result, {
        kind: "error",
        data: { code, retryable, message_key: `retained.${code}`, diagnostic_id: null },
      });
    });
  }

  const malformed = backendError("busy", false);
  const { client } = clientWith(malformed);
  const result = await client.read("claude_code", "project_local_config");
  assert.equal(result.data.code, "invalid_response");
});

test("browser and rejected invoke paths are fixed unavailable with no raw error", async () => {
  let calls = 0;
  const browser = createRetainedVersionsClient({
    isNative: () => false,
    createRequestId: () => requestId,
    invoke: async () => { calls += 1; return snapshot(); },
  });
  assert.deepEqual(await browser.read("claude_code", "user_config"), {
    kind: "error",
    data: { code: "unavailable", retryable: true, message_key: "retained.unavailable", diagnostic_id: null },
  });
  assert.equal(calls, 0);

  const rejecting = createRetainedVersionsClient({
    isNative: () => true,
    createRequestId: () => requestId,
    invoke: async () => { throw new Error("private path and token"); },
  });
  const result = await rejecting.read("claude_code", "user_config");
  assert.equal(result.data.code, "unavailable");
  assert.equal(JSON.stringify(result).includes("private path"), false);
});

test("invalid request UUID factory is unavailable without invoking native", async () => {
  let calls = 0;
  const client = createRetainedVersionsClient({
    isNative: () => true,
    createRequestId: () => "00000000-0000-0000-0000-000000000000",
    invoke: async () => { calls += 1; return snapshot(); },
  });
  assert.equal((await client.read("codex", "user_config")).data.code, "unavailable");
  assert.equal(calls, 0);
});

test("single flight releases after completion and after rejection", async () => {
  let resolveFirst;
  let calls = 0;
  const client = createRetainedVersionsClient({
    isNative: () => true,
    createRequestId: () => requestId,
    invoke: (_command, args) => {
      calls += 1;
      if (calls === 1) return new Promise((resolve) => { resolveFirst = resolve; });
      if (calls === 2) throw new Error("private native detail");
      return snapshot({ request_id: args.request.request_id, scope: args.request.scope });
    },
  });
  const first = client.read("claude_code", "user_config");
  const concurrent = await client.archive("claude_code", "user_config", fingerprint);
  assert.equal(concurrent.data.code, "busy");
  resolveFirst(snapshot({ scope: "user_config" }));
  assert.equal((await first).kind, "snapshot");
  assert.equal((await client.read("claude_code", "user_config")).data.code, "unavailable");
  const final = await client.read("claude_code", "user_config");
  assert.equal(final.kind, "snapshot");
  assert.equal(calls, 3);
});
