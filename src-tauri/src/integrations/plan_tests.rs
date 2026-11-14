use super::plan::{
    preview_config, preview_config_with_presence, AuthDeclaration, ClientKind, ConfigPresence,
    ConfigScope, ConfigSource, DeclarationState, EndpointBefore, HookShellEffect, PlanBlocker,
    PlanContext, PlanError, PlanStatus, StableEndpoint,
};

const CODEX_CONFIG: &[u8] = br#"
model = "gpt-5-codex"
model_reasoning_effort = "high"
"#;

fn context(client_kind: ClientKind) -> PlanContext {
    PlanContext {
        request_id: "019a0000-0000-7000-8000-000000000001".to_owned(),
        installation_id: "019a0000-0000-7000-8000-000000000002".to_owned(),
        client_kind,
        config_source: match client_kind {
            ClientKind::Codex => ConfigSource::CodexToml,
            ClientKind::ClaudeCode => ConfigSource::ClaudeJson,
            ClientKind::OpenCodeV1 => ConfigSource::OpenCodeJson,
        },
        surface: "cli".to_owned(),
        version: Some(
            match client_kind {
                ClientKind::Codex => "0.163.0-alpha.10.2",
                ClientKind::ClaudeCode => "2.1.158",
                ClientKind::OpenCodeV1 => "1.19.3",
            }
            .to_owned(),
        ),
        scope: ConfigScope::UserConfig,
        scan_generation: 7,
        endpoint: StableEndpoint {
            endpoint_id: "019a0000-0000-7000-8000-000000000003".to_owned(),
            host: "127.0.0.1".to_owned(),
            port: 41731,
        },
        now_unix_seconds: 1_800_000_000,
    }
}

#[test]
fn codex_user_config_plan_targets_only_stable_ingress_and_preserves_declared_model_settings() {
    let plan = preview_config(&context(ClientKind::Codex), CODEX_CONFIG)
        .expect("known Codex user config fixture should produce a preview");

    assert_eq!(plan.schema_version, 1);
    assert_eq!(plan.request_id, context(ClientKind::Codex).request_id);
    assert_eq!(plan.status, PlanStatus::Preview);
    assert_eq!(plan.scope, "user_config");
    assert_eq!(plan.scan_generation, 7);
    assert_eq!(
        plan.endpoint_id,
        context(ClientKind::Codex).endpoint.endpoint_id
    );
    assert_eq!(plan.endpoint_host, "127.0.0.1");
    assert_eq!(plan.endpoint_port, 41731);
    assert_eq!(plan.endpoint_field.before, EndpointBefore::Missing);
    assert_eq!(
        plan.endpoint_field.proposed_value,
        "http://127.0.0.1:41731/v1"
    );
    assert!(plan.endpoint_field.after_headroom);
    assert_eq!(plan.model, DeclarationState::Present);
    assert_eq!(plan.reasoning, DeclarationState::Present);
    assert!(plan.model_unchanged);
    assert!(plan.provider_unchanged);
    assert!(plan.reasoning_unchanged);
    assert!(plan.auth_unchanged);
    assert!(plan.agent_count_unchanged);
    assert_eq!(plan.runtime_effective, DeclarationState::Unknown);
    assert_eq!(plan.auth, AuthDeclaration::Unknown);
    assert_eq!(plan.hook_shell, HookShellEffect::Absent);
    assert!(plan.restart_required);
    assert!(plan.blockers.is_empty());
    assert_eq!(plan.expires_at_unix_seconds, 1_800_000_300);
    assert_eq!(plan.config_fingerprint.len(), 64);
}

