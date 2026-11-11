import { invoke, isTauri } from "@tauri-apps/api/core";
import type {
  ApplyConnectionRequest,
  ConnectionActionError,
  ConnectionActionErrorCode,
  ConnectionActionRequest,
  ConnectionActionResult,
  ConnectionActionSuccess,
  ConnectionOwnership,
  ConnectionPlan,
  ConnectionScope,
  OwnershipSnapshotRequest,
  UndoConnectionRequest,
} from "./contracts.ts";
import { decodeOpenCodeRouteCoverage } from "./connectionPlan.ts";

type UnknownRecord = Record<string, unknown>;
type InvokeFunction = (command: string, args?: Record<string, unknown>) => Promise<unknown>;
type ClientKind = ConnectionPlan["client_kind"];
type Operation = "apply" | "undo" | "snapshot";
type ActionRequestBuilder = (requestId: string) => ConnectionActionRequest;

const uuidPattern = /^[0-9a-f]{8}-[0-9a-f]{4}-[1-8][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;
const hashPattern = /^[0-9a-f]{64}$/;
const maxSafe = Number.MAX_SAFE_INTEGER;
const maxDateMilliseconds = 8_640_000_000_000_000;
const topSuccessKeys = ["schema_version", "request_id", "status", "client_kind", "ownership"] as const;
const topErrorKeys = ["schema_version", "request_id", "status", "error"] as const;
const errorKeys = ["code", "retryable", "message_key", "diagnostic_id"] as const;
const ownershipKeys = [
  "state", "scope", "owner_id", "revision", "record_fingerprint", "config_source", "endpoint_id", "endpoint_port",
] as const;

const errorContracts: Record<ConnectionActionErrorCode, { retryable: boolean; messageKey: string }> = {
  validation_failed: { retryable: false, messageKey: "connection.validation_failed" },
  unsupported_schema: { retryable: false, messageKey: "connection.unsupported_schema" },
  busy: { retryable: true, messageKey: "connection.busy" },
  not_found: { retryable: false, messageKey: "connection.not_found" },
  plan_expired: { retryable: true, messageKey: "connection.plan_expired" },
  config_conflict: { retryable: true, messageKey: "connection.config_conflict" },
  config_invalid: { retryable: false, messageKey: "connection.config_invalid" },
  config_unsafe: { retryable: false, messageKey: "connection.config_unsafe" },
  config_unavailable: { retryable: true, messageKey: "connection.config_unavailable" },
  unsupported_filesystem: { retryable: false, messageKey: "connection.unsupported_filesystem" },
  commit_unknown: { retryable: true, messageKey: "connection.commit_unknown" },
  retained_versions_limit: { retryable: true, messageKey: "connection.retained_versions_limit" },
};

const clientKinds = ["codex", "claude_code", "opencode_v1"] as const;

function isRecord(value: unknown): value is UnknownRecord {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function exactKeys(value: UnknownRecord, expected: readonly string[]): boolean {
  const keys = Object.keys(value);
  return keys.length === expected.length && expected.every((key) => Object.hasOwn(value, key));
}

function isCanonicalUuid(value: unknown): value is string {
  return typeof value === "string" && uuidPattern.test(value);
}

function isHash(value: unknown): value is string {
  return typeof value === "string" && hashPattern.test(value);
}

function isSafePositive(value: unknown): value is number {
  return typeof value === "number" && Number.isSafeInteger(value) && value > 0 && value <= maxSafe;
}

function isClientKind(value: unknown): value is ClientKind {
  return typeof value === "string" && (clientKinds as readonly string[]).includes(value);
}

function isConnectionScope(value: unknown): value is ConnectionScope {
  return value === "user_config" || value === "project_local_config";
}

function scopeMatchesClient(kind: ClientKind, scope: ConnectionScope): boolean {
  return scope === "user_config" || kind === "claude_code";
}

function sourceMatches(kind: ClientKind, source: unknown): source is ConnectionPlan["config_source"] {
  if (kind === "codex") return source === "codex_toml";
  if (kind === "claude_code") return source === "claude_json";
  return source === "opencode_json" || source === "opencode_jsonc" || source === "opencode_missing";
}

function sourceMatchesPresence(plan: ConnectionPlan): boolean {
  if (plan.client_kind !== "opencode_v1") return plan.config_presence === "present" || plan.config_presence === "missing";
  return (plan.config_source === "opencode_missing") === (plan.config_presence === "missing");
}

function decodeOwnership(value: unknown, kind: ClientKind): ConnectionOwnership | null {
  if (!isRecord(value) || !exactKeys(value, ownershipKeys)
    || (value.state !== "active" && value.state !== "released")
    || !isConnectionScope(value.scope)
    || !scopeMatchesClient(kind, value.scope)
    || !isCanonicalUuid(value.owner_id)
    || !isSafePositive(value.revision)
    || !isHash(value.record_fingerprint)
    || !sourceMatches(kind, value.config_source)
    || !isCanonicalUuid(value.endpoint_id)
    || !isSafePositive(value.endpoint_port)
    || value.endpoint_port > 65_535) return null;

  return {
    state: value.state,
    scope: value.scope,
    owner_id: value.owner_id,
    revision: value.revision,
    record_fingerprint: value.record_fingerprint,
    config_source: value.config_source,
    endpoint_id: value.endpoint_id,
    endpoint_port: value.endpoint_port,
  };
}

function decodeActionError(value: unknown, requestId: string): ConnectionActionError | null {
  if (!isRecord(value) || !exactKeys(value, topErrorKeys)
    || value.schema_version !== 1 || value.request_id !== requestId || value.status !== "error"
    || !isRecord(value.error) || !exactKeys(value.error, errorKeys)) return null;

  const code = value.error.code;
  if (typeof code !== "string" || !Object.hasOwn(errorContracts, code)) return null;
  const contract = errorContracts[code as ConnectionActionErrorCode];
  if (value.error.retryable !== contract.retryable
    || value.error.message_key !== contract.messageKey
    || value.error.diagnostic_id !== null) return null;

  return {
    code: code as ConnectionActionErrorCode,
    retryable: contract.retryable,
    message_key: contract.messageKey as ConnectionActionError["message_key"],
    diagnostic_id: null,
  };
}

function planIsApplicable(plan: ConnectionPlan): boolean {
  if (!isRecord(plan) || !isRecord(plan.endpoint_field) || !Array.isArray(plan.blockers)
    || plan.schema_version !== 1 || !isCanonicalUuid(plan.request_id)
    || !isCanonicalUuid(plan.plan_id) || !isCanonicalUuid(plan.installation_id)
    || !isCanonicalUuid(plan.endpoint_id) || !isSafePositive(plan.scan_generation)
    || !isHash(plan.config_fingerprint) || !isClientKind(plan.client_kind)
    || !sourceMatches(plan.client_kind, plan.config_source) || !sourceMatchesPresence(plan)
    || !isConnectionScope(plan.scope) || !scopeMatchesClient(plan.client_kind, plan.scope)
    || plan.status !== "preview" || plan.blockers.length !== 0
    || plan.endpoint_host !== "127.0.0.1" || !isSafePositive(plan.endpoint_port)
    || plan.endpoint_port > 65_535 || !plan.endpoint_field.after_headroom
    || !plan.restart_required || plan.runtime_effective !== "unknown"
    || plan.model_unchanged !== true || plan.provider_unchanged !== true
    || plan.reasoning_unchanged !== true || plan.auth_unchanged !== true
    || plan.agent_count_unchanged !== true
    || plan.endpoint_field.after_headroom !== true) return false;
  if (plan.client_kind === "opencode_v1") {
    const coverage = decodeOpenCodeRouteCoverage(plan.opencode_coverage);
    if (!Object.hasOwn(plan, "opencode_coverage") || !coverage
      || coverage.eligible_count === 0 || plan.endpoint_field.before !== "provider_routes"
      || plan.config_presence !== "present"
      || (plan.config_source !== "opencode_json" && plan.config_source !== "opencode_jsonc")) return false;
  } else if (Object.hasOwn(plan, "opencode_coverage")
    || (plan.endpoint_field.before !== "missing" && plan.endpoint_field.before !== "same_stable_endpoint")) {
    return false;
  }
  const expectedField = plan.client_kind === "codex"
    ? "codex_openai_base_url"
    : plan.client_kind === "claude_code"
      ? "claude_anthropic_base_url"
      : "opencode_v1_openai_base_url";
  const endpointSuffix = plan.client_kind === "claude_code" ? "" : "/v1";
  if (plan.endpoint_field.field !== expectedField
    || plan.endpoint_field.proposed_value !== `http://127.0.0.1:${plan.endpoint_port}${endpointSuffix}`) return false;
  const millis = plan.expires_at_unix_seconds * 1_000;
  return Number.isSafeInteger(plan.expires_at_unix_seconds)
    && Number.isSafeInteger(millis)
    && millis <= maxDateMilliseconds
    && Number.isFinite(new Date(millis).getTime())
    && plan.expires_at_unix_seconds > Math.floor(Date.now() / 1_000);
}

function planMatchesOwnership(plan: ConnectionPlan, ownership: ConnectionOwnership): boolean {
  return ownership.scope === plan.scope
    && ownership.config_source === plan.config_source
    && ownership.endpoint_id === plan.endpoint_id
    && ownership.endpoint_port === plan.endpoint_port;
}

function decodeSuccess(
  value: unknown,
  expected: {
    requestId: string;
    clientKind: ClientKind;
    operation: Operation;
    plan?: ConnectionPlan;
    priorOwnership?: ConnectionOwnership;
  },
): ConnectionActionSuccess | null {
  if (!isRecord(value) || !exactKeys(value, topSuccessKeys)
    || value.schema_version !== 1 || value.request_id !== expected.requestId
    || value.client_kind !== expected.clientKind || !isClientKind(value.client_kind)) return null;

  let ownership: ConnectionOwnership | null = null;
  if (value.ownership !== null) {
    ownership = decodeOwnership(value.ownership, expected.clientKind);
    if (!ownership) return null;
  }

  if (expected.operation === "apply") {
    if (value.status !== "applied" && value.status !== "unchanged") return null;
    if (value.status === "applied" && (!ownership || ownership.state !== "active")) return null;
    if (ownership && (ownership.state !== "active" || !expected.plan || !planMatchesOwnership(expected.plan, ownership))) return null;
  } else if (expected.operation === "undo") {
    const prior = expected.priorOwnership;
    if (value.status !== "released" || !ownership || ownership.state !== "released" || !prior
      || prior.state !== "active" || ownership.owner_id !== prior.owner_id
      || ownership.revision !== prior.revision + 1 || !isSafePositive(ownership.revision)
      || ownership.config_source !== prior.config_source
      || ownership.scope !== prior.scope
      || ownership.endpoint_id !== prior.endpoint_id
      || ownership.endpoint_port !== prior.endpoint_port) return null;
  } else if (value.status !== "snapshot") {
    return null;
  }

  return {
    schema_version: 1,
    request_id: expected.requestId,
    status: value.status,
    client_kind: expected.clientKind,
    ownership,
  };
}

function decodeReply(
  value: unknown,
  expected: {
    requestId: string;
    clientKind: ClientKind;
    operation: Operation;
    plan?: ConnectionPlan;
    priorOwnership?: ConnectionOwnership;
  },
): ConnectionActionResult {
  if (isRecord(value) && value.status === "error") {
    const error = decodeActionError(value, expected.requestId);
    return error ? { kind: "action_error", error } : { kind: "invalid_response" };
  }
  const reply = decodeSuccess(value, expected);
  return reply ? { kind: "success", reply } : { kind: "invalid_response" };
}

export function createConnectionActionClient(options: {
  isNative: () => boolean;
  createRequestId: () => string | null;
  invoke: InvokeFunction;
}): {
  apply(plan: ConnectionPlan): Promise<ConnectionActionResult>;
  undo(clientKind: ClientKind, ownership: ConnectionOwnership): Promise<ConnectionActionResult>;
  snapshot(clientKind: ClientKind): Promise<ConnectionActionResult>;
} {
  async function nativeRequest(
    command: string,
    clientKind: ClientKind,
    operation: Operation,
    buildRequest: ActionRequestBuilder,
    expected: { plan?: ConnectionPlan; priorOwnership?: ConnectionOwnership },
  ): Promise<ConnectionActionResult> {
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
    if (!isCanonicalUuid(requestId)) return { kind: "request_failed" };
    const correlatedRequest = buildRequest(requestId);
    try {
      const response = await options.invoke(command, { request: correlatedRequest });
      return decodeReply(response, { requestId, clientKind, operation, ...expected });
    } catch {
      return { kind: "request_failed" };
    }
  }

  return {
    async apply(plan) {
      if (!planIsApplicable(plan)) return { kind: "invalid_request" };
      return nativeRequest(
        "apply_connection_plan",
        plan.client_kind,
        "apply",
        (requestId): ApplyConnectionRequest => ({
          schema_version: 1,
          request_id: requestId,
          plan_id: plan.plan_id,
          expected_config_fingerprint: plan.config_fingerprint,
        }),
        { plan },
      );
    },
    async undo(clientKind, ownership) {
      if (!isClientKind(clientKind) || !ownership || ownership.state !== "active"
        || !decodeOwnership(ownership, clientKind) || !isCanonicalUuid(ownership.owner_id)
        || !isSafePositive(ownership.revision) || ownership.revision >= maxSafe
        || !isHash(ownership.record_fingerprint)) return { kind: "invalid_request" };
      return nativeRequest(
        "undo_connection",
        clientKind,
        "undo",
        (requestId): UndoConnectionRequest => ({
          schema_version: 1,
          request_id: requestId,
          client_kind: clientKind,
          owner_id: ownership.owner_id,
          expected_revision: ownership.revision,
          expected_record_fingerprint: ownership.record_fingerprint,
        }),
        { priorOwnership: ownership },
      );
    },
    async snapshot(clientKind) {
      if (!isClientKind(clientKind)) return { kind: "invalid_request" };
      return nativeRequest(
        "get_connection_ownership",
        clientKind,
        "snapshot",
        (requestId): OwnershipSnapshotRequest => ({
          schema_version: 1,
          request_id: requestId,
          client_kind: clientKind,
        }),
        {},
      );
    },
  };
}

export const connectionActionClient = createConnectionActionClient({
  isNative: isTauri,
  createRequestId: () => crypto.randomUUID(),
  invoke,
});
