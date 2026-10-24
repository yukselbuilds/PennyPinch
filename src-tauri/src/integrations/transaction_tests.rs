use super::plan::{
    preview_config_with_presence, ClientKind, ConfigPresence, ConfigScope, ConfigSource,
    ConnectionPlan, PlanBlocker, PlanContext, PlanStatus, StableEndpoint,
};
use super::transaction::{
    apply_missing_endpoint, undo_owned_endpoint, ApplyResult, TransactionError,
};

fn plan(kind: ClientKind, config: &[u8]) -> ConnectionPlan {
    plan_with_presence(kind, config, ConfigPresence::Present)
}

fn missing_plan(kind: ClientKind) -> ConnectionPlan {
    let empty = match kind {
        ClientKind::Codex => b"".as_slice(),
        ClientKind::ClaudeCode | ClientKind::OpenCodeV1 => b"{}".as_slice(),
    };
    plan_with_presence(kind, empty, ConfigPresence::Missing)
}

fn plan_with_presence(kind: ClientKind, config: &[u8], presence: ConfigPresence) -> ConnectionPlan {
    plan_with_scope(kind, config, presence, ConfigScope::UserConfig)
}

fn plan_with_scope(
    kind: ClientKind,
    config: &[u8],
    presence: ConfigPresence,
    scope: ConfigScope,
) -> ConnectionPlan {
    let context = PlanContext {
        request_id: "019a0000-0000-7000-8000-000000000101".to_owned(),
        installation_id: "019a0000-0000-7000-8000-000000000102".to_owned(),
        client_kind: kind,
        config_source: match kind {
            ClientKind::Codex => ConfigSource::CodexToml,
            ClientKind::ClaudeCode => ConfigSource::ClaudeJson,
            ClientKind::OpenCodeV1 => match presence {
                ConfigPresence::Present => ConfigSource::OpenCodeJsonc,
                ConfigPresence::Missing => ConfigSource::OpenCodeMissing,
            },
        },
        surface: "cli".to_owned(),
        version: Some(
            match kind {
                ClientKind::Codex => "0.163.0-alpha.10.2",
                ClientKind::ClaudeCode => "2.1.158",
                ClientKind::OpenCodeV1 => "1.19.3",
            }
            .to_owned(),
        ),
        scope,
        scan_generation: 7,
        endpoint: StableEndpoint {
            endpoint_id: "019a0000-0000-7000-8000-000000000103".to_owned(),
            host: "127.0.0.1".to_owned(),
            port: 41731,
        },
        now_unix_seconds: 1_800_000_000,
    };
    preview_config_with_presence(&context, config, presence)
        .expect("fixture should have a safe preview")
}

fn patched(result: ApplyResult) -> (Vec<u8>, super::transaction::PatchReceipt) {
    match result {
        ApplyResult::Patched { bytes, receipt } => (bytes, receipt),
        ApplyResult::Unchanged => panic!("expected an owned patch"),
    }
}

#[test]
fn codex_patch_and_undo_preserve_crlf_unicode_and_nested_tables() {
    let original = "# café configuration\r\nmodel = \"gpt-5-codex\"\r\nmodel_reasoning_effort = \"high\"\r\n\r\n[ui.preferences]\r\nmode = \"compact\"\r\n";
    let plan = plan(ClientKind::Codex, original.as_bytes());

    let (updated, receipt) =
        patched(apply_missing_endpoint(&plan, original.as_bytes()).expect("apply should succeed"));
    let updated_text = std::str::from_utf8(&updated).unwrap();
    assert!(updated_text.contains("openai_base_url = \"http://127.0.0.1:41731/v1\""));
    assert!(updated_text.contains("# café configuration\r\nmodel = \"gpt-5-codex\""));
    assert!(updated_text.contains("[ui.preferences]\r\nmode = \"compact\""));
    let restored = undo_owned_endpoint(&receipt, &updated).expect("undo should succeed");
    assert_eq!(restored, original.as_bytes());
}

