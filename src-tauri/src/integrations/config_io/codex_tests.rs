use super::super::discovery::{
    DiscoveryInputs, DiscoveryManager, DiscoveryReply, InstallationSelectionError,
    InstallationSummary,
};
use super::codex::{build_snapshot, snapshot_for_selection, CodexStatusReply};
use super::connection_actions::OwnershipSummary;
use crate::runtime::manager::CodexRuntimeObservation;
use std::cell::Cell;

fn installation(version: &str, surface: &'static str) -> InstallationSummary {
    installation_for("codex", version, surface)
}

fn installation_for(
    client_kind: &'static str,
    version: &str,
    surface: &'static str,
) -> InstallationSummary {
    InstallationSummary {
        installation_id: "11111111-1111-4111-8111-111111111111".to_owned(),
        client_kind,
        surface,
        version: Some(version.to_owned()),
        source: "path",
        location_label: "PATH",
        status: "detected",
        connection_status: "not_verified",
    }
}

fn owner(endpoint_id: &str, port: u16) -> OwnershipSummary {
    OwnershipSummary {
        state: "active",
        owner_id: "22222222-2222-4222-8222-222222222222".to_owned(),
        revision: 1,
        record_fingerprint: "a".repeat(64),
        config_source: "codex_toml",
        endpoint_id: endpoint_id.to_owned(),
        endpoint_port: port,
    }
}

fn runtime(engine_ready: bool) -> CodexRuntimeObservation {
    CodexRuntimeObservation {
        endpoint_status: "ready",
        endpoint_id: Some("33333333-3333-4333-8333-333333333333".to_owned()),
        endpoint_port: Some(49_321),
        endpoint_ready: true,
        engine_status: if engine_ready { "ready" } else { "stopped" },
        engine_ready,
        runtime_generation: engine_ready.then(|| "44444444444444444444444444444444".to_owned()),
        active_connection_count: Some(2),
        error_code: None,
    }
}

#[test]
fn active_ownership_reports_same_endpoint_and_keeps_traffic_unverified() {
    let current = runtime(true);
    let result = build_snapshot(
        "55555555-5555-4555-8555-555555555555".to_owned(),
        installation("0.163.0-alpha.11.1", "cli"),
        Ok(Some(owner("33333333-3333-4333-8333-333333333333", 49_321))),
        current,
    );

    assert_eq!(result.ownership.state, "active");
    assert_eq!(result.ownership.endpoint_binding, "same");
    assert_eq!(result.ownership.config_source, Some("codex_toml"));
    assert!(result.runtime.stable_ingress_ready);
    assert!(result.runtime.engine_ready);
    assert_eq!(result.coverage.traffic_verification, "not_verified");
    assert_eq!(result.runtime.active_connection_count, Some(2));
    assert_eq!(result.installation.compatibility, "fixture_only");
    assert!(!result.policy.model_changed_by_headroom);
    assert!(!result.policy.reasoning_changed_by_headroom);
    assert!(!result.policy.auth_changed_by_headroom);
    assert_eq!(result.auth_scope.chatgpt_auth, "not_tested");
    assert_eq!(result.auth_scope.api_key_auth, "fixture_only");
    assert_eq!(result.coverage.main_agent, "unknown");
    assert_eq!(result.coverage.subagent, "not_tested");
}

#[test]
fn endpoint_mismatch_and_desktop_scope_remain_explicit() {
    let result = build_snapshot(
        "55555555-5555-4555-8555-555555555555".to_owned(),
        installation("0.163.0-alpha.11.1", "desktop"),
        Ok(Some(owner("66666666-6666-4666-8666-666666666666", 49_322))),
        runtime(false),
    );

    assert_eq!(result.installation.surface, "desktop");
    assert_eq!(result.installation.compatibility, "not_tested");
    assert_eq!(result.ownership.endpoint_binding, "different");
    assert!(!result.runtime.engine_ready);
    assert_eq!(result.coverage.traffic_verification, "not_verified");
}

#[test]
fn absent_ownership_does_not_claim_a_connection() {
    let result = build_snapshot(
        "55555555-5555-4555-8555-555555555555".to_owned(),
        installation("0.163.0-alpha.10.2", "cli"),
        Ok(None),
        runtime(false),
    );

    assert_eq!(result.ownership.state, "none");
    assert_eq!(result.ownership.endpoint_binding, "not_owned");
    assert_eq!(result.installation.compatibility, "fixture_only");
    assert_eq!(result.auth_scope.api_key_auth, "fixture_only");
    assert!(result.runtime.stable_ingress_ready);
    assert!(!result.runtime.engine_ready);
}

