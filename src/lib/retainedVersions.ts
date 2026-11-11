import { invoke, isTauri } from "@tauri-apps/api/core";

export type RetainedClientKind = "codex" | "claude_code" | "opencode_v1";
export type RetainedScope = "user_config" | "project_local_config";
export type RetainedBackendErrorCode =
  | "invalid_input"
  | "scope_changed"
  | "busy"
  | "conflict"
  | "unavailable"
  | "archive_limit"
  | "commit_unknown";
export type RetainedVersionsErrorCode = RetainedBackendErrorCode | "invalid_response";

export interface RetainedVersionsSnapshot {
  schema_version: 1;
  request_id: string;
  status: "snapshot" | "archived";
  client_kind: RetainedClientKind;
  scope: RetainedScope;
  supported: boolean;
  review_fingerprint: string | null;
  active_records: number;
  selected_records: number;
  unresolved_records: number;
  changed_versions: number;
  completed_archives: number;
  record_limit: 64;
  archive_limit: 64;
  can_archive: boolean;
}

export interface RetainedVersionsFailure {
  code: RetainedVersionsErrorCode;
  retryable: boolean;
  message_key: `retained.${RetainedVersionsErrorCode}`;
  diagnostic_id: null;
}

export type RetainedVersionsResult =
  | { kind: "snapshot"; data: RetainedVersionsSnapshot }
  | { kind: "error"; data: RetainedVersionsFailure };

type UnknownRecord = Record<string, unknown>;
type InvokeFunction = (command: string, args?: Record<string, unknown>) => Promise<unknown>;

