import test from "node:test";
import assert from "node:assert/strict";
import {
  createDesktopClient,
  decodeDesktopError,
  decodeDesktopSnapshot,
} from "../../src/lib/desktop-events.ts";

const requestId = "7132bced-3ce0-4c1b-8f48-d93d7b96f7ce";
const nextRequestId = "8132bced-3ce0-4c1b-8f48-d93d7b96f7ce";
const exitToken = "9132bced-3ce0-4c1b-8f48-d93d7b96f7ce";

function snapshot(id = requestId, overrides = {}) {
  return {
    schema_version: 1,
    request_id: id,
    status: "snapshot",
    tray_available: true,
    close_to_tray: true,
    exit_state: "idle",
    exit_token: null,
    ...overrides,
  };
}

function error(id = requestId, code = "busy") {
  return {
    schema_version: 1,
    request_id: id,
    status: "error",
    error: {
      code,
      retryable: code !== "validation_failed",
      message_key: `desktop.${code}`,
      diagnostic_id: null,
    },
  };
}

function client(invoke, options = {}) {
  const ids = options.ids ?? [requestId, nextRequestId, exitToken];
  let index = 0;
  return createDesktopClient({
    isNative: options.isNative ?? (() => true),
    invoke,
    createRequestId: () => ids[index++] ?? null,
  });
}

test("accepts only exact snapshots with valid exit-state/token combinations", () => {
  assert.deepEqual(decodeDesktopSnapshot(snapshot(), requestId), snapshot());
  assert.equal(decodeDesktopSnapshot(snapshot(requestId, {
    exit_state: "confirmation_required", exit_token: exitToken,
  }), requestId)?.exit_token, exitToken);
  assert.equal(decodeDesktopSnapshot({ ...snapshot(), extra: true }, requestId), null);
  assert.equal(decodeDesktopSnapshot(snapshot(requestId, {
    exit_state: "confirmation_required", exit_token: null,
  }), requestId), null);
  assert.equal(decodeDesktopSnapshot(snapshot(requestId, {
    exit_state: "idle", exit_token: exitToken,
  }), requestId), null);
  assert.equal(decodeDesktopSnapshot(snapshot(nextRequestId), requestId), null);
  assert.equal(decodeDesktopSnapshot(snapshot(requestId.toUpperCase()), requestId.toUpperCase()), null);
});

test("accepts only fixed desktop error codes and their exact retry mappings", () => {
  for (const code of ["validation_failed", "state_conflict", "busy"]) {
    assert.equal(decodeDesktopError(error(requestId, code), requestId)?.error.code, code);
  }
  assert.equal(decodeDesktopError(error(requestId, "busy"), requestId)?.error.retryable, true);
  assert.equal(decodeDesktopError({ ...error(), detail: "fixture-secret" }, requestId), null);
  assert.equal(decodeDesktopError(error(requestId, "unknown"), requestId), null);
  assert.equal(decodeDesktopError({ ...error(), error: { ...error().error, retryable: false } }, requestId), null);
});

test("sends exact correlated invoke names and camelCase arguments", async () => {
  const calls = [];
  const client = createDesktopClient({
    isNative: () => true,
    createRequestId: (() => {
      const ids = [requestId, nextRequestId, exitToken, "a132bced-3ce0-4c1b-8f48-d93d7b96f7ce"];
      return () => ids.shift() ?? null;
    })(),
    invoke: async (command, args) => {
      calls.push([command, args]);
      if (command === "get_desktop_state") return snapshot(requestId);
      if (command === "request_desktop_exit") return snapshot(nextRequestId, {
        exit_state: "confirmation_required", exit_token: exitToken,
      });
      if (command === "cancel_desktop_exit") return snapshot(exitToken);
      return snapshot("a132bced-3ce0-4c1b-8f48-d93d7b96f7ce", { exit_state: "shutting_down" });
    },
  });

  assert.equal((await client.getState()).kind, "snapshot");
  assert.equal((await client.requestExit()).kind, "snapshot");
  assert.equal((await client.cancelExit(exitToken)).kind, "snapshot");
  assert.equal((await client.confirmExit(exitToken)).kind, "snapshot");
  assert.deepEqual(calls, [
    ["get_desktop_state", { requestId }],
    ["request_desktop_exit", { requestId: nextRequestId }],
    ["cancel_desktop_exit", { requestId: exitToken, exitToken }],
    ["confirm_desktop_exit", { requestId: "a132bced-3ce0-4c1b-8f48-d93d7b96f7ce", exitToken }],
  ]);
});

test("requires the operation-specific accepted state and a valid token", async () => {
  let calls = 0;
  const api = client(async () => { calls += 1; return snapshot(); });
  assert.equal((await api.requestExit()).kind, "invalid_response");
  assert.equal((await api.confirmExit("not-a-uuid")).kind, "invalid_request");
  assert.equal(calls, 1);
});

test("native-unavailable client never invokes desktop commands", async () => {
  let calls = 0;
  const api = client(async () => { calls += 1; return snapshot(); }, { isNative: () => false });
  assert.equal((await api.getState()).kind, "desktop_unavailable");
  assert.equal((await api.requestExit()).kind, "desktop_unavailable");
  assert.equal(calls, 0);
});

test("invalid request IDs and invoke failures return fixed typed results", async () => {
  let calls = 0;
  const invalidId = createDesktopClient({
    isNative: () => true, createRequestId: () => "BAD-ID",
    invoke: async () => { calls += 1; return snapshot(); },
  });
  assert.equal((await invalidId.getState()).kind, "invalid_request");
  const failed = client(async () => { throw new Error("fixture-secret"); });
  assert.equal((await failed.getState()).kind, "request_failed");
  assert.equal(calls, 0);
});

test("accepts fixed native errors and rejects mismatched correlations", async () => {
  const api = client(async (_command, args) => error(args.requestId, "state_conflict"));
  const result = await api.requestExit();
  assert.equal(result.kind, "desktop_error");
  if (result.kind === "desktop_error") assert.equal(result.error.error.code, "state_conflict");
  const mismatch = client(async () => error(nextRequestId, "busy"));
  assert.equal((await mismatch.getState()).kind, "invalid_response");
});

test("allows one lifecycle mutation at a time and releases the lock afterward", async () => {
  let release;
  let entered;
  const started = new Promise(resolve => { entered = resolve; });
  const api = client(() => new Promise(resolve => {
    release = resolve;
    entered();
  }));
  const first = api.requestExit();
  await started;
  assert.equal((await api.cancelExit(exitToken)).kind, "client_busy");
  release(snapshot(requestId, { exit_state: "confirmation_required", exit_token: exitToken }));
  assert.equal((await first).kind, "snapshot");
});
