import test from "node:test";
import assert from "node:assert/strict";
import { createSessionClient, decodeSessionReply } from "../../src/lib/sessions.ts";

const snapshotId = "e7358031-a684-44fd-84e1-3b2577712578";
const requestId = "9dca9984-2b7d-4b8a-b3c0-a77c44f6a810";
const otherId = "1333248d-a929-45e6-81f8-c90173aa5aad";
const parentKey = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const childKey = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const observed = (client_kind, loaded_count = 1, partial = false) => ({
  client_kind, state: "observed", reason: null, server_version: "1.19.3", loaded_count, partial,
});
const unknown = (client_kind, reason = "unavailable") => ({
  client_kind, state: "unknown", reason, server_version: null, loaded_count: null, partial: false,
});
const mainRow = (patch = {}) => ({
  key: parentKey, id_hint: parentKey.slice(0, 8), client_kind: "codex",
  project: "fixture-project", model: "model-generic", provider: "provider-generic", reasoning: "balanced",
  created_at: "2026-11-13T10:00:00Z", updated_at: "2026-11-13T10:01:00Z",
  state: "loaded", role: "main", parent_key: null, traffic_assignment: "unverified", ...patch,
});
const childRow = (patch = {}) => ({
  ...mainRow({ key: childKey, id_hint: childKey.slice(0, 8), state: "working", role: "subagent", parent_key: parentKey }),
  ...patch,
});
const makeSnapshot = (patch = {}) => ({
  snapshot_id: snapshotId,
  observed_at_utc: "2026-11-13T10:02:00Z",
  scope: "current_cli_environment",
  clients: [observed("codex", 2), unknown("claude_code"), observed("opencode", 0)],
  rows: [mainRow(), childRow()],
  can_open_codex: true,
  traffic_assignment: "unverified",
  ...patch,
});
const envelope = (id, status, snapshot, error = null) => ({
  schema_version: 1, request_id: id, status, snapshot, error,
});

test("read sends the strict correlated request and accepts generic session metadata", async () => {
  const calls = [];
  const client = createSessionClient(() => true, async (command, args) => {
    calls.push({ command, args });
    return envelope(args.request.request_id, "snapshot", makeSnapshot());
  }, () => requestId);

  const result = await client.read();
  assert.equal(result.kind, "snapshot");
  assert.equal(calls.length, 1);
  assert.deepEqual(calls[0], {
    command: "manage_sessions",
    args: { request: { schema_version: 1, request_id: requestId, operation: "read", snapshot_id: null, confirm_open: false } },
  });
  assert.equal(result.snapshot.rows[1].model, "model-generic");
  assert.equal(result.snapshot.rows[1].role, "subagent");
  assert.equal(result.snapshot.rows[1].parent_key, parentKey);
});

test("unknown scopes stay null, partial counts are observed lower bounds, and zero stays distinct", () => {
  const data = makeSnapshot({
    clients: [observed("codex", 3, true), unknown("claude_code", "scope_unavailable"), observed("opencode", 0)],
  });
  const result = decodeSessionReply(envelope(requestId, "snapshot", data), {
    schema_version: 1, request_id: requestId, operation: "read", snapshot_id: null, confirm_open: false,
  });
  assert.equal(result.kind, "snapshot");
  assert.equal(result.snapshot.clients[0].loaded_count, 3);
  assert.equal(result.snapshot.clients[0].partial, true);
  assert.equal(result.snapshot.clients[1].loaded_count, null);
  assert.equal(result.snapshot.clients[2].loaded_count, 0);

  for (const invalid of [
    makeSnapshot({ clients: [observed("codex"), unknown("claude_code"), unknown("claude_code")] }),
    makeSnapshot({ clients: [observed("codex"), unknown("claude_code")] }),
    makeSnapshot({ clients: [unknown("codex"), unknown("claude_code"), observed("opencode")], can_open_codex: true }),
    makeSnapshot({ clients: [unknown("codex"), unknown("claude_code"), observed("opencode")], rows: [mainRow()] }),
  ]) {
    assert.equal(decodeSessionReply(envelope(requestId, "snapshot", invalid), {
      schema_version: 1, request_id: requestId, operation: "read", snapshot_id: null, confirm_open: false,
    }).kind, "error");
  }
});

