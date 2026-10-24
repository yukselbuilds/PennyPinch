import { invoke, isTauri } from "@tauri-apps/api/core";

export type CodexSurface = "cli" | "desktop";
export type CodexCompatibility = "fixture_only" | "not_tested" | "not_verified";
export type CodexCurrentConfigValue =
  | "matches_owned_endpoint"
  | "missing"
  | "different"
  | "unsupported_route"
  | "unavailable"
  | "not_owned"
  | "not_rechecked";
export type CodexStatusErrorCode =
  | "validation_failed"
  | "stale_selection"
  | "not_found"
  | "client_mismatch"
  | "unavailable";

export interface CodexStatusSelection {
  installation_id: string;
  surface: CodexSurface;
}

export interface CodexStatusSnapshot {
  schema_version: 1;
  request_id: string;
  status: "snapshot";
  installation: {
    installation_id: string;
    surface: CodexSurface;
    version: string | null;
    compatibility: CodexCompatibility;
  };
  config_scope: "user_config";
  ownership: {
    state: "active" | "released" | "none" | "unavailable";
    endpoint_binding: "same" | "different" | "not_owned" | "unknown";
    config_source: "codex_toml" | null;
    endpoint_id: string | null;
    endpoint_port: number | null;
    current_config_value: CodexCurrentConfigValue;
  };
  runtime: {
    ingress_status: "starting" | "ready" | "unknown" | "unavailable";
    stable_ingress_ready: boolean;
    engine_status: "unavailable" | "stopped" | "starting" | "ready" | "draining" | "failed" | "unknown";
    engine_ready: boolean;
    runtime_generation: string | null;
    active_connection_count: number | null;
    error_code:
      | "validation_failed"
      | "unsupported_protocol"
      | "environment_blocked"
      | "busy"
      | "port_conflict"
      | "engine_not_ready"
      | "ownership_lost"
      | "timeout"
      | "cleanup_failed"
      | "config_conflict"
      | null;
  };
  policy: {
    model_changed_by_headroom: false;
    reasoning_changed_by_headroom: false;
    auth_changed_by_headroom: false;
    restart_required_after_apply: true;
    runtime_effective: "unknown";
  };
  auth_scope: {
    current_method: "not_read";
    chatgpt_auth: "not_tested";
    api_key_auth: "fixture_only" | "not_tested";
  };
  coverage: {
    traffic_verification: "not_verified";
    main_agent: "unknown";
    subagent: "not_tested";
  };
}

type CodexRuntimeStatus = CodexStatusSnapshot["runtime"]["ingress_status"]
  | CodexStatusSnapshot["runtime"]["engine_status"];

export function visibleRuntimeStatus(
  status: CodexRuntimeStatus,
  verifiedReady: boolean,
): CodexRuntimeStatus {
  if (status === "ready" && !verifiedReady) return "unknown";
  return status;
}

export interface CodexStatusError {
  schema_version: 1;
  request_id: string;
  status: "error";
  error: {
    code: CodexStatusErrorCode;
    retryable: boolean;
    message_key: string;
    diagnostic_id: null;
  };
}

export type CodexStatusResult =
  | { kind: "snapshot"; snapshot: CodexStatusSnapshot }
  | { kind: "codex_error"; error: CodexStatusError }
  | { kind: "desktop_unavailable" }
  | { kind: "invalid_request" }
  | { kind: "invalid_response" }
  | { kind: "request_failed" };

type UnknownRecord = Record<string, unknown>;
type InvokeFunction = (command: string, args?: Record<string, unknown>) => Promise<unknown>;
type ExpectedReply = {
  requestId: string;
  installationId: string;
  surface: CodexSurface;
  scanGeneration: number;
};

const maxSafeInteger = Number.MAX_SAFE_INTEGER;
const apiKeyFixtureCliVersions = new Set(["0.163.0-alpha.10.2", "0.163.0-alpha.11.1"]);
const supportedCliVersions = new Set([...apiKeyFixtureCliVersions, "0.163.0-alpha.11.2"]);
const errorContracts: Record<CodexStatusErrorCode, { retryable: boolean; messageKey: string }> = {
  validation_failed: { retryable: false, messageKey: "codex.status.validation_failed" },
  stale_selection: { retryable: true, messageKey: "codex.status.stale_selection" },
  not_found: { retryable: false, messageKey: "codex.status.not_found" },
  client_mismatch: { retryable: false, messageKey: "codex.status.client_mismatch" },
  unavailable: { retryable: true, messageKey: "codex.status.unavailable" },
};
const runtimeErrorCodes = [
  "validation_failed", "unsupported_protocol", "environment_blocked", "busy", "port_conflict",
  "engine_not_ready", "ownership_lost", "timeout", "cleanup_failed", "config_conflict",
] as const;

