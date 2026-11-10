use super::*;
#[cfg(windows)]
use crate::runtime::ingress::StableIngress;
#[cfg(windows)]
use crate::runtime::ingress_relay::{IngressRelay, RelayLimits};
use std::io::Cursor;
use std::net::{Shutdown, TcpListener};
use std::sync::mpsc;

const REQUEST_ID: &str = "7132bced-3ce0-4c1b-8f48-d93d7b96f7ce";
const GENERATION: &str = "6132bced3ce04c1b8f48d93d7b96f7ce";

#[test]
fn windows_bundle_manifest_changes_only_payload_location() {
    let development: Value =
        serde_json::from_str(include_str!("../../../runtime/manifest.json")).unwrap();
    let windows_bytes = include_bytes!("../../../runtime/manifest.windows.json");
    let windows: Value = serde_json::from_slice(windows_bytes).unwrap();
    let mut expected = development.clone();
    expected["engine"]["payload_root"] = json!("payload");
    assert_eq!(windows, expected);
    assert_eq!(
        format!("{:x}", Sha256::digest(windows_bytes)),
        EXPECTED_WINDOWS_MANIFEST_SHA256
    );
    assert_eq!(
        validate_manifest(&serde_json::from_value(windows).unwrap()),
        Ok(())
    );
}

#[test]
fn windows_bundle_layout_uses_separate_state_and_rejects_a_development_manifest() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("work/proofs/windows-install/layouts")
        .join(Uuid::new_v4().to_string())
        .join("Kurulum çalışma");
    let resources = root.join("resources");
    let user_state = root.join("user state");
    fs::create_dir_all(resources.join("runtime")).unwrap();
    fs::create_dir_all(&user_state).unwrap();
    let manifest = resources.join("runtime/manifest.json");
    fs::write(&manifest, include_bytes!("../../../runtime/manifest.json")).unwrap();
    let layout = RuntimeLayout::from_bundle(resources.clone(), user_state.clone()).unwrap();
    assert_eq!(layout.manifest_sha256, EXPECTED_WINDOWS_MANIFEST_SHA256);
    assert!(!layout.state_root.starts_with(&resources));
    assert!(matches!(layout.verify(), Err(PublicError::ConfigConflict)));
    fs::write(
        manifest,
        include_bytes!("../../../runtime/manifest.windows.json"),
    )
    .unwrap();
    assert!(matches!(
        layout.verify(),
        Err(PublicError::EnvironmentBlocked)
    ));
    assert!(RuntimeLayout::from_bundle(root.join("missing"), user_state).is_err());
}

#[test]
#[ignore = "opt-in staged Windows bundle lifecycle; local readiness only, no provider calls"]
fn windows_bundle_real_staged_payload_starts_and_stops_without_development_layout() {
    fn files(root: &Path, folder: &Path, values: &mut std::collections::BTreeMap<PathBuf, String>) {
        for entry in fs::read_dir(folder).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                files(root, &path, values);
            } else {
                values.insert(
                    path.strip_prefix(root).unwrap().to_path_buf(),
                    sha256_file(&path).unwrap(),
                );
            }
        }
    }
    let project =
        fs::canonicalize(PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap()).unwrap();
    let resources = project.join("work/build/windows/resources");
    let state = project
        .join("work/proofs/windows-install/bundle-native")
        .join(Uuid::new_v4().to_string())
        .join("Kullanıcı çalışma state");
    fs::create_dir_all(&state).unwrap();
    let layout = RuntimeLayout::from_bundle(resources.clone(), state.clone()).unwrap();
    let mut before = std::collections::BTreeMap::new();
    files(&resources, &resources, &mut before);
    let verified = layout.verify().expect("strict staged bundle pins");
    assert_eq!(
        verified.payload_root,
        fs::canonicalize(resources.join("runtime/payload")).unwrap()
    );
    assert!(verified
        .state_root
        .starts_with(fs::canonicalize(&state).unwrap()));
    assert!(!verified.state_root.starts_with(&resources));
    let manager = RuntimeManager::from_layout(Ok(layout));
    let foreign = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let foreign_port = foreign.local_addr().unwrap().port();
    let started = manager.start_engine(REQUEST_ID.to_owned());
    let generation = started.generation_for_test().expect("bundled engine ready");
    assert_eq!(started.status_for_test(), Some("ready"));
    assert_optimization(&started, "enabled", Some(0), true);
    let stopped = manager.stop_engine(Uuid::new_v4().to_string(), generation);
    assert_eq!(stopped.status_for_test(), Some("stopped"));
    assert!(TcpStream::connect((Ipv4Addr::LOCALHOST, foreign_port)).is_ok());
    manager.shutdown();
    let mut after = std::collections::BTreeMap::new();
    files(&resources, &resources, &mut after);
    assert_eq!(before, after, "bundle resources must remain read-only");
}

#[test]
fn recovery_real_manager_failure_snapshots_do_not_start_repair_or_touch_another_listener() {
    use crate::integrations::{config_io::PlanManager, discovery::DiscoveryManager};
    use crate::metrics::{collector::OverviewManager, store::MetricsStore};
    use crate::runtime::recovery::{RecoveryManager, Request};
    let runtime = RuntimeManager::from_layout(Err(PublicError::EnvironmentBlocked));
    let discovery = DiscoveryManager::new();
    let plans = PlanManager::new(discovery.clone(), runtime.clone());
    let metrics_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("work/proofs/recovery/manager-tests")
        .join(Uuid::new_v4().to_string());
    let overview = OverviewManager::new(runtime.clone(), MetricsStore::in_directory(&metrics_path));
    let other_listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = other_listener.local_addr().unwrap();
    let recovery = RecoveryManager::default();
    for (status, code, expected) in [
        (
            EngineStatus::Unavailable,
            PublicError::EnvironmentBlocked,
            "unavailable",
        ),
        (EngineStatus::Failed, PublicError::OwnershipLost, "failed"),
        (EngineStatus::Unknown, PublicError::Timeout, "unknown"),
    ] {
        {
            let mut view = runtime.view.write().unwrap();
            *view = RuntimeView::new(status);
            view.error_code = Some(code.code());
        }
        let id = Uuid::new_v4().to_string();
        let wire = serde_json::to_value(recovery.read(
            Request {
                schema_version: 1,
                request_id: id.clone(),
            },
            &runtime,
            &discovery,
            &plans,
            &overview,
        ))
        .unwrap();
        assert_eq!(wire["request_id"], id);
        assert_eq!(wire["status"], "snapshot");
        assert_eq!(wire["runtime"]["engine_status"], expected);
        assert_eq!(wire["runtime"]["error_code"], code.code());
        assert_eq!(wire["traffic"]["status"], "none");
        assert_eq!(wire["traffic"]["outcome"], "unknown");
        assert_eq!(wire["clients"].as_array().unwrap().len(), 3);
        assert!(
            wire["clients"]
                .as_array()
                .unwrap()
                .iter()
                .all(|c| c["installation_status"] == "unknown"
                    && c["compatibility"] == "not_verified")
        );
        assert!(runtime.view.read().unwrap().runtime_generation.is_none());
        assert!(std::net::TcpStream::connect(address).is_ok());
        let encoded = wire.to_string();
        for forbidden in [
            "api_key",
            "auth.json",
            "config.toml",
            "runtime_generation",
            "endpoint_id",
            "PRIVATE",
        ] {
            assert!(!encoded.contains(forbidden));
        }
    }
    let invalid = serde_json::to_value(recovery.read(
        Request {
            schema_version: 2,
            request_id: REQUEST_ID.into(),
        },
        &runtime,
        &discovery,
        &plans,
        &overview,
    ))
    .unwrap();
    assert_eq!(invalid["error"]["code"], "invalid_input");
    assert!(overview.shutdown());
    assert!(runtime.shutdown());
    assert!(std::net::TcpStream::connect(address).is_ok());
}

