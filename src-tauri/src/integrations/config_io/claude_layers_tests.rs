use super::*;

fn context(scope: ConfigScope) -> PlanContext {
    PlanContext {
        request_id: "00000000-0000-0000-0000-000000000001".to_owned(),
        installation_id: "00000000-0000-0000-0000-000000000002".to_owned(),
        client_kind: ClientKind::ClaudeCode,
        config_source: ConfigSource::ClaudeJson,
        surface: "cli".to_owned(),
        version: Some("2.1.158".to_owned()),
        scope,
        scan_generation: 1,
        endpoint: crate::integrations::plan::StableEndpoint {
            endpoint_id: "00000000-0000-0000-0000-000000000003".to_owned(),
            host: "127.0.0.1".to_owned(),
            port: 41731,
        },
        now_unix_seconds: 1_800_000_000,
    }
}

#[test]
fn layer_summary_redacts_model_route_auth_and_hook_values() {
    let bytes = br#"{
      "model":"arbitrary-model-name",
      "effortLevel":"xhigh",
      "apiKeyHelper":"secret-helper-command",
      "hooks":{"PreToolUse":[{"command":"private-hook"}]},
      "env":{"ANTHROPIC_BASE_URL":"https://user:password@gateway.example","ANTHROPIC_API_KEY":"secret-token"}
    }"#;
    let summary = summary_from_bytes(
        ClaudeLayerSource::SharedProjectSettings,
        ConfigPresence::Present,
        bytes,
        &context(ConfigScope::ProjectLocalConfig),
    )
    .unwrap();
    let encoded = serde_json::to_string(&summary).unwrap();

    assert_eq!(
        summary.route,
        ClaudeRouteDeclaration::OtherConfiguredRedacted
    );
    assert_eq!(summary.model, DeclarationState::Present);
    assert_eq!(summary.reasoning, DeclarationState::Present);
    assert_eq!(summary.auth, AuthDeclaration::HelperDeclared);
    assert_eq!(summary.hooks, HookShellEffect::PresentUnchanged);
    for secret in [
        "arbitrary-model-name",
        "xhigh",
        "secret-helper-command",
        "gateway.example",
        "password",
        "secret-token",
        "private-hook",
    ] {
        assert!(!encoded.contains(secret));
    }
}

#[test]
fn route_environment_fingerprint_changes_without_serializing_values() {
    let one = vec![(
        "ANTHROPIC_BASE_URL".to_owned(),
        OsString::from("https://first-secret.example"),
    )];
    let two = vec![(
        "ANTHROPIC_BASE_URL".to_owned(),
        OsString::from("https://second-secret.example"),
    )];
    let (first_summary, first_fingerprint, blockers) =
        summarize_environment(&one, &context(ConfigScope::UserConfig)).unwrap();
    let (_second_summary, second_fingerprint, _) =
        summarize_environment(&two, &context(ConfigScope::UserConfig)).unwrap();
    let encoded = serde_json::to_string(&first_summary).unwrap();

    assert_ne!(first_fingerprint, second_fingerprint);
    assert!(blockers.contains(&PlanBlocker::CustomGateway));
    assert_eq!(
        first_summary.route,
        ClaudeRouteDeclaration::OtherConfiguredRedacted
    );
    assert!(!encoded.contains("first-secret.example"));
}

#[test]
fn environment_route_flags_block_without_copying_auth_environment() {
    let environment = vec![("CLAUDE_CODE_USE_BEDROCK".to_owned(), OsString::from("1"))];
    let (summary, _, blockers) =
        summarize_environment(&environment, &context(ConfigScope::UserConfig)).unwrap();

    assert_eq!(summary.route, ClaudeRouteDeclaration::Absent);
    assert!(blockers.contains(&PlanBlocker::UnsupportedRoute));
}

#[test]
fn duplicate_keys_in_observed_layer_fail_closed() {
    let error = summary_from_bytes(
        ClaudeLayerSource::UserSettings,
        ConfigPresence::Present,
        br#"{"env":{"KEEP":"a","KEEP":"b"}}"#,
        &context(ConfigScope::UserConfig),
    )
    .unwrap_err();

    assert_eq!(error, ClaudeLayersError::DuplicateKey);
}