#[test]
fn codex_config_contract_requires_platform_evidence_for_desktop() {
    for (surface, version) in [
        ("cli", "0.166.0"),
        ("cli", "0.166.0-alpha.5.1"),
        ("cli", "0.167.0"),
        ("desktop", "26.1103.1"),
        ("desktop", "26.1027.3384.0"),
        ("desktop", "26.1106.51428"),
        ("desktop", "26.1106.51427"),
        ("desktop", "26.1106.51429"),
        ("desktop", "26.1106.51428.0"),
        ("desktop", "0.167.0-alpha.4.1"),
    ] {
        let mut context = context(ClientKind::Codex);
        context.surface = surface.to_owned();
        context.version = Some(version.to_owned());
        let plan = preview_config(&context, CODEX_CONFIG).unwrap();
        if cfg!(target_os = "macos") && surface == "desktop" && version != "26.1106.51428" {
            assert!(plan
                .blockers
                .contains(&PlanBlocker::UnsupportedClientVersion));
            continue;
        }
        assert_eq!(plan.status, PlanStatus::Preview, "{surface}/{version}");
        assert!(plan.model_unchanged && plan.reasoning_unchanged && plan.auth_unchanged);
        assert!(plan.restart_required);
        assert_eq!(plan.runtime_effective, DeclarationState::Unknown);
    }
    for version in ["0.162.0", "1.0.0", "0.166.0 --other", "0.166"] {
        let mut context = context(ClientKind::Codex);
        context.version = Some(version.to_owned());
        let plan = preview_config(&context, CODEX_CONFIG).unwrap();
        assert!(plan
            .blockers
            .contains(&PlanBlocker::UnsupportedClientVersion));
    }
    let mut context = context(ClientKind::Codex);
    context.surface = "unknown".to_owned();
    assert!(preview_config(&context, CODEX_CONFIG)
        .unwrap()
        .blockers
        .contains(&PlanBlocker::UnsupportedClientSurface));
}

#[test]
fn codex_config_contract_accepts_known_and_neighbor_versions_without_claiming_traffic() {
    for version in [
        "0.163.0-alpha.10.2",
        "0.163.0-alpha.11.1",
        "0.163.0-alpha.11.2",
        "0.163.0-alpha.11.0",
        "0.163.0-alpha.11.3",
        "0.163.0-alpha.10.3",
    ] {
        let mut context = context(ClientKind::Codex);
        context.version = Some(version.to_owned());
        let plan = preview_config(&context, CODEX_CONFIG).unwrap();
        assert_eq!(plan.status, PlanStatus::Preview, "{version}");
        assert!(plan.blockers.is_empty(), "{version}");
        assert_eq!(plan.endpoint_field.before, EndpointBefore::Missing);
        assert!(plan.endpoint_field.after_headroom);
        assert_eq!(plan.model, DeclarationState::Present);
        assert_eq!(plan.reasoning, DeclarationState::Present);
        assert!(plan.model_unchanged && plan.reasoning_unchanged && plan.auth_unchanged);
    }

    for version in ["0.162.0", "1.0.0", "unrecognized"] {
        let mut context = context(ClientKind::Codex);
        context.version = Some(version.to_owned());
        let plan = preview_config(&context, CODEX_CONFIG).unwrap();
        assert_eq!(plan.status, PlanStatus::Blocked, "{version}");
        assert!(!plan.endpoint_field.after_headroom);
        assert!(plan
            .blockers
            .contains(&PlanBlocker::UnsupportedClientVersion));
    }

    let mut desktop = context(ClientKind::Codex);
    desktop.surface = "desktop".to_owned();
    desktop.version = Some("26.1106.51428".to_owned());
    let plan = preview_config(&desktop, CODEX_CONFIG).unwrap();
    assert_eq!(plan.status, PlanStatus::Preview);
    assert!(!plan
        .blockers
        .contains(&PlanBlocker::UnsupportedClientSurface));
}