struct LiveRunCleanup {
    manager: RuntimeManager,
    run_root: PathBuf,
    runs_root: PathBuf,
    complete: bool,
}

impl LiveRunCleanup {
    fn finish(&mut self) -> bool {
        let manager_stopped = self.manager.shutdown();
        let run_removed = manager_stopped && remove_owned_run(&self.run_root, &self.runs_root);
        self.complete = manager_stopped && run_removed;
        self.complete
    }
}

impl Drop for LiveRunCleanup {
    fn drop(&mut self) {
        if !self.complete && self.manager.shutdown() {
            let _ = remove_owned_run(&self.run_root, &self.runs_root);
        }
    }
}

fn remove_owned_run(run_root: &Path, runs_root: &Path) -> bool {
    if run_root.parent() != Some(runs_root) {
        return false;
    }
    let root_metadata = match fs::symlink_metadata(run_root) {
        Ok(value) => value,
        Err(_) => return false,
    };
    if !root_metadata.is_dir() || path_is_reparse(&root_metadata) {
        return false;
    }
    let (Ok(root), Ok(runs)) = (fs::canonicalize(run_root), fs::canonicalize(runs_root)) else {
        return false;
    };
    if root.parent() != Some(runs.as_path()) || path_is_reparse(&root_metadata) {
        return false;
    }
    let mut pending = vec![root.clone()];
    while let Some(directory) = pending.pop() {
        let entries = match fs::read_dir(&directory) {
            Ok(value) => value,
            Err(_) => return false,
        };
        for entry in entries {
            let Ok(entry) = entry else { return false };
            let path = entry.path();
            let Ok(metadata) = fs::symlink_metadata(&path) else {
                return false;
            };
            if path_is_reparse(&metadata) {
                return false;
            }
            let Ok(canonical) = fs::canonicalize(&path) else {
                return false;
            };
            if !canonical.starts_with(&root) {
                return false;
            }
            if metadata.is_dir() {
                pending.push(path);
            } else if !metadata.is_file() {
                return false;
            }
        }
    }
    fs::remove_dir_all(run_root).is_ok()
}

#[cfg(windows)]
fn path_is_reparse(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    metadata.file_attributes()
        & windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT
        != 0
}

#[cfg(not(windows))]
fn path_is_reparse(metadata: &fs::Metadata) -> bool {
    metadata.file_type().is_symlink()
}

fn is_reparse_free_path(path: &Path) -> bool {
    path.ancestors()
        .all(|component| match fs::symlink_metadata(component) {
            Ok(metadata) => !path_is_reparse(&metadata),
            Err(error) => error.kind() == std::io::ErrorKind::NotFound,
        })
}

