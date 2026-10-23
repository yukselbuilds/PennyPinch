import assert from "node:assert/strict";
import test from "node:test";

const api = await import("../../src/lib/codexStatus.ts").catch(() => null);
const requestId = "019a0000-0000-7000-8000-000000000001";
const installationId = "019a0000-0000-7000-8000-000000000002";
const endpointId = "019a0000-0000-7000-8000-000000000003";

function makeSnapshot(overrides = {}) {
  return {
    schema_version: 1,
    request_id: requestId,
    status: "snapshot",
    installation: {
      installation_id: installationId,
      surface: "cli",
      version: "0.163.0-alpha.11.1",
      compatibility: "fixture_only",
    },
    config_scope: "user_config",
    ownership: {
      state: "active",
      endpoint_binding: "same",
      config_source: "codex_toml",
      endpoint_id: endpointId,
      endpoint_port: 43125,
      current_config_value: "matches_owned_endpoint",
    },
    runtime: {
      ingress_status: "ready",
      stable_ingress_ready: true,
      engine_status: "stopped",
      engine_ready: false,
      runtime_generation: null,
      active_connection_count: 0,
      error_code: null,
    },
    policy: {
      model_changed_by_headroom: false,
      reasoning_changed_by_headroom: false,
      auth_changed_by_headroom: false,
      restart_required_after_apply: true,
      runtime_effective: "unknown",
    },
    auth_scope: {
      current_method: "not_read",
      chatgpt_auth: "not_tested",
      api_key_auth: "fixture_only",
    },
    coverage: {
      traffic_verification: "not_verified",
      main_agent: "unknown",
      subagent: "not_tested",
    },
    ...overrides,
  };
}

function expected(overrides = {}) {
  return { requestId, installationId, surface: "cli", scanGeneration: 7, ...overrides };
}

test("strictly decodes the selected Codex installation and preserves separate readiness claims", () => {
  assert.ok(api, "Codex status decoder module is available");
  const result = api.decodeCodexStatusReply(makeSnapshot(), expected());
  assert.equal(result.kind, "snapshot");
  assert.equal(result.snapshot.installation.installation_id, installationId);
  assert.equal(result.snapshot.runtime.stable_ingress_ready, true);
  assert.equal(result.snapshot.runtime.engine_ready, false);
  assert.equal(result.snapshot.coverage.traffic_verification, "not_verified");
});

test("does not label an unverified engine or ingress as ready", () => {
  assert.ok(api);
  const contradictory = makeSnapshot({
    runtime: { ...makeSnapshot().runtime, ingress_status: "ready", stable_ingress_ready: false, engine_status: "ready", engine_ready: false },
  });
  assert.equal(api.visibleRuntimeStatus(contradictory.runtime.ingress_status, contradictory.runtime.stable_ingress_ready), "unknown");
  assert.equal(api.visibleRuntimeStatus(contradictory.runtime.engine_status, contradictory.runtime.engine_ready), "unknown");
});

test("keeps ready gated by its matching verification flag and clear error state", () => {
  assert.ok(api);
  assert.equal(api.visibleRuntimeStatus("ready", true), "ready");
  const engineContradiction = makeSnapshot({
    runtime: { ...makeSnapshot().runtime, engine_status: "ready", engine_ready: true, error_code: "cleanup_failed" },
  });
  assert.equal(api.decodeCodexStatusReply(engineContradiction, expected()), null);
});

test("keeps verified ingress ready when a separate engine error is present", () => {
  assert.ok(api);
  const snapshot = makeSnapshot({
    runtime: { ...makeSnapshot().runtime, engine_status: "failed", engine_ready: false, error_code: "cleanup_failed" },
  });
  const decoded = api.decodeCodexStatusReply(snapshot, expected());
  assert.equal(decoded.kind, "snapshot");
  assert.equal(api.visibleRuntimeStatus(decoded.snapshot.runtime.ingress_status, decoded.snapshot.runtime.stable_ingress_ready), "ready");
  assert.equal(api.visibleRuntimeStatus(decoded.snapshot.runtime.engine_status, decoded.snapshot.runtime.engine_ready), "failed");
});

