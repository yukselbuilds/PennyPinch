import { invoke, isTauri } from "@tauri-apps/api/core";

export type ClaudeContextScope = "user_config" | "project_local_config";
export type ClaudeLayerSource =
  | "user_settings"
  | "shared_project_settings"
  | "local_project_settings"
  | "local_managed_settings"
  | "process_environment";
export type ClaudeDeclaration = "absent" | "present" | "unknown";
export type ClaudeAuthDeclaration = "unknown" | "secret_configured" | "helper_declared";
export type ClaudeHookEffect = "absent" | "present_unchanged" | "unknown";

export interface ClaudeContextState {
  schema_version: 1;
  request_id: string;
  status: "snapshot" | "selected" | "cancelled";
  scope: ClaudeContextScope;
  project_directory: string | null;
}

export interface ClaudeLayerSummary {
  source: ClaudeLayerSource;
  availability: "present" | "missing" | "not_observed";
  route: "absent" | "headroom_endpoint" | "other_configured_redacted" | "unknown";
  model: ClaudeDeclaration;
  reasoning: ClaudeDeclaration;
  auth: ClaudeAuthDeclaration;
  hooks: ClaudeHookEffect;
}

export interface ClaudeSettingsSnapshot {
  schema_version: 1;
  request_id: string;
  status: "snapshot";
  scope: ClaudeContextScope;
  layers: ClaudeLayerSummary[];
  model_declared_in_observed_files: ClaudeDeclaration;
  reasoning_declared_in_observed_files: ClaudeDeclaration;
  auth_declared_in_observed_files: ClaudeAuthDeclaration;
  hooks_declared_in_observed_files: ClaudeHookEffect;
  runtime_effective: "unknown";
  remote_managed_policy: "unknown";
  other_session_overrides: "unknown";
}

export type ClaudeContextErrorCode =
  | "validation_failed"
  | "unsupported_scope"
  | "unsafe_project_directory"
  | "picker_unavailable"
  | "busy"
  | "unavailable";
export type ClaudeSettingsErrorCode =
  | "validation_failed"
  | "unsupported_schema"
  | "busy"
  | "not_found"
  | "plan_expired"
  | "config_conflict"
  | "config_invalid"
  | "config_unsafe"
  | "config_unavailable"
  | "unsupported_filesystem"
  | "commit_unknown";

export interface ClaudeContextError {
  code: ClaudeContextErrorCode;
  retryable: boolean;
  message_key: `connection.${string}`;
  diagnostic_id: null;
}

export interface ClaudeSettingsError {
  code: ClaudeSettingsErrorCode;
  retryable: boolean;
  message_key: `connection.${string}`;
  diagnostic_id: null;
}

export type ClaudeContextResult =
  | { kind: "desktop_unavailable" | "request_failed" | "invalid_request" | "invalid_response" }
  | { kind: "state"; state: ClaudeContextState }
  | { kind: "context_error"; error: ClaudeContextError };
export type ClaudeSettingsResult =
  | { kind: "desktop_unavailable" | "request_failed" | "invalid_request" | "invalid_response" }
  | { kind: "snapshot"; snapshot: ClaudeSettingsSnapshot }
  | { kind: "action_error"; error: ClaudeSettingsError };

