import { invoke, isTauri } from "@tauri-apps/api/core";
import type {
  ConnectionPlan,
  ConnectionPlanError,
  ConnectionPlanErrorCode,
  ConnectionPlanResult,
  OpenCodeRouteCoverage,
  ConnectionScope,
} from "./contracts.ts";
import type { ClientKind as DiscoveredClientKind } from "./integrations.ts";

type UnknownRecord = Record<string, unknown>;
type InvokeFunction = (command: string, args?: Record<string, unknown>) => Promise<unknown>;

const replyKeys = ["schema_version", "request_id", "status", "plan"] as const;
const errorReplyKeys = ["schema_version", "request_id", "status", "error"] as const;
const errorKeys = ["code", "retryable", "message_key", "diagnostic_id"] as const;
const planKeys = [
  "schema_version", "request_id", "plan_id", "expires_at_unix_seconds", "installation_id",
  "client_kind", "scope", "scan_generation", "config_fingerprint", "config_presence",
  "config_source", "endpoint_id", "endpoint_host", "endpoint_port", "endpoint_field",
  "status", "model", "reasoning", "model_unchanged", "provider_unchanged",
  "reasoning_unchanged", "auth_unchanged", "agent_count_unchanged", "runtime_effective",
  "auth", "hook_shell", "restart_required", "blockers",
] as const;
const endpointFieldKeys = ["field", "before", "proposed_value", "after_headroom"] as const;
const openCodeCoverageKeys = [
  "eligible_count", "excluded_count", "already_same_count", "responses_count",
  "chat_completions_count", "partial", "configured_default_route", "runtime_effective",
] as const;

const errorRetryable: Record<ConnectionPlanErrorCode, boolean> = {
  validation_failed: false,
  unsupported_scope: false,
  stale_selection: true,
  not_found: false,
  endpoint_unavailable: true,
  config_unavailable: true,
  config_unsafe: false,
  config_too_large: false,
  config_conflict: true,
  config_invalid: false,
  busy: true,
};

const clientKinds = ["codex", "claude_code", "opencode_v1"] as const;
const connectionScopes = ["user_config", "project_local_config"] as const;
const configSources = ["codex_toml", "claude_json", "opencode_json", "opencode_jsonc", "opencode_missing"] as const;
const presences = ["present", "missing"] as const;
const endpointBefores = ["missing", "same_stable_endpoint", "other_configured_redacted", "unknown", "provider_routes"] as const;
const endpointFields = ["codex_openai_base_url", "claude_anthropic_base_url", "opencode_v1_openai_base_url"] as const;
const declarationStates = ["present", "absent", "unknown"] as const;
const authStates = ["unknown", "method_declared", "secret_configured", "helper_declared"] as const;
const hookStates = ["absent", "present_unchanged", "unknown"] as const;
const blockers = [
  "unsupported_schema", "custom_gateway", "unsupported_route", "client_version_unknown",
  "unsupported_client_version", "unsupported_client_surface",
  "retrieval_mcp_conflict",
] as const;
const errorCodes = Object.keys(errorRetryable) as ConnectionPlanErrorCode[];