const uuidPattern = /^[0-9a-f]{8}-[0-9a-f]{4}-[1-8][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;
const hashPattern = /^[0-9a-f]{64}$/;
const snapshotKeys = [
  "schema_version", "request_id", "status", "client_kind", "scope", "supported",
  "review_fingerprint", "active_records", "selected_records", "unresolved_records",
  "changed_versions", "completed_archives", "record_limit", "archive_limit", "can_archive",
] as const;
const envelopeKeys = ["schema_version", "request_id", "status", "error"] as const;
const errorKeys = ["code", "retryable", "message_key", "diagnostic_id"] as const;
const errorContracts: Record<RetainedBackendErrorCode, boolean> = {
  invalid_input: false,
  scope_changed: true,
  busy: true,
  conflict: true,
  unavailable: true,
  archive_limit: false,
  commit_unknown: true,
};
const clientKinds: readonly RetainedClientKind[] = ["codex", "claude_code", "opencode_v1"];

function isRecord(value: unknown): value is UnknownRecord {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function hasExactKeys(value: UnknownRecord, keys: readonly string[]): boolean {
  const actual = Object.keys(value);
  return actual.length === keys.length && keys.every((key) => Object.hasOwn(value, key));
}

function isUuid(value: unknown): value is string {
  return typeof value === "string" && uuidPattern.test(value);
}

function isHash(value: unknown): value is string {
  return typeof value === "string" && hashPattern.test(value);
}

function isClientKind(value: unknown): value is RetainedClientKind {
  return typeof value === "string" && clientKinds.includes(value as RetainedClientKind);
}

function isScope(value: unknown): value is RetainedScope {
  return value === "user_config" || value === "project_local_config";
}

function scopeAllowed(clientKind: RetainedClientKind, scope: RetainedScope): boolean {
  return scope === "user_config" || clientKind === "claude_code";
}

function count(value: unknown): value is number {
  return typeof value === "number" && Number.isInteger(value) && value >= 0 && value <= 64;
}

function failure(code: RetainedVersionsErrorCode): RetainedVersionsFailure {
  const retryable = code === "invalid_response" ? false : errorContracts[code];
  return {
    code,
    retryable,
    message_key: `retained.${code}`,
    diagnostic_id: null,
  };
}

function decodeSnapshot(
  value: unknown,
  expected: {
    requestId: string;
    clientKind: RetainedClientKind;
    scope: RetainedScope;
    status: "snapshot" | "archived";
  },
): RetainedVersionsSnapshot | null {
  if (!isRecord(value) || !hasExactKeys(value, snapshotKeys)
    || value.schema_version !== 1 || value.request_id !== expected.requestId
    || value.status !== expected.status || value.client_kind !== expected.clientKind
    || value.scope !== expected.scope || typeof value.supported !== "boolean"
    || !(value.review_fingerprint === null || isHash(value.review_fingerprint))
    || !count(value.active_records) || !count(value.selected_records)
    || !count(value.unresolved_records) || !count(value.changed_versions)
    || !count(value.completed_archives) || value.record_limit !== 64
    || value.archive_limit !== 64 || typeof value.can_archive !== "boolean") return null;

  const counts = [
    value.active_records, value.selected_records, value.unresolved_records,
    value.changed_versions, value.completed_archives,
  ] as const;
  if (!value.supported) {
    if (value.review_fingerprint !== null || value.can_archive || counts.some((item) => item !== 0)) return null;
  } else {
    if (!isHash(value.review_fingerprint)
      || value.selected_records > value.active_records
      || value.unresolved_records > value.selected_records
      || value.changed_versions > value.selected_records) return null;
    const canArchive = value.selected_records > 0
      && value.unresolved_records === 0
      && value.completed_archives < value.archive_limit;
    if (value.can_archive !== canArchive) return null;
  }
  if (expected.status === "archived" && !value.supported) return null;

  return {
    schema_version: 1,
    request_id: expected.requestId,
    status: expected.status,
    client_kind: expected.clientKind,
    scope: expected.scope,
    supported: value.supported,
    review_fingerprint: value.review_fingerprint,
    active_records: value.active_records,
    selected_records: value.selected_records,
    unresolved_records: value.unresolved_records,
    changed_versions: value.changed_versions,
    completed_archives: value.completed_archives,
    record_limit: 64,
    archive_limit: 64,
    can_archive: value.can_archive,
  };
}

function decodeError(value: unknown, requestId: string): RetainedVersionsFailure | null {
  if (!isRecord(value) || !hasExactKeys(value, envelopeKeys)
    || value.schema_version !== 1 || value.request_id !== requestId || value.status !== "error"
    || !isRecord(value.error) || !hasExactKeys(value.error, errorKeys)) return null;
  const code = value.error.code;
  if (typeof code !== "string" || !Object.hasOwn(errorContracts, code)) return null;
  const retryable = errorContracts[code as RetainedBackendErrorCode];
  if (value.error.retryable !== retryable
    || value.error.message_key !== `retained.${code}`
    || value.error.diagnostic_id !== null) return null;
  return failure(code as RetainedBackendErrorCode);
}

function decodeReply(
  value: unknown,
  expected: {
    requestId: string;
    clientKind: RetainedClientKind;
    scope: RetainedScope;
    status: "snapshot" | "archived";
  },
): RetainedVersionsResult {
  if (isRecord(value) && value.status === "error") {
    return { kind: "error", data: decodeError(value, expected.requestId) ?? failure("invalid_response") };
  }
  const snapshot = decodeSnapshot(value, expected);
  return snapshot ? { kind: "snapshot", data: snapshot } : { kind: "error", data: failure("invalid_response") };
}

function requestId(): string | null {
  try {
    const value = globalThis.crypto?.randomUUID?.();
    return isUuid(value) ? value : null;
  } catch {
    return null;
  }
}

export function createRetainedVersionsClient(options: {
  invoke?: InvokeFunction;
  isNative?: () => boolean;
  createRequestId?: () => string | null;
} = {}) {
  let pending = false;
  const call = options.invoke ?? ((command, args) => invoke(command, args));
  const native = options.isNative ?? (() => isTauri());
  const makeRequestId = options.createRequestId ?? requestId;

  async function send(
    operation: "read" | "archive",
    clientKind: RetainedClientKind,
    scope: RetainedScope,
    expectedFingerprint?: string,
  ): Promise<RetainedVersionsResult> {
    if (!isClientKind(clientKind) || !isScope(scope) || !scopeAllowed(clientKind, scope)
      || (operation === "archive" && !isHash(expectedFingerprint))) {
      return { kind: "error", data: failure("invalid_input") };
    }
    if (pending) return { kind: "error", data: failure("busy") };
    pending = true;
    try {
      if (!native()) return { kind: "error", data: failure("unavailable") };
      const id = makeRequestId();
      if (!isUuid(id)) return { kind: "error", data: failure("unavailable") };
      const request = {
        schema_version: 1,
        request_id: id,
        client_kind: clientKind,
        scope,
        ...(operation === "archive" ? { expected_review_fingerprint: expectedFingerprint } : {}),
      };
      const raw = await call(
        operation === "read" ? "get_retained_config_versions" : "archive_retained_config_versions",
        { request },
      );
      return decodeReply(raw, {
        requestId: id,
        clientKind,
        scope,
        status: operation === "read" ? "snapshot" : "archived",
      });
    } catch {
      return { kind: "error", data: failure("unavailable") };
    } finally {
      pending = false;
    }
  }

  return {
    read: (clientKind: RetainedClientKind, scope: RetainedScope) => send("read", clientKind, scope),
    archive: (
      clientKind: RetainedClientKind,
      scope: RetainedScope,
      expectedFingerprint: string,
    ) => send("archive", clientKind, scope, expectedFingerprint),
  };
}

export const retainedVersionsClient = createRetainedVersionsClient();