#[test]
fn fixture_only_scope_requires_exact_cli_fixture_version() {
    let cases = [
        ("0.163.0-alpha.11.1", "cli", "fixture_only", "fixture_only"),
        ("0.163.0-alpha.11.2", "cli", "not_verified", "not_tested"),
        ("0.163.0-alpha.10.2", "desktop", "not_tested", "not_tested"),
        (
            "0.163.0-alpha.10.2",
            "unknown",
            "not_verified",
            "not_tested",
        ),
    ];

    for (version, surface, compatibility, api_key_auth) in cases {
        let result = build_snapshot(
            "55555555-5555-4555-8555-555555555555".to_owned(),
            installation(version, surface),
            Ok(None),
            runtime(false),
        );
        assert_eq!(result.installation.compatibility, compatibility);
        assert_eq!(result.auth_scope.api_key_auth, api_key_auth);
    }

    let fixture = build_snapshot(
        "55555555-5555-4555-8555-555555555555".to_owned(),
        installation("0.163.0-alpha.10.2", "cli"),
        Ok(None),
        runtime(false),
    );
    assert_eq!(fixture.installation.compatibility, "fixture_only");
    assert_eq!(fixture.auth_scope.api_key_auth, "fixture_only");
}

#[test]
fn rejected_selection_never_builds_ownership_or_ingress_snapshot() {
    const REQUEST_ID: &str = "55555555-5555-4555-8555-555555555555";
    const INSTALLATION_ID: &str = "11111111-1111-4111-8111-111111111111";

    let discovery = DiscoveryManager::new();
    let first_generation =
        match discovery.scan_with_inputs(REQUEST_ID.to_owned(), DiscoveryInputs::default()) {
            DiscoveryReply::Snapshot(snapshot) => snapshot.scan_generation,
            DiscoveryReply::Error(_) | DiscoveryReply::Cancelled(_) => {
                panic!("isolated discovery fixture should scan")
            }
        };
    let current_generation =
        match discovery.scan_with_inputs(REQUEST_ID.to_owned(), DiscoveryInputs::default()) {
            DiscoveryReply::Snapshot(snapshot) => snapshot.scan_generation,
            DiscoveryReply::Error(_) | DiscoveryReply::Cancelled(_) => {
                panic!("isolated discovery fixture should rescan")
            }
        };
    assert!(current_generation > first_generation);

    let stale = discovery.select_installation(INSTALLATION_ID, first_generation);
    assert!(matches!(
        &stale,
        Err(InstallationSelectionError::StaleScanGeneration)
    ));
    assert_rejected_before_snapshot(stale, "stale_selection");

    let wrong_client = Ok(installation_for("claude_code", "0.163.0-alpha.10.2", "cli"));
    assert_rejected_before_snapshot(wrong_client, "client_mismatch");
}

fn assert_rejected_before_snapshot(
    selection: Result<InstallationSummary, InstallationSelectionError>,
    expected_code: &str,
) {
    let snapshot_builder_called = Cell::new(false);
    let reply = snapshot_for_selection(
        "55555555-5555-4555-8555-555555555555".to_owned(),
        selection,
        |_| {
            snapshot_builder_called.set(true);
            panic!("rejected selection must not reach ownership/runtime snapshot")
        },
    );
    assert!(!snapshot_builder_called.get());
    match reply {
        CodexStatusReply::Error(error) => {
            assert_eq!(error.error.code, expected_code);
            assert_eq!(error.status, "error");
        }
        CodexStatusReply::Snapshot(_) => panic!("selection error must not produce snapshot"),
    }
}

#[test]
fn ownership_read_failure_is_not_reported_as_no_connection() {
    let result = build_snapshot(
        "55555555-5555-4555-8555-555555555555".to_owned(),
        installation("0.163.0-alpha.11.1", "cli"),
        Err(()),
        runtime(false),
    );

    assert_eq!(result.ownership.state, "unavailable");
    assert_eq!(result.ownership.endpoint_binding, "unknown");
}

#[test]
fn stable_endpoint_binding_is_separate_from_engine_readiness() {
    let result = build_snapshot(
        "55555555-5555-4555-8555-555555555555".to_owned(),
        installation("0.163.0-alpha.11.1", "cli"),
        Ok(Some(owner("33333333-3333-4333-8333-333333333333", 49_321))),
        runtime(false),
    );

    assert_eq!(result.ownership.endpoint_binding, "same");
    assert!(result.runtime.stable_ingress_ready);
    assert_eq!(result.runtime.engine_status, "stopped");
    assert!(!result.runtime.engine_ready);
}

#[test]
fn status_request_rejects_unknown_fields() {
    let valid = serde_json::json!({
        "schema_version": 1,
        "request_id": "55555555-5555-4555-8555-555555555555",
        "installation_id": "11111111-1111-4111-8111-111111111111",
        "scan_generation": 1
    });
    assert!(serde_json::from_value::<super::codex::CodexStatusRequest>(valid.clone()).is_ok());

    let mut with_untrusted_value = valid;
    with_untrusted_value
        .as_object_mut()
        .expect("JSON fixture is an object")
        .insert(
            "config_path".to_owned(),
            serde_json::json!("C:\\private\\config"),
        );
    assert!(
        serde_json::from_value::<super::codex::CodexStatusRequest>(with_untrusted_value).is_err()
    );
}
