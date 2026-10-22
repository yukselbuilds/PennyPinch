import { invoke, isTauri } from "@tauri-apps/api/core";

export type EngineStatus = "unavailable" | "unknown" | "stopped" | "starting" | "ready" | "stopping" | "failed";
export type CompressionStatus = "unavailable" | "not_implemented";
export type DiscoveryStatus = "unavailable" | "not_scanned";

export const PUBLIC_ERROR_MESSAGES = {
  validation_failed: { retryable: false, messageKey: "runtime.command.validation_failed" },
  unsupported_protocol: { retryable: false, messageKey: "runtime.command.unsupported_protocol" },
  environment_blocked: { retryable: false, messageKey: "runtime.command.environment_blocked" },
  busy: { retryable: true, messageKey: "runtime.command.busy" },
  port_conflict: { retryable: true, messageKey: "runtime.command.port_conflict" },
  engine_not_ready: { retryable: true, messageKey: "runtime.command.engine_not_ready" },
  ownership_lost: { retryable: false, messageKey: "runtime.command.ownership_lost" },
  timeout: { retryable: true, messageKey: "runtime.command.timeout" },
  cleanup_failed: { retryable: true, messageKey: "runtime.command.cleanup_failed" },
  config_conflict: { retryable: false, messageKey: "runtime.command.config_conflict" },
} as const;

export type PublicErrorCode = keyof typeof PUBLIC_ERROR_MESSAGES;

export interface RuntimeSnapshot {
  schema_version: 1;
  request_id: string;
  snapshot_id: string;
  observed_at_utc: string;
  runtime_generation: string | null;
  engine_version: "0.41.0" | null;
  engine: { status: EngineStatus; error_code: PublicErrorCode | null };
  compression: { status: CompressionStatus; active_stream_count: null };
  discovery: { status: DiscoveryStatus; clients: [] };
  metrics: {
    status: "waiting";
    before_total: null;
    after_total: null;
    saved_total: null;
    saved_percent: null;
    request_count: null;
    covered_count: null;
  };
  capabilities: { start: boolean; stop: boolean; pause: false; connect: false };
}

export interface SnapshotErrorEnvelope {
  schema_version: 1;
  request_id: string;
  status: "error";
  error: {
    code: PublicErrorCode;
    retryable: boolean;
    message_key: (typeof PUBLIC_ERROR_MESSAGES)[PublicErrorCode]["messageKey"];
    diagnostic_id: null;
  };
}

export type SnapshotResult =
  | { kind: "desktop_unavailable" }
  | { kind: "snapshot"; snapshot: RuntimeSnapshot }
  | { kind: "snapshot_error"; error: SnapshotErrorEnvelope }
  | { kind: "invalid_response" }
  | { kind: "request_failed" };

type UnknownRecord = Record<string, unknown>;