fn ready_frame() -> Vec<u8> {
    let value = json!({
        "protocol_version": 1,
        "request_id": REQUEST_ID,
        "runtime_generation": GENERATION,
        "event": "ready",
        "helper_pid": 301,
        "engine_pid": 302,
        "listener_port": 48631,
        "listener_pid": 302,
        "engine_version": "0.41.0",
        "controller_consoleless": true,
        "manager_hidden_console": true,
        "engine_console_allocated": true,
        "config": {
            "mode": "cache",
            "savings_profile": "coding",
            "ttl_seconds": 7200,
            "telemetry_enabled": true,
            "protected_tools": [],
            "beacon_enabled": false,
            "memory_enabled": false,
            "learning_enabled": false,
            "output_shaper_enabled": false,
            "model_router_enabled": false
        },
        "optimization_control":{"protocol_version":1,"control_token":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","module_sha256":EXPECTED_OPTIMIZATION_SHA256,"sources_sha256":EXPECTED_OPTIMIZATION_SOURCES_SHA256},
        "shim": {
            "active": true,
            "version": "0.41.0",
            "prefix_tracker_sha256": EXPECTED_PREFIX_SHA256,
            "shim_sha256": EXPECTED_SHIM_SHA256
        }
    });
    let mut bytes = serde_json::to_vec(&value).unwrap();
    bytes.push(b'\n');
    bytes
}

#[test]
fn ready_parser_accepts_only_pinned_owned_readiness() {
    let parsed = parse_controller_ready(&ready_frame(), REQUEST_ID, 300).unwrap();
    assert_eq!(parsed.generation, GENERATION);
    assert_eq!(parsed.engine_pid, 302);
    assert_eq!(parsed.listener_port, 48631);

    let mut value: Value = serde_json::from_slice(&ready_frame()).unwrap();
    value["config"]["ttl_seconds"] = json!(1800);
    let mut raw = serde_json::to_vec(&value).unwrap();
    raw.push(b'\n');
    assert_eq!(
        parse_controller_ready(&raw, REQUEST_ID, 300),
        Err(PublicError::ConfigConflict)
    );

    value["config"]["ttl_seconds"] = json!(7200);
    value["unreviewed"] = json!(true);
    let mut raw = serde_json::to_vec(&value).unwrap();
    raw.push(b'\n');
    assert_eq!(
        parse_controller_ready(&raw, REQUEST_ID, 300),
        Err(PublicError::UnsupportedProtocol)
    );
}

#[test]
fn ready_parser_rejects_wrong_listener_request_or_engine_owner() {
    let mut value: Value = serde_json::from_slice(&ready_frame()).unwrap();
    value["listener_pid"] = json!(303);
    let mut raw = serde_json::to_vec(&value).unwrap();
    raw.push(b'\n');
    assert_eq!(
        parse_controller_ready(&raw, REQUEST_ID, 300),
        Err(PublicError::EngineNotReady)
    );

    value["listener_pid"] = json!(302);
    value["request_id"] = json!("7132bced-3ce0-4c1b-8f48-d93d7b96f7cf");
    let mut raw = serde_json::to_vec(&value).unwrap();
    raw.push(b'\n');
    assert_eq!(
        parse_controller_ready(&raw, REQUEST_ID, 300),
        Err(PublicError::UnsupportedProtocol)
    );
}

#[test]
fn frame_reader_contract_rejects_partial_oversize_and_wrong_generation() {
    assert_eq!(
        parse_json_frame(b"{}"),
        Err(PublicError::UnsupportedProtocol)
    );
    let oversized = vec![b'x'; MAX_HELPER_FRAME_BYTES + 1];
    assert_eq!(
        parse_json_frame(&oversized),
        Err(PublicError::UnsupportedProtocol)
    );

    let stopped = json!({
        "protocol_version": 1,
        "request_id": REQUEST_ID,
        "runtime_generation": "7132bced3ce04c1b8f48d93d7b96f7ce",
        "event": "stopped",
        "shutdown": "graceful",
        "engine_exit_code": 0,
        "owned_port_closed": true,
        "hidden_console_verified": true,
        "lifespan_complete": true,
        "proxy_summary": true,
        "force_fallback": false
    });
    let mut raw = serde_json::to_vec(&stopped).unwrap();
    raw.push(b'\n');
    assert_eq!(
        parse_controller_stopped(&raw, REQUEST_ID, GENERATION),
        Err(PublicError::OwnershipLost)
    );

    assert_eq!(
        parse_json_frame(b"{\"event\":\"ready\",\"event\":\"stopped\"}\n"),
        Err(PublicError::UnsupportedProtocol)
    );
    assert_eq!(
        parse_json_frame(b"{\"nested\":{\"value\":1,\"value\":2}}\n"),
        Err(PublicError::UnsupportedProtocol)
    );
    assert_eq!(
        read_bounded_frame(&mut Cursor::new(b"{\"partial\":")),
        Err(ReaderMessage::Invalid)
    );
}

#[test]
fn stalled_frame_receive_and_reader_join_have_deadlines() {
    let (_sender, receiver) = mpsc::sync_channel(1);
    let started = Instant::now();
    assert_eq!(
        receive_frame(&receiver, Instant::now() + Duration::from_millis(15), None),
        Err(PublicError::Timeout)
    );
    assert!(started.elapsed() < Duration::from_secs(1));

    let thread = thread::spawn(|| thread::sleep(Duration::from_secs(2)));
    let started = Instant::now();
    assert!(!join_reader_bounded(thread));
    assert!(started.elapsed() < Duration::from_secs(1));
}

#[test]
fn backend_lease_keeps_three_expected_process_guards_with_owned_console_members() {
    assert!(backend_lease_handles_valid(true, true, [true; 3]));
    assert!(!backend_lease_handles_valid(false, true, [true; 3]));
    assert!(!backend_lease_handles_valid(true, false, [true; 3]));
    for index in 0..3 {
        let mut handles_live = [true; 3];
        handles_live[index] = false;
        assert!(!backend_lease_handles_valid(true, true, handles_live));
    }

    assert!(backend_job_membership_valid(Some(5), [true; 3]));
    assert!(!backend_job_membership_valid(None, [true; 3]));
    assert!(!backend_job_membership_valid(Some(2), [true; 3]));
    for index in 0..3 {
        let mut members = [true; 3];
        members[index] = false;
        assert!(!backend_job_membership_valid(Some(5), members));
    }
}

#[cfg(windows)]
#[test]
fn owned_job_reports_empty_after_bounded_termination() {
    let job = OwnedJob::create().unwrap();
    assert_eq!(job.active_processes(), Some(0));
    assert!(job.terminate_and_wait_empty(Duration::from_millis(200)));
}

#[test]
fn health_probe_selects_pinned_readiness_fields_and_ignores_large_diagnostics() {
    let mut body = json!({
        "service": "headroom-proxy",
        "status": "healthy",
        "ready": true,
        "version": "0.41.0",
        "timestamp": "2026-10-20T00:00:00Z",
        "uptime_seconds": 1.25,
        "checks": {"worker": {"ready": true, "detail": "x".repeat(4096)}},
        "runtime": {"private": "discarded"},
        "rust_core": "loaded"
    });
    let encoded = serde_json::to_vec(&body).unwrap();
    assert!(encoded.len() > 2048);
    assert!(health_payload_matches(&encoded, "0.41.0"));

    body["service"] = json!("other-service");
    assert!(!health_payload_matches(
        &serde_json::to_vec(&body).unwrap(),
        "0.41.0"
    ));
    body["service"] = json!("headroom-proxy");
    body["status"] = json!("unhealthy");
    assert!(!health_payload_matches(
        &serde_json::to_vec(&body).unwrap(),
        "0.41.0"
    ));
    body["status"] = json!("healthy");
    body["version"] = json!("0.42.0");
    assert!(!health_payload_matches(
        &serde_json::to_vec(&body).unwrap(),
        "0.41.0"
    ));
    assert!(!health_payload_matches(
        br#"{"service":"headroom-proxy","status":"healthy","ready":true,"ready":false,"version":"0.41.0"}"#,
        "0.41.0"
    ));
}

#[test]
fn owned_process_chain_rejects_same_port_listener_from_another_tree() {
    let parents = std::collections::HashMap::from([(400, 300), (500, 400)]);
    assert!(process_chain_matches(&parents, 300, 400, 500));
    assert!(!process_chain_matches(&parents, 300, 401, 500));
    assert!(!process_chain_matches(&parents, 300, 400, 501));
}

#[cfg(windows)]
#[test]
fn tcp_owner_check_rejects_foreign_same_port_listener() {
    use windows_sys::Win32::NetworkManagement::IpHelper::MIB_TCPROW_OWNER_PID;
    let port = 48631u16;
    let rows = [MIB_TCPROW_OWNER_PID {
        dwLocalAddr: u32::from_ne_bytes(std::net::Ipv4Addr::LOCALHOST.octets()),
        dwLocalPort: u32::from(port.to_be()),
        dwOwningPid: 500,
        ..Default::default()
    }];
    assert_eq!(tcp_rows_owner(&rows, port, true), Ok(Some(500)));
    assert!(!owner_is_gone(Ok(Some(500)), 500));
    assert!(owner_is_gone(Ok(None), 500));
    assert!(owner_is_gone(Ok(Some(501)), 500));
    assert!(!owner_is_gone(Err(()), 500));
    assert_eq!(tcp_rows_owner(&[], port, false), Ok(None));
    let foreign = [
        rows[0],
        MIB_TCPROW_OWNER_PID {
            dwLocalAddr: 0,
            dwLocalPort: u32::from(port.to_be()),
            dwOwningPid: 501,
            ..Default::default()
        },
    ];
    assert_eq!(tcp_rows_owner(&foreign, port, false), Err(()));
    let wildcard_only = [MIB_TCPROW_OWNER_PID {
        dwLocalAddr: 0,
        dwLocalPort: u32::from(port.to_be()),
        dwOwningPid: 501,
        ..Default::default()
    }];
    assert_eq!(tcp_rows_owner(&wildcard_only, port, true), Ok(None));
    assert_eq!(tcp_rows_owner(&wildcard_only, port, false), Ok(Some(501)));
}

#[test]
fn runtime_manifest_rejects_profile_drift_and_unknown_fields() {
    let project = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf();
    let manifest = fs::read(project.join("runtime/manifest.json")).unwrap();
    let parsed: RuntimeManifest = serde_json::from_slice(&manifest).unwrap();
    assert_eq!(validate_manifest(&parsed), Ok(()));

    let mut value: Value = serde_json::from_slice(&manifest).unwrap();
    value["profile"]["ccr_ttl_seconds"] = json!(1800);
    let drifted: RuntimeManifest = serde_json::from_value(value).unwrap();
    assert_eq!(
        validate_manifest(&drifted),
        Err(PublicError::ConfigConflict)
    );

    let mut value: Value = serde_json::from_slice(&manifest).unwrap();
    value["profile"]["unreviewed"] = json!(true);
    assert!(serde_json::from_value::<RuntimeManifest>(value).is_err());
}

#[test]
fn compatibility_engine_pin_rejects_version_transition_and_accepts_restored_pin() {
    let original = include_bytes!("../../../runtime/manifest.json");
    let mut candidate: Value = serde_json::from_slice(original).unwrap();
    for version in ["0.42.0", "0.40.0", "latest"] {
        candidate["engine"]["version"] = json!(version);
        let parsed: RuntimeManifest = serde_json::from_value(candidate.clone()).unwrap();
        assert_eq!(validate_manifest(&parsed), Err(PublicError::ConfigConflict));
    }
    let restored: RuntimeManifest = serde_json::from_slice(original).unwrap();
    assert_eq!(validate_manifest(&restored), Ok(()));
    assert_eq!(
        format!("{:x}", Sha256::digest(original)),
        EXPECTED_MANIFEST_SHA256
    );
}

#[test]
fn unavailable_layout_and_generation_bound_stop_are_safe() {
    let project =
        fs::canonicalize(PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap()).unwrap();
    let missing_runtime = project.join("work/proofs/runtime-manager/not-installed-runtime");
    assert!(matches!(
        RuntimeLayout::from_roots(
            project.clone(),
            missing_runtime,
            project.join("work/app-state/runtime")
        ),
        Err(PublicError::EnvironmentBlocked)
    ));

    let unavailable = RuntimeManager::from_layout(Err(PublicError::EnvironmentBlocked));
    let reply = unavailable.get_runtime_snapshot(REQUEST_ID.to_owned());
    match reply {
        RuntimeReply::Snapshot(snapshot) => {
            assert_eq!(snapshot.engine.status, "unavailable");
            assert!(!snapshot.capabilities.start);
            assert!(!snapshot.capabilities.stop);
        }
        RuntimeReply::Error(_) => panic!("expected unavailable snapshot"),
    }
    assert_eq!(
        unavailable
            .stop_engine(REQUEST_ID.to_owned(), GENERATION.to_owned())
            .error_code_for_test(),
        Some("ownership_lost")
    );
    assert!(unavailable.shutdown());
}

#[test]
fn stable_ingress_port_conflict_blocks_engine_start_and_preserves_foreign_listener() {
    let project =
        fs::canonicalize(PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap()).unwrap();
    let runs_root = project.join("work/proofs/runtime-manager/runs");
    assert!(is_reparse_free_path(&runs_root));
    ensure_existing_ancestor_inside(&project, &runs_root).unwrap();
    fs::create_dir_all(&runs_root).unwrap();
    let runs_root = fs::canonicalize(runs_root).unwrap();
    assert!(runs_root.starts_with(&project) && is_reparse_free_path(&runs_root));
    let run_root = runs_root.join(Uuid::new_v4().to_string());
    assert_eq!(run_root.parent(), Some(runs_root.as_path()));
    fs::create_dir(&run_root).unwrap();

    let foreign_listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let foreign_port = foreign_listener.local_addr().unwrap().port();
    let foreign_pid = listener_owner(foreign_port).expect("foreign listener owner");
    let endpoint_record = json!({
        "schema_version": 1,
        "bind_address": "127.0.0.1",
        "port": foreign_port,
        "endpoint_id": Uuid::new_v4().to_string(),
        "revision": 1
    });
    fs::write(
        run_root.join("connection-endpoint.json"),
        serde_json::to_vec(&endpoint_record).unwrap(),
    )
    .unwrap();

    let layout =
        RuntimeLayout::from_roots(project.clone(), project.join("runtime"), run_root.clone())
            .unwrap();
    let manager = RuntimeManager::from_layout(Ok(layout));
    let mut cleanup = LiveRunCleanup {
        manager: manager.clone(),
        run_root: run_root.clone(),
        runs_root: runs_root.clone(),
        complete: false,
    };

    let started = manager.start_engine(REQUEST_ID.to_owned());
    assert_eq!(started.error_code_for_test(), Some("port_conflict"));
    assert_eq!(manager.listener_port_for_test(), None);
    let RuntimeReply::Snapshot(runtime) = manager.get_runtime_snapshot(REQUEST_ID.to_owned())
    else {
        panic!("expected unavailable engine snapshot");
    };
    assert_eq!(runtime.engine.status, "unavailable");
    assert_eq!(runtime.engine.error_code, Some("port_conflict"));
    let IngressReply::Snapshot(ingress) =
        manager.get_ingress_snapshot("8132bced-3ce0-4c1b-8f48-d93d7b96f7ce".to_owned())
    else {
        panic!("expected stable endpoint conflict snapshot");
    };
    assert_eq!(ingress.endpoint.status, "conflict");
    assert_eq!(ingress.endpoint.error_code, Some("port_conflict"));
    assert!(ingress.endpoint.address.is_none());
    assert_eq!(ingress.backend.status, "unavailable");

    let connection = TcpStream::connect_timeout(
        &SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), foreign_port),
        HEALTH_IO_TIMEOUT,
    )
    .expect("foreign listener remains connectable");
    drop(connection);
    assert_eq!(listener_owner(foreign_port), Some(foreign_pid));
    assert!(cleanup.finish());
    drop(foreign_listener);
}