#[test]
fn claude_json_patch_and_undo_keep_unicode_and_string_escapes_byte_exact() {
    let original = r#"{
  "model": "claude-opus-4-6",
  "env": {"DISPLAY_NAME": "München", "TEXT": "line\\nvalue"}
}
"#
    .as_bytes();
    let plan = plan(ClientKind::ClaudeCode, original);
    let (updated, receipt) = patched(apply_missing_endpoint(&plan, original).unwrap());

    assert!(std::str::from_utf8(&updated)
        .unwrap()
        .contains("\"ANTHROPIC_BASE_URL\""));
    assert!(updated
        .windows("München".len())
        .any(|window| window == "München".as_bytes()));
    assert!(updated
        .windows(b"line\\\\nvalue".len())
        .any(|window| window == b"line\\\\nvalue"));
    assert_eq!(undo_owned_endpoint(&receipt, &updated).unwrap(), original);
}

#[test]
fn claude_project_local_endpoint_patch_and_undo_preserve_other_settings() {
    let original = r#"{
  "model": "claude-sonnet",
  "effortLevel": "high",
  "env": {"ANTHROPIC_API_KEY": "fixture-secret"},
  "apiKeyHelper": "fixture-auth-helper",
  "theme": "München",
  "hooks": {"PreToolUse": [{"command": "fixture-hook"}]}
}"#
    .as_bytes();
    let project_plan = plan_with_scope(
        ClientKind::ClaudeCode,
        original,
        ConfigPresence::Present,
        ConfigScope::ProjectLocalConfig,
    );

    let (updated, receipt) = patched(
        apply_missing_endpoint(&project_plan, original)
            .expect("Claude project-local endpoint patch should succeed"),
    );
    let updated_text = std::str::from_utf8(&updated).unwrap();
    assert!(updated_text.contains("\"ANTHROPIC_BASE_URL\": \"http://127.0.0.1:41731\""));
    assert!(updated_text.contains("\"model\": \"claude-sonnet\""));
    assert!(updated_text.contains("\"effortLevel\": \"high\""));
    assert!(updated_text.contains("\"ANTHROPIC_API_KEY\": \"fixture-secret\""));
    assert!(updated_text.contains("\"apiKeyHelper\": \"fixture-auth-helper\""));
    assert!(updated_text.contains("\"theme\": \"München\""));
    assert!(updated_text.contains("\"command\": \"fixture-hook\""));
    assert_eq!(undo_owned_endpoint(&receipt, &updated).unwrap(), original);
}

#[test]
fn opencode_jsonc_patch_and_undo_preserve_comments_trailing_commas_and_add_ancestors() {
    let original = r#"{
  // Keep this comment and Unicode label: café
  "model": "openai/gpt-5-codex",
  "theme": "dark",
}
"#
    .as_bytes();
    let opencode_plan = plan(ClientKind::OpenCodeV1, original);
    let (updated, receipt) = patched(apply_missing_endpoint(&opencode_plan, original).unwrap());
    let text = std::str::from_utf8(&updated).unwrap();

    assert!(text.contains("// Keep this comment and Unicode label: café"));
    assert!(text.contains("\"baseURL\": \"http://127.0.0.1:41731/v1\""));
    let reparsed = super::config_document::parse_document(ClientKind::OpenCodeV1, &updated)
        .expect("patched JSONC remains valid");
    assert_eq!(
        reparsed["provider"]["openai"]["options"]["baseURL"],
        "http://127.0.0.1:41731/v1"
    );
    assert_eq!(undo_owned_endpoint(&receipt, &updated).unwrap(), original);
}

#[test]
fn opencode_routes_already_at_the_stable_endpoint_are_an_unowned_noop() {
    let original = br#"{
  "model": "lab-a/selector",
  "provider": {
    "lab-a": {"npm": "@ai-sdk/openai-compatible", "models": {"selector": {"id": "wire-a"}}, "options": {"baseURL": "http://127.0.0.1:41731/v1"}},
    "openai": {"npm": "@ai-sdk/openai", "models": {"responses": {"id": "wire-responses"}}, "options": {"baseURL": "http://127.0.0.1:41731/v1"}}
  }
}"#;
    let opencode_plan = plan(ClientKind::OpenCodeV1, original);
    assert_eq!(opencode_plan.status, PlanStatus::Preview);
    assert_eq!(
        apply_missing_endpoint(&opencode_plan, original).unwrap(),
        ApplyResult::Unchanged
    );
}