type UnknownRecord = Record<string, unknown>;
type InvokeFunction = (command: string, args?: Record<string, unknown>) => Promise<unknown>;
const uuidPattern = /^[0-9a-f]{8}-[0-9a-f]{4}-[1-8][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;
const contextStateKeys = ["schema_version", "request_id", "status", "scope", "project_directory"] as const;
const contextErrorKeys = ["schema_version", "request_id", "status", "error"] as const;
const errorDetailKeys = ["code", "retryable", "message_key", "diagnostic_id"] as const;
const snapshotKeys = [
  "schema_version", "request_id", "status", "scope", "layers",
  "model_declared_in_observed_files", "reasoning_declared_in_observed_files",
  "auth_declared_in_observed_files", "hooks_declared_in_observed_files",
  "runtime_effective", "remote_managed_policy", "other_session_overrides",
] as const;
const layerKeys = ["source", "availability", "route", "model", "reasoning", "auth", "hooks"] as const;
const layerSources: readonly ClaudeLayerSource[] = [
  "user_settings", "shared_project_settings", "local_project_settings", "local_managed_settings", "process_environment",
];
const layerOrderByScope: Record<ClaudeContextScope, readonly ClaudeLayerSource[]> = {
  user_config: layerSources,
  project_local_config: layerSources,
};
const contextErrors: Record<ClaudeContextErrorCode, { retryable: boolean; key: string }> = {
  validation_failed: { retryable: false, key: "connection.validation_failed" },
  unsupported_scope: { retryable: false, key: "connection.unsupported_scope" },
  unsafe_project_directory: { retryable: false, key: "connection.config_unsafe" },
  picker_unavailable: { retryable: true, key: "connection.config_unavailable" },
  busy: { retryable: true, key: "connection.busy" },
  unavailable: { retryable: true, key: "connection.config_unavailable" },
};
const settingsErrors: Record<ClaudeSettingsErrorCode, { retryable: boolean; key: string }> = {
  validation_failed: { retryable: false, key: "connection.validation_failed" },
  unsupported_schema: { retryable: false, key: "connection.unsupported_schema" },
  busy: { retryable: true, key: "connection.busy" },
  not_found: { retryable: false, key: "connection.not_found" },
  plan_expired: { retryable: true, key: "connection.plan_expired" },
  config_conflict: { retryable: true, key: "connection.config_conflict" },
  config_invalid: { retryable: false, key: "connection.config_invalid" },
  config_unsafe: { retryable: false, key: "connection.config_unsafe" },
  config_unavailable: { retryable: true, key: "connection.config_unavailable" },
  unsupported_filesystem: { retryable: false, key: "connection.unsupported_filesystem" },
  commit_unknown: { retryable: true, key: "connection.commit_unknown" },
};

function isRecord(value: unknown): value is UnknownRecord {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function exactKeys(value: UnknownRecord, expected: readonly string[]): boolean {
  const keys = Object.keys(value);
  return keys.length === expected.length && expected.every((key) => Object.hasOwn(value, key));
}

function isUuid(value: unknown): value is string {
  return typeof value === "string" && uuidPattern.test(value);
}

function isScope(value: unknown): value is ClaudeContextScope {
  return value === "user_config" || value === "project_local_config";
}

function isWindowsProjectDirectory(value: unknown): value is string {
  if (typeof value !== "string" || value.length < 3 || value.length > 32_767
    || !/^[A-Za-z]:\\/.test(value) || value.includes("/") || /[\u0000-\u001f]/.test(value)) return false;
  const components = value.slice(3).split("\\");
  return components.every((component, index) => {
    if (component === "" && index === components.length - 1) return true;
    return component.length > 0 && component !== "." && component !== ".."
      && !/[<>:"|?*]/.test(component);
  });
}

function decodeContextState(value: unknown, expected: { requestId: string; operation: "snapshot" | "selection"; scope?: ClaudeContextScope }): ClaudeContextState | null {
  if (!isRecord(value) || !exactKeys(value, contextStateKeys)
    || value.schema_version !== 1 || value.request_id !== expected.requestId
    || !isScope(value.scope)) return null;
  const status = value.status;
  if (status !== "snapshot" && status !== "selected" && status !== "cancelled") return null;
  if (expected.operation === "snapshot" && status !== "snapshot") return null;
  if (expected.operation === "selection" && status !== "selected" && status !== "cancelled") return null;
  if (status === "selected" && value.scope !== expected.scope) return null;
  if (status === "cancelled" && expected.scope !== "project_local_config") return null;
  const projectDirectory = value.project_directory;
  if (value.scope === "user_config") {
    if (projectDirectory !== null) return null;
  } else if (!isWindowsProjectDirectory(projectDirectory)) {
    return null;
  }
  return {
    schema_version: 1,
    request_id: expected.requestId,
    status,
    scope: value.scope,
    project_directory: projectDirectory,
  };
}

function decodeError<T extends string>(
  value: unknown,
  expected: { requestId: string; codes: Record<T, { retryable: boolean; key: string }> },
): { code: T; retryable: boolean; message_key: `connection.${string}`; diagnostic_id: null } | null {
  if (!isRecord(value) || !exactKeys(value, contextErrorKeys)
    || value.schema_version !== 1 || value.request_id !== expected.requestId || value.status !== "error"
    || !isRecord(value.error) || !exactKeys(value.error, errorDetailKeys) || value.diagnostic_id !== undefined) return null;
  const detail = value.error;
  const code = detail.code;
  if (typeof code !== "string" || !Object.hasOwn(expected.codes, code)) return null;
  const contract = expected.codes[code as T];
  if (detail.retryable !== contract.retryable || detail.message_key !== contract.key || detail.diagnostic_id !== null) return null;
  return { code: code as T, retryable: contract.retryable, message_key: contract.key as `connection.${string}`, diagnostic_id: null };
}

function isOneOf<T extends string>(value: unknown, values: readonly T[]): value is T {
  return typeof value === "string" && values.includes(value as T);
}

function decodeLayer(value: unknown): ClaudeLayerSummary | null {
  if (!isRecord(value) || !exactKeys(value, layerKeys)
    || !isOneOf(value.source, layerSources)
    || !isOneOf(value.availability, ["present", "missing", "not_observed"] as const)
    || !isOneOf(value.route, ["absent", "headroom_endpoint", "other_configured_redacted", "unknown"] as const)
    || !isOneOf(value.model, ["absent", "present", "unknown"] as const)
    || !isOneOf(value.reasoning, ["absent", "present", "unknown"] as const)
    || !isOneOf(value.auth, ["unknown", "secret_configured", "helper_declared"] as const)
    || !isOneOf(value.hooks, ["absent", "present_unchanged", "unknown"] as const)) return null;
  if (value.availability === "not_observed"
    && (value.route !== "unknown" || value.model !== "unknown" || value.reasoning !== "unknown"
      || value.auth !== "unknown" || value.hooks !== "unknown")) return null;
  if (value.availability === "missing"
    && (value.route !== "absent" || value.model !== "absent" || value.reasoning !== "absent"
      || value.auth !== "unknown" || value.hooks !== "absent")) return null;
  if (value.source === "process_environment"
    && (value.availability !== "present" || value.model !== "unknown" || value.reasoning !== "unknown"
      || value.auth !== "unknown" || value.hooks !== "unknown")) return null;
  return {
    source: value.source,
    availability: value.availability,
    route: value.route,
    model: value.model,
    reasoning: value.reasoning,
    auth: value.auth,
    hooks: value.hooks,
  };
}

function observedLayers(layers: readonly ClaudeLayerSummary[]): ClaudeLayerSummary[] {
  return layers.filter((layer) => layer.source !== "process_environment" && layer.availability !== "not_observed");
}

function aggregateDeclaration(
  layers: readonly ClaudeLayerSummary[],
  select: (layer: ClaudeLayerSummary) => ClaudeDeclaration | ClaudeHookEffect,
): ClaudeDeclaration | ClaudeHookEffect {
  const values = observedLayers(layers).map(select);
  if (values.includes("unknown")) return "unknown";
  if (values.some((value) => value === "present" || value === "present_unchanged")) {
    return values.includes("present_unchanged") ? "present_unchanged" : "present";
  }
  return "absent";
}

function aggregateAuth(layers: readonly ClaudeLayerSummary[]): ClaudeAuthDeclaration {
  let declared: Exclude<ClaudeAuthDeclaration, "unknown"> | null = null;
  for (const layer of observedLayers(layers)) {
    if (layer.auth === "unknown") continue;
    if (declared !== null && declared !== layer.auth) return "unknown";
    declared = layer.auth;
  }
  return declared ?? "unknown";
}

function decodeSettingsSnapshot(value: unknown, expected: { requestId: string; scope: ClaudeContextScope }): ClaudeSettingsSnapshot | null {
  if (!isRecord(value) || !exactKeys(value, snapshotKeys)
    || value.schema_version !== 1 || value.request_id !== expected.requestId
    || value.status !== "snapshot" || value.scope !== expected.scope || !Array.isArray(value.layers)
    || value.layers.length !== layerSources.length
    || !isOneOf(value.model_declared_in_observed_files, ["absent", "present", "unknown"] as const)
    || !isOneOf(value.reasoning_declared_in_observed_files, ["absent", "present", "unknown"] as const)
    || !isOneOf(value.auth_declared_in_observed_files, ["unknown", "secret_configured", "helper_declared"] as const)
    || !isOneOf(value.hooks_declared_in_observed_files, ["absent", "present_unchanged", "unknown"] as const)
    || value.runtime_effective !== "unknown" || value.remote_managed_policy !== "unknown"
    || value.other_session_overrides !== "unknown") return null;
  const layers = value.layers.map(decodeLayer);
  if (layers.some((layer) => layer === null)) return null;
  const decodedLayers = layers as ClaudeLayerSummary[];
  if (decodedLayers.some((layer, index) => layer.source !== layerOrderByScope[expected.scope][index])) return null;
  if (decodedLayers[0].availability === "not_observed" || decodedLayers[4].source !== "process_environment") return null;
  if (expected.scope === "user_config"
    && (decodedLayers[1].availability !== "not_observed" || decodedLayers[2].availability !== "not_observed")) return null;
  if (expected.scope === "project_local_config"
    && (decodedLayers[1].availability === "not_observed" || decodedLayers[2].availability === "not_observed")) return null;
  if (aggregateDeclaration(decodedLayers, (layer) => layer.model) !== value.model_declared_in_observed_files
    || aggregateDeclaration(decodedLayers, (layer) => layer.reasoning) !== value.reasoning_declared_in_observed_files
    || aggregateAuth(decodedLayers) !== value.auth_declared_in_observed_files
    || aggregateDeclaration(decodedLayers, (layer) => layer.hooks) !== value.hooks_declared_in_observed_files) return null;
  return {
    schema_version: 1,
    request_id: expected.requestId,
    status: "snapshot",
    scope: expected.scope,
    layers: decodedLayers,
    model_declared_in_observed_files: value.model_declared_in_observed_files,
    reasoning_declared_in_observed_files: value.reasoning_declared_in_observed_files,
    auth_declared_in_observed_files: value.auth_declared_in_observed_files,
    hooks_declared_in_observed_files: value.hooks_declared_in_observed_files,
    runtime_effective: "unknown",
    remote_managed_policy: "unknown",
    other_session_overrides: "unknown",
  };
}

export function createClaudeContextClient(options: {
  isNative: () => boolean;
  createRequestId: () => string | null;
  invoke: InvokeFunction;
}): {
  context(): Promise<ClaudeContextResult>;
  selectScope(scope: ClaudeContextScope): Promise<ClaudeContextResult>;
  settingsSnapshot(scope: ClaudeContextScope): Promise<ClaudeSettingsResult>;
} {
  async function invokeNative(command: string, request: UnknownRecord): Promise<{ requestId: string; value: unknown } | ClaudeContextResult | ClaudeSettingsResult> {
    try {
      if (!options.isNative()) return { kind: "desktop_unavailable" };
    } catch {
      return { kind: "desktop_unavailable" };
    }
    let requestId: string | null;
    try {
      requestId = options.createRequestId();
    } catch {
      return { kind: "request_failed" };
    }
    if (!isUuid(requestId)) return { kind: "request_failed" };
    try {
      const value = await options.invoke(command, { request: { schema_version: 1, request_id: requestId, ...request } });
      return { requestId, value };
    } catch {
      return { kind: "request_failed" };
    }
  }

  async function invokeContext(command: string, request: UnknownRecord, expected: { operation: "snapshot" | "selection"; scope?: ClaudeContextScope }): Promise<ClaudeContextResult> {
    const result = await invokeNative(command, request);
    if (!("requestId" in result)) return result as ClaudeContextResult;
    const state = decodeContextState(result.value, { requestId: result.requestId, ...expected });
    if (state) return { kind: "state", state };
    const error = decodeError(result.value, { requestId: result.requestId, codes: contextErrors });
    return error ? { kind: "context_error", error } : { kind: "invalid_response" };
  }

  return {
    context() {
      return invokeContext("get_claude_context_snapshot", {}, { operation: "snapshot" });
    },
    selectScope(scope) {
      if (!isScope(scope)) return Promise.resolve({ kind: "invalid_request" });
      return invokeContext("select_claude_scope", { scope }, { operation: "selection", scope });
    },
    async settingsSnapshot(scope) {
      if (!isScope(scope)) return { kind: "invalid_request" };
      const result = await invokeNative("get_claude_settings_snapshot", { scope });
      if (!("requestId" in result)) return result as ClaudeSettingsResult;
      const snapshot = decodeSettingsSnapshot(result.value, { requestId: result.requestId, scope });
      if (snapshot) return { kind: "snapshot", snapshot };
      const error = decodeError(result.value, { requestId: result.requestId, codes: settingsErrors });
      return error ? { kind: "action_error", error } : { kind: "invalid_response" };
    },
  };
}

export function createNativeClaudeContextClient(): ReturnType<typeof createClaudeContextClient> {
  return createClaudeContextClient({
    isNative: () => isTauri(),
    createRequestId: () => globalThis.crypto?.randomUUID?.() ?? null,
    invoke: (command, args) => invoke(command, args),
  });
}
