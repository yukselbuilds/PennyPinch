import assert from "node:assert/strict";
import test from "node:test";
import * as api from "../../src/lib/installationChoices.ts";

const requestId = "019a0000-0000-7000-8000-000000000001";
const installationId = "019a0000-0000-7000-8000-000000000002";

function choiceSnapshot(status = "available", overrides = {}) {
  return {
    schema_version: 1,
    request_id: requestId,
    status,
    client_kind: "claude_code",
    scan_generation: status === "not_scanned" ? null : 7,
    installation_id: status === "available" || status === "saved" ? installationId : null,
    ...overrides,
  };
}

function client(invoke, overrides = {}) {
  return api.createInstallationChoiceClient({
    isNative: () => true,
    createRequestId: () => requestId,
    invoke,
    ...overrides,
  });
}

function candidate(id = installationId, clientKind = "claude_code") {
  return { installation_id: id, client_kind: clientKind };
}

test("get and remember send only exact native request envelopes and correlate saved choice", async () => {
  const calls = [];
  const native = client(async (command, args) => {
    calls.push([command, args]);
    return command === "get_saved_installation_choice"
      ? choiceSnapshot("available")
      : choiceSnapshot("saved");
  });

  assert.deepEqual(await native.get("claude_code"), {
    kind: "snapshot",
    snapshot: choiceSnapshot("available"),
  });
  assert.deepEqual(await native.remember({
    clientKind: "claude_code",
    scanGeneration: 7,
    installationId,
  }), {
    kind: "snapshot",
    snapshot: choiceSnapshot("saved"),
  });
  assert.deepEqual(calls, [
    ["get_saved_installation_choice", { request: { schema_version: 1, request_id: requestId, client_kind: "claude_code" } }],
    ["remember_installation_choice", { request: { schema_version: 1, request_id: requestId, scan_generation: 7, installation_id: installationId } }],
  ]);
});

test("read snapshot statuses enforce exact keys, client correlation, and field invariants", async () => {
  const native = client(async (_command, args) => {
    const request = args.request;
    return choiceSnapshot("available", { request_id: request.request_id });
  });
  for (const reply of [
    choiceSnapshot("not_selected", { scan_generation: null }),
    choiceSnapshot("not_selected"),
    choiceSnapshot("unavailable", { scan_generation: null }),
    choiceSnapshot("not_scanned"),
    choiceSnapshot("available"),
  ]) {
    assert.equal((await client(async () => reply).get("claude_code")).kind, "snapshot");
  }

  for (const reply of [
    { ...choiceSnapshot("available"), raw_path: "C:\\private\\client.exe" },
    choiceSnapshot("available", { client_kind: "codex" }),
    choiceSnapshot("available", { request_id: "019a0000-0000-7000-8000-000000000099" }),
    choiceSnapshot("available", { scan_generation: null }),
    choiceSnapshot("available", { installation_id: null }),
    choiceSnapshot("not_selected", { installation_id: installationId }),
    choiceSnapshot("not_scanned", { scan_generation: 7 }),
    choiceSnapshot("saved"),
    choiceSnapshot("unknown"),
  ]) {
    assert.equal((await client(async () => reply).get("claude_code")).kind, "invalid_response");
  }
  assert.equal((await native.get("claude_code")).kind, "snapshot");
});

test("remember rejects bad inputs before invoke and rejects mismatched saved identity", async () => {
  let calls = 0;
  const badInput = client(async () => { calls += 1; return choiceSnapshot("saved"); });
  assert.equal((await badInput.remember({ clientKind: "claude_code", scanGeneration: 0, installationId })).kind, "invalid_request");
  assert.equal((await badInput.remember({ clientKind: "claude_code", scanGeneration: Number.MAX_SAFE_INTEGER + 1, installationId })).kind, "invalid_request");
  assert.equal((await badInput.remember({ clientKind: "claude_code", scanGeneration: 7, installationId: "not-a-uuid" })).kind, "invalid_request");
  assert.equal(calls, 0);

  for (const reply of [
    choiceSnapshot("available"),
    choiceSnapshot("saved", { scan_generation: 8 }),
    choiceSnapshot("saved", { installation_id: "019a0000-0000-7000-8000-000000000099" }),
    choiceSnapshot("saved", { client_kind: "codex" }),
  ]) {
    assert.equal((await client(async () => reply).remember({ clientKind: "claude_code", scanGeneration: 7, installationId })).kind, "invalid_response");
  }
});

test("fixed discovery errors are sanitized and transport failures never expose thrown text", async () => {
  const busy = {
    schema_version: 1,
    request_id: requestId,
    status: "error",
    error: { code: "busy", retryable: true, message_key: "discovery.busy", diagnostic_id: null },
  };
  assert.deepEqual(await client(async () => busy).get("claude_code"), {
    kind: "discovery_error",
    error: busy,
  });
  assert.equal((await client(async () => ({ ...busy, raw_path: "secret" })).get("claude_code")).kind, "invalid_response");
  const failed = await client(async () => { throw new Error("private path and details"); }).get("claude_code");
  assert.deepEqual(failed, { kind: "request_failed" });
  let browserInvocations = 0;
  const browser = client(async () => {
    browserInvocations += 1;
    return choiceSnapshot("not_selected");
  }, { isNative: () => false });
  assert.equal((await browser.get("claude_code")).kind, "desktop_unavailable");
  assert.equal((await browser.remember({ clientKind: "claude_code", scanGeneration: 7, installationId })).kind, "desktop_unavailable");
  assert.equal(browserInvocations, 0);
  assert.equal((await client(async () => choiceSnapshot(), { createRequestId: () => "bad" }).get("claude_code")).kind, "request_failed");
});

test("selection resolver honors available saved identity or a clean single-candidate preselection only", () => {
  const candidates = [candidate()];
  const available = { kind: "snapshot", snapshot: choiceSnapshot("available") };
  assert.equal(api.resolvePreferredInstallation(available, candidates, "claude_code", 7), installationId);
  assert.equal(api.resolvePreferredInstallation(available, candidates, "claude_code", 8), null);
  assert.equal(api.resolvePreferredInstallation(available, [candidate(installationId, "codex")], "claude_code", 7), null);
  assert.equal(api.resolvePreferredInstallation(available, [], "claude_code", 7), null);

  const notSelected = { kind: "snapshot", snapshot: choiceSnapshot("not_selected") };
  assert.equal(api.resolvePreferredInstallation(notSelected, candidates, "claude_code", 7), installationId);
  assert.equal(api.resolvePreferredInstallation(notSelected, [...candidates, candidate("019a0000-0000-7000-8000-000000000003")], "claude_code", 7), null);
  for (const status of ["unavailable", "not_scanned"]) {
    assert.equal(api.resolvePreferredInstallation({ kind: "snapshot", snapshot: choiceSnapshot(status) }, candidates, "claude_code", 7), null);
  }
  assert.equal(api.resolvePreferredInstallation({ kind: "request_failed" }, candidates, "claude_code", 7), null);
});