#[test]
fn recognized_opencode_v2_does_not_enable_the_v1_writer_for_either_schema() {
    let mut context = context(ClientKind::OpenCodeV1);
    context.version = Some("2.3.0".into());
    for bytes in [
        br#"{"model":"openai/slot","provider":{"openai":{}}}"#.as_slice(),
        br#"{"model":"openai/slot","providers":{"openai":{"models":{"slot":{"modelID":"fixture-model"}}}}}"#.as_slice(),
    ] {
        let original = bytes.to_vec();
        let plan = preview_config(&context, bytes).unwrap();
        assert_eq!(plan.status, PlanStatus::Blocked);
        assert!(plan.blockers.contains(&PlanBlocker::UnsupportedClientVersion));
        assert!(!plan.blockers.contains(&PlanBlocker::ClientVersionUnknown));
        assert!(!plan.endpoint_field.after_headroom);
        assert!(plan.model_unchanged && plan.auth_unchanged && plan.reasoning_unchanged && plan.agent_count_unchanged);
        assert_eq!(bytes, original);
    }
}

#[test]
fn claude_json_plan_summarizes_auth_and_hooks_without_returning_config_literals() {
    let fixture = br#"{
      "model": "claude-opus-4-6",
      "effortLevel": "high",
      "env": { "ANTHROPIC_API_KEY": "fixture-secret-not-for-output" },
      "hooks": { "PreToolUse": [{ "command": "fixture-hook-command-not-for-output" }] }
    }"#;

    let plan = preview_config(&context(ClientKind::ClaudeCode), fixture).unwrap();
    let serialized = serde_json::to_string(&plan).unwrap();

    assert_eq!(plan.status, PlanStatus::Preview);
    assert_eq!(plan.endpoint_field.before, EndpointBefore::Missing);
    assert_eq!(plan.endpoint_field.proposed_value, "http://127.0.0.1:41731");
    assert_eq!(plan.model, DeclarationState::Present);
    assert_eq!(plan.reasoning, DeclarationState::Present);
    assert_eq!(plan.auth, AuthDeclaration::SecretConfigured);
    assert_eq!(plan.hook_shell, HookShellEffect::PresentUnchanged);
    assert!(!serialized.contains("fixture-secret-not-for-output"));
    assert!(!serialized.contains("fixture-hook-command-not-for-output"));
    assert!(!serialized.contains("claude-opus-4-6"));
    assert!(!serialized.contains("high"));
}

#[test]
fn project_local_scope_is_only_planned_for_claude_and_is_reported_exactly() {
    let mut project_claude = context(ClientKind::ClaudeCode);
    project_claude.scope = ConfigScope::ProjectLocalConfig;
    let plan = preview_config(&project_claude, br#"{"model":"claude-sonnet"}"#)
        .expect("Claude project-local config should be previewable");
    assert_eq!(plan.scope, "project_local_config");
    assert_eq!(plan.status, PlanStatus::Preview);
    assert_eq!(plan.endpoint_field.before, EndpointBefore::Missing);
    assert_eq!(plan.endpoint_field.proposed_value, "http://127.0.0.1:41731");
    assert!(plan.endpoint_field.after_headroom);
    assert!(plan.model_unchanged && plan.reasoning_unchanged && plan.auth_unchanged);

    for kind in [ClientKind::Codex, ClientKind::OpenCodeV1] {
        let mut unsupported = context(kind);
        unsupported.scope = ConfigScope::ProjectLocalConfig;
        assert!(
            matches!(
                preview_config(&unsupported, b"{}"),
                Err(PlanError::InvalidContext)
            ),
            "{kind:?} project-local scope must stay unsupported"
        );
    }
}

#[test]
fn opencode_v1_jsonc_comments_and_trailing_comma_plan_preserves_provider_auth() {
    let fixture = br#"{
      // JSONC comments and trailing commas are accepted for the V1 profile.
      "model": "openai/gpt-5-codex",
      "provider": {
        "openai": { "options": { "apiKey": "opencode-fixture-secret", }, },
      },
    }"#;

    let plan = preview_config(&context(ClientKind::OpenCodeV1), fixture).unwrap();
    let serialized = serde_json::to_string(&plan).unwrap();

    assert_eq!(plan.status, PlanStatus::Preview);
    assert_eq!(plan.endpoint_field.before, EndpointBefore::ProviderRoutes);
    assert_eq!(
        plan.endpoint_field.proposed_value,
        "http://127.0.0.1:41731/v1"
    );
    assert_eq!(plan.auth, AuthDeclaration::SecretConfigured);
    assert_eq!(plan.runtime_effective, DeclarationState::Unknown);
    let coverage = plan
        .opencode_coverage
        .expect("OpenCode plan should include bounded route coverage");
    assert_eq!(coverage.eligible_count, 1);
    assert_eq!(coverage.excluded_count, 0);
    assert_eq!(coverage.responses_count, 1);
    assert_eq!(coverage.chat_completions_count, 0);
    assert!(!coverage.partial);
    assert_eq!(
        coverage.configured_default_route,
        super::opencode_routes::OpenCodeDefaultRouteCoverage::Covered
    );
    assert!(!serialized.contains("opencode-fixture-secret"));
    assert!(!serialized.contains("gpt-5-codex"));
}