test("preserves non-ready engine statuses", () => {
  assert.ok(api);
  assert.equal(api.visibleRuntimeStatus("stopped", false), "stopped");
  assert.equal(api.visibleRuntimeStatus("starting", false), "starting");
  assert.equal(api.visibleRuntimeStatus("draining", false), "draining");
  assert.equal(api.visibleRuntimeStatus("failed", false), "failed");
});

test("rejects correlation, selection, and unknown-field mismatches", () => {
  assert.ok(api);
  assert.equal(api.decodeCodexStatusReply(makeSnapshot(), expected({ requestId: "019a0000-0000-7000-8000-000000000010" })), null);
  assert.equal(api.decodeCodexStatusReply(makeSnapshot(), expected({ installationId: "019a0000-0000-7000-8000-000000000010" })), null);
  assert.equal(api.decodeCodexStatusReply(makeSnapshot(), expected({ surface: "desktop" })), null);

  const hostile = makeSnapshot();
  hostile.ownership.private_path = "C:\\private\\config";
  assert.equal(api.decodeCodexStatusReply(hostile, expected()), null);
});

test("enforces fixed scope labels and runtime ownership invariants", () => {
  assert.ok(api);
  const contradictory = makeSnapshot({
    runtime: { ...makeSnapshot().runtime, engine_ready: true },
  });
  assert.equal(api.decodeCodexStatusReply(contradictory, expected()), null);

  const badPort = makeSnapshot({
    ownership: { ...makeSnapshot().ownership, endpoint_port: 65536 },
  });
  assert.equal(api.decodeCodexStatusReply(badPort, expected()), null);

  const unsupportedFixtureClaim = makeSnapshot({
    installation: { ...makeSnapshot().installation, version: "0.163.0-alpha.11.2", compatibility: "fixture_only" },
    auth_scope: { ...makeSnapshot().auth_scope, api_key_auth: "fixture_only" },
  });
  assert.equal(api.decodeCodexStatusReply(unsupportedFixtureClaim, expected()), null);
});

test("limits local API fixture scope to exact CLI versions, never Desktop or neighbors", () => {
  assert.ok(api);
  for (const version of ["0.163.0-alpha.10.2", "0.163.0-alpha.11.1"]) {
    const fixtureCli = makeSnapshot({
      installation: { ...makeSnapshot().installation, version, compatibility: "fixture_only" },
      auth_scope: { ...makeSnapshot().auth_scope, api_key_auth: "fixture_only" },
    });
    assert.equal(api.decodeCodexStatusReply(fixtureCli, expected()).kind, "snapshot");
  }

  for (const version of ["0.163.0-alpha.11.0", "0.163.0-alpha.11.2", "0.163.0-alpha.10.3"]) {
    const neighborCli = makeSnapshot({
      installation: { ...makeSnapshot().installation, version, compatibility: "not_verified" },
      auth_scope: { ...makeSnapshot().auth_scope, api_key_auth: "not_tested" },
      ownership: { ...makeSnapshot().ownership, current_config_value: "not_rechecked" },
    });
    assert.equal(api.decodeCodexStatusReply(neighborCli, expected()).kind, "snapshot");
    assert.equal(api.decodeCodexStatusReply({
      ...neighborCli,
      installation: { ...neighborCli.installation, compatibility: "fixture_only" },
      auth_scope: { ...neighborCli.auth_scope, api_key_auth: "fixture_only" },
    }, expected()), null);
  }

  for (const version of ["0.163.0-alpha.10.2", "0.163.0-alpha.11.1"]) {
    const desktop = makeSnapshot({
      installation: { ...makeSnapshot().installation, surface: "desktop", version, compatibility: "not_tested" },
      auth_scope: { ...makeSnapshot().auth_scope, api_key_auth: "not_tested" },
      ownership: { ...makeSnapshot().ownership, current_config_value: "not_rechecked" },
    });
    assert.equal(api.decodeCodexStatusReply(desktop, expected({ surface: "desktop" })).kind, "snapshot");
    const desktopClaimingDiskMatch = {
      ...desktop,
      ownership: { ...desktop.ownership, current_config_value: "matches_owned_endpoint" },
    };
    assert.equal(api.decodeCodexStatusReply(desktopClaimingDiskMatch, expected({ surface: "desktop" })), null);
    assert.equal(api.decodeCodexStatusReply({
      ...desktop,
      installation: { ...desktop.installation, compatibility: "fixture_only" },
      auth_scope: { ...desktop.auth_scope, api_key_auth: "fixture_only" },
    }, expected({ surface: "desktop" })), null);
  }
});

