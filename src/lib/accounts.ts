import { invoke, isTauri } from "@tauri-apps/api/core";
import type { InstallationSummary } from "./integrations.ts";

export type Account = {
  installation_id: string;
  client_kind: "codex" | "claude_code" | "opencode";
  surface: "cli" | "desktop";
  state: "present" | "absent" | "unknown";
  method: "chatgpt" | "api_key" | "oauth" | "access_token" | "provider_credentials" | "none" | "unknown";
  source: "official_cli" | "local_store" | "unavailable";
  reason: "none" | "unsupported" | "environment_auth" | "read_not_passive" | "read_failed" | "timeout" | "cleanup_failed" | "unavailable" | "unsafe_store" | "invalid_store";
  login_supported: boolean;
  remote_verified: false;
  providers: { provider: string; method: "api_key" | "oauth" | "provider_token" }[];
};

export type LoginFlow = {
  flow_id: string;
  installation_id: string;
  client_kind: Account["client_kind"];
  state: "pending" | "finished" | "cancelled" | "failed" | "timed_out" | "cleanup_failed";
};

export type AccountResult = { kind: "ok"; account: Account | null; flow: LoginFlow | null }
  | { kind: "error"; code: "desktop_unavailable" | "invalid_response" | "validation_failed" | "busy" | "stale_selection" | "untrusted_installation" | "unsupported" | "unavailable" | "launch_failed" | "not_found" };
type Operation = "read" | "start_login" | "active_login" | "poll_login" | "cancel_login";
type Request = { schema_version: 1; request_id: string; operation: Operation; installation_id: string | null;
  scan_generation: number | null; flow_id: string | null; confirm_profile_change: boolean };
