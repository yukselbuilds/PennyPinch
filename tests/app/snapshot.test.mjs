import test from "node:test";
import assert from "node:assert/strict";
import { decodeSnapshot, decodeSnapshotError, LatestRequestSequence } from "../../src/runtime/snapshot.ts";

const requestId = "7132bced-3ce0-4c1b-8f48-d93d7b96f7ce";
const generation = "7132bced3ce04c1b8f48d93d7b96f7ce";

const snapshot = (status = "stopped") => ({
  schema_version: 1,
  request_id: requestId,
  snapshot_id: "d0d48df0-ff19-423d-935b-3b555356e9a1",
  observed_at_utc: "2026-10-19T12:00:00Z",
  runtime_generation: null,
  engine_version: null,
  engine: { status, error_code: null },
  compression: { status: "not_implemented", active_stream_count: null },
  discovery: { status: "not_scanned", clients: [] },
  metrics: {
    status: "waiting",
    before_total: null,
    after_total: null,
    saved_total: null,
    saved_percent: null,
    request_count: null,
    covered_count: null,
  },
  capabilities: { start: status === "stopped", stop: false, pause: false, connect: false },
});

test("accepts and reconstructs the correlated runtime snapshot allowlist", () => {
  const value = snapshot("ready");
  value.runtime_generation = generation;
  value.engine_version = "0.41.0";
  value.engine.error_code = null;
  value.capabilities = { start: false, stop: true, pause: false, connect: false };
  const decoded = decodeSnapshot(value, requestId);
  assert.equal(decoded?.request_id, requestId);
  assert.equal(decoded?.engine.status, "ready");
  assert.equal(decoded?.runtime_generation, generation);
  assert.deepEqual(decoded?.capabilities, { start: false, stop: true, pause: false, connect: false });
  assert.equal("future_field" in (decoded ?? {}), false);
  assert.equal(decodeSnapshot(value, "00000000-0000-4000-8000-000000000000"), null);
});

test("accepts fixed statuses and nullable manager error codes without inventing metrics", () => {
  const value = snapshot("failed");
  value.engine.error_code = "cleanup_failed";
  value.capabilities.start = false;
  assert.equal(decodeSnapshot(value, requestId)?.engine.error_code, "cleanup_failed");
  assert.equal(decodeSnapshot(value, requestId)?.metrics.saved_percent, null);
  assert.equal(decodeSnapshot(value, requestId)?.compression.status, "not_implemented");
});

test("rejects a ready response without a pinned owned generation", () => {
  const value = snapshot("ready");
  value.capabilities = { start: false, stop: false, pause: false, connect: false };
  assert.equal(decodeSnapshot(value, requestId), null);
});

test("rejects contradictory status, generation, version, stop, and ready error fields", () => {
  const readyWithError = snapshot("ready");
  readyWithError.runtime_generation = generation;
  readyWithError.engine_version = "0.41.0";
  readyWithError.engine.error_code = "timeout";
  readyWithError.capabilities = { start: false, stop: true, pause: false, connect: false };
  assert.equal(decodeSnapshot(readyWithError, requestId), null);

  const stoppedWithGeneration = snapshot("stopped");
  stoppedWithGeneration.runtime_generation = generation;
  stoppedWithGeneration.engine_version = "0.41.0";
  stoppedWithGeneration.capabilities = { start: true, stop: true, pause: false, connect: false };
  assert.equal(decodeSnapshot(stoppedWithGeneration, requestId), null);

  const stoppedWithFalseStopCapability = snapshot("stopped");
  stoppedWithFalseStopCapability.capabilities.stop = true;
  assert.equal(decodeSnapshot(stoppedWithFalseStopCapability, requestId), null);

  const startingWithGeneration = snapshot("starting");
  startingWithGeneration.runtime_generation = generation;
  startingWithGeneration.engine_version = "0.41.0";
  startingWithGeneration.capabilities = { start: false, stop: true, pause: false, connect: false };
  assert.equal(decodeSnapshot(startingWithGeneration, requestId), null);

  const halfPinnedPair = snapshot("unknown");
  halfPinnedPair.runtime_generation = generation;
  halfPinnedPair.capabilities.stop = true;
  assert.equal(decodeSnapshot(halfPinnedPair, requestId), null);

  const readyWithoutStopCapability = snapshot("ready");
  readyWithoutStopCapability.runtime_generation = generation;
  readyWithoutStopCapability.engine_version = "0.41.0";
  readyWithoutStopCapability.capabilities = { start: false, stop: false, pause: false, connect: false };
  assert.equal(decodeSnapshot(readyWithoutStopCapability, requestId), null);
});