function isRecord(value: unknown): value is UnknownRecord {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function hasExactKeys(value: UnknownRecord, keys: readonly string[]): boolean {
  const actual = Object.keys(value);
  return actual.length === keys.length && keys.every((key) => Object.hasOwn(value, key));
}

function isCanonicalUuid(value: unknown): value is string {
  return typeof value === "string" &&
    /^[0-9a-f]{8}-[0-9a-f]{4}-[1-8][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/.test(value);
}

function isVersion(value: unknown): value is string {
  if (typeof value !== "string" || value.length === 0 || value.length > 64 || /\s|[^\x00-\x7f]/.test(value)) return false;
  const buildSeparator = value.indexOf("+");
  const withoutBuild = buildSeparator === -1 ? value : value.slice(0, buildSeparator);
  const build = buildSeparator === -1 ? null : value.slice(buildSeparator + 1);
  if (buildSeparator !== -1 && (build === null || build.includes("+") || !validIdentifiers(build))) return false;
  const prereleaseSeparator = withoutBuild.indexOf("-");
  const core = prereleaseSeparator === -1 ? withoutBuild : withoutBuild.slice(0, prereleaseSeparator);
  const prerelease = prereleaseSeparator === -1 ? null : withoutBuild.slice(prereleaseSeparator + 1);
  if (prerelease !== null && !validIdentifiers(prerelease)) return false;
  const components = core.split(".");
  return components.length >= 2 && components.length <= 4 && components.every((part) =>
    part.length > 0 && part.length <= 10 && /^\d+$/.test(part) && (part.length === 1 || !part.startsWith("0")));
}

function validIdentifiers(value: string): boolean {
  return value.length > 0 && value.length <= 32 && value.split(".").every((part) =>
    part.length > 0 && /^[A-Za-z0-9-]+$/.test(part));
}

function isPositiveSafeInteger(value: unknown): value is number {
  return typeof value === "number" && Number.isSafeInteger(value) && value > 0 && value <= maxSafeInteger;
}

function isNullableSafeCount(value: unknown): value is number | null {
  return value === null || (typeof value === "number" && Number.isSafeInteger(value) && value >= 0 && value <= maxSafeInteger);
}

function decodeSnapshot(value: unknown, expected: ExpectedReply): CodexStatusSnapshot | null {
  if (!isRecord(value) || !isRecord(value.installation) || !isRecord(value.ownership)
    || !isRecord(value.runtime) || !isRecord(value.policy) || !isRecord(value.auth_scope)
    || !isRecord(value.coverage) || !isCanonicalUuid(expected.requestId)
    || !isCanonicalUuid(expected.installationId) || !isPositiveSafeInteger(expected.scanGeneration)
    || !["cli", "desktop"].includes(expected.surface)
    || !hasExactKeys(value, ["schema_version", "request_id", "status", "installation", "config_scope", "ownership", "runtime", "policy", "auth_scope", "coverage"])) return null;

  const installation = value.installation;
  const ownership = value.ownership;
  const runtime = value.runtime;
  const policy = value.policy;
  const auth = value.auth_scope;
  const coverage = value.coverage;
  if (!hasExactKeys(installation, ["installation_id", "surface", "version", "compatibility"])
    || !hasExactKeys(ownership, ["state", "endpoint_binding", "config_source", "endpoint_id", "endpoint_port", "current_config_value"])
    || !hasExactKeys(runtime, ["ingress_status", "stable_ingress_ready", "engine_status", "engine_ready", "runtime_generation", "active_connection_count", "error_code"])
    || !hasExactKeys(policy, ["model_changed_by_headroom", "reasoning_changed_by_headroom", "auth_changed_by_headroom", "restart_required_after_apply", "runtime_effective"])
    || !hasExactKeys(auth, ["current_method", "chatgpt_auth", "api_key_auth"])
    || !hasExactKeys(coverage, ["traffic_verification", "main_agent", "subagent"])) return null;

  if (value.schema_version !== 1 || value.request_id !== expected.requestId || value.status !== "snapshot"
    || value.config_scope !== "user_config"
    || installation.installation_id !== expected.installationId || installation.surface !== expected.surface
    || !(installation.version === null || isVersion(installation.version))) return null;

  const fixtureScope = expected.surface === "cli" && installation.version !== null
    && apiKeyFixtureCliVersions.has(installation.version);
  const expectedCompatibility = expected.surface === "desktop" ? "not_tested" : fixtureScope ? "fixture_only" : "not_verified";
  if (installation.compatibility !== expectedCompatibility) return null;

  if (!( ["active", "released", "none", "unavailable"] as unknown[]).includes(ownership.state)
    || !( ["same", "different", "not_owned", "unknown"] as unknown[]).includes(ownership.endpoint_binding)
    || !(ownership.config_source === null || ownership.config_source === "codex_toml")
    || !(ownership.endpoint_id === null || isCanonicalUuid(ownership.endpoint_id))
    || !(ownership.endpoint_port === null || (Number.isInteger(ownership.endpoint_port) && (ownership.endpoint_port as number) >= 1 && (ownership.endpoint_port as number) <= 65_535))
    || !( ["matches_owned_endpoint", "missing", "different", "unsupported_route", "unavailable", "not_owned", "not_rechecked"] as unknown[]).includes(ownership.current_config_value)) return null;

  const ownerHasEndpoint = ownership.config_source === "codex_toml" && ownership.endpoint_id !== null && ownership.endpoint_port !== null;
  if ((ownership.state === "active" && (!ownerHasEndpoint || !["same", "different", "unknown"].includes(ownership.endpoint_binding as string)))
    || (ownership.state === "released" && (!ownerHasEndpoint || ownership.endpoint_binding !== "not_owned"))
    || (ownership.state === "none" && (ownership.config_source !== null || ownership.endpoint_id !== null || ownership.endpoint_port !== null || ownership.endpoint_binding !== "not_owned"))
    || (ownership.state === "unavailable" && (ownership.config_source !== null || ownership.endpoint_id !== null || ownership.endpoint_port !== null || ownership.endpoint_binding !== "unknown"))) return null;

  const currentConfigValue = ownership.current_config_value as CodexCurrentConfigValue;
  const supportsConfigObservation = expected.surface === "cli" && installation.version !== null
    && supportedCliVersions.has(installation.version);
  if ((ownership.state === "active" && (supportsConfigObservation
      ? !["matches_owned_endpoint", "missing", "different", "unsupported_route", "unavailable"].includes(currentConfigValue)
      : currentConfigValue !== "not_rechecked"))
    || ((ownership.state === "released" || ownership.state === "none") && currentConfigValue !== "not_owned")
    || (ownership.state === "unavailable" && currentConfigValue !== "unavailable")) return null;

  if (!( ["starting", "ready", "unknown", "unavailable"] as unknown[]).includes(runtime.ingress_status)
    || typeof runtime.stable_ingress_ready !== "boolean"
    || !( ["unavailable", "stopped", "starting", "ready", "draining", "failed", "unknown"] as unknown[]).includes(runtime.engine_status)
    || typeof runtime.engine_ready !== "boolean"
    || !(runtime.runtime_generation === null || (typeof runtime.runtime_generation === "string" && /^[0-9a-f]{32}$/.test(runtime.runtime_generation)))
    || !isNullableSafeCount(runtime.active_connection_count)
    || !(runtime.error_code === null || (typeof runtime.error_code === "string" && (runtimeErrorCodes as readonly string[]).includes(runtime.error_code)))) return null;
  if (runtime.stable_ingress_ready && runtime.ingress_status !== "ready") return null;
  if (["unavailable", "stopped", "starting"].includes(runtime.engine_status as string) && runtime.runtime_generation !== null) return null;
  if (runtime.engine_status === "ready" && runtime.runtime_generation === null) return null;
  if (runtime.engine_ready && (runtime.engine_status !== "ready" || !runtime.stable_ingress_ready
    || runtime.runtime_generation === null || runtime.error_code !== null)) return null;

  if (policy.model_changed_by_headroom !== false || policy.reasoning_changed_by_headroom !== false
    || policy.auth_changed_by_headroom !== false || policy.restart_required_after_apply !== true
    || policy.runtime_effective !== "unknown"
    || auth.current_method !== "not_read" || auth.chatgpt_auth !== "not_tested"
    || auth.api_key_auth !== (fixtureScope ? "fixture_only" : "not_tested")
    || coverage.traffic_verification !== "not_verified" || coverage.main_agent !== "unknown"
    || coverage.subagent !== "not_tested") return null;

  return {
    schema_version: 1,
    request_id: expected.requestId,
    status: "snapshot",
    installation: {
      installation_id: expected.installationId,
      surface: expected.surface,
      version: installation.version as string | null,
      compatibility: expectedCompatibility,
    },
    config_scope: "user_config",
    ownership: {
      state: ownership.state as CodexStatusSnapshot["ownership"]["state"],
      endpoint_binding: ownership.endpoint_binding as CodexStatusSnapshot["ownership"]["endpoint_binding"],
      config_source: ownership.config_source as "codex_toml" | null,
      endpoint_id: ownership.endpoint_id as string | null,
      endpoint_port: ownership.endpoint_port as number | null,
      current_config_value: currentConfigValue,
    },
    runtime: {
      ingress_status: runtime.ingress_status as CodexStatusSnapshot["runtime"]["ingress_status"],
      stable_ingress_ready: runtime.stable_ingress_ready,
      engine_status: runtime.engine_status as CodexStatusSnapshot["runtime"]["engine_status"],
      engine_ready: runtime.engine_ready,
      runtime_generation: runtime.runtime_generation as string | null,
      active_connection_count: runtime.active_connection_count as number | null,
      error_code: runtime.error_code as CodexStatusSnapshot["runtime"]["error_code"],
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
      api_key_auth: fixtureScope ? "fixture_only" : "not_tested",
    },
    coverage: {
      traffic_verification: "not_verified",
      main_agent: "unknown",
      subagent: "not_tested",
    },
  };
}

export type CodexSettingPresentationKey =
  | "matches"
  | "missing"
  | "different"
  | "unsupported_route"
  | "unavailable"
  | "not_owned"
  | "not_rechecked";

export function presentCodexSetting(value: CodexCurrentConfigValue): {
  key: CodexSettingPresentationKey;
  tone: "positive" | "caution" | "muted" | "quiet";
} {
  switch (value) {
    case "matches_owned_endpoint": return { key: "matches", tone: "positive" };
    case "missing": return { key: "missing", tone: "caution" };
    case "different": return { key: "different", tone: "caution" };
    case "unsupported_route": return { key: "unsupported_route", tone: "caution" };
    case "unavailable": return { key: "unavailable", tone: "caution" };
    case "not_owned": return { key: "not_owned", tone: "quiet" };
    case "not_rechecked": return { key: "not_rechecked", tone: "muted" };
  }
}

export type CodexNextActionCode =
  | "unsupported_installation"
  | "refresh_status"
  | "manage_connection"
  | "inspect_codex_setting"
  | "review_connection"
  | "inspect_ingress"
  | "start_engine"
  | "wait_for_engine"
  | "inspect_runtime"
  | "local_prerequisites_ready";

export type CodexNextAction = {
  code: CodexNextActionCode;
  state: "action_required" | "local_only";
};

/** Gives one bounded, read-only next step from a fresh local snapshot. It never verifies client traffic. */
export function diagnoseCodexNextAction(snapshot: CodexStatusSnapshot): CodexNextAction {
  const { installation, ownership, runtime } = snapshot;
  if (installation.surface !== "cli" || installation.version === null || !supportedCliVersions.has(installation.version)) {
    return { code: "unsupported_installation", state: "action_required" };
  }
  if (ownership.state === "unavailable") return { code: "refresh_status", state: "action_required" };
  if (ownership.state === "none" || ownership.state === "released") {
    return { code: "manage_connection", state: "action_required" };
  }
  if (ownership.current_config_value !== "matches_owned_endpoint") {
    return { code: "inspect_codex_setting", state: "action_required" };
  }
  if (ownership.endpoint_binding !== "same") return { code: "review_connection", state: "action_required" };
  if (!runtime.stable_ingress_ready || runtime.ingress_status !== "ready") {
    return { code: "inspect_ingress", state: "action_required" };
  }
  if (runtime.error_code !== null) return { code: "inspect_runtime", state: "action_required" };
  if (runtime.engine_status === "stopped" && !runtime.engine_ready) {
    return { code: "start_engine", state: "action_required" };
  }
  if ((runtime.engine_status === "starting" || runtime.engine_status === "draining") && !runtime.engine_ready) {
    return { code: "wait_for_engine", state: "action_required" };
  }
  if (runtime.engine_status !== "ready" || !runtime.engine_ready || runtime.runtime_generation === null) {
    return { code: "inspect_runtime", state: "action_required" };
  }
  return { code: "local_prerequisites_ready", state: "local_only" };
}

function decodeError(value: unknown, expected: ExpectedReply): CodexStatusError | null {
  if (!isRecord(value) || !isRecord(value.error) || !isCanonicalUuid(expected.requestId)
    || !isCanonicalUuid(expected.installationId) || !isPositiveSafeInteger(expected.scanGeneration)
    || !["cli", "desktop"].includes(expected.surface)
    || !hasExactKeys(value, ["schema_version", "request_id", "status", "error"])
    || !hasExactKeys(value.error, ["code", "retryable", "message_key", "diagnostic_id"])) return null;
  const code = value.error.code;
  if (typeof code !== "string" || !Object.hasOwn(errorContracts, code)) return null;
  const contract = errorContracts[code as CodexStatusErrorCode];
  if (value.schema_version !== 1 || value.request_id !== expected.requestId || value.status !== "error"
    || value.error.retryable !== contract.retryable || value.error.message_key !== contract.messageKey
    || value.error.diagnostic_id !== null) return null;
  return {
    schema_version: 1,
    request_id: expected.requestId,
    status: "error",
    error: { code: code as CodexStatusErrorCode, retryable: contract.retryable, message_key: contract.messageKey, diagnostic_id: null },
  };
}

export function decodeCodexStatusReply(value: unknown, expected: ExpectedReply):
  | { kind: "snapshot"; snapshot: CodexStatusSnapshot }
  | { kind: "codex_error"; error: CodexStatusError }
  | null {
  const snapshot = decodeSnapshot(value, expected);
  if (snapshot) return { kind: "snapshot", snapshot };
  const error = decodeError(value, expected);
  return error ? { kind: "codex_error", error } : null;
}

export class CodexStatusRequestSequence {
  private sequence = 0;

  next(): number {
    this.sequence += 1;
    return this.sequence;
  }

  invalidate(): void {
    this.sequence += 1;
  }

  isCurrent(token: number): boolean {
    return token === this.sequence;
  }
}

export function createCodexStatusClient(options: {
  isNative: () => boolean;
  createRequestId: () => string | null;
  invoke: InvokeFunction;
}): {
  load(selection: CodexStatusSelection, scanGeneration: number): Promise<CodexStatusResult>;
} {
  return {
    async load(selection, scanGeneration) {
      try {
        if (!options.isNative()) return { kind: "desktop_unavailable" };
      } catch {
        return { kind: "desktop_unavailable" };
      }
      if (!selection || !isCanonicalUuid(selection.installation_id)
        || !["cli", "desktop"].includes(selection.surface)
        || !isPositiveSafeInteger(scanGeneration)) return { kind: "invalid_request" };

      let requestId: string | null;
      try {
        requestId = options.createRequestId();
      } catch {
        return { kind: "request_failed" };
      }
      if (!isCanonicalUuid(requestId)) return { kind: "request_failed" };
      const expected = {
        requestId,
        installationId: selection.installation_id,
        surface: selection.surface,
        scanGeneration,
      } as const;
      try {
        const raw = await options.invoke("get_codex_connection_status", {
          request: {
            schema_version: 1,
            request_id: requestId,
            installation_id: selection.installation_id,
            scan_generation: scanGeneration,
          },
        });
        return decodeCodexStatusReply(raw, expected) ?? { kind: "invalid_response" };
      } catch {
        return { kind: "request_failed" };
      }
    },
  };
}

export const codexStatusClient = createCodexStatusClient({
  isNative: isTauri,
  createRequestId: () => crypto.randomUUID(),
  invoke,
});

export function isCodexStatusAvailable(): boolean {
  try {
    return isTauri();
  } catch {
    return false;
  }
}

export type CodexVerifyFailureCode =
  | "unsupported_installation"
  | "runtime_unavailable"
  | "config_unavailable"
  | "route_unsupported"
  | "workspace_unavailable"
  | "process_ownership_unavailable"
  | "process_start_failed"
  | "process_ownership_lost"
  | "process_output_invalid"
  | "process_cancelled"
  | "process_timeout"
  | "process_wait_failed"
  | "transport_unverified"
  | "selection_changed"
  | "runtime_changed"
  | "process_failed"
  | "config_changed"
  | "state_changed";

export type CodexVerifyOutputDiagnostic =
  | "reader_timeout"
  | "reader_failed"
  | "output_oversize"
  | "line_or_event_limit"
  | "invalid_json"
  | "invalid_shape"
  | "unexpected_error"
  | "unexpected_tool"
  | "unexpected_item"
  | "unexpected_event"
  | "marker_missing"
  | "marker_multiple"
  | "terminal_missing"
  | "terminal_multiple";

export type CodexVerifyErrorCode =
  | "validation_failed"
  | "stale_selection"
  | "not_found"
  | "busy"
  | "unavailable"
  | "unsupported_platform";

export interface CodexVerifySelection {
  installation_id: string;
  client_kind: "codex";
  surface: CodexSurface;
  version: string | null;
}

export interface CodexVerifySnapshot {
  schema_version: 1;
  request_id: string;
  status: "snapshot";
  installation_id: string;
  scan_generation: number;
  probe_status: "blocked" | "failed" | "passed" | "response_verified_cli_errors";
  failure_code: CodexVerifyFailureCode | null;
  output_diagnostic: CodexVerifyOutputDiagnostic | null;
  cli_error_count: number;
  scope: "new_process_only";
  endpoint_id: string | null;
  runtime_generation: string | null;
  route_eligible: boolean;
  process_completed: boolean;
  marker_received: boolean;
  ingress_peer_observed: boolean;
  selection_unchanged: boolean;
  runtime_generation_unchanged: boolean;
  saved_config_adoption: "unknown";
  auth_general: "not_tested";
  subagent: "not_tested";
}

export interface CodexVerifyError {
  schema_version: 1;
  request_id: string;
  status: "error";
  error: {
    code: CodexVerifyErrorCode;
    retryable: boolean;
    message_key: string;
    diagnostic_id: null;
  };
}

export type CodexVerifyResult =
  | { kind: "snapshot"; snapshot: CodexVerifySnapshot }
  | { kind: "verify_error"; error: CodexVerifyError }
  | { kind: "desktop_unavailable" }
  | { kind: "unsupported_installation" }
  | { kind: "invalid_request" }
  | { kind: "invalid_response" }
  | { kind: "request_failed" };

type VerifyExpected = { requestId: string; installationId: string; scanGeneration: number };

const verifyFailureCodes = [
  "unsupported_installation", "runtime_unavailable", "config_unavailable", "route_unsupported",
  "workspace_unavailable", "process_ownership_unavailable", "process_start_failed",
  "process_ownership_lost", "process_output_invalid", "process_cancelled", "process_timeout",
  "process_wait_failed", "transport_unverified", "selection_changed", "runtime_changed",
  "process_failed", "config_changed", "state_changed",
] as const;
const verifyOutputDiagnostics = [
  "reader_timeout", "reader_failed", "output_oversize", "line_or_event_limit", "invalid_json",
  "invalid_shape", "unexpected_error", "unexpected_tool", "unexpected_item", "unexpected_event",
  "marker_missing", "marker_multiple", "terminal_missing", "terminal_multiple",
] as const;
const MAX_VERIFY_EVENTS = 2048;
const verifyErrorContracts: Record<CodexVerifyErrorCode, { retryable: boolean; messageKey: string }> = {
  validation_failed: { retryable: false, messageKey: "codex.verify.validation_failed" },
  stale_selection: { retryable: true, messageKey: "codex.verify.stale_selection" },
  not_found: { retryable: false, messageKey: "codex.verify.not_found" },
  busy: { retryable: true, messageKey: "codex.verify.busy" },
  unavailable: { retryable: true, messageKey: "codex.verify.unavailable" },
  unsupported_platform: { retryable: false, messageKey: "codex.verify.unsupported_platform" },
};

function decodeCodexVerifySnapshot(value: unknown, expected: VerifyExpected): CodexVerifySnapshot | null {
  if (!isRecord(value) || !hasExactKeys(value, [
    "schema_version", "request_id", "status", "installation_id", "scan_generation", "probe_status",
    "failure_code", "output_diagnostic", "cli_error_count", "scope", "endpoint_id", "runtime_generation", "route_eligible", "process_completed",
    "marker_received", "ingress_peer_observed", "selection_unchanged", "runtime_generation_unchanged",
    "saved_config_adoption", "auth_general", "subagent",
  ])) return null;
  if (value.schema_version !== 1 || value.request_id !== expected.requestId || value.status !== "snapshot"
    || value.installation_id !== expected.installationId || value.scan_generation !== expected.scanGeneration
    || !isCanonicalUuid(expected.requestId) || !isCanonicalUuid(expected.installationId)
    || !isPositiveSafeInteger(expected.scanGeneration)
    || !(value.probe_status === "blocked" || value.probe_status === "failed" || value.probe_status === "passed"
      || value.probe_status === "response_verified_cli_errors")
    || !(value.failure_code === null || (typeof value.failure_code === "string" && (verifyFailureCodes as readonly string[]).includes(value.failure_code)))
    || !(value.output_diagnostic === null || (typeof value.output_diagnostic === "string"
      && (verifyOutputDiagnostics as readonly string[]).includes(value.output_diagnostic)))
    || !Number.isSafeInteger(value.cli_error_count) || (value.cli_error_count as number) < 0
    || (value.cli_error_count as number) > MAX_VERIFY_EVENTS
    || ((value.cli_error_count as number) > 0 && value.output_diagnostic === null)
    || value.scope !== "new_process_only"
    || !(value.endpoint_id === null || isCanonicalUuid(value.endpoint_id))
    || !(value.runtime_generation === null || isCanonicalUuid(value.runtime_generation))
    || typeof value.route_eligible !== "boolean" || typeof value.process_completed !== "boolean"
    || typeof value.marker_received !== "boolean" || typeof value.ingress_peer_observed !== "boolean"
    || typeof value.selection_unchanged !== "boolean" || typeof value.runtime_generation_unchanged !== "boolean"
    || value.saved_config_adoption !== "unknown" || value.auth_general !== "not_tested" || value.subagent !== "not_tested") return null;

  const proofFlags = ["route_eligible", "process_completed", "marker_received", "ingress_peer_observed", "selection_unchanged", "runtime_generation_unchanged"] as const;
  if (value.probe_status === "passed") {
    if (value.failure_code !== null || value.output_diagnostic !== null || value.cli_error_count !== 0
      || value.endpoint_id === null || value.runtime_generation === null
      || proofFlags.some((key) => value[key] !== true)) return null;
  } else if (value.probe_status === "response_verified_cli_errors") {
    if (value.failure_code !== null || value.output_diagnostic !== "unexpected_error" || value.cli_error_count === 0
      || value.endpoint_id === null || value.runtime_generation === null
      || proofFlags.some((key) => value[key] !== true)) return null;
  } else if (value.failure_code === null || (value.probe_status === "blocked"
    && (value.output_diagnostic !== null || value.cli_error_count !== 0))) {
    return null;
  }

  return {
    schema_version: 1,
    request_id: expected.requestId,
    status: "snapshot",
    installation_id: expected.installationId,
    scan_generation: expected.scanGeneration,
    probe_status: value.probe_status,
    failure_code: value.failure_code as CodexVerifyFailureCode | null,
    output_diagnostic: value.output_diagnostic as CodexVerifyOutputDiagnostic | null,
    cli_error_count: value.cli_error_count as number,
    scope: "new_process_only",
    endpoint_id: value.endpoint_id as string | null,
    runtime_generation: value.runtime_generation as string | null,
    route_eligible: value.route_eligible,
    process_completed: value.process_completed,
    marker_received: value.marker_received,
    ingress_peer_observed: value.ingress_peer_observed,
    selection_unchanged: value.selection_unchanged,
    runtime_generation_unchanged: value.runtime_generation_unchanged,
    saved_config_adoption: "unknown",
    auth_general: "not_tested",
    subagent: "not_tested",
  };
}

function decodeCodexVerifyError(value: unknown, expected: VerifyExpected): CodexVerifyError | null {
  if (!isRecord(value) || !isRecord(value.error)
    || !hasExactKeys(value, ["schema_version", "request_id", "status", "error"])
    || !hasExactKeys(value.error, ["code", "retryable", "message_key", "diagnostic_id"])
    || !isCanonicalUuid(expected.requestId) || !isCanonicalUuid(expected.installationId)
    || !isPositiveSafeInteger(expected.scanGeneration)) return null;
  const code = value.error.code;
  if (typeof code !== "string" || !Object.hasOwn(verifyErrorContracts, code)) return null;
  const contract = verifyErrorContracts[code as CodexVerifyErrorCode];
  if (value.schema_version !== 1 || value.request_id !== expected.requestId || value.status !== "error"
    || value.error.retryable !== contract.retryable || value.error.message_key !== contract.messageKey
    || value.error.diagnostic_id !== null) return null;
  return {
    schema_version: 1,
    request_id: expected.requestId,
    status: "error",
    error: { code: code as CodexVerifyErrorCode, retryable: contract.retryable, message_key: contract.messageKey, diagnostic_id: null },
  };
}

export function decodeCodexVerifyReply(value: unknown, expected: VerifyExpected):
  | { kind: "snapshot"; snapshot: CodexVerifySnapshot }
  | { kind: "verify_error"; error: CodexVerifyError }
  | null {
  const snapshot = decodeCodexVerifySnapshot(value, expected);
  if (snapshot) return { kind: "snapshot", snapshot };
  const error = decodeCodexVerifyError(value, expected);
  return error ? { kind: "verify_error", error } : null;
}

export function codexVerifyGenerationChanged(
  snapshotGeneration: string | null,
  generationAtStart: string | null,
): boolean {
  if (snapshotGeneration === null) return false;
  return compactRuntimeGeneration(snapshotGeneration) !== compactRuntimeGeneration(generationAtStart);
}

function compactRuntimeGeneration(value: string | null): string | null {
  return value?.replaceAll("-", "") ?? null;
}

export function isCodexVerifySelectionSupported(selection: CodexVerifySelection): boolean {
  return selection.client_kind === "codex" && selection.surface === "cli"
    && selection.version === "0.163.0-alpha.11.2";
}

export function createCodexVerifyClient(options: {
  isNative: () => boolean;
  createRequestId: () => string | null;
  invoke: InvokeFunction;
}): {
  verify(selection: CodexVerifySelection, scanGeneration: number): Promise<CodexVerifyResult>;
} {
  return {
    async verify(selection, scanGeneration) {
      try {
        if (!options.isNative()) return { kind: "desktop_unavailable" };
      } catch {
        return { kind: "desktop_unavailable" };
      }
      if (!selection || !isCanonicalUuid(selection.installation_id)
        || !["cli", "desktop"].includes(selection.surface)
        || !isPositiveSafeInteger(scanGeneration)) return { kind: "invalid_request" };
      if (!isCodexVerifySelectionSupported(selection)) return { kind: "unsupported_installation" };

      let requestId: string | null;
      try {
        requestId = options.createRequestId();
      } catch {
        return { kind: "request_failed" };
      }
      if (!isCanonicalUuid(requestId)) return { kind: "request_failed" };
      const expected = { requestId, installationId: selection.installation_id, scanGeneration };
      try {
        const raw = await options.invoke("verify_codex_connection", {
          request: {
            schema_version: 1,
            request_id: requestId,
            installation_id: selection.installation_id,
            scan_generation: scanGeneration,
          },
        });
        return decodeCodexVerifyReply(raw, expected) ?? { kind: "invalid_response" };
      } catch {
        return { kind: "request_failed" };
      }
    },
  };
}

export const codexVerifyClient = createCodexVerifyClient({
  isNative: isTauri,
  createRequestId: () => crypto.randomUUID(),
  invoke,
});
