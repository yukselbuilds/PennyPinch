use super::super::discovery::{
    DiscoveryInputs, DiscoveryManager, DiscoveryReply, InstallationSelectionError,
    InstallationSummary,
};
use super::codex::{build_snapshot, snapshot_for_selection, CodexStatusReply, CurrentConfigValue};
use super::connection_actions::{
    ApplyConnectionRequest, OwnershipSnapshotRequest, UndoConnectionRequest,
};
use super::{
    ConfigRootInputs, ConnectionActionReply, OwnershipSummary, PlanManager, PlanReply,
    PreviewRequest,
};
use crate::runtime::manager::CodexRuntimeObservation;
use std::cell::Cell;
use std::fs;
#[cfg(windows)]
use std::os::windows::fs::OpenOptionsExt;
use std::path::{Component, Path, PathBuf};
use uuid::Uuid;

const RUNS_RELATIVE: &str = "work/proofs/codex-current-config/runs";
const ENDPOINT_ID: &str = "019a0000-0000-7000-8000-d00000000042";

struct Fixture {
    project_root: PathBuf,
    runs_root: PathBuf,
    uuid: String,
    root: PathBuf,
    cleaned: bool,
}

impl Fixture {
    fn new() -> Self {
        let project_root = normalize_path(
            &PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .expect("project root")
                .canonicalize()
                .expect("canonical project root"),
        );
        let mut runs = project_root.clone();
        for component in Path::new(RUNS_RELATIVE).components() {
            let Component::Normal(part) = component else {
                panic!("fixture path must be fixed and relative")
            };
            runs.push(part);
            match fs::symlink_metadata(&runs) {
                Ok(metadata) => assert_safe_directory(&runs, &project_root, &metadata),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    fs::create_dir(&runs).expect("create checked fixture ancestor");
                    let metadata = fs::symlink_metadata(&runs).expect("new ancestor metadata");
                    assert_safe_directory(&runs, &project_root, &metadata);
                }
                Err(_) => panic!("fixture ancestor unavailable"),
            }
        }
        let runs_root = normalize_path(&runs.canonicalize().expect("canonical runs root"));
        assert!(runs_root.starts_with(&project_root));
        let uuid = Uuid::new_v4().to_string();
        let root = runs_root.join(&uuid);
        assert!(matches!(
            fs::symlink_metadata(&root),
            Err(ref error) if error.kind() == std::io::ErrorKind::NotFound
        ));
        fs::create_dir(&root).expect("create unique fixture root");
        let metadata = fs::symlink_metadata(&root).expect("fixture root metadata");
        assert_safe_directory(&root, &runs_root, &metadata);
        Self {
            project_root,
            runs_root,
            uuid,
            root,
            cleaned: false,
        }
    }

    fn path(&self, relative: impl AsRef<Path>) -> PathBuf {
        self.root.join(relative)
    }

    fn cleanup(&mut self) {
        if self.cleaned {
            return;
        }
        assert_safe_runs_chain(&self.project_root, &self.runs_root);
        let original = fs::symlink_metadata(&self.root).expect("fixture root still exists");
        assert_safe_directory(&self.root, &self.runs_root, &original);
        let canonical = normalize_path(&self.root.canonicalize().expect("canonical UUID root"));
        let expected_root = self.runs_root.join(&self.uuid);
        assert_eq!(path_key(&canonical), path_key(&expected_root));
        let mut pending = vec![canonical.clone()];
        while let Some(path) = pending.pop() {
            let metadata = fs::symlink_metadata(&path).expect("fixture descendant metadata");
            assert!(!is_reparse(&metadata));
            assert!(
                normalize_path(&path.canonicalize().expect("descendant canonical"))
                    .starts_with(&canonical)
            );
            if metadata.is_dir() {
                for child in fs::read_dir(&path).expect("fixture listing") {
                    pending.push(child.expect("fixture entry").path());
                }
            }
        }
        assert_safe_runs_chain(&self.project_root, &self.runs_root);
        let final_metadata = fs::symlink_metadata(&self.root).expect("fixture UUID metadata");
        assert_safe_directory(&self.root, &self.runs_root, &final_metadata);
        let final_root = normalize_path(&self.root.canonicalize().expect("final UUID canonical"));
        assert_eq!(path_key(&final_root), path_key(&expected_root));
        fs::remove_dir_all(canonical).expect("remove only this UUID fixture");
        self.cleaned = true;
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        if !self.cleaned && !std::thread::panicking() {
            self.cleanup();
        }
    }
}

