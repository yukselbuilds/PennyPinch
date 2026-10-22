export type ConnectionPlanStatus = "preview" | "blocked";

export interface ConnectionPlan {
  schema_version: 1;
  request_id: string;
  plan_id: string;
  expires_at_unix_seconds: number;
  installation_id: string;
  client_kind: "codex" | "claude_code" | "opencode_v1";
  scope: "user_config";
  scan_generation: number;
  config_fingerprint: string;
  config_presence: "present" | "missing";
  config_source: "codex_toml" | "claude_json" | "opencode_json" | "opencode_jsonc" | "opencode_missing";
  endpoint_id: string;
  endpoint_host: "127.0.0.1";
  endpoint_port: number;
  endpoint_field: {
    field: "codex_openai_base_url" | "claude_anthropic_base_url" | "opencode_v1_openai_base_url";
    before: "missing" | "same_stable_endpoint" | "other_configured_redacted" | "unknown";
    proposed_value: string;
    after_headroom: boolean;
  };
  status: ConnectionPlanStatus;
  model: "present" | "absent" | "unknown";
  reasoning: "present" | "absent" | "unknown";
  model_unchanged: true;
  provider_unchanged: true;
  reasoning_unchanged: true;
  auth_unchanged: true;
  agent_count_unchanged: true;
  runtime_effective: "unknown";
  auth: "unknown" | "method_declared" | "secret_configured" | "helper_declared";
  hook_shell: "absent" | "present_unchanged" | "unknown";
  restart_required: true;
  blockers: Array<
    | "unsupported_schema"
    | "custom_gateway"
    | "unsupported_route"
    | "client_version_unknown"
    | "unsupported_client_version"
    | "unsupported_client_surface"
  >;
}

export type ConnectionPlanErrorCode =
  | "validation_failed"
  | "unsupported_scope"
  | "stale_selection"
  | "not_found"
  | "endpoint_unavailable"
  | "config_unavailable"
  | "config_unsafe"
  | "config_too_large"
  | "config_conflict"
  | "config_invalid"
  | "busy";

export interface ConnectionPlanError {
  code: ConnectionPlanErrorCode;
  retryable: boolean;
  message_key: `connection.${ConnectionPlanErrorCode}`;
  diagnostic_id: null;
}

export type ConnectionActionErrorCode =
  | "validation_failed"
  | "unsupported_schema"
  | "busy"
  | "not_found"
  | "plan_expired"
  | "config_conflict"
  | "config_invalid"
  | "config_unsafe"
  | "config_unavailable"
  | "unsupported_filesystem"
  | "commit_unknown";

export interface ApplyConnectionRequest {
  schema_version: 1;
  request_id: string;
  plan_id: string;
  expected_config_fingerprint: string;
}

export interface UndoConnectionRequest {
  schema_version: 1;
  request_id: string;
  client_kind: ConnectionPlan["client_kind"];
  owner_id: string;
  expected_revision: number;
  expected_record_fingerprint: string;
}

export interface OwnershipSnapshotRequest {
  schema_version: 1;
  request_id: string;
  client_kind: ConnectionPlan["client_kind"];
}

export type ConnectionActionRequest =
  | ApplyConnectionRequest
  | UndoConnectionRequest
  | OwnershipSnapshotRequest;

export interface ConnectionOwnership {
  state: "active" | "released";
  owner_id: string;
  revision: number;
  record_fingerprint: string;
  config_source: ConnectionPlan["config_source"];
  endpoint_id: string;
  endpoint_port: number;
}

export interface ConnectionActionSuccess {
  schema_version: 1;
  request_id: string;
  status: "applied" | "unchanged" | "released" | "snapshot";
  client_kind: ConnectionPlan["client_kind"];
  ownership: ConnectionOwnership | null;
}

export interface ConnectionActionError {
  code: ConnectionActionErrorCode;
  retryable: boolean;
  message_key: `connection.${ConnectionActionErrorCode}`;
  diagnostic_id: null;
}

export type ConnectionActionResult =
  | { kind: "desktop_unavailable" }
  | { kind: "invalid_request" }
  | { kind: "success"; reply: ConnectionActionSuccess }
  | { kind: "action_error"; error: ConnectionActionError }
  | { kind: "invalid_response" }
  | { kind: "request_failed" };

export type ConnectionPlanResult =
  | { kind: "desktop_unavailable" }
  | { kind: "invalid_request" }
  | { kind: "preview"; plan: ConnectionPlan }
  | { kind: "plan_error"; error: ConnectionPlanError }
  | { kind: "invalid_response" }
  | { kind: "request_failed" };