#[test]
fn opencode_routes_patch_and_undo_all_eligible_aliases_without_changing_models() {
    let original = r#"{
  // Route-specific wire IDs and credentials are user data.
  "model": "lab-a/selector",
  "provider": {
    "lab-a": {
      "npm": "@ai-sdk/openai-compatible",
      "models": {"selector": {"id": "wire-a"}},
      "options": {"apiKey": "fixture-secret-marker", "baseURL": "https:\/\/api.openai.com\/v1"}
    },
    "lab-b": {
      "npm": "@ai-sdk/openai-compatible",
      "models": {"other": {"id": "wire-b"}},
      "options": {"baseURL": "http://127.0.0.1:41731/v1"}
    },
    "openai": {
      "npm": "@ai-sdk/openai",
      "models": {"responses": {"id": "wire-responses"}}
    },
    "anthropic": {"npm": "@ai-sdk/anthropic"}
  }
}
"#
    .as_bytes();
    let opencode_plan = plan(ClientKind::OpenCodeV1, original);
    assert_eq!(opencode_plan.status, PlanStatus::Preview);

    let (updated, receipt) = patched(apply_missing_endpoint(&opencode_plan, original).unwrap());
    let text = std::str::from_utf8(&updated).unwrap();
    assert!(text.contains("// Route-specific wire IDs and credentials are user data."));
    assert!(text.contains("\"model\": \"lab-a/selector\""));
    assert!(text.contains("\"id\": \"wire-a\""));
    assert!(text.contains("\"id\": \"wire-b\""));
    assert!(text.contains("\"id\": \"wire-responses\""));
    assert!(text.contains("\"apiKey\": \"fixture-secret-marker\""));
    assert!(text.contains("\"baseURL\": \"http://127.0.0.1:41731/v1\""));

    let parsed = super::config_document::parse_document(ClientKind::OpenCodeV1, &updated).unwrap();
    assert_eq!(
        parsed["provider"]["lab-a"]["options"]["baseURL"],
        "http://127.0.0.1:41731/v1"
    );
    assert_eq!(
        parsed["provider"]["lab-b"]["options"]["baseURL"],
        "http://127.0.0.1:41731/v1"
    );
    assert_eq!(
        parsed["provider"]["openai"]["options"]["baseURL"],
        "http://127.0.0.1:41731/v1"
    );
    assert_eq!(
        parsed["provider"]["anthropic"]["options"]["baseURL"],
        serde_json::Value::Null
    );

    let mut unrelated_route_edit =
        super::config_document::parse_document(ClientKind::OpenCodeV1, &updated).unwrap();
    unrelated_route_edit["provider"]["lab-b"]["options"]["baseURL"] =
        serde_json::json!("https://user.example/v1");
    let unrelated_route_edit = serde_json::to_vec(&unrelated_route_edit).unwrap();
    let restored_with_edit = undo_owned_endpoint(&receipt, &unrelated_route_edit).unwrap();
    let restored_value =
        super::config_document::parse_document(ClientKind::OpenCodeV1, &restored_with_edit)
            .unwrap();
    assert_eq!(
        restored_value["provider"]["lab-a"]["options"]["baseURL"],
        "https://api.openai.com/v1"
    );
    assert_eq!(
        restored_value["provider"]["lab-b"]["options"]["baseURL"],
        "https://user.example/v1"
    );
    assert!(restored_value["provider"]["openai"]
        .get("options")
        .is_none());

    let mut missing_owned_route =
        super::config_document::parse_document(ClientKind::OpenCodeV1, &updated).unwrap();
    missing_owned_route["provider"]
        .as_object_mut()
        .unwrap()
        .remove("lab-a");
    let missing_owned_route = serde_json::to_vec(&missing_owned_route).unwrap();
    assert_eq!(
        undo_owned_endpoint(&receipt, &missing_owned_route),
        Err(TransactionError::Conflict)
    );

    let conflicting_edit = text.replacen("http://127.0.0.1:41731/v1", "https://user.example/v1", 1);
    assert_eq!(
        undo_owned_endpoint(&receipt, conflicting_edit.as_bytes()),
        Err(TransactionError::Conflict)
    );
    assert_eq!(undo_owned_endpoint(&receipt, &updated).unwrap(), original);
}

#[test]
fn opencode_route_drift_after_preview_conflicts_before_returning_a_patch() {
    let original = br#"{
  "model": "lab-a/selector",
  "provider": {
    "lab-a": {"npm": "@ai-sdk/openai-compatible", "models": {"selector": {"id": "wire-a"}}, "options": {"baseURL": "https://api.openai.com/v1"}}
  }
}"#;
    let opencode_plan = plan(ClientKind::OpenCodeV1, original);
    let changed = std::str::from_utf8(original)
        .unwrap()
        .replace("https://api.openai.com/v1", "https://user.example/v1");
    assert_eq!(
        apply_missing_endpoint(&opencode_plan, changed.as_bytes()),
        Err(TransactionError::Conflict)
    );
}

