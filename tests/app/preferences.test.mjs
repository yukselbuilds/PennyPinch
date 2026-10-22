import test from "node:test";
import assert from "node:assert/strict";
import {
  createPreferencesClient,
  decodePreferencesError,
  decodePreferencesSnapshot,
  PreferencesRequestSequence,
} from "../../src/lib/preferences.ts";

const requestId = "7132bced-3ce0-4c1b-8f48-d93d7b96f7ce";
const fingerprint = "ab".repeat(32);

function snapshot(overrides = {}) {
  return {
    schema_version: 1,
    request_id: requestId,
    status: "ok",
    revision: 3,
    state_fingerprint: fingerprint,
    preferences: { language: "en", theme: "dark" },
    runtime_manifest: {
      engine_version: "0.41.0",
      manifest_sha256: "8e4edf5f1328a7a6323ecf435604feaa48fd80c9648288f05b449584924b7908",
      mode: "cache",
      savings_profile: "coding",
      ccr_ttl_seconds: 7200,
    },
    ...overrides,
  };
}

test("decodes exact typed preferences and the pinned manifest reference", () => {
  const decoded = decodePreferencesSnapshot(snapshot(), requestId);
  assert.deepEqual(decoded?.preferences, { language: "en", theme: "dark" });
  assert.equal(decoded?.revision, 3);
  assert.equal(decoded?.state_fingerprint, fingerprint);
  assert.equal(decoded?.runtime_manifest.ccr_ttl_seconds, 7200);
  assert.equal("future_secret" in (decoded ?? {}), false);
});

test("rejects unknown fields, request mismatch, unsafe revisions, fingerprints, and pin drift", () => {
  assert.equal(decodePreferencesSnapshot({ ...snapshot(), token: "fixture-sentinel" }, requestId), null);
  assert.equal(decodePreferencesSnapshot(snapshot(), "00000000-0000-4000-8000-000000000000"), null);
  assert.equal(decodePreferencesSnapshot(snapshot({ revision: Number.MAX_SAFE_INTEGER + 1 }), requestId), null);
  assert.equal(decodePreferencesSnapshot(snapshot({ state_fingerprint: "AB".repeat(32) }), requestId), null);
  assert.equal(decodePreferencesSnapshot(snapshot({ runtime_manifest: { ...snapshot().runtime_manifest, ccr_ttl_seconds: 1800 } }), requestId), null);
  assert.equal(decodePreferencesSnapshot(snapshot({ unexpected: true }), requestId), null);
});

const fixedErrors = {
  validation_failed: [false, "preferences.validation_failed"],
  unsupported_schema: [false, "preferences.unsupported_schema"],
  environment_blocked: [false, "preferences.environment_blocked"],
  busy: [true, "preferences.busy"],
  config_conflict: [false, "preferences.config_conflict"],
};

test("accepts only the five fixed preference error mappings", () => {
  for (const [code, [retryable, messageKey]] of Object.entries(fixedErrors)) {
    const response = {
      schema_version: 1,
      request_id: requestId,
      status: "error",
      error: { code, retryable, message_key: messageKey, diagnostic_id: null },
    };
    assert.equal(decodePreferencesError(response, requestId)?.error.code, code);
  }
  const mismatch = {
    schema_version: 1,
    request_id: requestId,
    status: "error",
    error: { code: "busy", retryable: false, message_key: "preferences.busy", diagnostic_id: null },
  };
  assert.equal(decodePreferencesError(mismatch, requestId), null);
  mismatch.error.detail = "fixture-sentinel";
  assert.equal(decodePreferencesError(mismatch, requestId), null);
});

test("native client sends exact request IDs and revision/fingerprint compare fields", async () => {
  const calls = [];
  const client = createPreferencesClient({
    isNative: () => true,
    createRequestId: () => requestId,
    invoke: async (command, args) => {
      calls.push([command, args]);
      return snapshot({ revision: 4, preferences: { language: "tr", theme: "light" } });
    },
  });
  const loaded = await client.readPreferences();
  assert.equal(loaded.kind, "snapshot");
  assert.deepEqual(calls[0], ["get_preferences", { requestId }]);

  const saved = await client.savePreferences({ language: "tr", theme: "light" }, 3, fingerprint);
  assert.equal(saved.kind, "snapshot");
  assert.deepEqual(calls[1], ["save_preferences", {
    requestId,
    language: "tr",
    theme: "light",
    expectedRevision: 3,
    expectedFingerprint: fingerprint,
  }]);
});

test("browser mode stays session-only and thrown bridge errors expose no raw text", async () => {
  let called = false;
  const browser = createPreferencesClient({
    isNative: () => false,
    createRequestId: () => requestId,
    invoke: async () => { called = true; throw new Error("D:\\private\\state path fixture-sentinel"); },
  });
  assert.deepEqual(await browser.readPreferences(), { kind: "desktop_unavailable" });
  assert.deepEqual(await browser.savePreferences({ language: "en", theme: "dark" }, 0, null), { kind: "desktop_unavailable" });
  assert.equal(called, false);

  const failing = createPreferencesClient({
    isNative: () => true,
    createRequestId: () => requestId,
    invoke: async () => { throw new Error("D:\\private\\state path fixture-sentinel"); },
  });
  const result = await failing.readPreferences();
  assert.deepEqual(result, { kind: "request_failed" });
  assert.equal(JSON.stringify(result).includes("fixture-sentinel"), false);
});

test("preference sequences ignore stale reads and invalidate pending work on unmount", () => {
  const sequence = new PreferencesRequestSequence();
  const oldRead = sequence.nextRead();
  const currentRead = sequence.nextRead();
  const pendingWrite = sequence.nextWrite();
  assert.equal(sequence.isReadCurrent(oldRead), false);
  assert.equal(sequence.isReadCurrent(currentRead), true);
  assert.equal(sequence.isWriteCurrent(pendingWrite), true);
  sequence.invalidate();
  assert.equal(sequence.isReadCurrent(currentRead), false);
  assert.equal(sequence.isWriteCurrent(pendingWrite), false);
});