function isRecord(value: unknown): value is UnknownRecord {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function isOpaqueId(value: unknown): value is string {
  return typeof value === "string" && /^[0-9a-f]{8}-[0-9a-f]{4}-[1-8][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i.test(value);
}

function isGeneration(value: unknown): value is string {
  return typeof value === "string" && /^[0-9a-f]{32}$/i.test(value);
}

function isPublicErrorCode(value: unknown): value is PublicErrorCode {
  return typeof value === "string" && Object.hasOwn(PUBLIC_ERROR_MESSAGES, value);
}

function isEngineStatus(value: unknown): value is EngineStatus {
  return value === "unavailable" || value === "unknown" || value === "stopped" ||
    value === "starting" || value === "ready" || value === "stopping" || value === "failed";
}

function isUtcTimestamp(value: unknown): value is string {
  return typeof value === "string" && /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?Z$/.test(value);
}

function hasExactKeys(value: UnknownRecord, expected: readonly string[]): boolean {
  const actual = Object.keys(value).sort();
  const required = [...expected].sort();
  return actual.length === required.length && actual.every((key, index) => key === required[index]);
}

export function decodeSnapshot(value: unknown, expectedRequestId: string): RuntimeSnapshot | null {
  if (!isRecord(value) || !isRecord(value.engine) || !isRecord(value.compression) ||
      !isRecord(value.discovery) || !isRecord(value.metrics) || !isRecord(value.capabilities)) return null;

  if (!hasExactKeys(value, ["schema_version", "request_id", "snapshot_id", "observed_at_utc", "runtime_generation", "engine_version", "engine", "compression", "discovery", "metrics", "capabilities"]) ||
      !hasExactKeys(value.engine, ["status", "error_code"]) ||
      !hasExactKeys(value.compression, ["status", "active_stream_count"]) ||
      !hasExactKeys(value.discovery, ["status", "clients"]) ||
      !hasExactKeys(value.metrics, ["status", "before_total", "after_total", "saved_total", "saved_percent", "request_count", "covered_count"]) ||
      !hasExactKeys(value.capabilities, ["start", "stop", "pause", "connect"])) return null;

  if (value.schema_version !== 1 || value.request_id !== expectedRequestId || !isOpaqueId(value.request_id) ||
      !isOpaqueId(value.snapshot_id) || !isUtcTimestamp(value.observed_at_utc)) return null;

  const generation = value.runtime_generation;
  const engineVersion = value.engine_version;
  const engineStatus = value.engine.status;
  const engineError = value.engine.error_code;
  if ((generation !== null && !isGeneration(generation)) ||
      (engineVersion !== null && engineVersion !== "0.41.0") ||
      (engineError !== null && !isPublicErrorCode(engineError)) || !isEngineStatus(engineStatus)) return null;
  const hasGeneration = isGeneration(generation);
  const hasEngineVersion = engineVersion === "0.41.0";
  if (hasGeneration !== hasEngineVersion || value.capabilities.stop !== hasGeneration) return null;
  if ((engineStatus === "stopped" || engineStatus === "unavailable" || engineStatus === "starting") && hasGeneration) return null;
  if ((engineStatus === "ready" || engineStatus === "stopping") && !hasGeneration) return null;
  if (engineStatus === "ready" && (engineError !== null || value.capabilities.start !== false)) return null;
  if ((engineStatus === "stopped" && value.capabilities.start !== true) ||
      (engineStatus !== "stopped" && engineStatus !== "failed" && value.capabilities.start !== false)) return null;

  if (value.compression.status !== "not_implemented" || value.compression.active_stream_count !== null ||
      value.discovery.status !== "not_scanned" || !Array.isArray(value.discovery.clients) || value.discovery.clients.length !== 0) return null;
  if (value.metrics.status !== "waiting" || value.metrics.before_total !== null || value.metrics.after_total !== null ||
      value.metrics.saved_total !== null || value.metrics.saved_percent !== null || value.metrics.request_count !== null ||
      value.metrics.covered_count !== null) return null;
  if (typeof value.capabilities.start !== "boolean" || typeof value.capabilities.stop !== "boolean" ||
      value.capabilities.pause !== false || value.capabilities.connect !== false) return null;

  return {
    schema_version: 1,
    request_id: value.request_id,
    snapshot_id: value.snapshot_id,
    observed_at_utc: value.observed_at_utc,
    runtime_generation: generation,
    engine_version: engineVersion,
    engine: { status: engineStatus, error_code: engineError },
    compression: { status: "not_implemented", active_stream_count: null },
    discovery: { status: "not_scanned", clients: [] },
    metrics: {
      status: "waiting",
      before_total: null,
      after_total: null,
      saved_total: null,
      saved_percent: null,
      request_count: null,
      covered_count: null,
    },
    capabilities: {
      start: value.capabilities.start,
      stop: value.capabilities.stop,
      pause: false,
      connect: false,
    },
  };
}

export function decodeSnapshotError(value: unknown, expectedRequestId: string): SnapshotErrorEnvelope | null {
  if (!isRecord(value) || !isRecord(value.error) ||
      !hasExactKeys(value, ["schema_version", "request_id", "status", "error"]) ||
      !hasExactKeys(value.error, ["code", "retryable", "message_key", "diagnostic_id"])) return null;

  const code = value.error.code;
  if (!isPublicErrorCode(code)) return null;
  const contract = PUBLIC_ERROR_MESSAGES[code];
  if (value.schema_version !== 1 || value.status !== "error" ||
      value.request_id !== expectedRequestId || !isOpaqueId(value.request_id) ||
      value.error.retryable !== contract.retryable || value.error.message_key !== contract.messageKey ||
      value.error.diagnostic_id !== null) return null;
  return {
    schema_version: 1,
    request_id: value.request_id,
    status: "error",
    error: {
      code,
      retryable: contract.retryable,
      message_key: contract.messageKey,
      diagnostic_id: null,
    },
  };
}

export class LatestRequestSequence {
  #value = 0;

  next(): number {
    this.#value += 1;
    return this.#value;
  }

  isCurrent(value: number): boolean {
    return value === this.#value;
  }

  invalidate(): void {
    this.#value += 1;
  }
}

function newRequestId(): string | null {
  const requestId = globalThis.crypto?.randomUUID?.();
  return requestId && isOpaqueId(requestId) ? requestId : null;
}

async function invokeRuntimeCommand(command: "get_runtime_snapshot" | "start_engine" | "stop_engine", expectedGeneration?: string): Promise<SnapshotResult> {
  if (!isTauri()) return { kind: "desktop_unavailable" };
  if (expectedGeneration !== undefined && !isGeneration(expectedGeneration)) return { kind: "invalid_response" };
  const requestId = newRequestId();
  if (!requestId) return { kind: "request_failed" };
  try {
    const raw: unknown = await invoke(command, command === "stop_engine"
      ? { requestId, expectedGeneration }
      : { requestId });
    const snapshot = decodeSnapshot(raw, requestId);
    if (snapshot) return { kind: "snapshot", snapshot };
    const error = decodeSnapshotError(raw, requestId);
    return error ? { kind: "snapshot_error", error } : { kind: "invalid_response" };
  } catch {
    return { kind: "request_failed" };
  }
}

export function readRuntimeSnapshot(): Promise<SnapshotResult> {
  return invokeRuntimeCommand("get_runtime_snapshot");
}

export function startEngine(): Promise<SnapshotResult> {
  return invokeRuntimeCommand("start_engine");
}

export function stopEngine(expectedGeneration: string): Promise<SnapshotResult> {
  return invokeRuntimeCommand("stop_engine", expectedGeneration);
}