test("preserves valid stopping and failed cleanup states without inferring start from error code", () => {
  const stopping = snapshot("stopping");
  stopping.runtime_generation = generation;
  stopping.engine_version = "0.41.0";
  stopping.capabilities = { start: false, stop: true, pause: false, connect: false };
  assert.equal(decodeSnapshot(stopping, requestId)?.engine.status, "stopping");

  const failedAfterVerifiedCleanup = snapshot("failed");
  failedAfterVerifiedCleanup.engine.error_code = "cleanup_failed";
  failedAfterVerifiedCleanup.capabilities.start = true;
  assert.equal(decodeSnapshot(failedAfterVerifiedCleanup, requestId)?.capabilities.start, true);

  const failedCleanupUncertain = snapshot("failed");
  failedCleanupUncertain.engine.error_code = "cleanup_failed";
  failedCleanupUncertain.runtime_generation = generation;
  failedCleanupUncertain.engine_version = "0.41.0";
  failedCleanupUncertain.capabilities.start = false;
  failedCleanupUncertain.capabilities.stop = true;
  assert.equal(decodeSnapshot(failedCleanupUncertain, requestId)?.runtime_generation, generation);

  const unknownAfterHealthExpiry = snapshot("unknown");
  unknownAfterHealthExpiry.engine.error_code = "timeout";
  assert.equal(decodeSnapshot(unknownAfterHealthExpiry, requestId)?.engine.error_code, "timeout");

  const unknownWithOwnedGeneration = snapshot("unknown");
  unknownWithOwnedGeneration.runtime_generation = generation;
  unknownWithOwnedGeneration.engine_version = "0.41.0";
  unknownWithOwnedGeneration.engine.error_code = "timeout";
  unknownWithOwnedGeneration.capabilities.stop = true;
  assert.equal(decodeSnapshot(unknownWithOwnedGeneration, requestId)?.runtime_generation, generation);
});

test("rejects unknown status, code, malformed generation, and engine version", () => {
  const unknownStatus = snapshot("new_status");
  assert.equal(decodeSnapshot(unknownStatus, requestId), null);
  const unknownCode = snapshot("failed");
  unknownCode.engine.error_code = "raw_exception";
  assert.equal(decodeSnapshot(unknownCode, requestId), null);
  const badGeneration = snapshot("ready");
  badGeneration.runtime_generation = "not-a-generation";
  badGeneration.engine_version = "0.41.0";
  badGeneration.capabilities = { start: false, stop: true, pause: false, connect: false };
  assert.equal(decodeSnapshot(badGeneration, requestId), null);
  const wrongVersion = snapshot("ready");
  wrongVersion.runtime_generation = generation;
  wrongVersion.engine_version = "0.42.0";
  wrongVersion.capabilities = { start: false, stop: true, pause: false, connect: false };
  assert.equal(decodeSnapshot(wrongVersion, requestId), null);
});

test("rejects enabled pause/connect capabilities and secret-shaped unknown fields", () => {
  const pause = snapshot();
  pause.capabilities.pause = true;
  assert.equal(decodeSnapshot(pause, requestId), null);
  const connect = snapshot();
  connect.capabilities.connect = true;
  assert.equal(decodeSnapshot(connect, requestId), null);
  const extra = snapshot();
  extra.api_key = "fixture-sentinel";
  assert.equal(decodeSnapshot(extra, requestId), null);
});

const publicErrors = {
  validation_failed: [false, "runtime.command.validation_failed"],
  unsupported_protocol: [false, "runtime.command.unsupported_protocol"],
  environment_blocked: [false, "runtime.command.environment_blocked"],
  busy: [true, "runtime.command.busy"],
  port_conflict: [true, "runtime.command.port_conflict"],
  engine_not_ready: [true, "runtime.command.engine_not_ready"],
  ownership_lost: [false, "runtime.command.ownership_lost"],
  timeout: [true, "runtime.command.timeout"],
  cleanup_failed: [true, "runtime.command.cleanup_failed"],
  config_conflict: [false, "runtime.command.config_conflict"],
};

test("accepts only the fixed PublicError code, retryability, and message-key mapping", () => {
  for (const [code, [retryable, messageKey]] of Object.entries(publicErrors)) {
    const error = {
      schema_version: 1,
      request_id: requestId,
      status: "error",
      error: { code, retryable, message_key: messageKey, diagnostic_id: null },
    };
    assert.equal(decodeSnapshotError(error, requestId)?.error.code, code);
  }
  const mismatch = {
    schema_version: 1,
    request_id: requestId,
    status: "error",
    error: { code: "busy", retryable: false, message_key: "runtime.command.busy", diagnostic_id: null },
  };
  assert.equal(decodeSnapshotError(mismatch, requestId), null);
  mismatch.error.message = "private detail";
  assert.equal(decodeSnapshotError(mismatch, requestId), null);
});

test("latest request sequence prevents an older Ready response replacing newer Stopped state", () => {
  const sequence = new LatestRequestSequence();
  const oldReady = sequence.next();
  const newStopped = sequence.next();
  assert.equal(sequence.isCurrent(oldReady), false);
  assert.equal(sequence.isCurrent(newStopped), true);
  sequence.invalidate();
  assert.equal(sequence.isCurrent(newStopped), false);
});
