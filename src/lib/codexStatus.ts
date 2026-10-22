import { invoke, isTauri } from "@tauri-apps/api/core";

export type CodexSurface = "cli" | "desktop";
export type CodexCompatibility = "fixture_only" | "not_tested" | "not_verified";
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
    current_config_value: "not_rechecked";
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
const fixtureCliVersions = new Set(["0.163.0-alpha.10.2", "0.163.0-alpha.11.1"]);
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
    && fixtureCliVersions.has(installation.version);
  const expectedCompatibility = expected.surface === "desktop" ? "not_tested" : fixtureScope ? "fixture_only" : "not_verified";
  if (installation.compatibility !== expectedCompatibility) return null;

  if (!( ["active", "released", "none", "unavailable"] as unknown[]).includes(ownership.state)
    || !( ["same", "different", "not_owned", "unknown"] as unknown[]).includes(ownership.endpoint_binding)
    || !(ownership.config_source === null || ownership.config_source === "codex_toml")
    || !(ownership.endpoint_id === null || isCanonicalUuid(ownership.endpoint_id))
    || !(ownership.endpoint_port === null || (Number.isInteger(ownership.endpoint_port) && (ownership.endpoint_port as number) >= 1 && (ownership.endpoint_port as number) <= 65_535))
    || ownership.current_config_value !== "not_rechecked") return null;

  const ownerHasEndpoint = ownership.config_source === "codex_toml" && ownership.endpoint_id !== null && ownership.endpoint_port !== null;
  if ((ownership.state === "active" && (!ownerHasEndpoint || !["same", "different", "unknown"].includes(ownership.endpoint_binding as string)))
    || (ownership.state === "released" && (!ownerHasEndpoint || ownership.endpoint_binding !== "not_owned"))
    || (ownership.state === "none" && (ownership.config_source !== null || ownership.endpoint_id !== null || ownership.endpoint_port !== null || ownership.endpoint_binding !== "not_owned"))
    || (ownership.state === "unavailable" && (ownership.config_source !== null || ownership.endpoint_id !== null || ownership.endpoint_port !== null || ownership.endpoint_binding !== "unknown"))) return null;

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
      current_config_value: "not_rechecked",
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