#[test]
fn opencode_plan_covers_multiple_provider_routes_without_serializing_route_keys() {
    let fixture = br#"{
      "model":"openai/selected-wire-id",
      "provider":{
        "openai":{"options":{"baseURL":"https://api.openai.com/v1"}},
        "team-alias":{"npm":"@ai-sdk/openai-compatible","options":{"baseURL":"https://api.openai.com/v1"},"models":{"friendly":{"id":"vendor/wire-id"}}},
        "anthropic":{"npm":"@ai-sdk/anthropic"}
      }
    }"#;

    let plan = preview_config(&context(ClientKind::OpenCodeV1), fixture).unwrap();
    let serialized = serde_json::to_string(&plan).unwrap();
    let debug = format!("{plan:?}");
    let coverage = plan.opencode_coverage.unwrap();

    assert_eq!(plan.status, PlanStatus::Preview);
    assert_eq!(plan.endpoint_field.before, EndpointBefore::ProviderRoutes);
    assert_eq!(coverage.eligible_count, 2);
    assert_eq!(coverage.excluded_count, 1);
    assert_eq!(coverage.already_same_count, 0);
    assert_eq!(coverage.responses_count, 1);
    assert_eq!(coverage.chat_completions_count, 1);
    assert!(coverage.partial);
    assert_eq!(
        coverage.configured_default_route,
        super::opencode_routes::OpenCodeDefaultRouteCoverage::Covered
    );
    assert!(!serialized.contains("team-alias"));
    assert!(!serialized.contains("selected-wire-id"));
    assert!(!serialized.contains("vendor/wire-id"));
    assert!(!serialized.contains("https://api.openai.com/v1"));
    assert!(!debug.contains("team-alias"));
    assert!(!debug.contains("selected-wire-id"));
}

