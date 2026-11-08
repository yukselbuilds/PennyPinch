import { invoke, isTauri } from "@tauri-apps/api/core";
import { PUBLIC_ERROR_MESSAGES, type PublicErrorCode } from "../runtime/snapshot.ts";

export type RecoveryEngineStatus = "unavailable" | "stopped" | "starting" | "ready" | "draining" | "failed" | "unknown";
export type RecoveryIngressStatus = "unavailable" | "starting" | "ready" | "unknown";
export type RecoveryTrafficStatus = "observed" | "none" | "unavailable";
export type RecoveryTrafficOutcome = "unknown" | "no_error_reported" | "error_reported";
export type RecoveryClientKind = "codex" | "claude_code" | "opencode";
export type RecoveryInstallationStatus = "current" | "stale" | "not_found" | "unknown";
export type RecoveryCompatibility = "config_supported" | "not_verified";
export type RecoveryConfigState = "matches_owned_endpoint" | "conflict" | "not_owned" | "unavailable" | "busy" | "not_rechecked";
export type RecoveryConfigScope = "user_config" | "project_local_config" | "unknown";
export type RecoveryEndpointBinding = "same" | "different" | "not_owned" | "unknown";
export type RecoveryErrorCode = "invalid_input" | "busy" | "unavailable";

export interface RecoverySnapshot {
  schema_version: 1;
  request_id: string;
  status: "snapshot";
  observed_at_utc: string;
  runtime: {
    engine_status: RecoveryEngineStatus;
    ingress_status: RecoveryIngressStatus;
    error_code: PublicErrorCode | null;
  };
  traffic: {
    status: RecoveryTrafficStatus;
    period: "seven_days";
    last_observed_utc: string | null;
    outcome: RecoveryTrafficOutcome;
    error_layer: "unknown";
  };
  clients: [RecoveryClientSnapshot, RecoveryClientSnapshot, RecoveryClientSnapshot];
}

export interface RecoveryClientSnapshot {
  client_kind: RecoveryClientKind;
  detected_count: number;
  installation_status: RecoveryInstallationStatus;
  compatibility: RecoveryCompatibility;
  config_state: RecoveryConfigState;
  config_scope: RecoveryConfigScope;
  endpoint_binding: RecoveryEndpointBinding;
}

export type RecoveryResult =
  | { kind: "desktop_unavailable" }
  | { kind: "client_busy" }
  | { kind: "invalid_response" }
  | { kind: "request_failed" }
  | { kind: "recovery_error"; code: RecoveryErrorCode; retryable: boolean }
  | { kind: "snapshot"; snapshot: RecoverySnapshot };

export interface RecoveryTransport {
  isAvailable: () => boolean;
  requestId: () => string | null;
  invoke: (command: string, args: Record<string, unknown>) => Promise<unknown>;
}

type UnknownRecord = Record<string, unknown>;

