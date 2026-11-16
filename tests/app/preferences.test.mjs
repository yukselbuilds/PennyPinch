import test from "node:test";
import assert from "node:assert/strict";
import {
  createPreferencesClient,
  decodePreferencesError,
  decodePreferencesSnapshot,
  PreferencesRequestSequence,
  defaultOperatingPreferences,
  decodeOperatingPreferences,
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
      engine_version: "0.44.0",
      manifest_sha256: "e7274bed1e89c18b74c690c5bd414eabbd1579f7352d8a0094477804559d3dbe",
      mode: "cache",
      savings_profile: "coding",
      ccr_ttl_seconds: 7200,
    },
    ...overrides,
  };
}

test("decodes exact typed preferences and the pinned manifest reference", () => {
  const decoded = decodePreferencesSnapshot(snapshot(), requestId);
  assert.deepEqual(decoded?.preferences, { language: "en", theme: "dark", operating: defaultOperatingPreferences(), show_recommendations: false, experimental_rules: false });
  assert.equal(decoded?.revision, 3);
  assert.equal(decoded?.state_fingerprint, fingerprint);
  assert.equal(decoded?.runtime_manifest.ccr_ttl_seconds, 7200);
  assert.equal("future_secret" in (decoded ?? {}), false);
});

test("operating preferences migrate legacy replies and reject unsafe tools or retention",()=>{
  const operating={close_to_tray:false,retention_days:7,protected_tools:["ReadExact","mcp__local__read"]};
  const reply=snapshot({preferences:{language:"tr",theme:"system",operating}});
  assert.deepEqual(decodePreferencesSnapshot(reply,requestId)?.preferences.operating,operating);
  for(const bad of [ {...operating,retention_days:0},{...operating,protected_tools:["a","a"]},{...operating,protected_tools:["z","a"]},{...operating,protected_tools:["bad name"]},{...operating,protected_tools:Array.from({length:33},(_,i)=>`tool${i}`)},{...operating,unexpected:true} ]) {
    assert.equal(decodeOperatingPreferences(bad),null);
  }
});

test("rejects unknown fields, request mismatch, unsafe revisions, fingerprints, and pin drift", () => {
  assert.equal(decodePreferencesSnapshot({ ...snapshot(), token: "fixture-sentinel" }, requestId), null);
  assert.equal(decodePreferencesSnapshot(snapshot(), "00000000-0000-4000-8000-000000000000"), null);
  assert.equal(decodePreferencesSnapshot(snapshot({ revision: Number.MAX_SAFE_INTEGER + 1 }), requestId), null);
  assert.equal(decodePreferencesSnapshot(snapshot({ state_fingerprint: "AB".repeat(32) }), requestId), null);
  assert.equal(decodePreferencesSnapshot(snapshot({ runtime_manifest: { ...snapshot().runtime_manifest, ccr_ttl_seconds: 1800 } }), requestId), null);
  assert.equal(decodePreferencesSnapshot(snapshot({ unexpected: true }), requestId), null);
  assert.equal(decodePreferencesSnapshot(snapshot({ preferences: { language: "tr", theme: "light", show_recommendations: "true" } }), requestId), null);
  assert.equal(decodePreferencesSnapshot(snapshot({ preferences: { language: "tr", theme: "light", experimental_rules: "true" } }), requestId), null);
});

test("experimental preview is optional in old replies and sends its own strict preference", async () => {
  const calls = [];
  const client = createPreferencesClient({
    isNative: () => true,
    createRequestId: () => requestId,
    invoke: async (command, args) => {
      calls.push([command, args]);
      return snapshot({ preferences: { language: "en", theme: "dark", experimental_rules: true } });
    },
  });
  const reply = await client.readPreferences();
  assert.equal(reply.snapshot.preferences.experimental_rules, true);
  assert.equal(reply.snapshot.preferences.show_recommendations, false);
  await client.savePreferences({ ...reply.snapshot.preferences, experimental_rules: false }, 3, fingerprint);
  assert.equal(calls[1][1].experimentalRules, false);
  assert.equal(calls[1][1].showRecommendations, false);
  assert.deepEqual(calls[1][1].operating, defaultOperatingPreferences());
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
  await client.savePreferences({ language: "tr", theme: "light", show_recommendations: true }, 3, fingerprint);
  await client.savePreferences({ language: "tr", theme: "light", show_recommendations: false }, 4, fingerprint);
  assert.equal(calls[2][1].showRecommendations, true);
  assert.equal(calls[3][1].showRecommendations, false);
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
