import { invoke, isTauri } from "@tauri-apps/api/core";

export type UpdateChannel = "stable" | "preview";
export type UpdateAction = { kind: "snapshot" }
  | { kind: "check"; channel: UpdateChannel }
  | { kind: "download"; candidate_id: string }
  | { kind: "apply"; candidate_id: string }
  | { kind: "rollback"; recovery_id: string };

const statuses = ["unavailable", "idle", "current", "available", "waiting_for_engine", "ready", "exiting"] as const;
const failureCodes = ["validation_failed", "busy", "unavailable", "source_failed", "invalid_release",
  "candidate_changed", "storage_failed", "download_failed", "integrity_failed", "preparation_failed", "engine_running"] as const;

type UpdateStatus = (typeof statuses)[number];
export type UpdateFailure = (typeof failureCodes)[number] | "invalid_response" | "request_failed";

interface Candidate {
  id: string;
  version: string;
  notes: string;
  size_bytes: number;
}

export interface UpdateSnapshot {
  installed_version: string;
  status: UpdateStatus;
  channel: UpdateChannel | null;
  candidate: Candidate | null;
  can_apply: boolean;
  recovery: { id: string; version: string; can_restore: boolean } | null;
}

export type UpdateResult = { kind: "snapshot"; snapshot: UpdateSnapshot } | { kind: "error"; code: UpdateFailure };

const uuid = (value: unknown): value is string => typeof value === "string"
  && /^[0-9a-f]{8}-[0-9a-f]{4}-[1-8][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/.test(value);
const version = (value: unknown): value is string => typeof value === "string" && value.length <= 64
  && /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?$/.test(value);
const channel = (value: unknown): value is UpdateChannel => value === "stable" || value === "preview";

function fields(value: unknown, keys: readonly string[]): value is Record<string, unknown> {
  if (typeof value !== "object" || value === null || Array.isArray(value)) return false;
  return Object.keys(value).length === keys.length && keys.every(key => Object.hasOwn(value, key));
}

export function decodeUpdateReply(value: unknown, id: string): UpdateResult {
  const invalid = { kind: "error", code: "invalid_response" } as const;
  if (!uuid(id) || !fields(value, ["schema_version", "request_id", "installed_version", "status", "channel", "candidate", "error", "can_apply", "recovery"])
    || value.schema_version !== 1 || value.request_id !== id || !version(value.installed_version)) return invalid;
  if (typeof value.can_apply !== "boolean" || (value.can_apply && value.status !== "ready")) return invalid;
  const rawRecovery = value.recovery;
  let recovery: UpdateSnapshot["recovery"] = null;
  if (rawRecovery !== null) {
    if (!fields(rawRecovery, ["id", "version", "can_restore"]) || !uuid(rawRecovery.id)
      || !version(rawRecovery.version) || typeof rawRecovery.can_restore !== "boolean") return invalid;
    recovery = { id: rawRecovery.id, version: rawRecovery.version, can_restore: rawRecovery.can_restore };
  }
  const capabilities = { can_apply: value.can_apply, recovery };
  if (value.status === "error") {
    return value.channel === null && value.candidate === null && recovery === null && failureCodes.includes(value.error as (typeof failureCodes)[number])
      ? { kind: "error", code: value.error as (typeof failureCodes)[number] } : invalid;
  }
  if (value.error !== null || !statuses.includes(value.status as UpdateStatus)) return invalid;
  const status = value.status as UpdateStatus;
  if (status === "unavailable" || status === "idle" || status === "exiting") {
    if (value.channel !== null || value.candidate !== null) return invalid;
    return { kind: "snapshot", snapshot: { installed_version: value.installed_version, status, channel: null, candidate: null, ...capabilities } };
  }
  if (!channel(value.channel)) return invalid;
  if (status === "current") {
    if (value.candidate !== null) return invalid;
    return { kind: "snapshot", snapshot: { installed_version: value.installed_version, status, channel: value.channel, candidate: null, ...capabilities } };
  }
  const candidate = value.candidate;
  if (!fields(candidate, ["id", "version", "notes", "size_bytes"]) || !uuid(candidate.id) || !version(candidate.version)
    || typeof candidate.notes !== "string" || new TextEncoder().encode(candidate.notes).length > 4096
    || typeof candidate.size_bytes !== "number" || !Number.isSafeInteger(candidate.size_bytes)
    || candidate.size_bytes < 1 || candidate.size_bytes > 1024 ** 3) return invalid;
  return { kind: "snapshot", snapshot: { installed_version: value.installed_version, status, channel: value.channel,
    candidate: { id: candidate.id, version: candidate.version, notes: candidate.notes, size_bytes: candidate.size_bytes }, ...capabilities } };
}

export function createUpdateClient(options: {
  isNative: () => boolean;
  requestId: () => string | null;
  invoke: (command: string, args: Record<string, unknown>) => Promise<unknown>;
}) {
  async function request(action: UpdateAction): Promise<UpdateResult> {
    if (!options.isNative()) return { kind: "error", code: "unavailable" };
    const id = options.requestId();
    if (!uuid(id)) return { kind: "error", code: "request_failed" };
    try {
      const result = decodeUpdateReply(await options.invoke("manage_updates", { request: { request_id: id, action } }), id);
      if (result.kind === "snapshot" && (
        (action.kind === "check" && result.snapshot.status !== "unavailable" && result.snapshot.channel !== action.channel)
        || (action.kind === "download" && (result.snapshot.candidate?.id !== action.candidate_id
          || !["ready", "waiting_for_engine"].includes(result.snapshot.status)))
        || (["apply", "rollback"].includes(action.kind) && result.snapshot.status !== "exiting")
      )) return { kind: "error", code: "invalid_response" };
      return result;
    } catch {
      return { kind: "error", code: "request_failed" };
    }
  }
  return { request };
}

export const updateClient = createUpdateClient({
  isNative: isTauri,
  requestId: () => globalThis.crypto?.randomUUID?.() ?? null,
  invoke: (command, args) => invoke(command, args),
});
