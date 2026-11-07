import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export type DesktopExitState = "idle" | "confirmation_required" | "shutting_down";
export type DesktopErrorCode = "validation_failed" | "state_conflict" | "busy";

export interface DesktopSnapshot {
  schema_version: 1;
  request_id: string;
  status: "snapshot";
  tray_available: boolean;
  exit_state: DesktopExitState;
  exit_token: string | null;
}

export interface DesktopErrorEnvelope {
  schema_version: 1;
  request_id: string;
  status: "error";
  error: {
    code: DesktopErrorCode;
    retryable: boolean;
    message_key: `desktop.${DesktopErrorCode}`;
    diagnostic_id: null;
  };
}

export type DesktopResult =
  | { kind: "desktop_unavailable" }
  | { kind: "client_busy" }
  | { kind: "invalid_request" }
  | { kind: "invalid_response" }
  | { kind: "request_failed" }
  | { kind: "desktop_error"; error: DesktopErrorEnvelope }
  | { kind: "snapshot"; snapshot: DesktopSnapshot };

type UnknownRecord = Record<string, unknown>;
type InvokeFunction = (command: string, args: Record<string, unknown>) => Promise<unknown>;
type ClientOptions = {
  isNative: () => boolean;
  invoke: InvokeFunction;
  createRequestId: () => string | null;
};

const uuidPattern = /^[0-9a-f]{8}-[0-9a-f]{4}-[1-8][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;
const snapshotKeys = ["schema_version", "request_id", "status", "tray_available", "exit_state", "exit_token"] as const;
const errorKeys = ["schema_version", "request_id", "status", "error"] as const;
const nestedErrorKeys = ["code", "retryable", "message_key", "diagnostic_id"] as const;
const errorRules = {
  validation_failed: { retryable: false },
  state_conflict: { retryable: true },
  busy: { retryable: true },
} as const;

function isRecord(value: unknown): value is UnknownRecord {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function hasExactKeys(value: UnknownRecord, expected: readonly string[]): boolean {
  const actual = Object.keys(value).sort();
  const required = [...expected].sort();
  return actual.length === required.length && actual.every((key, index) => key === required[index]);
}

function isCanonicalUuid(value: unknown): value is string {
  return typeof value === "string" && uuidPattern.test(value);
}

export function decodeDesktopSnapshot(value: unknown, requestId: string): DesktopSnapshot | null {
  if (!isCanonicalUuid(requestId) || !isRecord(value) || !hasExactKeys(value, snapshotKeys)
    || value.schema_version !== 1 || value.request_id !== requestId || value.status !== "snapshot"
    || typeof value.tray_available !== "boolean"
    || (value.exit_state !== "idle" && value.exit_state !== "confirmation_required" && value.exit_state !== "shutting_down")) return null;
  if (value.exit_state === "confirmation_required") {
    if (!isCanonicalUuid(value.exit_token)) return null;
  } else if (value.exit_token !== null) return null;
  return {
    schema_version: 1,
    request_id: requestId,
    status: "snapshot",
    tray_available: value.tray_available,
    exit_state: value.exit_state,
    exit_token: value.exit_token as string | null,
  };
}

export function decodeDesktopError(value: unknown, requestId: string): DesktopErrorEnvelope | null {
  if (!isCanonicalUuid(requestId) || !isRecord(value) || !hasExactKeys(value, errorKeys)
    || value.schema_version !== 1 || value.request_id !== requestId || value.status !== "error"
    || !isRecord(value.error) || !hasExactKeys(value.error, nestedErrorKeys)) return null;
  const code = value.error.code;
  if (typeof code !== "string" || !Object.hasOwn(errorRules, code)) return null;
  const validCode = code as DesktopErrorCode;
  const expected = errorRules[validCode];
  if (value.error.retryable !== expected.retryable || value.error.message_key !== `desktop.${code}`
    || value.error.diagnostic_id !== null) return null;
  return {
    schema_version: 1,
    request_id: requestId,
    status: "error",
    error: {
      code: validCode,
      retryable: expected.retryable,
      message_key: `desktop.${validCode}`,
      diagnostic_id: null,
    },
  };
}

export function createDesktopClient(options: ClientOptions) {
  let mutationPending = false;

  async function request(
    command: string,
    expectedState: DesktopExitState | null,
    args: (requestId: string) => Record<string, unknown>,
    mutation: boolean,
  ): Promise<DesktopResult> {
    if (!options.isNative()) return { kind: "desktop_unavailable" };
    if (mutation && mutationPending) return { kind: "client_busy" };
    let requestId: string | null;
    try { requestId = options.createRequestId(); } catch { return { kind: "request_failed" }; }
    if (!isCanonicalUuid(requestId)) return { kind: "invalid_request" };
    if (mutation) mutationPending = true;
    try {
      const raw = await options.invoke(command, args(requestId));
      const snapshot = decodeDesktopSnapshot(raw, requestId);
      if (snapshot) return expectedState === null || snapshot.exit_state === expectedState
        ? { kind: "snapshot", snapshot }
        : { kind: "invalid_response" };
      const error = decodeDesktopError(raw, requestId);
      return error ? { kind: "desktop_error", error } : { kind: "invalid_response" };
    } catch {
      return { kind: "request_failed" };
    } finally {
      if (mutation) mutationPending = false;
    }
  }

  return {
    available: () => options.isNative(),
    getState: () => request("get_desktop_state", null, requestId => ({ requestId }), false),
    requestExit: () => request("request_desktop_exit", "confirmation_required", requestId => ({ requestId }), true),
    cancelExit: (exitToken: string) => isCanonicalUuid(exitToken)
      ? request("cancel_desktop_exit", "idle", requestId => ({ requestId, exitToken }), true)
      : Promise.resolve({ kind: "invalid_request" } as const),
    confirmExit: (exitToken: string) => isCanonicalUuid(exitToken)
      ? request("confirm_desktop_exit", "shutting_down", requestId => ({ requestId, exitToken }), true)
      : Promise.resolve({ kind: "invalid_request" } as const),
  };
}

export function listenForDesktopChanges(onChange: () => void): Promise<(() => void) | null> {
  if (!isTauri()) return Promise.resolve(null);
  return listen("headroom:desktop-change", () => onChange());
}

export const desktopClient = createDesktopClient({
  isNative: () => isTauri(),
  invoke: (command, args) => invoke(command, args),
  createRequestId: () => globalThis.crypto?.randomUUID?.() ?? null,
});