test("decodes disk setting states only when consistent with ownership and fixture scope", () => {
  assert.ok(api);
  for (const currentValue of ["matches_owned_endpoint", "missing", "different", "unavailable"]) {
    const snapshot = makeSnapshot({
      ownership: { ...makeSnapshot().ownership, current_config_value: currentValue },
    });
    assert.equal(api.decodeCodexStatusReply(snapshot, expected()).kind, "snapshot", currentValue);
  }

  const settingIndependentOfRuntime = makeSnapshot({
    ownership: { ...makeSnapshot().ownership, endpoint_binding: "different", current_config_value: "matches_owned_endpoint" },
    runtime: { ...makeSnapshot().runtime, ingress_status: "unknown", stable_ingress_ready: false, engine_status: "stopped", engine_ready: false },
  });
  assert.equal(api.decodeCodexStatusReply(settingIndependentOfRuntime, expected()).kind, "snapshot");

  const activeNotOwned = makeSnapshot({
    ownership: { ...makeSnapshot().ownership, current_config_value: "not_owned" },
  });
  assert.equal(api.decodeCodexStatusReply(activeNotOwned, expected()), null);

  const released = makeSnapshot({
    ownership: { ...makeSnapshot().ownership, state: "released", endpoint_binding: "not_owned", current_config_value: "not_owned" },
  });
  assert.equal(api.decodeCodexStatusReply(released, expected()).kind, "snapshot");

  const noOwner = makeSnapshot({
    ownership: { state: "none", endpoint_binding: "not_owned", config_source: null, endpoint_id: null, endpoint_port: null, current_config_value: "not_owned" },
  });
  assert.equal(api.decodeCodexStatusReply(noOwner, expected()).kind, "snapshot");

  const ownerUnavailable = makeSnapshot({
    ownership: { state: "unavailable", endpoint_binding: "unknown", config_source: null, endpoint_id: null, endpoint_port: null, current_config_value: "unavailable" },
  });
  assert.equal(api.decodeCodexStatusReply(ownerUnavailable, expected()).kind, "snapshot");

  const unsupportedCli = makeSnapshot({
    installation: { ...makeSnapshot().installation, version: "0.163.0-alpha.99", compatibility: "not_verified" },
    ownership: { ...makeSnapshot().ownership, current_config_value: "not_rechecked" },
    auth_scope: { ...makeSnapshot().auth_scope, api_key_auth: "not_tested" },
  });
  assert.equal(api.decodeCodexStatusReply(unsupportedCli, expected()).kind, "snapshot");
  assert.equal(api.decodeCodexStatusReply(makeSnapshot(), expected({ surface: "desktop" })), null);
  assert.equal(api.decodeCodexStatusReply({
    ...unsupportedCli,
    ownership: { ...unsupportedCli.ownership, current_config_value: "matches_owned_endpoint" },
  }, expected()), null);
  assert.equal(api.decodeCodexStatusReply({
    ...makeSnapshot(),
    ownership: { ...makeSnapshot().ownership, current_config_value: "not_rechecked" },
  }, expected()), null);
});

test("presents disk-setting outcomes without turning no ownership into a warning", () => {
  assert.ok(api);
  assert.deepEqual(api.presentCodexSetting("matches_owned_endpoint"), { key: "matches", tone: "positive" });
  assert.deepEqual(api.presentCodexSetting("missing"), { key: "missing", tone: "caution" });
  assert.deepEqual(api.presentCodexSetting("different"), { key: "different", tone: "caution" });
  assert.deepEqual(api.presentCodexSetting("unavailable"), { key: "unavailable", tone: "caution" });
  assert.deepEqual(api.presentCodexSetting("not_rechecked"), { key: "not_rechecked", tone: "muted" });
  assert.deepEqual(api.presentCodexSetting("not_owned"), { key: "not_owned", tone: "quiet" });
});

test("rejects unknown current-config observation codes", () => {
  assert.ok(api);
  const base = makeSnapshot();
  const hostile = {
    ...base,
    ownership: { ...base.ownership, current_config_value: "verified_by_magic" },
  };
  assert.equal(api.decodeCodexStatusReply(hostile, expected()), null);
});

