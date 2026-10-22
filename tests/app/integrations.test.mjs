import test from "node:test";
import assert from "node:assert/strict";
import {
  createDiscoveryClient,
  createDiscoverySession,
  decodeDiscoveryReply,
  decodeSelectClientFileReply,
  DiscoveryRequestSequence,
} from "../../src/lib/integrations.ts";

const requestId = "7132bced-3ce0-4c1b-8f48-d93d7b96f7ce";
const installationId = "9232bced-3ce0-4c1b-8f48-d93d7b96f7ce";

function installation(overrides = {}) {
  return {
    installation_id: installationId,
    client_kind: "codex",
    surface: "cli",
    version: "0.163.0-alpha.10.2",
    source: "user_local_npm",
    location_label: "User-local npm",
    status: "detected",
    connection_status: "not_verified",
    ...overrides,
  };
}

function snapshot(overrides = {}) {
  return {
    schema_version: 1,
    request_id: requestId,
    status: "partial",
    scan_generation: 1,
    installations: [installation()],
    warnings: ["absence_is_not_unsupported", "scope_partial"],
    ...overrides,
  };
}

function errorReply(code = "busy", overrides = {}) {
  const mappings = {
    validation_failed: [false, "discovery.validation_failed"],
    busy: [true, "discovery.busy"],
    environment_blocked: [false, "discovery.environment_blocked"],
  };
  const [retryable, message_key] = mappings[code];
  return {
    schema_version: 1,
    request_id: requestId,
    status: "error",
    error: { code, retryable, message_key, diagnostic_id: null },
    ...overrides,
  };
}

test("decodes exact correlated snapshots and reconstructs installation fields", () => {
  const decoded = decodeDiscoveryReply(snapshot({
    installations: [installation({ unexpected: "fixture-sentinel" })],
  }), requestId);
  assert.equal(decoded, null);

  const valid = decodeDiscoveryReply(snapshot(), requestId);
  assert.equal(valid?.kind, "snapshot");
  assert.deepEqual(valid.snapshot.installations[0], installation());
  assert.equal("installation_id" in valid.snapshot.installations[0], true);
  assert.equal("path" in valid.snapshot.installations[0], false);
  const semverWithHyphenatedPrerelease = decodeDiscoveryReply(snapshot({
    installations: [installation({ version: "1.2.3-alpha-test.1+build.2" })],
  }), requestId);
  assert.equal(semverWithHyphenatedPrerelease?.kind, "snapshot");
});

test("rejects correlation, schema, warning, generation, count, duplicate and cross-field drift", () => {
  assert.equal(decodeDiscoveryReply(snapshot({ request_id: "00000000-0000-4000-8000-000000000000" }), requestId), null);
  assert.equal(decodeDiscoveryReply({ ...snapshot(), secret: "fixture-sentinel" }, requestId), null);
  assert.equal(decodeDiscoveryReply(snapshot({ scan_generation: Number.MAX_SAFE_INTEGER + 1 }), requestId), null);
  assert.equal(decodeDiscoveryReply(snapshot({ warnings: ["scope_partial", "unknown_warning"] }), requestId), null);
  assert.equal(decodeDiscoveryReply(snapshot({ installations: [installation(), installation()] }), requestId), null);
  assert.equal(decodeDiscoveryReply(snapshot({ installations: Array.from({ length: 513 }, (_, i) => installation({ installation_id: `${String(i).padStart(8, "0")}-3ce0-4c1b-8f48-d93d7b96f7ce` })) }), requestId), null);
  assert.equal(decodeDiscoveryReply(snapshot({ status: "not_scanned", scan_generation: 0 }), requestId), null);
  assert.equal(decodeDiscoveryReply(snapshot({ installations: [installation({ source: "path", location_label: "User-local npm" })] }), requestId), null);
});

test("decodes only the fixed discovery error DTOs", () => {
  for (const code of ["validation_failed", "busy", "environment_blocked"]) {
    const decoded = decodeDiscoveryReply(errorReply(code), requestId);
    assert.equal(decoded?.kind, "discovery_error");
    assert.equal(decoded.error.error.code, code);
  }
  assert.equal(decodeDiscoveryReply(errorReply("busy", { request_id: "wrong" }), requestId), null);
  assert.equal(decodeDiscoveryReply(errorReply("busy", { error: { ...errorReply().error, retryable: false } }), requestId), null);
});

test("client keeps browser mode unavailable and correlates native command arguments", async () => {
  let invocations = 0;
  const unavailable = createDiscoveryClient({
    isNative: () => false,
    createRequestId: () => requestId,
    invoke: async () => { invocations += 1; return snapshot(); },
  });
  assert.deepEqual(await unavailable.getDiscoverySnapshot(), { kind: "desktop_unavailable" });
  assert.deepEqual(await unavailable.scanDiscovery(), { kind: "desktop_unavailable" });
  assert.equal(invocations, 0);

  const calls = [];
  const native = createDiscoveryClient({
    isNative: () => true,
    createRequestId: () => requestId,
    invoke: async (command, args) => {
      calls.push([command, args]);
      return snapshot();
    },
  });
  assert.equal((await native.getDiscoverySnapshot()).kind, "snapshot");
  assert.equal((await native.scanDiscovery()).kind, "snapshot");
  assert.deepEqual(calls, [
    ["get_discovery_snapshot", { requestId }],
    ["scan_discovery", { requestId }],
  ]);
});