fn is_reparse(metadata: &fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        false
    }
}

fn normalize_path(path: &Path) -> PathBuf {
    let value = path.to_string_lossy();
    value
        .strip_prefix(r"\\?\")
        .filter(|local| local.len() >= 3 && local.as_bytes().get(1) == Some(&b':'))
        .map(PathBuf::from)
        .unwrap_or_else(|| path.to_path_buf())
}

fn path_key(path: &Path) -> String {
    normalize_path(path)
        .to_string_lossy()
        .trim_end_matches(['\\', '/'])
        .to_lowercase()
}

fn assert_safe_directory(path: &Path, boundary: &Path, metadata: &fs::Metadata) {
    assert!(metadata.is_dir());
    assert!(!is_reparse(metadata));
    assert!(
        normalize_path(&path.canonicalize().expect("canonical fixture directory"))
            .starts_with(boundary)
    );
}

fn assert_safe_runs_chain(project_root: &Path, runs_root: &Path) {
    let mut current = project_root.to_path_buf();
    let metadata = fs::symlink_metadata(&current).expect("project root metadata");
    assert_safe_directory(&current, project_root, &metadata);
    for component in Path::new(RUNS_RELATIVE).components() {
        let Component::Normal(part) = component else {
            panic!("fixture runs path must be fixed and relative");
        };
        current.push(part);
        let metadata = fs::symlink_metadata(&current).expect("fixture ancestor metadata");
        assert_safe_directory(&current, project_root, &metadata);
    }
    let canonical = normalize_path(&current.canonicalize().expect("canonical runs root"));
    assert_eq!(path_key(&canonical), path_key(runs_root));
}

fn write_file(path: &Path, contents: &[u8]) {
    fs::create_dir_all(path.parent().expect("fixture parent")).expect("fixture directory");
    fs::write(path, contents).expect("fixture file");
}

fn fixture_discovery(fixture: &Fixture) -> (DiscoveryManager, String, u64) {
    let package_root = fixture.path("npm/node_modules/@openai/codex");
    write_file(
        &package_root.join("package.json"),
        br#"{"name":"@openai/codex","version":"0.163.0-alpha.10.2","bin":{"codex":"bin/codex.js"}}"#,
    );
    write_file(&package_root.join("bin/codex.js"), b"fixture metadata only");
    let discovery = DiscoveryManager::new();
    let scan = match discovery.scan_with_inputs(
        Uuid::new_v4().to_string(),
        DiscoveryInputs {
            path_entries: vec![fixture.path("npm")],
            npm_roots: vec![fixture.path("npm")],
            ..DiscoveryInputs::default()
        },
    ) {
        DiscoveryReply::Snapshot(value) => value,
        _ => panic!("fixture discovery should complete"),
    };
    let installation = scan
        .installations
        .iter()
        .find(|item| item.client_kind == "codex")
        .expect("fixture Codex candidate");
    (
        discovery,
        installation.installation_id.clone(),
        scan.scan_generation,
    )
}

fn fixture_manager(fixture: &Fixture, config: &Path) -> (PlanManager, String, u64) {
    let (discovery, installation_id, scan_generation) = fixture_discovery(fixture);
    let roots = ConfigRootInputs {
        codex_home: Some(config.parent().expect("config parent").to_path_buf()),
        ..ConfigRootInputs::default()
    };
    let manager = PlanManager::with_test_providers(
        discovery,
        move || roots.clone(),
        || {
            Ok(crate::runtime::manager::ReadyStableEndpoint {
                endpoint_id: ENDPOINT_ID.to_owned(),
                host: "127.0.0.1",
                port: 41731,
            })
        },
    )
    .with_ownership_store(
        crate::state::ownership::OwnershipStore::new(&fixture.root, Path::new("state/ownership"))
            .expect("passive fixture ownership store"),
    );
    (manager, installation_id, scan_generation)
}

fn active_codex_fixture(fixture: &Fixture) -> (PathBuf, PlanManager, String, u64) {
    let config = fixture.path("codex-home/config.toml");
    write_file(&config, b"model = \"fixture-model\"\n");
    let (manager, installation_id, scan_generation) = fixture_manager(fixture, &config);
    let plan = match manager.preview(PreviewRequest {
        schema_version: 1,
        request_id: Uuid::new_v4().to_string(),
        installation_id: installation_id.clone(),
        scan_generation,
        scope: "user_config".to_owned(),
    }) {
        PlanReply::Preview(value) => value.plan,
        PlanReply::Error(_) => panic!("fixture plan should be previewable"),
    };
    let applied = manager.apply_connection(ApplyConnectionRequest {
        schema_version: 1,
        request_id: Uuid::new_v4().to_string(),
        plan_id: plan.plan_id,
        expected_config_fingerprint: plan.config_fingerprint,
    });
    match applied {
        ConnectionActionReply::Success(value) => assert_eq!(value.status, "applied"),
        ConnectionActionReply::Error(_) => panic!("fixture apply should succeed"),
    }
    (config, manager, installation_id, scan_generation)
}

fn selected_codex(
    manager: &PlanManager,
    installation_id: &str,
    generation: u64,
) -> InstallationSummary {
    manager
        .discovery
        .select_installation(installation_id, generation)
        .expect("fixture Codex selection")
}

fn current_codex_value(
    manager: &PlanManager,
    installation_id: &str,
    generation: u64,
) -> CurrentConfigValue {
    let selected = selected_codex(manager, installation_id, generation);
    manager.codex_config_observation(&selected, generation).1
}

#[test]
fn active_owned_codex_value_matches_even_when_engine_is_stopped_and_other_fields_change() {
    let fixture = Fixture::new();
    let (config, manager, installation_id, generation) = active_codex_fixture(&fixture);
    assert_eq!(
        current_codex_value(&manager, &installation_id, generation),
        CurrentConfigValue::MatchesOwnedEndpoint
    );

    fs::write(
        &config,
        b"model = \"user-changed-model\"\nopenai_base_url = \"http://127.0.0.1:41731/v1\"\n",
    )
    .expect("unrelated user edit");
    let selected = selected_codex(&manager, &installation_id, generation);
    let (ownership, current) = manager.codex_config_observation(&selected, generation);
    assert_eq!(current, CurrentConfigValue::MatchesOwnedEndpoint);
    let snapshot = build_snapshot(
        Uuid::new_v4().to_string(),
        selected,
        ownership,
        CodexRuntimeObservation {
            endpoint_status: "ready",
            endpoint_id: Some("33333333-3333-4333-8333-333333333333".to_owned()),
            endpoint_port: Some(49_321),
            endpoint_ready: true,
            engine_status: "stopped",
            engine_ready: false,
            runtime_generation: None,
            active_connection_count: Some(0),
            error_code: None,
        },
        current,
    );
    assert_eq!(
        snapshot.ownership.current_config_value,
        "matches_owned_endpoint"
    );
    assert!(!snapshot.runtime.engine_ready);
    assert_eq!(snapshot.coverage.traffic_verification, "not_verified");
}

#[test]
fn active_codex_config_reports_different_without_serializing_url_or_path() {
    let fixture = Fixture::new();
    let (config, manager, installation_id, generation) = active_codex_fixture(&fixture);
    let sentinel = "https://sensitive.example.invalid/private?token=sentinel";
    fs::write(
        &config,
        format!("model = \"user-model\"\nopenai_base_url = \"{sentinel}\"\n").as_bytes(),
    )
    .expect("replace only fixture config");
    let selected = selected_codex(&manager, &installation_id, generation);
    let (ownership, current) = manager.codex_config_observation(&selected, generation);
    assert_eq!(current, CurrentConfigValue::Different);
    let snapshot = build_snapshot(
        Uuid::new_v4().to_string(),
        selected,
        ownership,
        CodexRuntimeObservation {
            endpoint_status: "ready",
            endpoint_id: Some("33333333-3333-4333-8333-333333333333".to_owned()),
            endpoint_port: Some(49_321),
            endpoint_ready: true,
            engine_status: "stopped",
            engine_ready: false,
            runtime_generation: None,
            active_connection_count: Some(0),
            error_code: None,
        },
        current,
    );
    let encoded = serde_json::to_string(&snapshot).expect("redacted snapshot serializes");
    assert!(!encoded.contains(sentinel));
    assert!(!encoded.contains("token=sentinel"));
    assert!(!encoded.contains(&config.to_string_lossy().to_string()));
    assert_eq!(snapshot.coverage.traffic_verification, "not_verified");
}

#[test]
fn active_codex_config_reports_missing_file_or_field_without_writing() {
    let fixture = Fixture::new();
    let (config, manager, installation_id, generation) = active_codex_fixture(&fixture);
    fs::remove_file(&config).expect("remove fixture config");
    assert_eq!(
        current_codex_value(&manager, &installation_id, generation),
        CurrentConfigValue::Missing
    );
    assert!(!config.exists());

    fs::write(&config, b"model = \"user-edit\"\n").expect("write fixture without field");
    assert_eq!(
        current_codex_value(&manager, &installation_id, generation),
        CurrentConfigValue::Missing
    );
    assert_eq!(fs::read(&config).unwrap(), b"model = \"user-edit\"\n");
}

#[test]
fn active_codex_config_malformed_duplicate_non_string_or_oversized_is_unavailable() {
    let cases: Vec<Vec<u8>> = vec![
        b"openai_base_url = [\n".to_vec(),
        b"openai_base_url = \"http://127.0.0.1:41731/v1\"\nopenai_base_url = \"other\"\n".to_vec(),
        b"openai_base_url = 41731\n".to_vec(),
        vec![b' '; 256 * 1024 + 1],
    ];
    for contents in cases {
        let fixture = Fixture::new();
        let (config, manager, installation_id, generation) = active_codex_fixture(&fixture);
        fs::write(&config, &contents).expect("replace fixture config");
        assert_eq!(
            current_codex_value(&manager, &installation_id, generation),
            CurrentConfigValue::Unavailable
        );
        assert_eq!(fs::read(&config).unwrap(), contents);
    }
}

#[cfg(windows)]
#[test]
fn no_codex_owner_does_not_open_or_parse_user_config() {
    let fixture = Fixture::new();
    let config = fixture.path("codex-home/config.toml");
    write_file(&config, b"malformed = [\n");
    let (manager, installation_id, generation) = fixture_manager(&fixture, &config);
    let _exclusive = fs::OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(&config)
        .expect("hold fixture config without sharing");
    let selected = selected_codex(&manager, &installation_id, generation);
    let (ownership, current) = manager.codex_config_observation(&selected, generation);
    assert!(matches!(ownership, Ok(None)));
    assert_eq!(current, CurrentConfigValue::NotOwned);
}

#[cfg(windows)]
#[test]
fn released_codex_owner_does_not_open_user_config() {
    let fixture = Fixture::new();
    let (config, manager, installation_id, generation) = active_codex_fixture(&fixture);
    let ownership = manager.ownership_snapshot(OwnershipSnapshotRequest {
        schema_version: 1,
        request_id: Uuid::new_v4().to_string(),
        client_kind: "codex".to_owned(),
    });
    let active = match ownership {
        ConnectionActionReply::Success(value) => value.ownership.expect("active owner"),
        ConnectionActionReply::Error(_) => panic!("active ownership snapshot should succeed"),
    };
    let released = manager.undo_connection(UndoConnectionRequest {
        schema_version: 1,
        request_id: Uuid::new_v4().to_string(),
        client_kind: "codex".to_owned(),
        owner_id: active.owner_id,
        expected_revision: active.revision,
        expected_record_fingerprint: active.record_fingerprint,
    });
    match released {
        ConnectionActionReply::Success(value) => assert_eq!(value.status, "released"),
        ConnectionActionReply::Error(_) => panic!("fixture owner should release"),
    }
    let _exclusive = fs::OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(&config)
        .expect("hold fixture config without sharing");
    let selected = selected_codex(&manager, &installation_id, generation);
    let (ownership, current) = manager.codex_config_observation(&selected, generation);
    assert!(matches!(ownership, Ok(Some(ref owner)) if owner.state == "released"));
    assert_eq!(current, CurrentConfigValue::NotOwned);
}

#[cfg(windows)]
#[test]
fn unknown_cli_version_does_not_open_config_and_reports_not_rechecked() {
    let fixture = Fixture::new();
    let (config, manager, _installation_id, _generation) = active_codex_fixture(&fixture);
    let _exclusive = fs::OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(&config)
        .expect("hold fixture config without sharing");
    write_file(
        &fixture.path("npm/node_modules/@openai/codex/package.json"),
        br#"{"name":"@openai/codex","version":"0.163.0-alpha.99","bin":{"codex":"bin/codex.js"}}"#,
    );
    let scan = manager.rescan_with_test_inputs(
        Uuid::new_v4().to_string(),
        DiscoveryInputs {
            path_entries: vec![fixture.path("npm")],
            npm_roots: vec![fixture.path("npm")],
            ..DiscoveryInputs::default()
        },
    );
    let scan = match scan {
        DiscoveryReply::Snapshot(value) => value,
        _ => panic!("fixture rescan should complete"),
    };
    let installation_id = scan
        .installations
        .iter()
        .find(|item| item.client_kind == "codex")
        .expect("unknown-version fixture candidate")
        .installation_id
        .clone();
    let selected = selected_codex(&manager, &installation_id, scan.scan_generation);
    let (ownership, current) = manager.codex_config_observation(&selected, scan.scan_generation);
    assert!(matches!(ownership, Ok(Some(_))));
    assert_eq!(current, CurrentConfigValue::NotRechecked);
}

#[test]
fn busy_or_stale_selection_never_reports_a_config_match() {
    let fixture = Fixture::new();
    let (_config, manager, installation_id, generation) = active_codex_fixture(&fixture);
    let selected = selected_codex(&manager, &installation_id, generation);
    let guard = super::PreviewGuard::try_acquire(&manager.preview_in_flight)
        .expect("test holds shared operation guard");
    let (ownership, current) = manager.codex_config_observation(&selected, generation);
    assert!(ownership.is_err());
    assert_eq!(current, CurrentConfigValue::Unavailable);
    drop(guard);

    let discovery = manager.discovery.clone();
    let package_root = fixture.path("npm");
    let manager = manager.with_codex_status_rollback_hook(move || {
        let _ = discovery.scan_with_inputs(
            Uuid::new_v4().to_string(),
            DiscoveryInputs {
                path_entries: vec![package_root.clone()],
                npm_roots: vec![package_root.clone()],
                ..DiscoveryInputs::default()
            },
        );
    });
    let (ownership, current) = manager.codex_config_observation(&selected, generation);
    assert!(matches!(ownership, Ok(Some(_))));
    assert_eq!(current, CurrentConfigValue::Unavailable);
}

#[test]
fn root_change_after_config_rollback_invalidates_the_match() {
    let fixture = Fixture::new();
    let config = fixture.path("codex-home/config.toml");
    write_file(&config, b"model = \"fixture-model\"\n");
    let roots = std::sync::Arc::new(std::sync::Mutex::new(ConfigRootInputs {
        codex_home: Some(config.parent().unwrap().to_path_buf()),
        ..ConfigRootInputs::default()
    }));
    let (discovery, installation_id, generation) = fixture_discovery(&fixture);
    let roots_provider = std::sync::Arc::clone(&roots);
    let manager = PlanManager::with_test_providers(
        discovery,
        move || roots_provider.lock().expect("roots fixture lock").clone(),
        || {
            Ok(crate::runtime::manager::ReadyStableEndpoint {
                endpoint_id: ENDPOINT_ID.to_owned(),
                host: "127.0.0.1",
                port: 41731,
            })
        },
    )
    .with_ownership_store(
        crate::state::ownership::OwnershipStore::new(&fixture.root, Path::new("state/ownership"))
            .expect("fixture ownership store"),
    );
    let plan = match manager.preview(PreviewRequest {
        schema_version: 1,
        request_id: Uuid::new_v4().to_string(),
        installation_id: installation_id.clone(),
        scan_generation: generation,
        scope: "user_config".to_owned(),
    }) {
        PlanReply::Preview(value) => value.plan,
        PlanReply::Error(_) => panic!("fixture plan should be previewable"),
    };
    let applied = manager.apply_connection(ApplyConnectionRequest {
        schema_version: 1,
        request_id: Uuid::new_v4().to_string(),
        plan_id: plan.plan_id,
        expected_config_fingerprint: plan.config_fingerprint,
    });
    assert!(matches!(applied, ConnectionActionReply::Success(_)));
    let selected = selected_codex(&manager, &installation_id, generation);
    let roots_after = std::sync::Arc::clone(&roots);
    let alternate = fixture.path("alternate-codex-home");
    fs::create_dir(&alternate).expect("alternate fixture root");
    let manager = manager.with_codex_status_rollback_hook(move || {
        roots_after.lock().expect("roots fixture lock").codex_home = Some(alternate.clone());
    });
    let (ownership, current) = manager.codex_config_observation(&selected, generation);
    assert!(matches!(ownership, Ok(Some(_))));
    assert_eq!(current, CurrentConfigValue::Unavailable);
}

#[test]
fn alternate_codex_root_with_matching_url_does_not_match_original_owner_binding() {
    let fixture = Fixture::new();
    let (_original_config, original_manager, installation_id, generation) =
        active_codex_fixture(&fixture);
    let alternate = fixture.path("alternate-codex-home");
    let alternate_config = alternate.join("config.toml");
    write_file(
        &alternate_config,
        b"model = \"fixture-model\"\nopenai_base_url = \"http://127.0.0.1:41731/v1\"\n",
    );
    let roots = ConfigRootInputs {
        codex_home: Some(alternate),
        ..ConfigRootInputs::default()
    };
    let manager = PlanManager::with_test_providers(
        original_manager.discovery.clone(),
        move || roots.clone(),
        || {
            Ok(crate::runtime::manager::ReadyStableEndpoint {
                endpoint_id: ENDPOINT_ID.to_owned(),
                host: "127.0.0.1",
                port: 41731,
            })
        },
    )
    .with_ownership_store(
        crate::state::ownership::OwnershipStore::new(&fixture.root, Path::new("state/ownership"))
            .expect("fixture ownership store"),
    );
    let selected = selected_codex(&manager, &installation_id, generation);
    let (ownership, current) = manager.codex_config_observation(&selected, generation);
    assert!(matches!(ownership, Ok(Some(ref owner)) if owner.state == "active"));
    assert_eq!(current, CurrentConfigValue::Unavailable);
}

#[cfg(windows)]
#[test]
fn pinned_codex_read_blocks_write_and_rename_until_dropped() {
    let fixture = Fixture::new();
    let config = fixture.path("codex-home/config.toml");
    let before = b"model = \"pinned-fixture\"\n";
    write_file(&config, before);
    let roots = ConfigRootInputs {
        codex_home: Some(config.parent().expect("config parent").to_path_buf()),
        ..ConfigRootInputs::default()
    };

    let pinned = super::read_codex_config_pinned(&roots).expect("bounded pinned config read");
    assert_eq!(pinned.bytes, before);
    assert_eq!(pinned.presence, super::ConfigPresence::Present);
    assert!(fs::OpenOptions::new().write(true).open(&config).is_err());
    assert!(fs::rename(&config, fixture.path("renamed-config.toml")).is_err());
    assert_eq!(fs::read(&config).expect("fixture config remains"), before);

    drop(pinned);
    let after = b"model = \"after-pin\"\n";
    let mut writer = fs::OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(&config)
        .expect("same fixture file writable after pin release");
    std::io::Write::write_all(&mut writer, after).expect("write fixture after pin release");
    assert_eq!(fs::read(&config).expect("read updated fixture"), after);
}

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
        super::codex::CurrentConfigValue::MatchesOwnedEndpoint,
    );

    assert_eq!(result.ownership.state, "active");
    assert_eq!(result.ownership.endpoint_binding, "same");
    assert_eq!(result.ownership.config_source, Some("codex_toml"));
    assert_eq!(
        result.ownership.current_config_value,
        "matches_owned_endpoint"
    );
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
        super::codex::CurrentConfigValue::NotRechecked,
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
        super::codex::CurrentConfigValue::NotOwned,
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
            super::codex::CurrentConfigValue::NotOwned,
        );
        assert_eq!(result.installation.compatibility, compatibility);
        assert_eq!(result.auth_scope.api_key_auth, api_key_auth);
    }

    let fixture = build_snapshot(
        "55555555-5555-4555-8555-555555555555".to_owned(),
        installation("0.163.0-alpha.10.2", "cli"),
        Ok(None),
        runtime(false),
        super::codex::CurrentConfigValue::NotOwned,
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
        super::codex::CurrentConfigValue::Unavailable,
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
        super::codex::CurrentConfigValue::MatchesOwnedEndpoint,
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