test("diagnoses safe next steps from decoded snapshots without inferring traffic", () => {
  assert.ok(api);
  const base = makeSnapshot();
  const decoded = (changes = {}) => {
    const installation = { ...base.installation, ...changes.installation };
    const raw = {
      ...base,
      ...changes,
      installation,
      ownership: { ...base.ownership, ...changes.ownership },
      runtime: {
        ...base.runtime,
        ingress_status: "ready",
        stable_ingress_ready: true,
        engine_status: "ready",
        engine_ready: true,
        runtime_generation: "0123456789abcdef0123456789abcdef",
        active_connection_count: 17,
        error_code: null,
        ...changes.runtime,
      },
      policy: { ...base.policy, ...changes.policy },
      auth_scope: { ...base.auth_scope, ...changes.auth_scope },
      coverage: { ...base.coverage, ...changes.coverage },
    };
    const result = api.decodeCodexStatusReply(raw, expected({ surface: installation.surface }));
    assert.equal(result.kind, "snapshot", "scenario must be a valid decoded DTO");
    return result.snapshot;
  };
  const diagnose = (snapshot) => api.diagnoseCodexNextAction(snapshot);
  const assertAction = (snapshot, code) => assert.deepEqual(diagnose(snapshot), { code, state: "action_required" });

  const unsupportedDesktop = decoded({
    installation: { surface: "desktop", compatibility: "not_tested" },
    ownership: { state: "none", endpoint_binding: "not_owned", config_source: null, endpoint_id: null, endpoint_port: null, current_config_value: "not_owned" },
    auth_scope: { api_key_auth: "not_tested" },
  });
  assertAction(unsupportedDesktop, "unsupported_installation");
  const unsupportedVersion = decoded({
    installation: { version: "0.163.0-alpha.99", compatibility: "not_verified" },
    ownership: { current_config_value: "not_rechecked" },
    auth_scope: { api_key_auth: "not_tested" },
  });
  assertAction(unsupportedVersion, "unsupported_installation");

  assertAction(decoded({
    ownership: { state: "unavailable", endpoint_binding: "unknown", config_source: null, endpoint_id: null, endpoint_port: null, current_config_value: "unavailable" },
  }), "refresh_status");
  assertAction(decoded({
    ownership: { state: "none", endpoint_binding: "not_owned", config_source: null, endpoint_id: null, endpoint_port: null, current_config_value: "not_owned" },
  }), "manage_connection");
  assertAction(decoded({
    ownership: { state: "released", endpoint_binding: "not_owned", current_config_value: "not_owned" },
  }), "manage_connection");

  for (const currentConfigValue of ["missing", "different", "unavailable"]) {
    assertAction(decoded({ ownership: { current_config_value: currentConfigValue } }), "inspect_codex_setting");
  }
  for (const endpointBinding of ["different", "unknown"]) {
    assertAction(decoded({ ownership: { endpoint_binding: endpointBinding } }), "review_connection");
  }
  assertAction(decoded({
    runtime: { ingress_status: "unknown", stable_ingress_ready: false, engine_status: "ready", engine_ready: false },
  }), "inspect_ingress");
  assertAction(decoded({
    runtime: { engine_status: "stopped", engine_ready: false, runtime_generation: null },
  }), "start_engine");
  for (const engineStatus of ["starting", "draining"]) {
    assertAction(decoded({
      runtime: { engine_status: engineStatus, engine_ready: false, runtime_generation: null },
    }), "wait_for_engine");
  }
  for (const engineStatus of ["failed", "unavailable", "unknown"]) {
    assertAction(decoded({
      runtime: { engine_status: engineStatus, engine_ready: false, runtime_generation: null, error_code: "timeout" },
    }), "inspect_runtime");
  }
  assertAction(decoded({
    runtime: { engine_status: "starting", engine_ready: false, runtime_generation: null, error_code: "timeout" },
  }), "inspect_runtime");
  assertAction(decoded({
    runtime: { engine_status: "ready", engine_ready: false },
  }), "inspect_runtime");

  const readyCounts = [null, 0, 17].map((activeConnectionCount) => decoded({
    runtime: { active_connection_count: activeConnectionCount },
  }));
  for (const snapshot of readyCounts) {
    const before = structuredClone(snapshot);
    assert.deepEqual(diagnose(snapshot), { code: "local_prerequisites_ready", state: "local_only" });
    assert.deepEqual(snapshot, before, "diagnosis must not mutate the snapshot");
    assert.deepEqual(snapshot.coverage, { traffic_verification: "not_verified", main_agent: "unknown", subagent: "not_tested" });
  }
  assert.deepEqual(readyCounts.map((snapshot) => diagnose(snapshot).code), Array(3).fill("local_prerequisites_ready"));

  const stoppedCounts = [null, 0, 17].map((activeConnectionCount) => decoded({
    runtime: { engine_status: "stopped", engine_ready: false, runtime_generation: null, active_connection_count: activeConnectionCount },
  }));
  for (const snapshot of stoppedCounts) {
    const before = structuredClone(snapshot);
    assertAction(snapshot, "start_engine");
    assert.deepEqual(snapshot, before, "diagnosis must not mutate the snapshot");
    assert.deepEqual(snapshot.policy, base.policy);
    assert.deepEqual(snapshot.auth_scope, base.auth_scope);
    assert.deepEqual(snapshot.coverage, base.coverage);
  }
  assert.deepEqual(stoppedCounts.map((snapshot) => diagnose(snapshot).code), Array(3).fill("start_engine"));
});

