import { invoke, isTauri } from "@tauri-apps/api/core";

export type SessionClient = "codex" | "claude_code" | "opencode";
export type SessionRow = {
  key: string; id_hint: string; client_kind: SessionClient;
  project: string | null; model: string | null; provider: string | null; reasoning: string | null;
  created_at: string | null; updated_at: string | null;
  state: "working" | "waiting" | "open" | "loaded" | "stored" | "ended" | "error" | "unknown";
  role: "main" | "subagent" | "unknown"; parent_key: string | null; traffic_assignment: "unverified";
};
export type SessionReason = "scope_unavailable" | "endpoint_unavailable" | "unsafe_scope" | "untrusted_endpoint"
  | "unsupported_version" | "timeout" | "read_failed" | "invalid_response" | "unsupported_metadata" | "unavailable";
export type SessionScope = {
  client_kind: SessionClient; state: "observed" | "unknown"; reason: SessionReason | null;
  server_version: string | null; loaded_count: number | null; partial: boolean;
};
export type SessionSnapshot = {
  snapshot_id: string; observed_at_utc: string; scope: "current_cli_environment";
  clients: SessionScope[]; rows: SessionRow[]; can_open_codex: boolean; traffic_assignment: "unverified";
};
export type SessionError = "desktop_unavailable" | "invalid_response" | "validation_failed" | "busy" | "unavailable"
  | "stale_snapshot" | "launch_failed" | "untrusted_endpoint" | "unsafe_scope";
export type SessionResult = { kind: "snapshot"; snapshot: SessionSnapshot } | { kind: "launched" }
  | { kind: "error"; code: SessionError };
type Request = { schema_version: 1; request_id: string; operation: "read" | "open_codex_center";
  snapshot_id: string | null; confirm_open: boolean };