test("client reduces thrown bridge errors to a fixed result", async () => {
  const client = createDiscoveryClient({
    isNative: () => true,
    createRequestId: () => requestId,
    invoke: async () => { throw new Error("fixture-sentinel private path"); },
  });
  assert.deepEqual(await client.getDiscoverySnapshot(), { kind: "request_failed" });
});

test("initial session scans a not-scanned snapshot once and then reuses backend cache", async () => {
  let scans = 0;
  const cached = snapshot({ installations: [], warnings: ["absence_is_not_unsupported", "scope_partial"] });
  const client = {
    getDiscoverySnapshot: async () => ({
      kind: "snapshot",
      snapshot: scans === 0
        ? { schema_version: 1, request_id: requestId, status: "not_scanned", scan_generation: 0, installations: [], warnings: ["scan_not_run"] }
        : cached,
    }),
    scanDiscovery: async () => { scans += 1; return { kind: "snapshot", snapshot: cached }; },
  };
  const session = createDiscoverySession(client);
  const results = await Promise.all([session.loadInitial(), session.loadInitial()]);
  assert.equal(scans, 1);
  assert.ok(results.every((result) => result.kind === "snapshot" && result.snapshot.status === "partial"));
  await session.loadInitial();
  assert.equal(scans, 1);
});

test("remounting while the initial scan is pending joins it instead of starting another", async () => {
  let scans = 0;
  let finishScan;
  let notifyScanStarted;
  const scanStarted = new Promise((resolve) => { notifyScanStarted = resolve; });
  const cached = snapshot({ installations: [], warnings: ["scope_partial", "absence_is_not_unsupported"] });
  const notScanned = {
    schema_version: 1,
    request_id: requestId,
    status: "not_scanned",
    scan_generation: 0,
    installations: [],
    warnings: ["scan_not_run"],
  };
  const client = {
    getDiscoverySnapshot: async () => ({ kind: "snapshot", snapshot: notScanned }),
    scanDiscovery: () => {
      scans += 1;
      notifyScanStarted();
      return new Promise((resolve) => { finishScan = () => resolve({ kind: "snapshot", snapshot: cached }); });
    },
  };
  const session = createDiscoverySession(client);
  const firstMount = session.loadInitial();
  await scanStarted;
  const remount = session.loadInitial();
  assert.equal(scans, 1);
  finishScan();
  const results = await Promise.all([firstMount, remount]);
  assert.ok(results.every((result) => result.kind === "snapshot" && result.snapshot.status === "partial"));
});

test("late not-scanned read from a concurrent mount reuses the completed initial scan", async () => {
  let scans = 0;
  const reads = [];
  const cached = snapshot({ installations: [], warnings: ["scope_partial", "absence_is_not_unsupported"] });
  const notScanned = {
    schema_version: 1,
    request_id: requestId,
    status: "not_scanned",
    scan_generation: 0,
    installations: [],
    warnings: ["scan_not_run"],
  };
  const client = {
    getDiscoverySnapshot: () => new Promise((resolve) => { reads.push(resolve); }),
    scanDiscovery: async () => { scans += 1; return { kind: "snapshot", snapshot: cached }; },
  };
  const session = createDiscoverySession(client);
  const firstMount = session.loadInitial();
  const concurrentMount = session.loadInitial();
  assert.equal(reads.length, 2);
  reads[0]({ kind: "snapshot", snapshot: notScanned });
  const firstResult = await firstMount;
  reads[1]({ kind: "snapshot", snapshot: notScanned });
  const concurrentResult = await concurrentMount;
  assert.equal(scans, 1);
  assert.equal(firstResult.kind, "snapshot");
  assert.equal(concurrentResult.kind, "snapshot");
  assert.equal(concurrentResult.snapshot.status, "partial");
});

test("request sequence rejects older reads and invalidates on unmount", () => {
  const sequence = new DiscoveryRequestSequence();
  const oldRequest = sequence.next();
  const latestRequest = sequence.next();
  assert.equal(sequence.isCurrent(oldRequest), false);
  assert.equal(sequence.isCurrent(latestRequest), true);
  sequence.invalidate();
  assert.equal(sequence.isCurrent(latestRequest), false);
});

