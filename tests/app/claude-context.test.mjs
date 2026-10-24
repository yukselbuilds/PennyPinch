import assert from "node:assert/strict";
import test from "node:test";

const api = await import("../../src/lib/claudeContext.ts").catch(() => null);
const requestId = "019a0000-0000-7000-8000-000000000001";
const projectDirectory = "C:\\Users\\tester\\work\\project";

function state(status = "snapshot", scope = "user_config", directory = null, overrides = {}) {
  return {
    schema_version: 1,
    request_id: requestId,
    status,
    scope,
    project_directory: directory,
    ...overrides,
  };
}

function contextError(code = "busy", overrides = {}) {
  const contracts = {
    validation_failed: [false, "connection.validation_failed"],
    unsupported_scope: [false, "connection.unsupported_scope"],
    unsafe_project_directory: [false, "connection.config_unsafe"],
    picker_unavailable: [true, "connection.config_unavailable"],
    busy: [true, "connection.busy"],
    unavailable: [true, "connection.config_unavailable"],
  };
  const [retryable, message_key] = contracts[code] ?? [false, "connection.other"];
  return {
    schema_version: 1,
    request_id: requestId,
    status: "error",
    error: { code, retryable, message_key, diagnostic_id: null, ...overrides },
  };
}

function settingsError(code = "config_unavailable", overrides = {}) {
  const contracts = {
    validation_failed: [false, "connection.validation_failed"],
    unsupported_schema: [false, "connection.unsupported_schema"],
    busy: [true, "connection.busy"],
    not_found: [false, "connection.not_found"],
    plan_expired: [true, "connection.plan_expired"],
    config_conflict: [true, "connection.config_conflict"],
    config_invalid: [false, "connection.config_invalid"],
    config_unsafe: [false, "connection.config_unsafe"],
    config_unavailable: [true, "connection.config_unavailable"],
    unsupported_filesystem: [false, "connection.unsupported_filesystem"],
    commit_unknown: [true, "connection.commit_unknown"],
  };
  const [retryable, message_key] = contracts[code] ?? [false, "connection.other"];
  return {
    schema_version: 1,
    request_id: requestId,
    status: "error",
    error: { code, retryable, message_key, diagnostic_id: null, ...overrides },
  };
}

function layer(source, overrides = {}) {
  return {
    source,
    availability: source === "process_environment" ? "present" : "not_observed",
    route: "unknown",
    model: "unknown",
    reasoning: "unknown",
    auth: "unknown",
    hooks: "unknown",
    ...overrides,
  };
}

function settingsSnapshot(scope = "project_local_config", overrides = {}) {
  return {
    schema_version: 1,
    request_id: requestId,
    status: "snapshot",
    scope,
    layers: [
      layer("user_settings", { availability: "present", route: "absent", model: "present", reasoning: "present", auth: "helper_declared", hooks: "present_unchanged" }),
      layer("shared_project_settings", { availability: "missing", route: "absent", model: "absent", reasoning: "absent", hooks: "absent" }),
      layer("local_project_settings", { availability: "present", route: "headroom_endpoint", model: "absent", reasoning: "absent", hooks: "absent" }),
      layer("local_managed_settings", { availability: "missing", route: "absent", model: "absent", reasoning: "absent", hooks: "absent" }),
      layer("process_environment", { route: "absent" }),
    ],
    model_declared_in_observed_files: "present",
    reasoning_declared_in_observed_files: "present",
    auth_declared_in_observed_files: "helper_declared",
    hooks_declared_in_observed_files: "present_unchanged",
    runtime_effective: "unknown",
    remote_managed_policy: "unknown",
    other_session_overrides: "unknown",
    ...overrides,
  };
}

function client(invoke, native = true) {
  return api.createClaudeContextClient({
    isNative: () => native,
    createRequestId: () => requestId,
    invoke,
  });
}

test("the Claude context client is exported", () => {
  assert.equal(typeof api?.createClaudeContextClient, "function");
});

test("context snapshot sends a narrow correlated request and decodes user scope", async () => {
  const result = await client(async (command, args) => {
    assert.equal(command, "get_claude_context_snapshot");
    assert.deepEqual(args, { request: { schema_version: 1, request_id: requestId } });
    return state();
  }).context();

  assert.equal(result.kind, "state");
  assert.equal(result.state.scope, "user_config");
  assert.equal(result.state.project_directory, null);
});

test("project selection sends only scope and accepts the backend-selected directory", async () => {
  const result = await client(async (command, args) => {
    assert.equal(command, "select_claude_scope");
    assert.deepEqual(args, { request: { schema_version: 1, request_id: requestId, scope: "project_local_config" } });
    assert.equal("path" in args.request, false);
    return state("selected", "project_local_config", projectDirectory);
  }).selectScope("project_local_config");

  assert.equal(result.kind, "state");
  assert.equal(result.state.project_directory, projectDirectory);
});

test("cancelled project selection preserves the prior backend scope", async () => {
  const result = await client(async () => state("cancelled", "user_config", null))
    .selectScope("project_local_config");
  assert.equal(result.kind, "state");
  assert.equal(result.state.status, "cancelled");
  assert.equal(result.state.scope, "user_config");
});