type Invoke = (command: string, args: Record<string, unknown>) => Promise<unknown>;
const uuid = /^[0-9a-f]{8}-[0-9a-f]{4}-[1-8][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;
const hash = /^[0-9a-f]{64}$/;
const clients = ["codex", "claude_code", "opencode"] as const;
const reasons = ["scope_unavailable", "endpoint_unavailable", "unsafe_scope", "untrusted_endpoint", "unsupported_version", "timeout", "read_failed", "invalid_response", "unsupported_metadata", "unavailable"];
const errors = ["invalid_response", "validation_failed", "busy", "unavailable", "stale_snapshot", "launch_failed", "untrusted_endpoint", "unsafe_scope"];
function record(v: unknown): v is Record<string, unknown> { return typeof v === "object" && v !== null && !Array.isArray(v); }
function keys(v: Record<string, unknown>, expected: string[]): boolean { return Object.keys(v).length === expected.length && expected.every(k => Object.hasOwn(v, k)); }
function member<T>(items: readonly T[], v: unknown): v is T { return items.includes(v as T); }
function date(v: unknown): v is string { return typeof v === "string" && /^\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d(?:\.\d{1,9})?Z$/.test(v) && Number.isFinite(Date.parse(v)) && new Date(v).toISOString().slice(0, 19) === v.slice(0, 19); }
function label(v: unknown): boolean {
  return v === null || typeof v === "string" && v.length > 0 && v.length <= 128 && /^[A-Za-z0-9._\-/:+@]+$/.test(v)
    && !v.includes(":/") && !v.startsWith("/") && !/^(sk-|ghp_|github_pat_)/i.test(v) && v.toLowerCase() !== "unknown";
}
function row(v: unknown): v is SessionRow {
  return record(v) && keys(v, ["key", "id_hint", "client_kind", "project", "model", "provider", "reasoning", "created_at", "updated_at", "state", "role", "parent_key", "traffic_assignment"])
    && typeof v.key === "string" && hash.test(v.key) && v.id_hint === v.key.slice(0, 8) && member(clients, v.client_kind)
    && (v.project === null || typeof v.project === "string" && Array.from(v.project).length > 0 && Array.from(v.project).length <= 80 && !/[\u0000-\u001F\u007F-\u009F]/.test(v.project))
    && label(v.model) && label(v.provider) && label(v.reasoning)
    && (v.created_at === null || date(v.created_at)) && (v.updated_at === null || date(v.updated_at))
    && member(["working", "waiting", "open", "loaded", "stored", "ended", "error", "unknown"], v.state)
    && member(["main", "subagent", "unknown"], v.role)
    && (v.parent_key === null || typeof v.parent_key === "string" && hash.test(v.parent_key) && v.parent_key !== v.key)
    && !(v.role === "main" && v.parent_key !== null) && v.traffic_assignment === "unverified";
}
function scope(v: unknown): v is SessionScope {
  if (!record(v) || !keys(v, ["client_kind", "state", "reason", "server_version", "loaded_count", "partial"])
    || !member(clients, v.client_kind) || typeof v.partial !== "boolean") return false;
  return v.state === "observed" ? v.reason === null && typeof v.server_version === "string" && v.server_version.length > 0 && v.server_version.length <= 64
    && Number.isInteger(v.loaded_count) && (v.loaded_count as number) >= 0 && (v.loaded_count as number) <= 128
    : v.state === "unknown" && member(reasons, v.reason) && v.server_version === null && v.loaded_count === null && v.partial === false;
}
function snapshot(v: unknown): v is SessionSnapshot {
  if (!record(v) || !keys(v, ["snapshot_id", "observed_at_utc", "scope", "clients", "rows", "can_open_codex", "traffic_assignment"])
    || typeof v.snapshot_id !== "string" || !uuid.test(v.snapshot_id) || !date(v.observed_at_utc) || v.scope !== "current_cli_environment"
    || !Array.isArray(v.clients) || v.clients.length !== 3 || !v.clients.every(scope) || new Set(v.clients.map(v => v.client_kind)).size !== 3
    || !Array.isArray(v.rows) || v.rows.length > 256 || !v.rows.every(row) || new Set(v.rows.map(v => v.key)).size !== v.rows.length
    || typeof v.can_open_codex !== "boolean" || v.traffic_assignment !== "unverified") return false;
  const scopes = v.clients as SessionScope[], rows = v.rows as SessionRow[];
  if (v.can_open_codex && !scopes.some(s => s.client_kind === "codex" && s.state === "observed")) return false;
  return rows.every(r => scopes.some(s => s.client_kind === r.client_kind && s.state === "observed")
    && (!r.parent_key || !rows.some(p => p.key === r.parent_key && p.client_kind !== r.client_kind)));
}
export function decodeSessionReply(raw: unknown, request: Request): SessionResult {
  const invalid: SessionResult = { kind: "error", code: "invalid_response" };
  if (!record(raw) || !keys(raw, ["schema_version", "request_id", "status", "snapshot", "error"])
    || raw.schema_version !== 1 || raw.request_id !== request.request_id) return invalid;
  if (raw.status === "error") return raw.snapshot === null && member(errors, raw.error) ? { kind: "error", code: raw.error as SessionError } : invalid;
  if (raw.error !== null) return invalid;
  if (request.operation === "read") return raw.status === "snapshot" && snapshot(raw.snapshot) ? { kind: "snapshot", snapshot: raw.snapshot } : invalid;
  return raw.status === "launched" && raw.snapshot === null ? { kind: "launched" } : invalid;
}
export function createSessionClient(available: () => boolean = isTauri, call: Invoke = invoke, requestId = () => crypto.randomUUID()) {
  async function execute(operation: Request["operation"], snapshotId: string | null, confirm: boolean): Promise<SessionResult> {
    if (!available()) return { kind: "error", code: "desktop_unavailable" };
    if (operation === "open_codex_center" && (!snapshotId || !uuid.test(snapshotId) || !confirm)) return { kind: "error", code: "validation_failed" };
    const request: Request = { schema_version: 1, request_id: requestId(), operation, snapshot_id: snapshotId, confirm_open: confirm };
    try { return decodeSessionReply(await call("manage_sessions", { request }), request); }
    catch { return { kind: "error", code: "unavailable" }; }
  }
  return { read: () => execute("read", null, false), openCodex: (snapshotId: string, confirm: boolean) => execute("open_codex_center", snapshotId, confirm) };
}
export const sessionClient = createSessionClient();