#[test]
fn uncertain_cleanup_disables_start_and_exposes_only_fixed_error_code() {
    let mut view = RuntimeView::new(EngineStatus::Failed);
    view.error_code = Some("cleanup_failed");
    view.cleanup_uncertain = true;
    let RuntimeReply::Snapshot(snapshot) = view.snapshot(REQUEST_ID.to_owned()) else {
        panic!("expected failure snapshot");
    };
    assert!(!snapshot.capabilities.start);
    assert_eq!(snapshot.engine.error_code, Some("cleanup_failed"));
}

#[test]
fn state_paths_reject_a_resolved_root_outside_the_trusted_boundary() {
    let boundary =
        fs::canonicalize(PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap()).unwrap();
    let outside = fs::canonicalize(std::env::temp_dir()).unwrap();
    assert!(!outside.starts_with(&boundary));
    assert_eq!(
        ensure_existing_ancestor_inside(&boundary, &outside.join("headroom-never-created")),
        Err(PublicError::EnvironmentBlocked)
    );
}

#[test]
fn expired_ready_snapshot_becomes_unknown_and_recovers_after_fresh_health() {
    let mut view = RuntimeView::new(EngineStatus::Ready);
    view.runtime_generation = Some(GENERATION.to_owned());
    view.engine_version = Some(EXPECTED_ENGINE_VERSION.to_owned());
    view.listener_port = Some(48631);
    view.last_health = Some(Instant::now() - HEALTH_EXPIRY - Duration::from_secs(1));
    let shared_view = Arc::new(RwLock::new(view));
    let (sender, _receiver) = mpsc::sync_channel(1);
    let manager = RuntimeManager {
        sender,
        view: Arc::clone(&shared_view),
        ingress_view: Arc::new(RwLock::new(IngressView::starting())),
        shutting_down: Arc::new(AtomicBool::new(false)),
        update_start_gate: Arc::new(std::sync::Mutex::new(false)),
        actor_available: true,
    };
    let RuntimeReply::Snapshot(stale) = manager.get_runtime_snapshot(REQUEST_ID.to_owned()) else {
        panic!("expected stale snapshot");
    };
    assert_eq!(stale.engine.status, "unknown");
    assert_eq!(stale.engine.error_code, Some("timeout"));
    assert!(stale.capabilities.stop);

    mark_ready_view(
        &shared_view,
        GENERATION,
        48631,
        302,
        Instant::now(),
        None,
        Vec::new(),
    );
    let fresh = manager.get_runtime_snapshot("8132bced-3ce0-4c1b-8f48-d93d7b96f7ce".to_owned());
    assert_eq!(fresh.status_for_test(), Some("ready"));
}