test("context replies reject stale correlation, wrong scope shape, unsafe paths, and extra fields", async () => {
  const invalid = [
    state("snapshot", "user_config", null, { request_id: "019a0000-0000-7000-8000-000000000099" }),
    state("snapshot", "user_config", projectDirectory),
    state("snapshot", "project_local_config", null),
    state("snapshot", "project_local_config", "\\\\server\\share\\project"),
    state("snapshot", "project_local_config", "C:\\work\\..\\private"),
    state("snapshot", "project_local_config", `C:\\${"x".repeat(32_768)}`),
    state("snapshot", "project_local_config", projectDirectory, { endpoint: "http://127.0.0.1" }),
    { ...contextError("busy"), debug_path: projectDirectory },
  ];
  for (const reply of invalid) {
    assert.equal((await client(async () => reply).context()).kind, "invalid_response");
  }
});

test("context errors accept only the fixed error vocabulary and tuple", async () => {
  assert.equal((await client(async () => contextError("picker_unavailable")).selectScope("project_local_config")).kind, "context_error");
  assert.equal((await client(async () => contextError("busy", { retryable: false })).context()).kind, "invalid_response");
  assert.equal((await client(async () => contextError("other_error")).context()).kind, "invalid_response");
  assert.equal((await client(async () => contextError("busy", { message: "private details" })).context()).kind, "invalid_response");
  assert.equal((await client(async () => state("selected", "user_config", null)).selectScope("project_local_config")).kind, "invalid_response");
});

test("settings snapshot returns fixed layer metadata and preserves unknown effective state", async () => {
  const result = await client(async (command, args) => {
    assert.equal(command, "get_claude_settings_snapshot");
    assert.deepEqual(args, { request: { schema_version: 1, request_id: requestId, scope: "project_local_config" } });
    return settingsSnapshot();
  }).settingsSnapshot("project_local_config");

  assert.equal(result.kind, "snapshot");
  assert.deepEqual(result.snapshot.layers.map(({ source }) => source), [
    "user_settings", "shared_project_settings", "local_project_settings", "local_managed_settings", "process_environment",
  ]);
  assert.equal(result.snapshot.runtime_effective, "unknown");
  assert.equal(result.snapshot.remote_managed_policy, "unknown");
  assert.equal(result.snapshot.other_session_overrides, "unknown");
});

test("settings snapshot rejects unknown fields, layer duplicates/order, raw values, and false effective claims", async () => {
  const valid = settingsSnapshot();
  const invalid = [
    { ...valid, extra_config: "private" },
    { ...valid, scope: "user_config" },
    { ...valid, layers: [valid.layers[0], ...valid.layers.slice(1, 4), valid.layers[0]] },
    { ...valid, layers: [valid.layers[1], ...valid.layers.slice(1)] },
    { ...valid, layers: valid.layers.map((item, index) => index === 0 ? { ...item, model: "private-model" } : item) },
    { ...valid, layers: valid.layers.map((item, index) => index === 2 ? { ...item, route: "private_route" } : item) },
    { ...valid, runtime_effective: "present" },
    { ...valid, layers: valid.layers.map((item, index) => index === 1 ? { ...item, path: projectDirectory } : item) },
    { ...valid, layers: valid.layers.map((item, index) => index === 3 ? { ...item, auth: "secret_configured" } : item) },
  ];
  for (const reply of invalid) {
    assert.equal((await client(async () => reply).settingsSnapshot("project_local_config")).kind, "invalid_response");
  }
});

test("settings aggregate declarations must match the observed file layers", async () => {
  const valid = settingsSnapshot();
  const inconsistent = [
    { ...valid, model_declared_in_observed_files: "absent" },
    { ...valid, reasoning_declared_in_observed_files: "unknown" },
    { ...valid, auth_declared_in_observed_files: "secret_configured" },
    { ...valid, hooks_declared_in_observed_files: "absent" },
  ];
  for (const reply of inconsistent) {
    assert.equal((await client(async () => reply).settingsSnapshot("project_local_config")).kind, "invalid_response");
  }
});

test("project directory replies reject Windows control characters", async () => {
  const reply = state("selected", "project_local_config", `C:\\Users\\tester\\bad\u0001name`);
  assert.equal((await client(async () => reply).selectScope("project_local_config")).kind, "invalid_response");
});

test("user snapshot requires project layers to remain explicitly unobserved", async () => {
  const base = settingsSnapshot("user_config");
  base.layers[1] = layer("shared_project_settings", { availability: "missing", route: "absent" });
  assert.equal((await client(async () => base).settingsSnapshot("user_config")).kind, "invalid_response");
});

test("desktop unavailability, request ID failure, and invoke failure are distinct", async () => {
  assert.equal((await client(async () => state(), false).context()).kind, "desktop_unavailable");
  const badId = api.createClaudeContextClient({ isNative: () => true, createRequestId: () => "bad", invoke: async () => state() });
  assert.equal((await badId.context()).kind, "request_failed");
  assert.equal((await client(async () => { throw new Error("raw transport text"); }).context()).kind, "request_failed");
});

test("runtime or settings read failure stays a fixed action error", async () => {
  const result = await client(async (command) => {
    assert.equal(command, "get_claude_settings_snapshot");
    return settingsError("config_unavailable");
  }).settingsSnapshot("user_config");
  assert.equal(result.kind, "action_error");
  assert.equal(result.error.code, "config_unavailable");
  assert.equal("message" in result.error, false);
});