#[test]
fn opencode_zero_routes_and_analysis_errors_block_with_fixed_coverage_behavior() {
    let custom = br#"{
      "model":"gateway/custom-model",
      "provider":{"gateway":{"npm":"@ai-sdk/openai-compatible","options":{"baseURL":"https://gateway.example/private"},"models":{"custom":{}}}}
    }"#;
    let zero_routes = preview_config(&context(ClientKind::OpenCodeV1), custom).unwrap();
    assert_eq!(zero_routes.status, PlanStatus::Blocked);
    assert_eq!(zero_routes.endpoint_field.before, EndpointBefore::Unknown);
    assert!(zero_routes
        .blockers
        .contains(&PlanBlocker::UnsupportedRoute));
    let coverage = zero_routes.opencode_coverage.unwrap();
    assert_eq!(coverage.eligible_count, 0);
    assert_eq!(coverage.excluded_count, 1);
    assert_eq!(
        coverage.configured_default_route,
        super::opencode_routes::OpenCodeDefaultRouteCoverage::Excluded
    );

    let malformed = br#"{"provider":{"openai":{"options":true}}}"#;
    let malformed_plan = preview_config(&context(ClientKind::OpenCodeV1), malformed).unwrap();
    assert_eq!(malformed_plan.status, PlanStatus::Blocked);
    assert!(malformed_plan
        .blockers
        .contains(&PlanBlocker::UnsupportedSchema));
    assert!(!malformed_plan
        .blockers
        .contains(&PlanBlocker::UnsupportedRoute));
    assert!(malformed_plan.opencode_coverage.is_none());

    let v2 = br#"{"providers":{"openai":{}}}"#;
    let v2_plan = preview_config(&context(ClientKind::OpenCodeV1), v2).unwrap();
    assert_eq!(v2_plan.status, PlanStatus::Blocked);
    assert!(v2_plan.blockers.contains(&PlanBlocker::UnsupportedRoute));
    assert!(v2_plan.opencode_coverage.is_none());
}

#[test]
fn non_opencode_plans_keep_the_legacy_serialized_contract() {
    for (kind, config) in [
        (ClientKind::Codex, CODEX_CONFIG),
        (ClientKind::ClaudeCode, br#"{"model":"fixture-model"}"#),
    ] {
        let plan = preview_config(&context(kind), config).unwrap();
        let value = serde_json::to_value(plan).unwrap();
        assert!(value.get("opencode_coverage").is_none());
        assert_eq!(value.as_object().unwrap().len(), 28);
    }
}

#[test]
fn json_and_jsonc_duplicate_keys_at_any_depth_fail_closed() {
    let claude = br#"{"env":{"A":"first","A":"second"}}"#;
    let opencode = br#"{
      "model":"openai/gpt-5",
      "provider":{"openai":{"options":{"baseURL":"http://127.0.0.1:41731/v1","baseURL":"http://elsewhere.invalid"}}}
    }"#;

    assert!(matches!(
        preview_config(&context(ClientKind::ClaudeCode), claude),
        Err(PlanError::DuplicateKey)
    ));
    assert!(matches!(
        preview_config(&context(ClientKind::OpenCodeV1), opencode),
        Err(PlanError::DuplicateKey)
    ));
}

#[test]
fn codex_toml_duplicate_keys_and_json_over_limit_are_rejected() {
    let duplicate = br#"model = "one"
model = "two"
"#;
    assert!(matches!(
        preview_config(&context(ClientKind::Codex), duplicate),
        Err(PlanError::InvalidConfig)
    ));

    let oversized = vec![b' '; 256 * 1024 + 1];
    assert!(matches!(
        preview_config(&context(ClientKind::ClaudeCode), &oversized),
        Err(PlanError::ConfigTooLarge)
    ));
}

#[test]
fn custom_gateway_and_opencode_v2_are_blocked_without_echoing_route_values() {
    let custom_codex = br#"model = "gpt-5-codex"
model_provider = "private-provider"
openai_base_url = "https://gateway.example.invalid/private/path"
"#;
    let codex_plan = preview_config(&context(ClientKind::Codex), custom_codex).unwrap();
    let codex_serialized = serde_json::to_string(&codex_plan).unwrap();
    assert_eq!(codex_plan.status, PlanStatus::Blocked);
    assert!(codex_plan
        .blockers
        .contains(&super::plan::PlanBlocker::CustomGateway));
    assert_eq!(
        codex_plan.endpoint_field.before,
        EndpointBefore::OtherConfiguredRedacted
    );
    assert!(!codex_serialized.contains("gateway.example.invalid"));

    let opencode_v2 = br#"{"model":"openai/gpt-5","providers":{"openai":{}}}"#;
    let v2_plan = preview_config(&context(ClientKind::OpenCodeV1), opencode_v2).unwrap();
    assert_eq!(v2_plan.status, PlanStatus::Blocked);
    assert!(v2_plan
        .blockers
        .contains(&super::plan::PlanBlocker::UnsupportedRoute));
}