const uuidPattern = /^[0-9a-f]{8}-[0-9a-f]{4}-[1-8][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;
const hashPattern = /^[0-9a-f]{64}$/;
const maxSafeInteger = Number.MAX_SAFE_INTEGER;
const maxDateMilliseconds = 8_640_000_000_000_000;

function record(value: unknown): value is UnknownRecord {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function exactKeys(value: UnknownRecord, expected: readonly string[]): boolean {
  const keys = Object.keys(value);
  return keys.length === expected.length && expected.every((key) => Object.hasOwn(value, key));
}

function enumValue<const T extends readonly string[]>(value: unknown, values: T): value is T[number] {
  return typeof value === "string" && (values as readonly string[]).includes(value);
}

function safePositiveInteger(value: unknown): value is number {
  return typeof value === "number" && Number.isSafeInteger(value) && value > 0 && value <= maxSafeInteger;
}

function safeCount(value: unknown, maximum: number): value is number {
  return typeof value === "number" && Number.isSafeInteger(value) && value >= 0 && value <= maximum;
}

export function decodeOpenCodeRouteCoverage(value: unknown): OpenCodeRouteCoverage | null {
  if (!record(value) || !exactKeys(value, openCodeCoverageKeys)
    || !safeCount(value.eligible_count, 16)
    || !safeCount(value.excluded_count, 32)
    || !safeCount(value.already_same_count, 16)
    || value.already_same_count > value.eligible_count
    || !safeCount(value.responses_count, 16)
    || !safeCount(value.chat_completions_count, 16)
    || value.responses_count + value.chat_completions_count !== value.eligible_count
    || value.eligible_count + value.excluded_count > 32
    || typeof value.partial !== "boolean"
    || (value.excluded_count > 0 && !value.partial)
    || !enumValue(value.configured_default_route, ["covered", "excluded", "unknown"] as const)
    || value.runtime_effective !== "unknown"
    || (value.eligible_count === 0 && value.configured_default_route === "covered")) return null;

  return {
    eligible_count: value.eligible_count,
    excluded_count: value.excluded_count,
    already_same_count: value.already_same_count,
    responses_count: value.responses_count,
    chat_completions_count: value.chat_completions_count,
    partial: value.partial,
    configured_default_route: value.configured_default_route,
    runtime_effective: "unknown",
  };
}

function safeDateSeconds(value: unknown): value is number {
  if (!safePositiveInteger(value)) return false;
  const milliseconds = value * 1_000;
  return Number.isSafeInteger(milliseconds)
    && milliseconds <= maxDateMilliseconds
    && Number.isFinite(new Date(milliseconds).getTime());
}

function isCanonicalUuid(value: unknown): value is string {
  return typeof value === "string" && uuidPattern.test(value);
}

function decodeError(value: unknown): ConnectionPlanError | null {
  if (!record(value) || !exactKeys(value, errorKeys) || !enumValue(value.code, errorCodes)) return null;
  if (value.retryable !== errorRetryable[value.code]
    || value.message_key !== `connection.${value.code}`
    || value.diagnostic_id !== null) return null;
  return {
    code: value.code,
    retryable: errorRetryable[value.code],
    message_key: `connection.${value.code}`,
    diagnostic_id: null,
  };
}

function expectedField(clientKind: ConnectionPlan["client_kind"]): ConnectionPlan["endpoint_field"]["field"] {
  switch (clientKind) {
    case "codex": return "codex_openai_base_url";
    case "claude_code": return "claude_anthropic_base_url";
    case "opencode_v1": return "opencode_v1_openai_base_url";
  }
}

function plannedClientKind(clientKind: DiscoveredClientKind): ConnectionPlan["client_kind"] {
  switch (clientKind) {
    case "codex": return "codex";
    case "claude_code": return "claude_code";
    case "opencode": return "opencode_v1";
  }
}

function scopeMatchesClient(clientKind: ConnectionPlan["client_kind"], scope: ConnectionScope): boolean {
  return scope === "user_config" || clientKind === "claude_code";
}

function sourceMatches(
  clientKind: ConnectionPlan["client_kind"],
  source: ConnectionPlan["config_source"],
  presence: ConnectionPlan["config_presence"],
): boolean {
  if (clientKind === "codex") return source === "codex_toml";
  if (clientKind === "claude_code") return source === "claude_json";
  if (source === "opencode_missing") return presence === "missing";
  return (source === "opencode_json" || source === "opencode_jsonc") && presence === "present";
}

function decodePlan(
  value: unknown,
  expected: {
    requestId: string;
    installationId: string;
    scanGeneration: number;
    clientKind: ConnectionPlan["client_kind"];
    scope: ConnectionScope;
  },
): ConnectionPlan | null {
  if (!record(value)
    || !enumValue(value.client_kind, clientKinds)
    || value.client_kind !== expected.clientKind) return null;
  const openCode = value.client_kind === "opencode_v1";
  const hasOpenCodeCoverage = Object.hasOwn(value, "opencode_coverage");
  const hasRetrieval = Object.hasOwn(value, "opencode_retrieval");
  const expectedKeys = [...planKeys,
    ...(openCode && hasOpenCodeCoverage ? ["opencode_coverage"] : []),
    ...(openCode && hasRetrieval ? ["opencode_retrieval"] : []),
  ];
  if (!exactKeys(value, expectedKeys)) return null;
  const retrieval = enumValue(value.opencode_retrieval, ["install_local_bridge", "conflict"] as const)
    ? value.opencode_retrieval : null;
  if (hasRetrieval && (!openCode || retrieval === null)) return null;
  const openCodeCoverage = hasOpenCodeCoverage ? decodeOpenCodeRouteCoverage(value.opencode_coverage) : null;
  if (hasOpenCodeCoverage && (!openCode || openCodeCoverage === null)) return null;
  if (value.schema_version !== 1
    || value.request_id !== expected.requestId
    || value.installation_id !== expected.installationId
    || !isCanonicalUuid(value.plan_id)
    || !safeDateSeconds(value.expires_at_unix_seconds)
    || !safePositiveInteger(value.scan_generation)
    || value.scan_generation !== expected.scanGeneration
    || !enumValue(value.client_kind, clientKinds)
    || value.client_kind !== expected.clientKind
    || !enumValue(value.scope, connectionScopes)
    || value.scope !== expected.scope
    || !scopeMatchesClient(value.client_kind, value.scope)
    || typeof value.config_fingerprint !== "string"
    || !hashPattern.test(value.config_fingerprint)
    || !enumValue(value.config_presence, presences)
    || !enumValue(value.config_source, configSources)
    || !sourceMatches(value.client_kind, value.config_source, value.config_presence)
    || !isCanonicalUuid(value.endpoint_id)
    || value.endpoint_host !== "127.0.0.1"
    || !safePositiveInteger(value.endpoint_port)
    || value.endpoint_port > 65_535
    || !enumValue(value.status, ["preview", "blocked"] as const)
    || !enumValue(value.model, declarationStates)
    || !enumValue(value.reasoning, declarationStates)
    || value.model_unchanged !== true
    || value.provider_unchanged !== true
    || value.reasoning_unchanged !== true
    || value.auth_unchanged !== true
    || value.agent_count_unchanged !== true
    || value.runtime_effective !== "unknown"
    || !enumValue(value.auth, authStates)
    || !enumValue(value.hook_shell, hookStates)
    || value.restart_required !== true
    || !Array.isArray(value.blockers)
    || value.blockers.length > blockers.length
    || !value.blockers.every((item) => enumValue(item, blockers))
    || new Set(value.blockers).size !== value.blockers.length) return null;
  if (retrieval === "conflict"
    && (value.status !== "blocked" || !value.blockers.includes("retrieval_mcp_conflict"))) return null;
  if (retrieval === "install_local_bridge" && value.status !== "preview") return null;
  if (value.blockers.includes("retrieval_mcp_conflict") && retrieval !== "conflict") return null;

  const endpointField = value.endpoint_field;
  if (!record(endpointField)
    || !exactKeys(endpointField, endpointFieldKeys)
    || !enumValue(endpointField.field, endpointFields)
    || endpointField.field !== expectedField(value.client_kind)
    || !enumValue(endpointField.before, endpointBefores)
    || typeof endpointField.proposed_value !== "string"
    || typeof endpointField.after_headroom !== "boolean") return null;

  if (openCode) {
    if (openCodeCoverage) {
      if (openCodeCoverage.eligible_count > 0) {
        if (endpointField.before !== "provider_routes") return null;
      } else if (value.status !== "blocked" || endpointField.before !== "unknown") {
        return null;
      }
    } else if (value.status !== "blocked" || endpointField.before !== "unknown") {
      return null;
    }
  } else if (endpointField.before === "provider_routes") {
    return null;
  }

  const suffix = value.client_kind === "claude_code" ? "" : "/v1";
  const proposed = `http://127.0.0.1:${value.endpoint_port}${suffix}`;
  if (endpointField.proposed_value !== proposed) return null;
  const shouldApply = value.status === "preview";
  if (endpointField.after_headroom !== shouldApply
    || (shouldApply && value.blockers.length !== 0)
    || (!shouldApply && value.blockers.length === 0)) return null;
  if (value.config_presence === "missing"
    && (value.model !== "absent"
      || value.reasoning !== (openCode ? "unknown" : "absent")
      || value.auth !== "unknown"
      || value.hook_shell !== "absent"
      || endpointField.before !== (openCode ? "unknown" : "missing")
      || (openCode && value.status !== "blocked"))) return null;

  return {
    schema_version: 1,
    request_id: value.request_id,
    plan_id: value.plan_id,
    expires_at_unix_seconds: value.expires_at_unix_seconds,
    installation_id: value.installation_id,
    client_kind: value.client_kind,
    scope: value.scope,
    scan_generation: value.scan_generation,
    config_fingerprint: value.config_fingerprint,
    config_presence: value.config_presence,
    config_source: value.config_source,
    endpoint_id: value.endpoint_id,
    endpoint_host: "127.0.0.1",
    endpoint_port: value.endpoint_port,
    endpoint_field: {
      field: endpointField.field,
      before: endpointField.before,
      proposed_value: proposed,
      after_headroom: shouldApply,
    },
    ...(openCodeCoverage ? { opencode_coverage: openCodeCoverage } : {}),
    ...(retrieval ? { opencode_retrieval: retrieval } : {}),
    status: value.status,
    model: value.model,
    reasoning: value.reasoning,
    model_unchanged: true,
    provider_unchanged: true,
    reasoning_unchanged: true,
    auth_unchanged: true,
    agent_count_unchanged: true,
    runtime_effective: "unknown",
    auth: value.auth,
    hook_shell: value.hook_shell,
    restart_required: true,
    blockers: [...value.blockers],
  };
}

export function decodeConnectionPlanReply(
  value: unknown,
  expected: {
    requestId: string;
    installationId: string;
    scanGeneration: number;
    clientKind: DiscoveredClientKind;
    scope: ConnectionScope;
  },
): Extract<ConnectionPlanResult, { kind: "preview" | "plan_error" }> | null {
  if (!isCanonicalUuid(expected.requestId)
    || !isCanonicalUuid(expected.installationId)
    || !safePositiveInteger(expected.scanGeneration)
    || !enumValue(expected.clientKind, ["codex", "claude_code", "opencode"] as const)
    || !enumValue(expected.scope, connectionScopes)
    || !scopeMatchesClient(plannedClientKind(expected.clientKind), expected.scope)
    || !record(value)
    || value.schema_version !== 1
    || value.request_id !== expected.requestId) return null;

  if (value.status === "error") {
    if (!exactKeys(value, errorReplyKeys)) return null;
    const error = decodeError(value.error);
    return error === null ? null : { kind: "plan_error", error };
  }
  if (value.status !== "preview" || !exactKeys(value, replyKeys)) return null;
  const plan = decodePlan(value.plan, {
    ...expected,
    clientKind: plannedClientKind(expected.clientKind),
  });
  return plan === null ? null : { kind: "preview", plan };
}

export function createConnectionPlanClient(options: {
  isNative: () => boolean;
  createRequestId: () => string;
  invoke: InvokeFunction;
}): { preview(input: { installationId: string; scanGeneration: number; clientKind: DiscoveredClientKind; scope?: ConnectionScope }): Promise<ConnectionPlanResult> } {
  return {
    async preview(input) {
      try {
        if (!options.isNative()) return { kind: "desktop_unavailable" };
      } catch {
        return { kind: "desktop_unavailable" };
      }
      if (!isCanonicalUuid(input.installationId)
        || !safePositiveInteger(input.scanGeneration)
        || !enumValue(input.clientKind, ["codex", "claude_code", "opencode"] as const)
        || (input.scope !== undefined && !enumValue(input.scope, connectionScopes))) {
        return { kind: "invalid_request" };
      }
      const scope = input.scope ?? "user_config";
      if (!scopeMatchesClient(plannedClientKind(input.clientKind), scope)) return { kind: "invalid_request" };
      let requestId: string;
      try {
        requestId = options.createRequestId();
      } catch {
        return { kind: "request_failed" };
      }
      if (!isCanonicalUuid(requestId)) return { kind: "request_failed" };

      const request = {
        schema_version: 1,
        request_id: requestId,
        installation_id: input.installationId,
        scan_generation: input.scanGeneration,
        scope,
      } as const;
      try {
        const reply = await options.invoke("preview_connection_plan", { request });
        return decodeConnectionPlanReply(reply, {
          requestId,
          installationId: input.installationId,
          scanGeneration: input.scanGeneration,
          clientKind: input.clientKind,
          scope,
        }) ?? { kind: "invalid_response" };
      } catch {
        return { kind: "request_failed" };
      }
    },
  };
}

export const connectionPlanClient = createConnectionPlanClient({
  isNative: isTauri,
  createRequestId: () => crypto.randomUUID(),
  invoke,
});
