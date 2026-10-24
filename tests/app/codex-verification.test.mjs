import assert from "node:assert/strict";
import test from "node:test";

const api = await import("../../src/lib/codexStatus.ts").catch(() => null);
const requestId = "019a0000-0000-7000-8000-000000000011";
const installationId = "019a0000-0000-7000-8000-000000000012";
const endpointId = "019a0000-0000-7000-8000-000000000013";
const generation = "019a0000-0000-7000-8000-000000000014";

function passedReply(overrides = {}) {
  return {
    schema_version: 1,
    request_id: requestId,
    status: "snapshot",
    installation_id: installationId,
    scan_generation: 8,
    probe_status: "passed",
    failure_code: null,
    output_diagnostic: null,
    cli_error_count: 0,
    scope: "new_process_only",
    endpoint_id: endpointId,
    runtime_generation: generation,
    route_eligible: true,
    process_completed: true,
    marker_received: true,
    ingress_peer_observed: true,
    selection_unchanged: true,
    runtime_generation_unchanged: true,
    saved_config_adoption: "unknown",
    auth_general: "not_tested",
    subagent: "not_tested",
    ...overrides,
  };
}

const expected = { requestId, installationId, scanGeneration: 8 };

test("strictly decodes a passed process-only verification with every proof flag", () => {
  assert.ok(api, "Codex verification client module is available");
  const result = api.decodeCodexVerifyReply(passedReply(), expected);
  assert.equal(result.kind, "snapshot");
  assert.equal(result.snapshot.probe_status, "passed");
  assert.equal(result.snapshot.scope, "new_process_only");
  assert.equal(result.snapshot.saved_config_adoption, "unknown");
  assert.equal(result.snapshot.auth_general, "not_tested");
  assert.equal(result.snapshot.subagent, "not_tested");
});

test("rejects incomplete or contradictory success claims and unexpected fields", () => {
  assert.ok(api);
  for (const value of [
    passedReply({ marker_received: false }),
    passedReply({ failure_code: "process_failed" }),
    passedReply({ endpoint_id: null }),
    passedReply({ runtime_generation: null }),
    passedReply({ scan_generation: 9 }),
    { ...passedReply(), raw_path: "C:\\private" },
    passedReply({ scope: "saved_config" }),
    passedReply({ subagent: "verified" }),
    passedReply({ cli_error_count: 1 }),
  ]) {
    assert.equal(api.decodeCodexVerifyReply(value, expected), null);
  }
});

test("accepts only a complete response with explicitly retained CLI errors", () => {
  assert.ok(api);
  const result = api.decodeCodexVerifyReply(passedReply({
    probe_status: "response_verified_cli_errors",
    output_diagnostic: "unexpected_error",
    cli_error_count: 1,
  }), expected);
  assert.equal(result?.kind, "snapshot");
  assert.equal(result?.snapshot.probe_status, "response_verified_cli_errors");
  assert.equal(result?.snapshot.cli_error_count, 1);

  for (const invalid of [
    passedReply({ probe_status: "response_verified_cli_errors", output_diagnostic: "unexpected_error", cli_error_count: 0 }),
    passedReply({ probe_status: "response_verified_cli_errors", output_diagnostic: null, cli_error_count: 1 }),
    passedReply({ probe_status: "response_verified_cli_errors", output_diagnostic: "unexpected_tool", cli_error_count: 1 }),
    passedReply({ probe_status: "response_verified_cli_errors", failure_code: "process_failed", output_diagnostic: "unexpected_error", cli_error_count: 1 }),
    passedReply({ probe_status: "response_verified_cli_errors", marker_received: false, output_diagnostic: "unexpected_error", cli_error_count: 1 }),
    passedReply({ probe_status: "response_verified_cli_errors", cli_error_count: 2049, output_diagnostic: "unexpected_error" }),
    passedReply({ probe_status: "failed", failure_code: "process_output_invalid", cli_error_count: 1 }),
    passedReply({ probe_status: "response_verified_cli_errors", cli_error_count: 1.5, output_diagnostic: "unexpected_error" }),
    passedReply({ probe_status: "blocked", failure_code: "runtime_unavailable", endpoint_id: null, runtime_generation: null,
      route_eligible: false, process_completed: false, marker_received: false, ingress_peer_observed: false,
      selection_unchanged: false, runtime_generation_unchanged: false, cli_error_count: 1 }),
  ]) {
    assert.equal(api.decodeCodexVerifyReply(invalid, expected), null);
  }

  const fatalAfterNotice = api.decodeCodexVerifyReply(passedReply({
    probe_status: "failed",
    failure_code: "process_output_invalid",
    output_diagnostic: "unexpected_tool",
    cli_error_count: 1,
  }), expected);
  assert.equal(fatalAfterNotice?.snapshot.probe_status, "failed");
  assert.equal(fatalAfterNotice?.snapshot.output_diagnostic, "unexpected_tool");
  assert.equal(fatalAfterNotice?.snapshot.cli_error_count, 1);
});