test("manual file candidates decode only with their fixed source metadata", () => {
  const manual = installation({
    version: null,
    source: "manual_file",
    location_label: "Selected file",
  });
  assert.equal(decodeDiscoveryReply(snapshot({ installations: [manual] }), requestId)?.kind, "snapshot");
  assert.equal(decodeDiscoveryReply(snapshot({ installations: [installation({
    source: "manual_file",
    location_label: "Selected file",
  })] }), requestId), null);
  assert.equal(decodeDiscoveryReply(snapshot({ installations: [installation({
    version: "0.163.0-alpha.11.1",
    source: "manual_file",
    location_label: "Selected file",
  })] }), requestId)?.kind, "snapshot");
  assert.equal(decodeDiscoveryReply(snapshot({ installations: [installation({
    version: "0.163.0-alpha.11.2",
    source: "manual_file",
    location_label: "Selected file",
  })] }), requestId), null);
  assert.equal(decodeDiscoveryReply(snapshot({ installations: [installation({
    client_kind: "claude_code",
    version: "0.163.0-alpha.11.1",
    source: "manual_file",
    location_label: "Selected file",
  })] }), requestId), null);
  assert.equal(decodeDiscoveryReply(snapshot({ installations: [installation({ ...manual, surface: "desktop" })] }), requestId), null);
  const overManualCap = Array.from({ length: 33 }, (_, index) => ({
    ...manual,
    installation_id: `20000000-0000-4000-8000-${String(index).padStart(12, "0")}`,
  }));
  assert.equal(decodeDiscoveryReply(snapshot({ installations: overManualCap }), requestId), null);
});

test("cancelled envelope is exact, correlated and accepted only by the file picker decoder", () => {
  const cancelled = { schema_version: 1, request_id: requestId, status: "cancelled" };
  assert.deepEqual(decodeSelectClientFileReply(cancelled, requestId), { kind: "cancelled", request_id: requestId });
  assert.equal(decodeSelectClientFileReply({ ...cancelled, extra: "fixture" }, requestId), null);
  assert.equal(decodeSelectClientFileReply({ ...cancelled, request_id: "wrong" }, requestId), null);
  assert.equal(decodeDiscoveryReply(cancelled, requestId), null);
});

test("discovery reads and scans reject the picker-only cancelled envelope", async () => {
  const cancelled = { schema_version: 1, request_id: requestId, status: "cancelled" };
  for (const method of ["getDiscoverySnapshot", "scanDiscovery"]) {
    const client = createDiscoveryClient({
      isNative: () => true,
      createRequestId: () => requestId,
      invoke: async () => cancelled,
    });
    assert.deepEqual(await client[method](), { kind: "invalid_response" });
  }
});

test("file picker uses only the typed native command and is unavailable in browser preview", async () => {
  let browserInvocations = 0;
  const browser = createDiscoveryClient({
    isNative: () => false,
    createRequestId: () => requestId,
    invoke: async () => { browserInvocations += 1; return null; },
  });
  assert.deepEqual(await browser.selectClientFile("codex"), { kind: "desktop_unavailable" });
  assert.equal(browserInvocations, 0);

  const calls = [];
  const native = createDiscoveryClient({
    isNative: () => true,
    createRequestId: () => requestId,
    invoke: async (command, args) => {
      calls.push([command, args]);
      return { schema_version: 1, request_id: requestId, status: "cancelled" };
    },
  });
  assert.deepEqual(await native.selectClientFile("claude_code"), { kind: "cancelled", request_id: requestId });
  assert.deepEqual(calls, [["select_client_file", { requestId, clientKind: "claude_code" }]]);
});

test("pending picker serializes mutations and a remount reads the completed backend snapshot", async () => {
  let finishPicker;
  let notifyPickerStarted;
  let scans = 0;
  let reads = 0;
  const pickerStarted = new Promise((resolve) => { notifyPickerStarted = resolve; });
  const selectedSnapshot = snapshot({ installations: [installation({
    version: null,
    source: "manual_file",
    location_label: "Selected file",
  })] });
  const client = {
    getDiscoverySnapshot: async () => {
      reads += 1;
      return { kind: "snapshot", snapshot: selectedSnapshot };
    },
    scanDiscovery: async () => { scans += 1; return { kind: "snapshot", snapshot: selectedSnapshot }; },
    selectClientFile: () => {
      notifyPickerStarted();
      return new Promise((resolve) => { finishPicker = resolve; });
    },
  };
  const session = createDiscoverySession(client);
  const picker = session.selectClientFile("codex");
  await pickerStarted;
  assert.equal(session.isMutationPending(), true);
  assert.deepEqual(await session.scanDiscovery(), { kind: "busy" });
  assert.deepEqual(await session.selectClientFile("opencode"), { kind: "busy" });
  const remountRead = session.loadInitial();
  await Promise.resolve();
  assert.equal(reads, 0);
  finishPicker({ kind: "cancelled", request_id: requestId });
  assert.deepEqual(await picker, { kind: "cancelled", request_id: requestId });
  const loaded = await remountRead;
  assert.equal(loaded.kind, "snapshot");
  assert.equal(loaded.snapshot.installations[0].source, "manual_file");
  assert.equal(reads, 1);
  assert.equal(scans, 0);
  assert.equal(session.isMutationPending(), false);
});