#[test]
fn existing_stable_endpoint_keeps_restart_requirement_conservative() {
    let fixture = br#"model = "gpt-5-codex"
model_reasoning_effort = "high"
openai_base_url = "http://127.0.0.1:41731/v1"
"#;
    let plan = preview_config(&context(ClientKind::Codex), fixture).unwrap();

    assert_eq!(plan.status, PlanStatus::Preview);
    assert_eq!(
        plan.endpoint_field.before,
        EndpointBefore::SameStableEndpoint
    );
    assert!(plan.endpoint_field.after_headroom);
    assert!(plan.restart_required);
}

#[test]
fn claude_json_rejects_jsonc_extensions_while_opencode_route_shape_errors_block() {
    let json_with_comment = br#"{ // not valid settings JSON
      "model": "claude-opus-4-6"
    }"#;
    assert!(matches!(
        preview_config(&context(ClientKind::ClaudeCode), json_with_comment),
        Err(PlanError::InvalidConfig)
    ));

    let malformed_opencode = br#"{"model":"openai/gpt-5","provider":{"openai":{"options":true}}}"#;
    let plan = preview_config(&context(ClientKind::OpenCodeV1), malformed_opencode).unwrap();
    assert_eq!(plan.status, PlanStatus::Blocked);
    assert!(plan
        .blockers
        .contains(&super::plan::PlanBlocker::UnsupportedSchema));
}

#[test]
fn jsonc_depth_limit_and_invalid_endpoint_context_fail_closed() {
    let mut deeply_nested = String::from("{\"value\":");
    deeply_nested.push_str(&"[".repeat(66));
    deeply_nested.push('0');
    deeply_nested.push_str(&"]".repeat(66));
    deeply_nested.push('}');
    assert!(matches!(
        preview_config(&context(ClientKind::OpenCodeV1), deeply_nested.as_bytes()),
        Err(PlanError::InvalidConfig)
    ));

    let deeply_nested_toml = format!("value = {}0{}", "[".repeat(81), "]".repeat(81));
    assert!(matches!(
        preview_config(&context(ClientKind::Codex), deeply_nested_toml.as_bytes()),
        Err(PlanError::InvalidConfig)
    ));

    let mut invalid = context(ClientKind::Codex);
    invalid.endpoint.host = "0.0.0.0".to_owned();
    assert!(matches!(
        preview_config(&invalid, CODEX_CONFIG),
        Err(PlanError::InvalidContext)
    ));
}

#[test]
fn scan_generation_must_fit_a_positive_javascript_safe_integer() {
    let mut zero = context(ClientKind::Codex);
    zero.scan_generation = 0;
    assert!(matches!(
        preview_config(&zero, CODEX_CONFIG),
        Err(PlanError::InvalidContext)
    ));

    let mut too_large = context(ClientKind::Codex);
    too_large.scan_generation = 9_007_199_254_740_992;
    assert!(matches!(
        preview_config(&too_large, CODEX_CONFIG),
        Err(PlanError::InvalidContext)
    ));
}

#[test]
fn codex_profile_and_claude_cloud_routes_are_blocked_as_unverified() {
    let profiled_codex = br#"model = "gpt-5-codex"
profile = "work"
"#;
    let codex = preview_config(&context(ClientKind::Codex), profiled_codex).unwrap();
    assert_eq!(codex.status, PlanStatus::Blocked);
    assert!(codex
        .blockers
        .contains(&super::plan::PlanBlocker::UnsupportedRoute));

    let bedrock_claude = br#"{
      "model":"claude-opus-4-6",
      "env":{"CLAUDE_CODE_USE_BEDROCK":"1"}
    }"#;
    let claude = preview_config(&context(ClientKind::ClaudeCode), bedrock_claude).unwrap();
    assert_eq!(claude.status, PlanStatus::Blocked);
    assert!(claude
        .blockers
        .contains(&super::plan::PlanBlocker::UnsupportedRoute));
}

