use super::*;

fn installation(client: &'static str, surface: &'static str, version: &str) -> InstallationSummary {
    InstallationSummary {
        installation_id: Uuid::new_v4().to_string(),
        client_kind: client,
        surface,
        version: Some(version.into()),
        source: "manual_file",
        location_label: "selected_file",
        status: "detected",
        connection_status: "not_verified",
    }
}

#[test]
fn account_requests_require_action_specific_fields_and_explicit_profile_consent() {
    let mut r = Request {
        schema_version: 1,
        request_id: Uuid::new_v4().to_string(),
        operation: Operation::Read,
        installation_id: Some(Uuid::new_v4().to_string()),
        scan_generation: Some(1),
        flow_id: None,
        confirm_profile_change: false,
    };
    assert!(valid_request(&r));
    r.operation = Operation::StartLogin;
    assert!(!valid_request(&r));
    r.confirm_profile_change = true;
    assert!(valid_request(&r));
    r.flow_id = Some(Uuid::new_v4().to_string());
    assert!(!valid_request(&r));
    r.operation = Operation::CancelLogin;
    r.installation_id = None;
    r.scan_generation = None;
    r.confirm_profile_change = false;
    assert!(valid_request(&r));
    r.request_id = Uuid::nil().to_string();
    assert!(!valid_request(&r));
}

#[test]
fn account_requests_cannot_supply_an_executable_model_or_auth_value() {
    let base = serde_json::json!({
        "schema_version": 1, "request_id": Uuid::new_v4().to_string(),
        "operation": "active_login", "installation_id": null,
        "scan_generation": null, "flow_id": null, "confirm_profile_change": false
    });
    assert!(serde_json::from_value::<Request>(base.clone()).is_ok());
    for field in ["executable", "model", "auth", "url"] {
        let mut value = base.clone();
        value[field] = "untrusted-sentinel".into();
        assert!(serde_json::from_value::<Request>(value).is_err());
    }
}

#[test]
fn codex_status_never_serializes_key_fragments_or_claims_live_access() {
    let i = installation("codex", "cli", "0.163.0-alpha.11.2");
    let v = parse_status(
        &i,
        0,
        b"",
        b"Logged in using an API key - sk-prefix***suffix\n",
    );
    assert_eq!(
        (v.state, v.method, v.remote_verified),
        ("present", "api_key", false)
    );
    let serialized = serde_json::to_string(&v).unwrap();
    for forbidden in ["sk-prefix", "suffix", "token", "auth.json"] {
        assert!(!serialized.contains(forbidden));
    }
    let no = parse_status(&i, 1, b"", b"Not logged in\n");
    assert_eq!((no.state, no.method), ("absent", "none"));
    assert_eq!(
        parse_status(&i, 0, b"", b"Logged in using ChatGPT\n").state,
        "present"
    );
}

#[test]
fn account_parser_keeps_malformed_failures_and_future_modes_unknown() {
    let i = installation("codex", "cli", "0.163.0-alpha.11.2");
    for (code, bytes) in [
        (1, b"Logged in using ChatGPT".as_slice()),
        (0, b"Not logged in"),
        (1, b"Error checking login status: secret-sentinel"),
        (0, b"Logged in using workload identity"),
        (0, b"Logged in using ChatGPT\nUnexpected"),
        (0, &[255u8]),
    ] {
        let value = parse_status(&i, code, b"", bytes);
        assert_eq!((value.state, value.remote_verified), ("unknown", false));
        assert!(!serde_json::to_string(&value)
            .unwrap()
            .contains("secret-sentinel"));
    }
    assert_eq!(
        parse_status(&i, 0, &vec![b'x'; 65537], b"Logged in using ChatGPT").state,
        "unknown"
    );
}

#[test]
fn account_capabilities_are_separate_from_model_route_and_desktop_presence() {
    assert!(supported(
        &installation("codex", "cli", "0.163.0-alpha.11.2"),
        "0.163.0-alpha.11.2"
    ));
    assert!(!supported(
        &installation("codex", "desktop", "26.1106.51428"),
        "26.1106.51428"
    ));
    assert!(!supported(
        &installation("opencode", "cli", "2.0.0"),
        "2.0.0"
    ));
    assert!(!supported(
        &installation("opencode", "cli", "2.3.0"),
        "2.3.0"
    ));
    assert!(!supported(
        &installation("claude_code", "cli", "9.0.0"),
        "9.0.0"
    ));
}

#[test]
fn nonpassive_claude_command_is_not_used_to_infer_account_state() {
    let i = installation("claude_code", "cli", "2.1.158");
    let bytes = br#"{"loggedIn":true,"authMethod":"api_key","email":"private-sentinel","key":"secret-sentinel"}"#;
    let v = parse_status(&i, 0, bytes, b"");
    assert_eq!((v.state, v.method), ("unknown", "unknown"));
    assert!(!serde_json::to_string(&v).unwrap().contains("sentinel"));
    for bytes in [
        br#"{"loggedIn":true,"authMethod":"third_party"}"#.as_slice(),
        br#"{"loggedIn":true,"authMethod":"api_key_helper"}"#,
        br#"{"loggedIn":"true","authMethod":"api_key"}"#,
    ] {
        assert_eq!(parse_status(&i, 0, bytes, b"").state, "unknown");
    }
}

#[cfg(windows)]
#[test]
fn nonpassive_status_paths_do_not_launch_a_process_or_create_client_state() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("work/proofs/accounts/nonpassive")
        .join(Uuid::new_v4().to_string());
    for client in ["claude_code", "opencode"] {
        let i = installation(client, "cli", "2.1.158");
        assert_eq!(
            super::platform::read(&root, std::path::Path::new("missing.exe"), &i).err(),
            Some("read_not_passive")
        );
    }
    assert!(!root.exists());
}

#[test]
fn stale_account_request_cannot_launch_or_read_any_executable() {
    let manager = AccountManager::new(None);
    let request = Request {
        schema_version: 1,
        request_id: Uuid::new_v4().to_string(),
        operation: Operation::Read,
        installation_id: Some(Uuid::new_v4().to_string()),
        scan_generation: Some(1),
        flow_id: None,
        confirm_profile_change: false,
    };
    let result = manager.execute(request, &DiscoveryManager::new());
    assert_eq!(result.error, Some("stale_selection"));
    assert!(result.account.is_none() && result.flow.is_none());
}