#[test]
fn undo_preserves_unrelated_user_edits_and_conflicts_if_owned_value_changed() {
    let original = br#"{"model":"claude-opus-4-6"}"#;
    let plan = plan(ClientKind::ClaudeCode, original);
    let (updated, receipt) = patched(apply_missing_endpoint(&plan, original).unwrap());
    let mut with_user_edit = updated.clone();
    let root_open = with_user_edit
        .iter()
        .position(|byte| *byte == b'{')
        .unwrap();
    with_user_edit.splice(
        root_open + 1..root_open + 1,
        b"\n  \"theme\": \"dark\",".iter().copied(),
    );
    let restored = undo_owned_endpoint(&receipt, &with_user_edit).unwrap();
    assert!(restored
        .windows(b"\"theme\": \"dark\"".len())
        .any(|w| w == b"\"theme\": \"dark\""));
    assert!(!restored
        .windows(b"ANTHROPIC_BASE_URL".len())
        .any(|w| w == b"ANTHROPIC_BASE_URL"));

    let changed = String::from_utf8(updated)
        .unwrap()
        .replace("127.0.0.1:41731", "127.0.0.1:41732")
        .into_bytes();
    assert_eq!(
        undo_owned_endpoint(&receipt, &changed),
        Err(TransactionError::Conflict)
    );
}