#[test]
fn codex_auth_declaration_is_only_reported_for_known_method_values() {
    let supported = br#"forced_login_method = "api"
"#;
    let unsupported = br#"forced_login_method = "mystery"
"#;
    assert_eq!(
        preview_config(&context(ClientKind::Codex), supported)
            .unwrap()
            .auth,
        AuthDeclaration::MethodDeclared
    );
    assert_eq!(
        preview_config(&context(ClientKind::Codex), unsupported)
            .unwrap()
            .auth,
        AuthDeclaration::Unknown
    );
}

#[test]
fn opencode_multiple_providers_and_route_overrides_are_blocked() {
    let fixture = br#"{
      "model":"openai/gpt-5",
      "provider":{
        "openai":{"options":{"headers":{"Host":"fixture.invalid"}},"models":{"gpt-5":{"options":{"baseURL":"https://other.invalid"}}}},
        "anthropic":{"options":{}}
      }
    }"#;
    let plan = preview_config(&context(ClientKind::OpenCodeV1), fixture).unwrap();
    assert_eq!(plan.status, PlanStatus::Blocked);
    assert!(plan
        .blockers
        .contains(&super::plan::PlanBlocker::UnsupportedRoute));
}

#[test]
fn unknown_version_and_surface_block_a_parseable_preview_without_erasing_it() {
    let mut context = context(ClientKind::Codex);
    context.version = None;
    context.surface = "unknown".to_owned();

    let plan = preview_config(&context, CODEX_CONFIG).unwrap();
    assert_eq!(plan.status, PlanStatus::Blocked);
    assert_eq!(
        plan.endpoint_field.field,
        super::plan::EndpointField::CodexOpenaiBaseUrl
    );
    assert_eq!(plan.endpoint_field.before, EndpointBefore::Missing);
    assert!(!plan.endpoint_field.after_headroom);
    assert!(plan.blockers.contains(&PlanBlocker::ClientVersionUnknown));
    assert!(plan
        .blockers
        .contains(&PlanBlocker::UnsupportedClientSurface));
    assert_eq!(plan.runtime_effective, DeclarationState::Unknown);
}

#[test]
fn unsupported_client_version_keeps_the_user_config_draft_blocked() {
    let mut context = context(ClientKind::ClaudeCode);
    context.version = Some("2.1.159".to_owned());
    let fixture = br#"{"model":"fixture-model","effortLevel":"high"}"#;
    let plan = preview_config(&context, fixture).unwrap();

    assert_eq!(plan.status, PlanStatus::Blocked);
    assert_eq!(plan.endpoint_field.before, EndpointBefore::Missing);
    assert!(!plan.endpoint_field.after_headroom);
    assert!(plan
        .blockers
        .contains(&PlanBlocker::UnsupportedClientVersion));
    assert_eq!(plan.model, DeclarationState::Present);
    assert_eq!(plan.reasoning, DeclarationState::Present);
    assert_eq!(plan.runtime_effective, DeclarationState::Unknown);
}

#[test]
fn missing_config_has_domain_separated_fingerprint_and_is_not_present_empty_bytes() {
    let context = context(ClientKind::Codex);
    let missing = preview_config_with_presence(&context, b"", ConfigPresence::Missing).unwrap();
    let present_empty = preview_config(&context, b"").unwrap();

    assert_eq!(missing.config_presence, ConfigPresence::Missing);
    assert_eq!(missing.config_fingerprint.len(), 64);
    assert_ne!(missing.config_fingerprint, present_empty.config_fingerprint);
    assert_eq!(present_empty.config_presence, ConfigPresence::Present);
}
