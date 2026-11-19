import test from "node:test";
import assert from "node:assert/strict";
import { createUpdateClient, decodeUpdateReply } from "../../src/lib/updates.ts";

const requestId = "7132bced-3ce0-4c1b-8f48-d93d7b96f7ce";
const candidateId = "8132bced-3ce0-4c1b-8f48-d93d7b96f7ce";

function reply(overrides = {}) {
  return {
    schema_version: 1,
    request_id: requestId,
    installed_version: "0.1.0",
    status: "waiting_for_engine",
    channel: "stable",
    candidate: { id: candidateId, version: "0.1.1", size_bytes: 131036003, notes: "Release note" },
    error: null,
    can_apply: false,
    recovery: null,
    ...overrides,
  };
}

const invalid = { kind: "error", code: "invalid_response" };

test("preserves the distinction between a downloaded package waiting for the engine and a ready package", () => {
  for (const status of ["waiting_for_engine", "ready"]) {
    const result = decodeUpdateReply(reply({ status }), requestId);
    assert.equal(result.kind, "snapshot");
    assert.equal(result.snapshot.status, status);
    assert.equal(result.snapshot.candidate.size_bytes, 131036003);
  }
  assert.deepEqual(decodeUpdateReply(reply({ candidate: null }), requestId), invalid);
  assert.deepEqual(decodeUpdateReply(reply({ status: "idle", channel: null }), requestId), invalid);
});

test("rejects unrelated replies, file paths, and unbounded release metadata", () => {
  for (const patch of [
    { request_id: candidateId },
    { candidate: { ...reply().candidate, path: "C:\\fixture.exe" } },
    { candidate: { ...reply().candidate, size_bytes: -1 } },
    { candidate: { ...reply().candidate, notes: "λ".repeat(2049) } },
  ]) assert.deepEqual(decodeUpdateReply(reply(patch), requestId), invalid);
});

test("binds native checks and downloads to the selected channel and candidate", async () => {
  const calls = [];
  const client = createUpdateClient({
    isNative: () => true,
    requestId: () => requestId,
    invoke: async (command, args) => {
      calls.push({ command, args });
      return reply({ status: args.request.action.kind === "check" ? "available" : "ready" });
    },
  });
  const check = { kind: "check", channel: "preview" };
  assert.deepEqual(await client.request(check), invalid);
  assert.deepEqual(calls[0], { command: "manage_updates", args: { request: { request_id: requestId, action: check } } });
  assert.deepEqual(await client.request({ kind: "download", candidate_id: requestId }), invalid);
  assert.equal((await client.request({ kind: "download", candidate_id: candidateId })).snapshot.status, "ready");
});

test("does not use browser transport and preserves safe retry errors", async () => {
  const browser = createUpdateClient({ isNative: () => false, requestId: () => requestId, invoke: () => assert.fail("browser call") });
  assert.deepEqual(await browser.request({ kind: "snapshot" }), { kind: "error", code: "unavailable" });
  const error = reply({ status: "error", channel: null, candidate: null, error: "integrity_failed" });
  assert.deepEqual(decodeUpdateReply(error, requestId), { kind: "error", code: "integrity_failed" });
  assert.deepEqual(decodeUpdateReply({ ...error, error: "file C:\\fixture.exe" }, requestId), invalid);
});

test("installation capability is native and bound to a ready candidate", () => {
  const ready = reply({ status: "ready", can_apply: true });
  assert.equal(decodeUpdateReply(ready, requestId).snapshot.can_apply, true);
  for (const patch of [{ status: "waiting_for_engine" }, { status: "available" }, { can_apply: "true" }]) {
    assert.deepEqual(decodeUpdateReply({ ...ready, ...patch }, requestId), invalid);
  }
  const recovery = { id: candidateId, version: "0.0.9", can_restore: false };
  const status = decodeUpdateReply(reply({ status: "unavailable", channel: null, candidate: null, recovery }), requestId);
  assert.deepEqual(status.snapshot.recovery, recovery);
  assert.deepEqual(decodeUpdateReply(reply({ recovery: { ...recovery, path: "D:\\private.exe" } }), requestId), invalid);
});

test("apply and restore acknowledge the actual exit transition", async () => {
  let response = reply({ status: "ready", can_apply: true });
  const client = createUpdateClient({ isNative: () => true, requestId: () => requestId, invoke: async () => response });
  assert.deepEqual(await client.request({ kind: "apply", candidate_id: candidateId }), invalid);
  response = reply({ status: "exiting", channel: null, candidate: null });
  assert.equal((await client.request({ kind: "apply", candidate_id: candidateId })).snapshot.status, "exiting");
  assert.equal((await client.request({ kind: "rollback", recovery_id: candidateId })).snapshot.status, "exiting");
});

test("checks once at startup using the installed release channel and never downloads", async () => {
  for (const [installedVersion, channel] of [["0.1.0+win-10", "stable"], ["0.1.0-beta.1", "preview"]]) {
    const calls = [];
    const client = createUpdateClient({
      isNative: () => true,
      requestId: () => requestId,
      invoke: async (_command, { request: { action } }) => {
        calls.push(action);
        return action.kind === "snapshot"
          ? reply({ installed_version: installedVersion, status: "idle", channel: null, candidate: null })
          : reply({ installed_version: installedVersion, status: "available", channel });
      },
    });
    assert.deepEqual(await Promise.all([client.checkOnStartup(), client.checkOnStartup()]), ["0.1.1", "0.1.1"]);
    assert.deepEqual(calls, [{ kind: "snapshot" }, { kind: "check", channel }]);
  }
});

test("stays quiet without a trusted source or a verified newer release", async () => {
  for (const [initial, check, expectedCalls] of [
    ["unavailable", null, 1], ["idle", "current", 2], ["idle", "error", 2],
  ]) {
    let calls = 0;
    const client = createUpdateClient({
      isNative: () => true,
      requestId: () => requestId,
      invoke: async () => reply(++calls === 1
        ? { status: initial, channel: null, candidate: null }
        : { status: check, channel: check === "current" ? "stable" : null, candidate: null,
          error: check === "error" ? "invalid_release" : null }),
    });
    assert.equal(await client.checkOnStartup(), null);
    assert.equal(calls, expectedCalls);
  }
});