test("decodes only the correlated fixed error envelope", () => {
  assert.ok(api);
  const error = {
    schema_version: 1,
    request_id: requestId,
    status: "error",
    error: {
      code: "stale_selection",
      retryable: true,
      message_key: "codex.status.stale_selection",
      diagnostic_id: null,
    },
  };
  const decoded = api.decodeCodexStatusReply(error, expected());
  assert.equal(decoded.kind, "codex_error");
  assert.equal(decoded.error.error.code, "stale_selection");
  assert.equal(api.decodeCodexStatusReply({ ...error, debug: "raw path" }, expected()), null);
  assert.equal(api.decodeCodexStatusReply({ ...error, error: { ...error.error, retryable: false } }, expected()), null);
});

test("sends only a correlated status request and never retries or exposes thrown errors", async () => {
  assert.ok(api);
  const calls = [];
  const client = api.createCodexStatusClient({
    isNative: () => true,
    createRequestId: () => requestId,
    invoke: async (...args) => { calls.push(args); return makeSnapshot(); },
  });
  const result = await client.load({ installation_id: installationId, surface: "cli" }, 7);
  assert.equal(result.kind, "snapshot");
  assert.deepEqual(calls, [["get_codex_connection_status", {
    request: {
      schema_version: 1,
      request_id: requestId,
      installation_id: installationId,
      scan_generation: 7,
    },
  }]]);

  let attempted = 0;
  const failing = api.createCodexStatusClient({
    isNative: () => true,
    createRequestId: () => requestId,
    invoke: async () => { attempted += 1; throw new Error("C:\\secret\\config"); },
  });
  assert.deepEqual(await failing.load({ installation_id: installationId, surface: "cli" }, 7), { kind: "request_failed" });
  assert.equal(attempted, 1);
});

test("browser, invalid request inputs, and stale UI request tokens stay unavailable", async () => {
  assert.ok(api);
  let calls = 0;
  const browser = api.createCodexStatusClient({
    isNative: () => false,
    createRequestId: () => requestId,
    invoke: async () => { calls += 1; return makeSnapshot(); },
  });
  assert.deepEqual(await browser.load({ installation_id: installationId, surface: "cli" }, 7), { kind: "desktop_unavailable" });

  const native = api.createCodexStatusClient({
    isNative: () => true,
    createRequestId: () => requestId,
    invoke: async () => { calls += 1; return makeSnapshot(); },
  });
  assert.deepEqual(await native.load({ installation_id: installationId, surface: "cli" }, Number.MAX_SAFE_INTEGER + 1), { kind: "invalid_request" });
  assert.equal(calls, 0);

  const sequence = new api.CodexStatusRequestSequence();
  const oldRequest = sequence.next();
  sequence.invalidate();
  const newRequest = sequence.next();
  assert.equal(sequence.isCurrent(oldRequest), false);
  assert.equal(sequence.isCurrent(newRequest), true);
});