test("open Codex requires explicit consent and a valid snapshot before IPC", async () => {
  const calls = [];
  const client = createSessionClient(() => true, async (command, args) => {
    calls.push({ command, args });
    return envelope(args.request.request_id, "launched", null);
  }, () => requestId);

  assert.deepEqual(await client.openCodex(snapshotId, false), { kind: "error", code: "validation_failed" });
  assert.deepEqual(await client.openCodex("not-a-snapshot", true), { kind: "error", code: "validation_failed" });
  assert.equal(calls.length, 0);
  assert.deepEqual(await client.openCodex(snapshotId, true), { kind: "launched" });
  assert.deepEqual(calls, [{
    command: "manage_sessions",
    args: { request: { schema_version: 1, request_id: requestId, operation: "open_codex_center", snapshot_id: snapshotId, confirm_open: true } },
  }]);
});

test("wrong correlation, extra fields, hostile metadata and invalid enums or dates are rejected", () => {
  const request = { schema_version: 1, request_id: requestId, operation: "read", snapshot_id: null, confirm_open: false };
  const valid = envelope(requestId, "snapshot", makeSnapshot());
  assert.deepEqual(decodeSessionReply({ ...valid, request_id: otherId }, request), { kind: "error", code: "invalid_response" });
  assert.deepEqual(decodeSessionReply({ ...valid, extra: "response-canary" }, request), { kind: "error", code: "invalid_response" });
  assert.deepEqual(decodeSessionReply(envelope(requestId, "snapshot", makeSnapshot({ title: "title-canary" })), request), { kind: "error", code: "invalid_response" });
  assert.deepEqual(decodeSessionReply(envelope(requestId, "snapshot", makeSnapshot({ rows: [mainRow({ preview: "preview-canary" })] })), request), { kind: "error", code: "invalid_response" });
  assert.deepEqual(decodeSessionReply(envelope(requestId, "snapshot", makeSnapshot({ rows: [mainRow({ model: "sk-private-canary" })] })), request), { kind: "error", code: "invalid_response" });
  assert.deepEqual(decodeSessionReply(envelope(requestId, "snapshot", makeSnapshot({ rows: [mainRow({ state: "resuming" })] })), request), { kind: "error", code: "invalid_response" });
  assert.deepEqual(decodeSessionReply(envelope(requestId, "snapshot", makeSnapshot({ rows: [mainRow({ updated_at: "2026-99-99T10:01:00Z" })] })), request), { kind: "error", code: "invalid_response" });
  assert.deepEqual(decodeSessionReply(envelope(requestId, "snapshot", makeSnapshot({ rows: [mainRow({ updated_at: "2026-02-30T10:01:00Z" })] })), request), { kind: "error", code: "invalid_response" });
  assert.deepEqual(decodeSessionReply(envelope(requestId, "snapshot", makeSnapshot({ snapshot_id: "bad-id" })), request), { kind: "error", code: "invalid_response" });
  assert.deepEqual(decodeSessionReply(envelope(requestId, "snapshot", makeSnapshot({ rows: [childRow({ parent_key: childKey })] })), request), { kind: "error", code: "invalid_response" });
  assert.deepEqual(decodeSessionReply(envelope(requestId, "snapshot", makeSnapshot({ rows: [mainRow(), childRow({ client_kind: "opencode", parent_key: parentKey })] })), request), { kind: "error", code: "invalid_response" });
});

test("rejected IPC content and transport exceptions never escape the fixed error result", async () => {
  const rejected = createSessionClient(() => true, async (_command, args) => ({
    ...envelope(args.request.request_id, "error", null, "raw-ipc-canary"),
  }), () => requestId);
  const rejectedResult = await rejected.read();
  assert.deepEqual(rejectedResult, { kind: "error", code: "invalid_response" });
  assert.equal(JSON.stringify(rejectedResult).includes("canary"), false);

  let invoked = 0;
  const unavailable = createSessionClient(() => false, async () => { invoked++; throw new Error("raw-transport-canary"); }, () => requestId);
  assert.deepEqual(await unavailable.read(), { kind: "error", code: "desktop_unavailable" });
  assert.equal(invoked, 0);

  const transportFailure = createSessionClient(() => true, async () => { throw new Error("raw-transport-canary"); }, () => requestId);
  const transportResult = await transportFailure.read();
  assert.deepEqual(transportResult, { kind: "error", code: "unavailable" });
  assert.equal(JSON.stringify(transportResult).includes("canary"), false);
});