#[test]
fn undo_keeps_edits_inside_created_opencode_ancestors_and_rejects_bad_shape_or_size() {
    let original = r#"{
  "model": "openai/gpt-5-codex"
}"#
    .as_bytes();
    let opencode_plan = plan(ClientKind::OpenCodeV1, original);
    let (updated, receipt) = patched(apply_missing_endpoint(&opencode_plan, original).unwrap());
    let changed = String::from_utf8(updated).unwrap().replace(
        "\"options\": {\"baseURL\":",
        "\"options\": {\"custom\": true, \"baseURL\":",
    );
    let undone = undo_owned_endpoint(&receipt, changed.as_bytes()).unwrap();
    let parsed = super::config_document::parse_document(ClientKind::OpenCodeV1, &undone).unwrap();
    assert_eq!(parsed["provider"]["openai"]["options"]["custom"], true);

    let (commented, comment_receipt) =
        patched(apply_missing_endpoint(&opencode_plan, original).unwrap());
    let commented = String::from_utf8(commented).unwrap().replace(
        "\"baseURL\": \"http://127.0.0.1:41731/v1\"",
        "\"baseURL\": \"http://127.0.0.1:41731/v1\" /* retain */",
    );
    let comment_undo = undo_owned_endpoint(&comment_receipt, commented.as_bytes()).unwrap();
    let comment_undo_text = std::str::from_utf8(&comment_undo).unwrap();
    assert!(comment_undo_text.contains("/* retain */"));
    assert!(comment_undo_text.contains("\"options\": { /* retain */}"));

    let (trailing, trailing_receipt) =
        patched(apply_missing_endpoint(&opencode_plan, original).unwrap());
    let trailing = String::from_utf8(trailing).unwrap().replace(
        "\"baseURL\": \"http://127.0.0.1:41731/v1\"",
        "\"baseURL\": \"http://127.0.0.1:41731/v1\", /* trailing note */",
    );
    super::config_document::parse_document(ClientKind::OpenCodeV1, trailing.as_bytes())
        .expect("fixture with one-property trailing comment parses");
    let trailing_undo = undo_owned_endpoint(&trailing_receipt, trailing.as_bytes()).unwrap();
    assert!(std::str::from_utf8(&trailing_undo)
        .unwrap()
        .contains("/* trailing note */"));
    super::config_document::parse_document(ClientKind::OpenCodeV1, &trailing_undo)
        .expect("single-property trailing-comma comments remain valid after undo");

    let claude_plan = plan(ClientKind::ClaudeCode, br#"{"model":"claude-opus-4-6"}"#);
    assert_eq!(
        apply_missing_endpoint(&claude_plan, br#"{"env":false}"#),
        Err(TransactionError::Conflict)
    );
    let oversized = vec![b' '; 256 * 1024 + 1];
    assert_eq!(
        apply_missing_endpoint(&claude_plan, &oversized),
        Err(TransactionError::InvalidConfig)
    );
}

#[test]
fn missing_codex_and_claude_documents_can_be_patched_and_undone_in_memory() {
    for (kind, empty) in [
        (ClientKind::Codex, b"".as_slice()),
        (ClientKind::ClaudeCode, b"{}".as_slice()),
    ] {
        let missing = missing_plan(kind);
        assert_eq!(missing.config_presence, ConfigPresence::Missing);
        let (updated, receipt) = patched(apply_missing_endpoint(&missing, empty).unwrap());
        assert!(receipt.metadata().config_was_missing);
        assert_eq!(undo_owned_endpoint(&receipt, &updated).unwrap(), empty);
        assert_eq!(
            apply_missing_endpoint(&missing, br#"{"model":"user-edit"}"#),
            Err(TransactionError::Conflict)
        );
    }

    let blocked_missing_opencode = missing_plan(ClientKind::OpenCodeV1);
    assert_eq!(blocked_missing_opencode.status, PlanStatus::Blocked);
    assert_eq!(
        apply_missing_endpoint(&blocked_missing_opencode, b"{}"),
        Err(TransactionError::InvalidPlan)
    );
}

#[test]
fn blocked_or_inconsistent_plan_metadata_never_patches() {
    let original = br#"{"model":"claude-opus-4-6"}"#;
    let valid = plan(ClientKind::ClaudeCode, original);

    let mut blocked = valid.clone();
    blocked.status = PlanStatus::Blocked;
    blocked.blockers.push(PlanBlocker::ClientVersionUnknown);
    assert_eq!(
        apply_missing_endpoint(&blocked, original),
        Err(TransactionError::InvalidPlan)
    );

    let mut wrong_port = valid.clone();
    wrong_port.endpoint_port = 41732;
    assert_eq!(
        apply_missing_endpoint(&wrong_port, original),
        Err(TransactionError::InvalidPlan)
    );

    let mut wrong_source = valid;
    wrong_source.config_source = ConfigSource::CodexToml;
    assert_eq!(
        apply_missing_endpoint(&wrong_source, original),
        Err(TransactionError::InvalidPlan)
    );

    let mut codex_project_scope = plan(ClientKind::Codex, b"model = \"gpt-5\"\n");
    codex_project_scope.scope = "project_local_config";
    assert_eq!(
        apply_missing_endpoint(&codex_project_scope, b"model = \"gpt-5\"\n"),
        Err(TransactionError::InvalidPlan)
    );

    let mut opencode_project_scope = plan(ClientKind::OpenCodeV1, b"{}");
    opencode_project_scope.scope = "project_local_config";
    assert_eq!(
        apply_missing_endpoint(&opencode_project_scope, b"{}"),
        Err(TransactionError::InvalidPlan)
    );
}

#[test]
fn same_endpoint_is_a_byte_exact_noop_without_receipt() {
    let original = b"model = \"gpt-5-codex\"\nopenai_base_url = \"http://127.0.0.1:41731/v1\"\n";
    let plan = plan(ClientKind::Codex, original);
    assert_eq!(
        apply_missing_endpoint(&plan, original).unwrap(),
        ApplyResult::Unchanged
    );
}

#[test]
fn custom_endpoint_is_not_patched_and_malformed_current_document_is_rejected() {
    let custom = b"model = \"gpt-5-codex\"\nopenai_base_url = \"https://custom.invalid/api\"\n";
    let custom_plan = plan(ClientKind::Codex, custom);
    assert_eq!(
        custom_plan.endpoint_field.before,
        super::plan::EndpointBefore::OtherConfiguredRedacted
    );
    assert_eq!(
        apply_missing_endpoint(&custom_plan, custom),
        Err(TransactionError::InvalidPlan)
    );

    let missing_plan = plan(ClientKind::ClaudeCode, br#"{"model":"claude-opus-4-6"}"#);
    let malformed = b"{\"model\":\"claude-opus-4-6\",\"env\":{\"x\":1,\"x\":2}}";
    assert_eq!(
        apply_missing_endpoint(&missing_plan, malformed),
        Err(TransactionError::InvalidConfig)
    );
}