test("requires the fixed output diagnostic field and rejects unknown diagnostics", () => {
  assert.ok(api);
  const diagnostics = [
    "reader_timeout", "reader_failed", "output_oversize", "line_or_event_limit", "invalid_json",
    "invalid_shape", "unexpected_error", "unexpected_tool", "unexpected_item", "unexpected_event",
    "marker_missing", "marker_multiple", "terminal_missing", "terminal_multiple",
  ];
  const missing = passedReply();
  delete missing.output_diagnostic;
  assert.equal(api.decodeCodexVerifyReply(missing, expected), null);
  for (const output_diagnostic of diagnostics) {
    const failed = passedReply({
      probe_status: "failed",
      failure_code: "process_output_invalid",
      output_diagnostic,
    });
    const decoded = api.decodeCodexVerifyReply(failed, expected);
    assert.equal(decoded?.kind, "snapshot");
    assert.equal(decoded?.snapshot.output_diagnostic, output_diagnostic);
  }
  assert.equal(api.decodeCodexVerifyReply(passedReply({ output_diagnostic: "raw_output" }), expected), null);
  assert.equal(api.decodeCodexVerifyReply(passedReply({ output_diagnostic: "marker_missing" }), expected), null);
});

test("decodes fixed blocked outcomes and rejects uncorrelated error details", () => {
  assert.ok(api);
  const blocked = passedReply({
    probe_status: "blocked",
    failure_code: "runtime_unavailable",
    endpoint_id: null,
    runtime_generation: null,
    route_eligible: false,
    process_completed: false,
    marker_received: false,
    ingress_peer_observed: false,
    selection_unchanged: false,
    runtime_generation_unchanged: false,
  });
  assert.equal(api.decodeCodexVerifyReply(blocked, expected).snapshot.probe_status, "blocked");
  assert.equal(api.decodeCodexVerifyReply({ ...blocked, failure_code: "raw_error" }, expected), null);
  assert.equal(api.decodeCodexVerifyReply({ ...blocked, output_diagnostic: "reader_failed" }, expected), null);
  const error = {
    schema_version: 1,
    request_id: requestId,
    status: "error",
    error: { code: "busy", retryable: true, message_key: "codex.verify.busy", diagnostic_id: null },
  };
  assert.equal(api.decodeCodexVerifyReply(error, expected).kind, "verify_error");
  assert.equal(api.decodeCodexVerifyReply({ ...error, request_id: generation }, expected), null);
  assert.equal(api.decodeCodexVerifyReply({ ...error, error: { ...error.error, message: "secret" } }, expected), null);
});

test("a generation-less blocked reply is not stale, but known generation changes are", () => {
  assert.ok(api);
  assert.equal(api.codexVerifyGenerationChanged(null, generation), false);
  assert.equal(api.codexVerifyGenerationChanged(generation, generation), false);
  assert.equal(api.codexVerifyGenerationChanged("019a0000-0000-7000-8000-000000000099", generation), true);
  assert.equal(api.codexVerifyGenerationChanged(generation, null), true);

  const blocked = passedReply({
    probe_status: "blocked",
    failure_code: "runtime_unavailable",
    endpoint_id: null,
    runtime_generation: null,
    route_eligible: false,
    process_completed: false,
    marker_received: false,
    ingress_peer_observed: false,
    selection_unchanged: false,
    runtime_generation_unchanged: false,
  });
  assert.equal(api.decodeCodexVerifyReply(blocked, expected).kind, "snapshot");
  assert.equal(api.decodeCodexVerifyReply(passedReply({ runtime_generation: null }), expected), null);
});

test("invokes only the correlated selection and scan generation without automatic retry", async () => {
  assert.ok(api);
  const calls = [];
  const client = api.createCodexVerifyClient({
    isNative: () => true,
    createRequestId: () => requestId,
    invoke: async (...args) => { calls.push(args); return passedReply(); },
  });
  const result = await client.verify({
    installation_id: installationId,
    client_kind: "codex",
    surface: "cli",
    version: "0.163.0-alpha.11.2",
  }, 8);
  assert.equal(result.kind, "snapshot");
  assert.deepEqual(calls, [["verify_codex_connection", {
    request: { schema_version: 1, request_id: requestId, installation_id: installationId, scan_generation: 8 },
  }]]);

  let callsAfterFailure = 0;
  const noRetry = api.createCodexVerifyClient({
    isNative: () => true,
    createRequestId: () => requestId,
    invoke: async () => { callsAfterFailure += 1; throw new Error("C:\\private\\config"); },
  });
  assert.deepEqual(await noRetry.verify({
    installation_id: installationId, client_kind: "codex", surface: "cli", version: "0.163.0-alpha.11.2",
  }, 8), { kind: "request_failed" });
  assert.equal(callsAfterFailure, 1);
});

test("does not invoke from browser or for unsupported installations", async () => {
  assert.ok(api);
  let calls = 0;
  const client = api.createCodexVerifyClient({
    isNative: () => false,
    createRequestId: () => requestId,
    invoke: async () => { calls += 1; return passedReply(); },
  });
  const selected = { installation_id: installationId, client_kind: "codex", surface: "cli", version: "0.163.0-alpha.11.2" };
  assert.deepEqual(await client.verify(selected, 8), { kind: "desktop_unavailable" });
  assert.equal(calls, 0);
  const native = api.createCodexVerifyClient({
    isNative: () => true,
    createRequestId: () => requestId,
    invoke: async () => { calls += 1; return passedReply(); },
  });
  assert.deepEqual(await native.verify(selected, Number.MAX_SAFE_INTEGER + 1), { kind: "invalid_request" });
  assert.deepEqual(await native.verify({ ...selected, surface: "desktop" }, 8), { kind: "unsupported_installation" });
  assert.equal(calls, 0);
});