type Invoke = (command: string, args: Record<string, unknown>) => Promise<unknown>;
const uuid = /^[0-9a-f]{8}-[0-9a-f]{4}-[1-8][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;
const methods = ["chatgpt", "api_key", "oauth", "access_token", "provider_credentials", "none", "unknown"];
const reasons = ["none", "unsupported", "environment_auth", "read_not_passive", "read_failed", "timeout", "cleanup_failed", "unavailable", "unsafe_store", "invalid_store"];
const providers = ["openai", "anthropic", "google", "opencode", "github-copilot", "amazon-bedrock", "azure", "other"];
const errors = ["validation_failed", "busy", "stale_selection", "untrusted_installation", "unsupported", "unavailable", "launch_failed", "not_found"];
function record(v: unknown): v is Record<string, unknown> { return typeof v === "object" && v !== null && !Array.isArray(v); }
function keys(v: Record<string, unknown>, names: readonly string[]): boolean { return Object.keys(v).length === names.length && names.every(k => Object.hasOwn(v, k)); }
function member<T>(items: readonly T[], v: unknown): v is T { return items.includes(v as T); }
function isAccount(v: unknown): v is Account {
  if (!record(v) || !keys(v, ["installation_id", "client_kind", "surface", "state", "method", "source", "reason", "login_supported", "remote_verified", "providers"])) return false;
  if (typeof v.installation_id !== "string" || !uuid.test(v.installation_id) ||
    !member(["codex", "claude_code", "opencode"], v.client_kind) || !member(["cli", "desktop"], v.surface) ||
    !member(["present", "absent", "unknown"], v.state) || !member(methods, v.method) ||
    !member(["official_cli", "local_store", "unavailable"], v.source) || !member(reasons, v.reason) ||
    typeof v.login_supported !== "boolean" || v.remote_verified !== false || !Array.isArray(v.providers) || v.providers.length > 24) return false;
  if (!v.providers.every(p => record(p) && keys(p, ["provider", "method"]) && member(providers, p.provider) && member(["api_key", "oauth", "provider_token"], p.method))) return false;
  if (v.state === "unknown") return v.method === "unknown" && v.reason !== "none" && v.providers.length === 0;
  if (v.reason !== "none" || v.source === "unavailable") return false;
  return v.state === "absent" ? v.method === "none" && v.providers.length === 0 : v.method !== "none" && v.method !== "unknown";
}
function isFlow(v: unknown): v is LoginFlow {
  return record(v) && keys(v, ["flow_id", "installation_id", "client_kind", "state"]) &&
    typeof v.flow_id === "string" && uuid.test(v.flow_id) && typeof v.installation_id === "string" && uuid.test(v.installation_id) &&
    member(["codex", "claude_code", "opencode"], v.client_kind) &&
    member(["pending", "finished", "cancelled", "failed", "timed_out", "cleanup_failed"], v.state);
}

export function decodeAccountReply(raw: unknown, request: Request, installation?: InstallationSummary, flow?: LoginFlow): AccountResult {
  const invalid: AccountResult = { kind: "error", code: "invalid_response" };
  if (!record(raw) || !keys(raw, ["schema_version", "request_id", "status", "account", "flow", "error"]) || raw.schema_version !== 1 || raw.request_id !== request.request_id) return invalid;
  if (raw.status === "error") return raw.account === null && raw.flow === null && member(errors, raw.error)
    ? { kind: "error", code: raw.error as Extract<AccountResult, {kind: "error"}>["code"] } : invalid;
  if (raw.status !== "snapshot" || raw.error !== null) return invalid;
  if (request.operation === "read") return isAccount(raw.account) && raw.flow === null && installation &&
    raw.account.installation_id === installation.installation_id && raw.account.client_kind === installation.client_kind && raw.account.surface === installation.surface
    ? { kind: "ok", account: raw.account, flow: null } : invalid;
  if (raw.account !== null) return invalid;
  if (request.operation === "active_login" && raw.flow === null) return { kind: "ok", account: null, flow: null };
  if (!isFlow(raw.flow)) return invalid;
  if (request.operation === "start_login" && (!installation || raw.flow.installation_id !== installation.installation_id || raw.flow.client_kind !== installation.client_kind || raw.flow.state !== "pending")) return invalid;
  if (flow && (raw.flow.flow_id !== flow.flow_id || raw.flow.installation_id !== flow.installation_id || raw.flow.client_kind !== flow.client_kind)) return invalid;
  return { kind: "ok", account: null, flow: raw.flow };
}

export function createAccountClient(available: () => boolean = isTauri, call: Invoke = invoke, requestId = () => crypto.randomUUID()) {
  async function execute(operation: Operation, installation?: InstallationSummary, generation?: number, flow?: LoginFlow, consent = false): Promise<AccountResult> {
    if (!available()) return { kind: "error", code: "desktop_unavailable" };
    if (installation && (!uuid.test(installation.installation_id) || !Number.isSafeInteger(generation) || generation! < 1)) return { kind: "error", code: "validation_failed" };
    if (operation === "start_login" && !consent) return { kind: "error", code: "validation_failed" };
    if (flow && !uuid.test(flow.flow_id)) return { kind: "error", code: "validation_failed" };
    const request: Request = { schema_version: 1, request_id: requestId(), operation,
      installation_id: installation?.installation_id ?? null, scan_generation: generation ?? null,
      flow_id: flow?.flow_id ?? null, confirm_profile_change: consent };
    try { return decodeAccountReply(await call("manage_account", { request }), request, installation, flow); }
    catch { return { kind: "error", code: "unavailable" }; }
  }
  return {
    read: (installation: InstallationSummary, generation: number) => execute("read", installation, generation),
    start: (installation: InstallationSummary, generation: number, confirmProfileChange: boolean) => execute("start_login", installation, generation, undefined, confirmProfileChange),
    active: () => execute("active_login"),
    poll: (flow: LoginFlow) => execute("poll_login", undefined, undefined, flow),
    cancel: (flow: LoginFlow) => execute("cancel_login", undefined, undefined, flow),
  };
}
export const accountClient = createAccountClient();