const uuidPattern = /^[0-9a-f]{8}-[0-9a-f]{4}-[1-8][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;
const snapshotKeys = ["schema_version", "request_id", "status", "observed_at_utc", "runtime", "traffic", "clients"] as const;
const runtimeKeys = ["engine_status", "ingress_status", "error_code"] as const;
const trafficKeys = ["status", "period", "last_observed_utc", "outcome", "error_layer"] as const;
const clientKeys = ["client_kind", "detected_count", "installation_status", "compatibility", "config_state", "config_scope", "endpoint_binding"] as const;
const errorKeys = ["schema_version", "request_id", "status", "error"] as const;
const nestedErrorKeys = ["code", "retryable", "message_key", "diagnostic_id"] as const;
const recoveryErrorRules = {
  invalid_input: { retryable: false },
  busy: { retryable: true },
  unavailable: { retryable: false },
} as const;
const clientKinds = ["codex", "claude_code", "opencode"] as const;
const engineStatuses = ["unavailable", "stopped", "starting", "ready", "draining", "failed", "unknown"] as const;
const ingressStatuses = ["unavailable", "starting", "ready", "unknown"] as const;
const installationStatuses = ["current", "stale", "not_found", "unknown"] as const;
const configStates = ["matches_owned_endpoint", "conflict", "not_owned", "unavailable", "busy", "not_rechecked"] as const;

function isRecord(value: unknown): value is UnknownRecord {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function hasExactKeys(value: UnknownRecord, expected: readonly string[]): boolean {
  const actual = Object.keys(value).sort();
  const required = [...expected].sort();
  return actual.length === required.length && actual.every((key, index) => key === required[index]);
}

function isUuid(value: unknown): value is string {
  return typeof value === "string" && uuidPattern.test(value);
}

function isUtcTimestamp(value: unknown): value is string {
  if (typeof value !== "string" || !/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d{1,9})?Z$/.test(value)) return false;
  const millis = Date.parse(value);
  return Number.isFinite(millis) && new Date(millis).toISOString().slice(0, 19) === value.slice(0, 19);
}

function isOneOf<T extends readonly string[]>(value: unknown, choices: T): value is T[number] {
  return typeof value === "string" && choices.includes(value);
}

export function decodeRecoverySnapshot(value: unknown, expectedRequestId: string): RecoverySnapshot | null {
  if (!isUuid(expectedRequestId) || !isRecord(value) || !hasExactKeys(value, snapshotKeys)
    || value.schema_version !== 1 || value.request_id !== expectedRequestId || value.status !== "snapshot"
    || !isUtcTimestamp(value.observed_at_utc) || !isRecord(value.runtime) || !hasExactKeys(value.runtime, runtimeKeys)
    || !isOneOf(value.runtime.engine_status, engineStatuses) || !isOneOf(value.runtime.ingress_status, ingressStatuses)
    || !(value.runtime.error_code === null || (typeof value.runtime.error_code === "string" && Object.hasOwn(PUBLIC_ERROR_MESSAGES, value.runtime.error_code)))
    || !isRecord(value.traffic) || !hasExactKeys(value.traffic, trafficKeys)
    || !isOneOf(value.traffic.status, ["observed", "none", "unavailable"] as const)
    || value.traffic.period !== "seven_days" || !isOneOf(value.traffic.outcome, ["unknown", "no_error_reported", "error_reported"] as const)
    || value.traffic.error_layer !== "unknown"
    || !(value.traffic.last_observed_utc === null || isUtcTimestamp(value.traffic.last_observed_utc))) return null;

  if (value.traffic.status === "observed") {
    if (value.traffic.last_observed_utc === null) return null;
  } else if (value.traffic.last_observed_utc !== null || value.traffic.outcome !== "unknown") return null;

  if (!Array.isArray(value.clients) || value.clients.length !== 3) return null;
  const clients: RecoveryClientSnapshot[] = [];
  for (let index = 0; index < clientKinds.length; index++) {
    const row = value.clients[index];
    if (!isRecord(row) || !hasExactKeys(row, clientKeys) || row.client_kind !== clientKinds[index]
      || typeof row.detected_count !== "number" || !Number.isSafeInteger(row.detected_count) || row.detected_count < 0 || row.detected_count > 512
      || !isOneOf(row.installation_status, installationStatuses)
      || !isOneOf(row.compatibility, ["config_supported", "not_verified"] as const)
      || !isOneOf(row.config_state, configStates)
      || !isOneOf(row.config_scope, ["user_config", "project_local_config", "unknown"] as const)
      || !isOneOf(row.endpoint_binding, ["same", "different", "not_owned", "unknown"] as const)) return null;
    if ((row.installation_status === "not_found" && row.detected_count !== 0)
      || (row.installation_status === "current" && row.detected_count === 0)
      || (row.installation_status === "stale" && row.detected_count === 0)
      || (row.compatibility === "config_supported" && (row.installation_status !== "current" || row.detected_count === 0))
      || (row.config_state === "matches_owned_endpoint" && (row.config_scope === "unknown" || row.compatibility !== "config_supported"))
      || (row.config_state === "not_owned" && row.endpoint_binding !== "not_owned" && row.endpoint_binding !== "unknown")) return null;
    clients.push({
      client_kind: clientKinds[index],
      detected_count: row.detected_count,
      installation_status: row.installation_status,
      compatibility: row.compatibility,
      config_state: row.config_state,
      config_scope: row.config_scope,
      endpoint_binding: row.endpoint_binding,
    });
  }

  return {
    schema_version: 1,
    request_id: expectedRequestId,
    status: "snapshot",
    observed_at_utc: value.observed_at_utc,
    runtime: {
      engine_status: value.runtime.engine_status,
      ingress_status: value.runtime.ingress_status,
      error_code: value.runtime.error_code as PublicErrorCode | null,
    },
    traffic: {
      status: value.traffic.status,
      period: "seven_days",
      last_observed_utc: value.traffic.last_observed_utc,
      outcome: value.traffic.outcome,
      error_layer: "unknown",
    },
    clients: clients as [RecoveryClientSnapshot, RecoveryClientSnapshot, RecoveryClientSnapshot],
  };
}

function decodeRecoveryError(value: unknown, expectedRequestId: string): { code: RecoveryErrorCode; retryable: boolean } | null {
  if (!isUuid(expectedRequestId) || !isRecord(value) || !hasExactKeys(value, errorKeys)
    || value.schema_version !== 1 || value.request_id !== expectedRequestId || value.status !== "error"
    || !isRecord(value.error) || !hasExactKeys(value.error, nestedErrorKeys)) return null;
  const code = value.error.code;
  if (typeof code !== "string" || !Object.hasOwn(recoveryErrorRules, code)) return null;
  const expectedRetryable = recoveryErrorRules[code as RecoveryErrorCode].retryable;
  if (value.error.retryable !== expectedRetryable || value.error.message_key !== `recovery.${code}` || value.error.diagnostic_id !== null) return null;
  return { code: code as RecoveryErrorCode, retryable: expectedRetryable };
}

export function createRecoveryClient(transport: RecoveryTransport) {
  let pending = false;

  async function getSnapshot(): Promise<RecoveryResult> {
    if (pending) return { kind: "client_busy" };
    let available: boolean;
    try { available = transport.isAvailable(); } catch { return { kind: "request_failed" }; }
    if (!available) return { kind: "desktop_unavailable" };

    let requestId: string | null;
    try { requestId = transport.requestId(); } catch { return { kind: "request_failed" }; }
    if (!isUuid(requestId)) return { kind: "request_failed" };
    pending = true;
    try {
      const value = await transport.invoke("get_recovery_snapshot", { request: { schema_version: 1, request_id: requestId } });
      const snapshot = decodeRecoverySnapshot(value, requestId);
      if (snapshot) return { kind: "snapshot", snapshot };
      const error = decodeRecoveryError(value, requestId);
      return error ? { kind: "recovery_error", ...error } : { kind: "invalid_response" };
    } catch {
      return { kind: "request_failed" };
    } finally {
      pending = false;
    }
  }

  return { getSnapshot };
}

export const recoveryClient = createRecoveryClient({
  isAvailable: () => isTauri(),
  requestId: () => globalThis.crypto?.randomUUID?.() ?? null,
  invoke: (command, args) => invoke(command, args),
});