#[test]
fn ingress_snapshot_is_narrow_and_only_reports_matching_ready_generation() {
    let mut ingress = IngressView::starting();
    ingress.endpoint_status = "ready";
    ingress.endpoint_id = Some("b3d3d7d7-2361-47b4-98ed-1ea94cfb74f0".to_owned());
    ingress.address = Some(IngressAddressValue {
        host: "127.0.0.1",
        port: 41237,
    });
    ingress.route_status = "ready";
    ingress.route_generation = Some(GENERATION.to_owned());

    let mut runtime = RuntimeView::new(EngineStatus::Ready);
    runtime.runtime_generation = Some(GENERATION.to_owned());
    let IngressReply::Snapshot(snapshot) = ingress.snapshot(REQUEST_ID.to_owned(), &runtime) else {
        panic!("expected ingress snapshot");
    };
    assert_eq!(snapshot.endpoint.status, "ready");
    assert_eq!(snapshot.backend.status, "ready");
    assert_eq!(snapshot.backend.generation.as_deref(), Some(GENERATION));
    assert_eq!(snapshot.active_connection_count, Some(0));
    let encoded = serde_json::to_value(snapshot).unwrap();
    assert_eq!(
        encoded
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect::<std::collections::BTreeSet<_>>(),
        [
            "active_connection_count",
            "backend",
            "endpoint",
            "observed_at_utc",
            "request_id",
            "schema_version",
            "snapshot_id",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect()
    );
    let serialized = encoded.to_string();
    assert!(!serialized.contains("engine_pid"));
    assert!(!serialized.contains("helper_pid"));
    assert!(!serialized.contains("listener_port"));

    runtime.runtime_generation = Some("7132bced3ce04c1b8f48d93d7b96f7ce".to_owned());
    let IngressReply::Snapshot(mismatched) = ingress.snapshot(REQUEST_ID.to_owned(), &runtime)
    else {
        panic!("expected ingress snapshot");
    };
    assert_eq!(mismatched.backend.status, "unknown");
    assert_eq!(mismatched.backend.generation, None);
    assert_eq!(mismatched.backend.error_code, Some("ownership_lost"));
}

#[test]
fn ingress_snapshot_rejects_invalid_request_correlation() {
    let manager = RuntimeManager::from_layout(Err(PublicError::EnvironmentBlocked));
    let reply = manager.get_ingress_snapshot("not-a-uuid".to_owned());
    match reply {
        IngressReply::Error(error) => {
            assert_eq!(error.status, "error");
            assert_eq!(error.request_id, "");
            assert_eq!(error.error.code, "validation_failed");
            assert_eq!(error.error.message_key, "runtime.command.validation_failed");
        }
        IngressReply::Snapshot(_) => panic!("invalid correlation must fail"),
    }
    assert!(manager.shutdown());
}

fn ready_test_ingress_view() -> IngressView {
    let mut ingress = IngressView::starting();
    ingress.endpoint_status = "ready";
    ingress.endpoint_id = Some("b3d3d7d7-2361-47b4-98ed-1ea94cfb74f0".to_owned());
    ingress.address = Some(IngressAddressValue {
        host: "127.0.0.1",
        port: 41237,
    });
    ingress.last_endpoint_observation = Some(Instant::now());
    ingress
}

fn manager_for_endpoint_test(runtime: RuntimeView, ingress: IngressView) -> RuntimeManager {
    let (sender, receiver) = mpsc::sync_channel(1);
    drop(receiver);
    RuntimeManager {
        sender,
        view: Arc::new(RwLock::new(runtime)),
        ingress_view: Arc::new(RwLock::new(ingress)),
        shutting_down: Arc::new(AtomicBool::new(false)),
        update_start_gate: Arc::new(std::sync::Mutex::new(false)),
        actor_available: false,
    }
}

#[cfg(windows)]
#[test]
fn update_reservation_requires_a_clean_stop_and_excludes_another_reservation() {
    let mut manager = manager_for_endpoint_test(
        RuntimeView::new(EngineStatus::Stopped),
        IngressView::starting(),
    );
    manager.actor_available = true;
    assert!(manager.reserve_for_update());
    assert!(!manager.reserve_for_update());
    manager.release_update_reservation();
    manager.view.write().unwrap().cleanup_uncertain = true;
    assert!(!manager.reserve_for_update());
    manager.view.write().unwrap().cleanup_uncertain = false;
    manager
        .ingress_view
        .write()
        .unwrap()
        .active_connection_count = Some(1);
    assert!(!manager.reserve_for_update());
    manager
        .ingress_view
        .write()
        .unwrap()
        .active_connection_count = Some(0);
    assert!(manager.reserve_for_update());
}

#[test]
fn stable_endpoint_accessor_allows_fresh_endpoint_when_engine_is_stopped() {
    let manager = manager_for_endpoint_test(
        RuntimeView::new(EngineStatus::Stopped),
        ready_test_ingress_view(),
    );
    let endpoint = manager
        .ready_stable_endpoint()
        .expect("the stable ingress stays selectable while the engine is stopped");
    assert_eq!(endpoint.endpoint_id, "b3d3d7d7-2361-47b4-98ed-1ea94cfb74f0");
    assert_eq!(endpoint.host, "127.0.0.1");
    assert_eq!(endpoint.port, 41237);
}

#[test]
fn stable_endpoint_accessor_requires_matching_fresh_ready_generation() {
    let mut runtime = RuntimeView::new(EngineStatus::Ready);
    runtime.runtime_generation = Some(GENERATION.to_owned());
    runtime.last_health = Some(Instant::now());
    let mut ingress = ready_test_ingress_view();
    ingress.route_status = "ready";
    ingress.route_generation = Some(GENERATION.to_owned());
    let manager = manager_for_endpoint_test(runtime.clone(), ingress.clone());
    assert!(manager.ready_stable_endpoint().is_ok());

    ingress.route_generation = Some("7132bced3ce04c1b8f48d93d7b96f7ce".to_owned());
    let mismatch = manager_for_endpoint_test(runtime.clone(), ingress);
    assert_eq!(
        mismatch.ready_stable_endpoint(),
        Err(StableEndpointAccessError::BackendNotReady)
    );

    runtime.last_health = Some(Instant::now() - HEALTH_EXPIRY - Duration::from_secs(1));
    let expired = manager_for_endpoint_test(runtime, {
        let mut ingress = ready_test_ingress_view();
        ingress.route_status = "ready";
        ingress.route_generation = Some(GENERATION.to_owned());
        ingress
    });
    assert_eq!(
        expired.ready_stable_endpoint(),
        Err(StableEndpointAccessError::BackendNotReady)
    );
}

#[test]
fn stable_endpoint_accessor_rejects_stale_or_failed_endpoint_observation() {
    let mut stale = ready_test_ingress_view();
    stale.last_endpoint_observation = Some(Instant::now() - HEALTH_EXPIRY - Duration::from_secs(1));
    let stale_manager = manager_for_endpoint_test(RuntimeView::new(EngineStatus::Stopped), stale);
    assert_eq!(
        stale_manager.ready_stable_endpoint(),
        Err(StableEndpointAccessError::Stale)
    );

    let mut failed = ready_test_ingress_view();
    failed.endpoint_error_code = Some("environment_blocked");
    let failed_manager = manager_for_endpoint_test(RuntimeView::new(EngineStatus::Stopped), failed);
    assert_eq!(
        failed_manager.ready_stable_endpoint(),
        Err(StableEndpointAccessError::Unavailable)
    );
}

#[test]
fn stable_endpoint_accessor_rejects_invalid_id_or_non_loopback_address() {
    let mut invalid_id = ready_test_ingress_view();
    invalid_id.endpoint_id = Some("not-a-uuid".to_owned());
    let invalid_id_manager =
        manager_for_endpoint_test(RuntimeView::new(EngineStatus::Stopped), invalid_id);
    assert_eq!(
        invalid_id_manager.ready_stable_endpoint(),
        Err(StableEndpointAccessError::Invalid)
    );

    let mut invalid_host = ready_test_ingress_view();
    invalid_host.address.as_mut().unwrap().host = "0.0.0.0";
    let invalid_host_manager =
        manager_for_endpoint_test(RuntimeView::new(EngineStatus::Stopped), invalid_host);
    assert_eq!(
        invalid_host_manager.ready_stable_endpoint(),
        Err(StableEndpointAccessError::Invalid)
    );

    let mut invalid_port = ready_test_ingress_view();
    invalid_port.address.as_mut().unwrap().port = 0;
    let invalid_port_manager =
        manager_for_endpoint_test(RuntimeView::new(EngineStatus::Stopped), invalid_port);
    assert_eq!(
        invalid_port_manager.ready_stable_endpoint(),
        Err(StableEndpointAccessError::Invalid)
    );
}

#[test]
fn ingress_snapshot_downgrades_expired_ready_health_and_hides_stale_count() {
    let mut runtime_view = RuntimeView::new(EngineStatus::Ready);
    runtime_view.runtime_generation = Some(GENERATION.to_owned());
    runtime_view.engine_version = Some(EXPECTED_ENGINE_VERSION.to_owned());
    runtime_view.last_health = Some(Instant::now() - HEALTH_EXPIRY - Duration::from_secs(1));
    let shared_runtime = Arc::new(RwLock::new(runtime_view));
    let mut ingress_view = IngressView::starting();
    ingress_view.endpoint_status = "ready";
    ingress_view.endpoint_id = Some("b3d3d7d7-2361-47b4-98ed-1ea94cfb74f0".to_owned());
    ingress_view.address = Some(IngressAddressValue {
        host: "127.0.0.1",
        port: 41237,
    });
    ingress_view.route_status = "ready";
    ingress_view.route_generation = Some(GENERATION.to_owned());
    ingress_view.last_endpoint_observation = Some(Instant::now());
    ingress_view.active_connection_count = Some(2);
    let (sender, receiver) = mpsc::sync_channel(1);
    drop(receiver);
    let manager = RuntimeManager {
        sender,
        view: shared_runtime,
        ingress_view: Arc::new(RwLock::new(ingress_view)),
        shutting_down: Arc::new(AtomicBool::new(false)),
        update_start_gate: Arc::new(std::sync::Mutex::new(false)),
        actor_available: false,
    };

    let IngressReply::Snapshot(snapshot) = manager.get_ingress_snapshot(REQUEST_ID.to_owned())
    else {
        panic!("expected ingress snapshot");
    };
    assert_eq!(snapshot.endpoint.status, "ready");
    assert_eq!(snapshot.backend.status, "unknown");
    assert_eq!(snapshot.backend.error_code, Some("timeout"));
    assert_eq!(snapshot.active_connection_count, None);
    assert_eq!(
        manager.view.read().unwrap().engine_version.as_deref(),
        Some(EXPECTED_ENGINE_VERSION)
    );
}

#[test]
fn expired_endpoint_observation_becomes_unknown_when_engine_is_stopped() {
    let runtime_view = Arc::new(RwLock::new(RuntimeView::new(EngineStatus::Stopped)));
    let mut ingress_view = IngressView::starting();
    ingress_view.endpoint_status = "ready";
    ingress_view.endpoint_id = Some("b3d3d7d7-2361-47b4-98ed-1ea94cfb74f0".to_owned());
    ingress_view.address = Some(IngressAddressValue {
        host: "127.0.0.1",
        port: 41237,
    });
    ingress_view.last_endpoint_observation =
        Some(Instant::now() - HEALTH_EXPIRY - Duration::from_secs(1));
    ingress_view.active_connection_count = Some(0);
    let (sender, receiver) = mpsc::sync_channel(1);
    drop(receiver);
    let manager = RuntimeManager {
        sender,
        view: runtime_view,
        ingress_view: Arc::new(RwLock::new(ingress_view)),
        shutting_down: Arc::new(AtomicBool::new(false)),
        update_start_gate: Arc::new(std::sync::Mutex::new(false)),
        actor_available: false,
    };

    let IngressReply::Snapshot(snapshot) = manager.get_ingress_snapshot(REQUEST_ID.to_owned())
    else {
        panic!("expected ingress snapshot");
    };
    assert_eq!(snapshot.endpoint.status, "unknown");
    assert_eq!(snapshot.endpoint.error_code, Some("timeout"));
    assert_eq!(snapshot.backend.status, "stopped");
    assert_eq!(snapshot.active_connection_count, None);
}

#[test]
#[cfg(windows)]
fn closed_accept_loop_is_rejected_by_start_gate_before_engine_launch() {
    let project =
        fs::canonicalize(PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap()).unwrap();
    let runs_root = project.join("work/proofs/runtime-manager/runs");
    assert!(is_reparse_free_path(&runs_root));
    ensure_existing_ancestor_inside(&project, &runs_root).unwrap();
    fs::create_dir_all(&runs_root).unwrap();
    let runs_root = fs::canonicalize(runs_root).unwrap();
    assert!(runs_root.starts_with(&project) && is_reparse_free_path(&runs_root));
    let run_root = runs_root.join(Uuid::new_v4().to_string());
    assert_eq!(run_root.parent(), Some(runs_root.as_path()));
    fs::create_dir(&run_root).unwrap();
    let unavailable_manager = RuntimeManager::from_layout(Err(PublicError::EnvironmentBlocked));
    let mut cleanup = LiveRunCleanup {
        manager: unavailable_manager,
        run_root: run_root.clone(),
        runs_root: runs_root.clone(),
        complete: false,
    };

    let stable = StableIngress::open_or_create(&run_root).unwrap();
    let mut relay = IngressRelay::new(stable, RelayLimits::default()).unwrap();
    let endpoint_id = relay.endpoint_id().map(str::to_owned).unwrap();
    let address = match relay.local_addr() {
        SocketAddr::V4(address) if address.ip() == &Ipv4Addr::LOCALHOST => IngressAddressValue {
            host: "127.0.0.1",
            port: address.port(),
        },
        _ => panic!("expected loopback test listener"),
    };
    let report = relay.shutdown(INGRESS_SHUTDOWN_TIMEOUT);
    assert!(report.complete && report.listener_released);

    let ingress_view = Arc::new(RwLock::new(IngressView::starting()));
    write_ingress_view(&ingress_view, |state| {
        state.endpoint_status = "ready";
        state.endpoint_id = Some(endpoint_id);
        state.address = Some(address);
        state.last_endpoint_observation = Some(Instant::now());
    });
    let managed_ingress = ManagedIngress {
        relay: Some(relay),
        startup_error: None,
        view: Arc::clone(&ingress_view),
    };

    assert_eq!(
        managed_ingress.ensure_endpoint_for_start(),
        Err(PublicError::OwnershipLost)
    );
    let ingress = ingress_view.read().unwrap().clone();
    assert_eq!(ingress.endpoint_status, "unknown");
    assert_eq!(ingress.endpoint_error_code, Some("ownership_lost"));
    assert_eq!(ingress.active_connection_count, None);
    let runtime = Arc::new(RwLock::new(RuntimeView::new(EngineStatus::Stopped)));
    mark_no_active_ingress_failure(&runtime, PublicError::OwnershipLost);
    let RuntimeReply::Snapshot(snapshot) = snapshot_for(&runtime, REQUEST_ID) else {
        panic!("expected unavailable engine snapshot");
    };
    assert_eq!(snapshot.engine.status, "unavailable");
    assert!(!snapshot.capabilities.start);
    drop(managed_ingress);
    assert!(cleanup.finish());
}

#[test]
fn unverified_ingress_start_cleanup_reports_cleanup_failed() {
    assert_eq!(
        cleanup_result_error(PublicError::OwnershipLost, false),
        PublicError::CleanupFailed
    );
    assert_eq!(
        cleanup_result_error(PublicError::OwnershipLost, true),
        PublicError::OwnershipLost
    );
}

fn request_readyz_through_ingress(address: SocketAddr) {
    let mut ingress_probe = TcpStream::connect_timeout(&address, HEALTH_IO_TIMEOUT)
        .expect("stable ingress accepts while the engine route is ready");
    ingress_probe
        .set_read_timeout(Some(HEALTH_IO_TIMEOUT))
        .unwrap();
    ingress_probe
        .write_all(b"GET /readyz HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")
        .unwrap();
    let (status, body) = read_http_body(&mut ingress_probe, Instant::now() + HEALTH_IO_TIMEOUT)
        .expect("bounded ingress /readyz response");
    assert_eq!(status, 200);
    assert!(health_payload_matches(&body, EXPECTED_ENGINE_VERSION));
}

#[test]
fn private_control_ready_attestation_is_pinned_and_debug_redacted() {
    let ready = parse_controller_ready(&ready_frame(), REQUEST_ID, 300).unwrap();
    assert!(!format!("{ready:?}").contains(&"a".repeat(32)));
    for (key, value) in [
        ("control_token", json!("0".repeat(32))),
        ("module_sha256", json!("0".repeat(64))),
        ("sources_sha256", json!("0".repeat(64))),
        ("protocol_version", json!(2)),
        ("extra", json!(true)),
    ] {
        let mut value_frame: Value = serde_json::from_slice(&ready_frame()).unwrap();
        value_frame["optimization_control"][key] = value;
        let mut raw = serde_json::to_vec(&value_frame).unwrap();
        raw.push(b'\n');
        assert!(parse_controller_ready(&raw, REQUEST_ID, 300).is_err());
    }
}

fn assert_optimization(reply: &RuntimeReply, status: &str, revision: Option<u64>, available: bool) {
    let RuntimeReply::Snapshot(snapshot) = reply else {
        panic!("expected runtime snapshot")
    };
    assert_eq!(snapshot.compression.status, status);
    assert_eq!(snapshot.compression.revision, revision);
    assert_eq!(snapshot.capabilities.pause, available);
    assert_eq!(snapshot.compression.active_stream_count, None);
    let public = serde_json::to_string(snapshot).unwrap();
    assert!(!public.contains("control_token"));
    assert!(!public.contains("optimization_control"));
}

#[test]
fn public_optimization_state_requires_ready_and_known_control() {
    let mut view = RuntimeView::new(EngineStatus::Ready);
    view.runtime_generation = Some(GENERATION.to_owned());
    view.engine_version = Some(EXPECTED_ENGINE_VERSION.to_owned());
    assert_optimization(
        &view.snapshot(REQUEST_ID.to_owned()),
        "unknown",
        None,
        false,
    );
    view.optimization = Some(OptimizationState {
        enabled: true,
        revision: 0,
        can_pause: true,
        active_http_requests: 3,
    });
    assert_optimization(
        &view.snapshot(REQUEST_ID.to_owned()),
        "enabled",
        Some(0),
        true,
    );
    view.optimization.as_mut().unwrap().can_pause = false;
    assert_optimization(
        &view.snapshot(REQUEST_ID.to_owned()),
        "enabled",
        Some(0),
        false,
    );
    view.optimization.as_mut().unwrap().enabled = false;
    assert_optimization(
        &view.snapshot(REQUEST_ID.to_owned()),
        "paused",
        Some(0),
        true,
    );
    view.status = EngineStatus::Stopping;
    assert_optimization(
        &view.snapshot(REQUEST_ID.to_owned()),
        "unavailable",
        None,
        false,
    );
}

#[cfg(windows)]
#[test]
#[ignore = "opt-in real pinned helper/controller pause; loopback control only, no model/provider request"]
fn live_manager_pause_is_owned_revision_checked_and_reversible() {
    let project =
        fs::canonicalize(PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap()).unwrap();
    let runs = project.join("work/proofs/runtime-manager/runs");
    fs::create_dir_all(&runs).unwrap();
    let runs = fs::canonicalize(runs).unwrap();
    let make_manager = || {
        let root = runs.join(Uuid::new_v4().to_string());
        fs::create_dir(&root).unwrap();
        let layout =
            RuntimeLayout::from_roots(project.clone(), project.join("runtime"), root.clone())
                .unwrap();
        let manager = RuntimeManager::from_layout(Ok(layout));
        LiveRunCleanup {
            manager,
            run_root: root,
            runs_root: runs.clone(),
            complete: false,
        }
    };
    let mut first = make_manager();
    let mut second = make_manager();
    reset_startup_diagnostic();
    let started = first.manager.start_engine(REQUEST_ID.to_owned());
    assert_eq!(
        started.status_for_test(),
        Some("ready"),
        "start_error={:?}; diagnostic={:?}",
        started.error_code_for_test(),
        startup_diagnostic_for_test()
    );
    let generation = started.generation_for_test().unwrap();
    assert_optimization(&started, "enabled", Some(0), true);
    let port = first.manager.listener_port_for_test().unwrap();
    let owner = first.manager.listener_owner_pid_for_test().unwrap();
    let other = second.manager.start_engine(Uuid::new_v4().to_string());
    assert_eq!(other.status_for_test(), Some("ready"));
    let other_generation = other.generation_for_test().unwrap();
    let other_port = second.manager.listener_port_for_test().unwrap();
    let other_owner = second.manager.listener_owner_pid_for_test().unwrap();
    assert_ne!(generation, other_generation);
    assert_ne!(port, other_port);
    let paused =
        first
            .manager
            .set_optimization(Uuid::new_v4().to_string(), generation.clone(), 0, false);
    assert_optimization(&paused, "paused", Some(1), true);
    assert_eq!(
        paused.generation_for_test().as_deref(),
        Some(generation.as_str())
    );
    assert_eq!(listener_owner(port), Some(owner));
    let stale =
        first
            .manager
            .set_optimization(Uuid::new_v4().to_string(), generation.clone(), 0, true);
    assert_eq!(stale.error_code_for_test(), Some("config_conflict"));
    let foreign = first.manager.set_optimization(
        Uuid::new_v4().to_string(),
        other_generation.clone(),
        1,
        true,
    );
    assert_eq!(foreign.error_code_for_test(), Some("ownership_lost"));
    assert_optimization(
        &second.manager.get_runtime_snapshot(REQUEST_ID.to_owned()),
        "enabled",
        Some(0),
        true,
    );
    let resumed =
        first
            .manager
            .set_optimization(Uuid::new_v4().to_string(), generation.clone(), 1, true);
    assert_optimization(&resumed, "enabled", Some(2), true);
    let repeat =
        first
            .manager
            .set_optimization(Uuid::new_v4().to_string(), generation.clone(), 2, true);
    assert_optimization(&repeat, "enabled", Some(2), true);
    let stopped = first
        .manager
        .stop_engine(Uuid::new_v4().to_string(), generation);
    assert_eq!(stopped.status_for_test(), Some("stopped"));
    assert_optimization(&stopped, "unavailable", None, false);
    assert!(owned_listener_closed(port, owner, PORT_CLOSE_TIMEOUT));
    assert_optimization(
        &second.manager.get_runtime_snapshot(REQUEST_ID.to_owned()),
        "enabled",
        Some(0),
        true,
    );
    assert_eq!(listener_owner(other_port), Some(other_owner));
    assert_eq!(
        second
            .manager
            .stop_engine(Uuid::new_v4().to_string(), other_generation)
            .status_for_test(),
        Some("stopped")
    );
    assert!(owned_listener_closed(
        other_port,
        other_owner,
        PORT_CLOSE_TIMEOUT
    ));
    assert!(first.finish());
    assert!(second.finish());
}

fn is_expected_closed_socket_error(kind: std::io::ErrorKind) -> bool {
    matches!(
        kind,
        std::io::ErrorKind::ConnectionReset
            | std::io::ErrorKind::BrokenPipe
            | std::io::ErrorKind::NotConnected
            | std::io::ErrorKind::ConnectionAborted
    )
}

#[test]
#[ignore = "opt-in pinned runtime lifecycle proof; local /readyz only, no provider API"]
fn live_manager_stable_ingress_survives_pinned_engine_restart() {
    let project =
        fs::canonicalize(PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap()).unwrap();
    let runtime = project.join("runtime");
    let runs_root = project.join("work/proofs/runtime-manager/runs");
    fs::create_dir_all(&runs_root).unwrap();
    let runs_root = fs::canonicalize(runs_root).unwrap();
    let run_root = runs_root.join(Uuid::new_v4().to_string());
    assert!(run_root.starts_with(&runs_root));
    fs::create_dir(&run_root).unwrap();
    let layout = RuntimeLayout::from_roots(project, runtime, run_root.clone()).unwrap();
    let manager = RuntimeManager::from_layout(Ok(layout));
    let mut cleanup = LiveRunCleanup {
        manager: manager.clone(),
        run_root: run_root.clone(),
        runs_root: runs_root.clone(),
        complete: false,
    };
    let foreign_listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let foreign_port = foreign_listener.local_addr().unwrap().port();
    let foreign_pid = listener_owner(foreign_port).expect("test-owned foreign listener PID");

    reset_startup_diagnostic();
    let started = manager.start_engine(REQUEST_ID.to_owned());
    let first_generation = started.generation_for_test().unwrap_or_else(|| {
        panic!(
            "pinned manager start did not produce a ready generation: reply_status={:?}, error_code={:?}, diagnostic={:?}",
            started.status_for_test(),
            started.error_code_for_test(),
            startup_diagnostic_for_test()
        )
    });
    let IngressReply::Snapshot(initial_ingress) =
        manager.get_ingress_snapshot(REQUEST_ID.to_owned())
    else {
        panic!("expected stable ingress snapshot");
    };
    assert_eq!(initial_ingress.endpoint.status, "ready");
    assert_eq!(initial_ingress.backend.status, "ready");
    assert_eq!(
        initial_ingress.backend.generation.as_deref(),
        Some(first_generation.as_str())
    );
    let endpoint_id = initial_ingress
        .endpoint
        .endpoint_id
        .clone()
        .expect("endpoint id");
    let endpoint_address = initial_ingress
        .endpoint
        .address
        .as_ref()
        .map(|address| SocketAddr::new(address.host.parse().unwrap(), address.port))
        .expect("stable loopback address");
    assert!(matches!(endpoint_address.ip(), IpAddr::V4(address) if address.is_loopback()));
    let port = manager
        .listener_port_for_test()
        .expect("owned listener port");
    let engine_pid = manager
        .listener_owner_pid_for_test()
        .expect("owned listener PID");
    assert_ne!(foreign_port, port);
    assert_eq!(listener_owner(port), Some(engine_pid));

    // The stable ingress forwards a harmless engine readiness request byte-for-byte.
    request_readyz_through_ingress(endpoint_address);

    // The production monitor polls /readyz once per second and permits a 5s age.
    thread::sleep(HEALTH_EXPIRY + Duration::from_secs(1));
    let refreshed = manager.get_runtime_snapshot("8132bced-3ce0-4c1b-8f48-d93d7b96f7ce".to_owned());
    assert_eq!(refreshed.status_for_test(), Some("ready"));
    assert_eq!(
        refreshed.generation_for_test().as_deref(),
        Some(first_generation.as_str())
    );

    let stale_generation = format!(
        "{}{}",
        if first_generation.starts_with('0') {
            '1'
        } else {
            '0'
        },
        &first_generation[1..]
    );
    let stale_stop = manager.stop_engine(
        "9132bced-3ce0-4c1b-8f48-d93d7b96f7ce".to_owned(),
        stale_generation,
    );
    assert_eq!(stale_stop.error_code_for_test(), Some("ownership_lost"));
    let still_ready =
        manager.get_runtime_snapshot("8132bced-3ce0-4c1b-8f48-d93d7b96f7ce".to_owned());
    assert_eq!(still_ready.status_for_test(), Some("ready"));
    assert_eq!(manager.listener_port_for_test(), Some(port));
    assert_eq!(listener_owner(port), Some(engine_pid));

    let duplicate = manager.start_engine("8132bced-3ce0-4c1b-8f48-d93d7b96f7ce".to_owned());
    assert_eq!(
        duplicate.generation_for_test().as_deref(),
        Some(first_generation.as_str())
    );
    assert_eq!(manager.listener_port_for_test(), Some(port));

    let stopped = manager.stop_engine(
        "9132bced-3ce0-4c1b-8f48-d93d7b96f7ce".to_owned(),
        first_generation.clone(),
    );
    assert_eq!(
        stopped.status_for_test(),
        Some("stopped"),
        "fixed_stop_error_code={:?}; stop_diagnostics={:?}",
        stopped.error_code_for_test(),
        manager
            .stop_diagnostics_for_test()
            .map(|(failure, report)| { (failure.stage, failure.error.code(), report) })
    );
    assert!(owned_listener_closed(port, engine_pid, PORT_CLOSE_TIMEOUT));
    assert_ne!(listener_owner(port), Some(engine_pid));
    let IngressReply::Snapshot(stopped_ingress) =
        manager.get_ingress_snapshot("8132bced-3ce0-4c1b-8f48-d93d7b96f7ce".to_owned())
    else {
        panic!("expected stopped ingress snapshot");
    };
    assert_eq!(
        stopped_ingress.endpoint.endpoint_id.as_deref(),
        Some(endpoint_id.as_str())
    );
    assert_eq!(
        stopped_ingress
            .endpoint
            .address
            .as_ref()
            .map(|address| address.port),
        Some(endpoint_address.port())
    );
    assert_eq!(stopped_ingress.backend.status, "stopped");
    assert_eq!(stopped_ingress.active_connection_count, Some(0));

    // The stable endpoint remains bound while the engine route is absent.
    let mut no_route = TcpStream::connect_timeout(&endpoint_address, HEALTH_IO_TIMEOUT)
        .expect("stable endpoint stays bound while stopped");
    no_route.set_read_timeout(Some(HEALTH_IO_TIMEOUT)).unwrap();
    if let Err(error) = no_route.write_all(b"GET /readyz HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n") {
        assert!(is_expected_closed_socket_error(error.kind()));
    }
    if let Err(error) = no_route.shutdown(Shutdown::Write) {
        assert!(is_expected_closed_socket_error(error.kind()));
    }
    let mut no_route_response = Vec::new();
    let no_route_read = no_route.read_to_end(&mut no_route_response);
    assert!(no_route_response.is_empty());
    if let Err(error) = no_route_read {
        assert!(is_expected_closed_socket_error(error.kind()));
    }

    let second_start = manager.start_engine("8132bced-3ce0-4c1b-8f48-d93d7b96f7ce".to_owned());
    let second_generation = second_start
        .generation_for_test()
        .expect("second ready generation");
    assert_ne!(second_generation, first_generation);
    let IngressReply::Snapshot(second_ingress) =
        manager.get_ingress_snapshot("9132bced-3ce0-4c1b-8f48-d93d7b96f7ce".to_owned())
    else {
        panic!("expected second ingress snapshot");
    };
    assert_eq!(
        second_ingress.endpoint.endpoint_id.as_deref(),
        Some(endpoint_id.as_str())
    );
    assert_eq!(
        second_ingress
            .endpoint
            .address
            .as_ref()
            .map(|address| address.port),
        Some(endpoint_address.port())
    );
    assert_eq!(
        second_ingress.backend.generation.as_deref(),
        Some(second_generation.as_str())
    );
    request_readyz_through_ingress(endpoint_address);
    let second_stopped = manager.stop_engine(
        "a132bced-3ce0-4c1b-8f48-d93d7b96f7ce".to_owned(),
        second_generation,
    );
    assert_eq!(second_stopped.status_for_test(), Some("stopped"));
    let foreign_address = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), foreign_port);
    let connection = TcpStream::connect_timeout(&foreign_address, HEALTH_IO_TIMEOUT)
        .expect("test-owned foreign listener remains alive after engine stop");
    drop(connection);
    assert!(manager.shutdown());
    let reopened_endpoint = StableIngress::open_or_create(&run_root)
        .expect("stable endpoint is reusable after normal manager shutdown");
    assert_eq!(reopened_endpoint.endpoint_id(), endpoint_id);
    assert_eq!(reopened_endpoint.port(), endpoint_address.port());
    drop(reopened_endpoint);
    assert!(cleanup.finish());
    drop(foreign_listener);
    assert!(owned_listener_closed(
        foreign_port,
        foreign_pid,
        PORT_CLOSE_TIMEOUT
    ));
}

impl RuntimeReply {
    fn error_code_for_test(&self) -> Option<&'static str> {
        match self {
            RuntimeReply::Error(error) => Some(error.error.code),
            RuntimeReply::Snapshot(_) => None,
        }
    }

    fn generation_for_test(&self) -> Option<String> {
        match self {
            RuntimeReply::Snapshot(snapshot) if snapshot.engine.status == "ready" => {
                snapshot.runtime_generation.clone()
            }
            _ => None,
        }
    }

    fn status_for_test(&self) -> Option<&'static str> {
        match self {
            RuntimeReply::Snapshot(snapshot) => Some(snapshot.engine.status),
            _ => None,
        }
    }
}
