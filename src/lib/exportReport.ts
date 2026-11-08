import { invoke, isTauri } from "@tauri-apps/api/core";
import type { PreferenceLanguage } from "./preferences";

export type ExportKind = "statistics_json" | "statistics_csv" | "diagnostics_json";
export type ExportErrorCode = "invalid_input" | "snapshot_expired" | "busy" | "unavailable" | "unsafe_path" | "file_exists" | "write_failed";

export type ExportResult =
  | { kind: "saved"; bytes_written: number }
  | { kind: "cancelled" }
  | { kind: "export_error"; code: ExportErrorCode; retryable: boolean }
  | { kind: "client_busy" }
  | { kind: "desktop_unavailable" }
  | { kind: "invalid_response" }
  | { kind: "request_failed" };

export interface ExportTransport {
  isNative: () => boolean;
  createRequestId: () => string | null;
  invoke: (command: string, args: Record<string, unknown>) => Promise<unknown>;
}

const errorRetryability = {
  invalid_input: false,
  snapshot_expired: true,
  busy: true,
  unavailable: false,
  unsafe_path: false,
  file_exists: true,
  write_failed: true,
} as const;

const kinds: readonly ExportKind[] = ["statistics_json", "statistics_csv", "diagnostics_json"];
const languages: readonly PreferenceLanguage[] = ["tr", "en"];
const maxReportBytes = 262_144;
type UnknownRecord = Record<string, unknown>;

function isRecord(value: unknown): value is UnknownRecord {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function hasExactKeys(value: UnknownRecord, expected: readonly string[]): boolean {
  const actual = Object.keys(value).sort();
  const required = [...expected].sort();
  return actual.length === required.length && actual.every((key, index) => key === required[index]);
}

function isUuid(value: unknown): value is string {
  return typeof value === "string" &&
    /^[0-9a-f]{8}-[0-9a-f]{4}-[1-8][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/.test(value);
}

function decodeReply(value: unknown, requestId: string, kind: ExportKind): ExportResult {
  if (!isRecord(value) || value.schema_version !== 1 || value.request_id !== requestId) return { kind: "invalid_response" };

  if (value.status === "saved" || value.status === "cancelled") {
    if (!hasExactKeys(value, ["schema_version", "request_id", "status", "kind", "bytes_written"]) || value.kind !== kind
      || typeof value.bytes_written !== "number" || !Number.isSafeInteger(value.bytes_written)
      || value.bytes_written < 0 || value.bytes_written > maxReportBytes) return { kind: "invalid_response" };
    if (value.status === "cancelled") return value.bytes_written === 0 ? { kind: "cancelled" } : { kind: "invalid_response" };
    return value.bytes_written > 0 ? { kind: "saved", bytes_written: value.bytes_written } : { kind: "invalid_response" };
  }

  if (value.status !== "error" || !hasExactKeys(value, ["schema_version", "request_id", "status", "error"])
    || !isRecord(value.error) || !hasExactKeys(value.error, ["code", "retryable", "message_key", "diagnostic_id"])) {
    return { kind: "invalid_response" };
  }
  const code = value.error.code;
  if (typeof code !== "string" || !Object.hasOwn(errorRetryability, code)) return { kind: "invalid_response" };
  const retryable = errorRetryability[code as ExportErrorCode];
  if (value.error.retryable !== retryable || value.error.message_key !== `export.${code}` || value.error.diagnostic_id !== null) {
    return { kind: "invalid_response" };
  }
  return { kind: "export_error", code: code as ExportErrorCode, retryable };
}

export function createExportClient(transport: ExportTransport) {
  let inFlight = false;

  async function exportReport(kind: ExportKind, language: PreferenceLanguage, snapshotRequestId: string | null): Promise<ExportResult> {
    if (inFlight) return { kind: "client_busy" };
    if (!kinds.includes(kind) || !languages.includes(language)
      || (kind !== "diagnostics_json" && !isUuid(snapshotRequestId))
      || (snapshotRequestId !== null && !isUuid(snapshotRequestId))) return { kind: "invalid_response" };

    let native: boolean;
    try { native = transport.isNative(); } catch { return { kind: "request_failed" }; }
    if (!native) return { kind: "desktop_unavailable" };

    inFlight = true;
    try {
      const requestId = transport.createRequestId();
      if (!isUuid(requestId)) return { kind: "request_failed" };
      const value = await transport.invoke("export_report", { request: {
        request_id: requestId,
        kind,
        language,
        snapshot_request_id: snapshotRequestId,
      } });
      return decodeReply(value, requestId, kind);
    } catch {
      return { kind: "request_failed" };
    } finally {
      inFlight = false;
    }
  }

  return {
    exportReport,
    isAvailable: () => {
      try { return transport.isNative(); } catch { return false; }
    },
  };
}

export const exportClient = createExportClient({
  isNative: () => isTauri(),
  createRequestId: () => globalThis.crypto?.randomUUID?.() ?? null,
  invoke: (command, args) => invoke(command, args),
});
