import test from "node:test";
import assert from "node:assert/strict";
import {
  createOperatingClient,
  decodeOperatingSnapshot,
} from "../../src/lib/operatingSettings.ts";

const requestId = "7132bced-3ce0-4c1b-8f48-d93d7b96f7ce";
const nextRequestId = "8132bced-3ce0-4c1b-8f48-d93d7b96f7ce";
const fingerprint = "ab".repeat(32);
const generation = "0123456789abcdef0123456789abcdef";

function snapshot(id = requestId, overrides = {}) {
  return {
    schema_version: 1,
    request_id: id,
    status: "snapshot",
    autostart: { status: "disabled", fingerprint: null },
    runtime: {
      status: "unknown",
      runtime_generation: null,
      protected_tools: null,
      local_stats_enabled: null,
      beacon_enabled: null,
      mode: null,
      savings_profile: null,
      ccr_ttl_seconds: null,
    },
    ...overrides,
  };
}

function verifiedRuntime(overrides = {}) {
  return {
    status: "verified",
    runtime_generation: generation,
    protected_tools: ["read_file", "write_file"],
    local_stats_enabled: true,
    beacon_enabled: false,
    mode: "cache",
    savings_profile: "coding",
    ccr_ttl_seconds: 7200,
    ...overrides,
  };
}

function makeClient(invoke, options = {}) {
  const ids = options.ids ?? [requestId, nextRequestId, requestId];
  return createOperatingClient({
    isNative: options.isNative ?? (() => true),
    createRequestId: () => ids.shift() ?? null,
    invoke,
  });
}

test("decodes verified runtime pins separately from unknown and inactive states", () => {
  const verified = snapshot(requestId, {
    autostart: { status: "registered", fingerprint },
    runtime: verifiedRuntime(),
  });
  assert.deepEqual(decodeOperatingSnapshot(verified, requestId)?.runtime, verifiedRuntime());
  assert.equal(decodeOperatingSnapshot(snapshot(), requestId)?.runtime.status, "unknown");
  assert.equal(decodeOperatingSnapshot(snapshot(requestId, {
    runtime: { status: "inactive", runtime_generation: null, protected_tools: null, local_stats_enabled: null,
      beacon_enabled: null, mode: null, savings_profile: null, ccr_ttl_seconds: null },
  }), requestId)?.runtime.status, "inactive");
});

test("rejects extra fields, correlation drift, unsafe autostart and inconsistent runtime claims", () => {
  assert.equal(decodeOperatingSnapshot({ ...snapshot(), extra: true }, requestId), null);
  assert.equal(decodeOperatingSnapshot(snapshot(nextRequestId), requestId), null);
  assert.equal(decodeOperatingSnapshot(snapshot(requestId, {
    autostart: { status: "registered", fingerprint: null },
  }), requestId), null);
  assert.equal(decodeOperatingSnapshot(snapshot(requestId, {
    runtime: verifiedRuntime({ beacon_enabled: true }),
  }), requestId), null);
  assert.equal(decodeOperatingSnapshot(snapshot(requestId, {
    runtime: verifiedRuntime({ runtime_generation: generation.toUpperCase() }),
  }), requestId), null);
  assert.equal(decodeOperatingSnapshot(snapshot(requestId, {
    runtime: { ...snapshot().runtime, status: "inactive", mode: "cache" },
  }), requestId), null);
});

test("reads the exact desktop snapshot command and request ID", async () => {
  const calls = [];
  const client = makeClient(async (command, args) => {
    calls.push([command, args]);
    return snapshot(args.requestId, { runtime: verifiedRuntime() });
  });
  const result = await client.read();
  assert.equal(result.kind, "snapshot");
  assert.deepEqual(calls, [["get_operating_snapshot", { requestId }]]);
});

test("sets startup only with the last observed fingerprint and exact bool", async () => {
  const calls = [];
  const client = makeClient(async (command, args) => {
    calls.push([command, args]);
    return snapshot(args.requestId, { autostart: { status: "registered", fingerprint } });
  });
  const result = await client.setAutostart(true, fingerprint);
  assert.equal(result.kind, "snapshot");
  assert.deepEqual(calls, [["set_autostart", { requestId, enabled: true, expectedFingerprint: fingerprint }]]);
  assert.equal((await client.setAutostart("yes", fingerprint)).kind, "error");
});

test("unavailable native bridge does not invoke commands", async () => {
  let calls = 0;
  const client = makeClient(async () => { calls += 1; return snapshot(); }, { isNative: () => false });
  assert.equal((await client.read()).kind, "unavailable");
  assert.equal((await client.setAutostart(true, null)).kind, "unavailable");
  assert.equal(calls, 0);
});

test("serializes startup mutations and returns only fixed errors", async () => {
  let entered;
  let release;
  const started = new Promise(resolve => { entered = resolve; });
  const client = makeClient(() => new Promise(resolve => {
    release = resolve;
    entered();
  }));
  const first = client.setAutostart(true, null);
  await started;
  assert.equal((await client.setAutostart(false, null)).kind, "client_busy");
  release(snapshot(requestId, { autostart: { status: "registered", fingerprint } }));
  assert.equal((await first).kind, "snapshot");

  const errorClient = makeClient(async (_command, args) => ({
    schema_version: 1,
    request_id: args.requestId,
    status: "error",
    error: { code: "config_conflict", retryable: true, message_key: "operating.config_conflict", diagnostic_id: null },
  }));
  assert.deepEqual(await errorClient.read(), { kind: "error", code: "config_conflict" });
});
