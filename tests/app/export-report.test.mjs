import test from "node:test";
import assert from "node:assert/strict";
import { createExportClient } from "../../src/lib/exportReport.ts";

const requestId = "7132bced-3ce0-4c1b-8f48-d93d7b96f7ce";
const snapshotRequestId = "a87abcea-c90c-4dbb-bede-3f3de05f161f";

function client(invoke, overrides = {}) {
  return createExportClient({
    isNative: () => true,
    createRequestId: () => requestId,
    invoke,
    ...overrides,
  });
}

function savedReply(overrides = {}) {
  return {
    schema_version: 1,
    request_id: requestId,
    status: "saved",
    kind: "statistics_json",
    bytes_written: 128,
    ...overrides,
  };
}

test("sends the exact native request and returns only the saved byte count", async () => {
  const calls = [];
  const api = client(async (...args) => { calls.push(args); return savedReply(); });
  assert.deepEqual(await api.exportReport("statistics_json", "tr", snapshotRequestId), { kind: "saved", bytes_written: 128 });
  assert.deepEqual(calls, [["export_report", { request: {
    request_id: requestId,
    kind: "statistics_json",
    language: "tr",
    snapshot_request_id: snapshotRequestId,
  } }]]);
});

test("accepts cancellation only with zero bytes and the requested report kind", async () => {
  const api = client(async () => ({
    schema_version: 1, request_id: requestId, status: "cancelled", kind: "statistics_csv", bytes_written: 0,
  }));
  assert.deepEqual(await api.exportReport("statistics_csv", "en", snapshotRequestId), { kind: "cancelled" });
  assert.deepEqual(await client(async () => savedReply({ bytes_written: 262_144 }))
    .exportReport("statistics_json", "en", snapshotRequestId), { kind: "saved", bytes_written: 262_144 });
  assert.deepEqual(await client(async () => savedReply({ status: "cancelled", bytes_written: 1 }))
    .exportReport("statistics_json", "en", snapshotRequestId), { kind: "invalid_response" });
});

test("requires a snapshot for statistics, permits null for diagnostics, and skips invalid input", async () => {
  const calls = [];
  const api = client(async (...args) => { calls.push(args); return { schema_version: 1, request_id: requestId, status: "cancelled", kind: "diagnostics_json", bytes_written: 0 }; });
  assert.deepEqual(await api.exportReport("statistics_json", "en", null), { kind: "invalid_response" });
  assert.deepEqual(await api.exportReport("unknown", "en", snapshotRequestId), { kind: "invalid_response" });
  assert.deepEqual(await api.exportReport("diagnostics_json", "fr", null), { kind: "invalid_response" });
  assert.deepEqual(await api.exportReport("diagnostics_json", "en", "not-a-uuid"), { kind: "invalid_response" });
  assert.deepEqual(await api.exportReport("diagnostics_json", "en", null), { kind: "cancelled" });
  assert.equal(calls.length, 1);
  assert.deepEqual(calls[0][1].request, {
    request_id: requestId, kind: "diagnostics_json", language: "en", snapshot_request_id: null,
  });
});

test("rejects malformed UUIDs and does not invoke outside the native app", async () => {
  let calls = 0;
  const malformed = client(async () => { calls++; return savedReply(); }, { createRequestId: () => "00000000-0000-0000-0000-000000000000" });
  assert.deepEqual(await malformed.exportReport("statistics_json", "en", snapshotRequestId), { kind: "request_failed" });
  const browser = client(async () => { calls++; return savedReply(); }, { isNative: () => false });
  assert.deepEqual(await browser.exportReport("statistics_json", "en", snapshotRequestId), { kind: "desktop_unavailable" });
  assert.equal(calls, 0);
});

test("requires exact success fields, valid byte ranges, matching IDs, and matching kinds", async () => {
  const invalidReplies = [
    savedReply({ extra: "private-path" }),
    savedReply({ request_id: snapshotRequestId }),
    savedReply({ schema_version: 2 }),
    savedReply({ kind: "diagnostics_json" }),
    savedReply({ bytes_written: 0 }),
    savedReply({ bytes_written: 262145 }),
    savedReply({ bytes_written: Number.MAX_SAFE_INTEGER + 1 }),
    savedReply({ bytes_written: 1.5 }),
  ];
  for (const reply of invalidReplies) {
    assert.deepEqual(await client(async () => reply).exportReport("statistics_json", "en", snapshotRequestId), { kind: "invalid_response" });
  }
});

test("accepts only fixed redacted error codes and exact retryability metadata", async () => {
  const errors = {
    invalid_input: false,
    snapshot_expired: true,
    busy: true,
    unavailable: false,
    unsafe_path: false,
    file_exists: true,
    write_failed: true,
  };
  for (const [code, retryable] of Object.entries(errors)) {
    const reply = { schema_version: 1, request_id: requestId, status: "error", error: {
      code, retryable, message_key: `export.${code}`, diagnostic_id: null,
    } };
    assert.deepEqual(await client(async () => reply).exportReport("statistics_json", "en", snapshotRequestId), {
      kind: "export_error", code, retryable,
    });
  }
  const invalid = [
    { code: "busy", retryable: false, message_key: "export.busy", diagnostic_id: null },
    { code: "busy", retryable: true, message_key: "export.busy", diagnostic_id: "raw-diagnostic" },
    { code: "future", retryable: false, message_key: "export.future", diagnostic_id: null },
    { code: "write_failed", retryable: true, message_key: "export.write_failed", diagnostic_id: null, detail: "C:\\private" },
  ];
  for (const error of invalid) {
    const result = await client(async () => ({ schema_version: 1, request_id: requestId, status: "error", error }))
      .exportReport("statistics_json", "en", snapshotRequestId);
    assert.deepEqual(result, { kind: "invalid_response" });
    assert.equal(JSON.stringify(result).includes("private"), false);
  }
  assert.deepEqual(await client(async () => ({
    schema_version: 1, request_id: requestId, status: "error",
    error: { code: "busy", retryable: true, message_key: "export.busy", diagnostic_id: null }, extra: "private",
  })).exportReport("statistics_json", "en", snapshotRequestId), { kind: "invalid_response" });
});

test("suppresses thrown native errors and releases the single-flight guard", async () => {
  let calls = 0;
  const api = client(async () => {
    calls++;
    if (calls === 1) throw new Error("private path and contents");
    return savedReply();
  });
  assert.deepEqual(await api.exportReport("statistics_json", "en", snapshotRequestId), { kind: "request_failed" });
  assert.deepEqual(await api.exportReport("statistics_json", "en", snapshotRequestId), { kind: "saved", bytes_written: 128 });
  assert.equal(calls, 2);
});

test("gates concurrent exports and permits a new call after the pending one settles", async () => {
  let resolveInvoke;
  let calls = 0;
  const api = client(() => {
    calls++;
    return new Promise(resolve => { resolveInvoke = resolve; });
  });
  const first = api.exportReport("statistics_json", "en", snapshotRequestId);
  assert.deepEqual(await api.exportReport("statistics_csv", "en", snapshotRequestId), { kind: "client_busy" });
  assert.equal(calls, 1);
  resolveInvoke(savedReply());
  assert.deepEqual(await first, { kind: "saved", bytes_written: 128 });
  const third = api.exportReport("statistics_json", "en", snapshotRequestId);
  assert.equal(calls, 2);
  resolveInvoke(savedReply());
  assert.deepEqual(await third, { kind: "saved", bytes_written: 128 });
});
