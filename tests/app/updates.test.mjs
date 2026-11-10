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
