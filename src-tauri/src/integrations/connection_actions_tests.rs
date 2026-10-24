use super::config_io::{
    ApplyConnectionRequest, ConfigRootInputs, ConnectionActionReply, OwnershipSnapshotRequest,
    PlanManager, PlanReply, PreviewRequest, UndoConnectionRequest,
};
use super::discovery::{DiscoveryInputs, DiscoveryManager, DiscoveryReply};
use crate::runtime::manager::{ReadyStableEndpoint, StableEndpointAccessError};
use crate::state::ownership::OwnershipStore;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

const RUNS_RELATIVE: &str = "work/proofs/config-transaction/actions-c1/runs";
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
                panic!("fixture path must be fixed and relative");
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
        let original = fs::symlink_metadata(&self.root).expect("fixture root still exists");
        assert_safe_directory(&self.root, &self.runs_root, &original);
        let canonical = normalize_path(&self.root.canonicalize().expect("fixture canonical root"));
        assert_eq!(
            path_key(&canonical),
            path_key(&self.runs_root.join(&self.uuid))
        );
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
        fs::remove_dir_all(canonical).expect("remove only this UUID fixture");
        self.cleaned = true;
        let _ = &self.project_root;
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        if !self.cleaned {
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

fn assert_safe_directory(path: &Path, boundary: &Path, metadata: &fs::Metadata) {
    assert!(metadata.is_dir());
    assert!(!is_reparse(metadata));
    assert!(
        normalize_path(&path.canonicalize().expect("canonical directory")).starts_with(boundary)
    );
}

fn path_key(path: &Path) -> String {
    normalize_path(path)
        .to_string_lossy()
        .trim_end_matches(['\\', '/'])
        .to_lowercase()
}

fn normalize_path(path: &Path) -> PathBuf {
    let value = path.to_string_lossy();
    value
        .strip_prefix(r"\\?\")
        .filter(|local| {
            let bytes = local.as_bytes();
            bytes.len() >= 3
                && bytes[0].is_ascii_alphabetic()
                && bytes[1] == b':'
                && matches!(bytes[2], b'\\' | b'/')
        })
        .map(PathBuf::from)
        .unwrap_or_else(|| path.to_path_buf())
}

fn write_file(path: &Path, content: &[u8]) {
    fs::create_dir_all(path.parent().expect("parent path")).expect("fixture parent");
    fs::write(path, content).expect("fixture file");
}

fn manager(fixture: &Fixture, client: &str, config: PathBuf) -> (PlanManager, String, u64) {
    let roots = root_inputs(client, &config);
    manager_with_providers(
        fixture,
        client,
        move || roots.clone(),
        || stable_endpoint(41731),
    )
}

fn root_inputs(client: &str, config: &Path) -> ConfigRootInputs {
    match client {
        "codex" => ConfigRootInputs {
            codex_home: Some(config.parent().expect("config parent").to_path_buf()),
            ..ConfigRootInputs::default()
        },
        "claude_code" => ConfigRootInputs {
            claude_config_dir: Some(config.parent().expect("config parent").to_path_buf()),
            ..ConfigRootInputs::default()
        },
        "opencode_v1" => ConfigRootInputs {
            xdg_config_home: Some(
                config
                    .parent()
                    .and_then(Path::parent)
                    .expect("XDG root")
                    .to_path_buf(),
            ),
            ..ConfigRootInputs::default()
        },
        _ => unreachable!(),
    }
}

fn stable_endpoint(port: u16) -> Result<ReadyStableEndpoint, StableEndpointAccessError> {
    Ok(ReadyStableEndpoint {
        endpoint_id: ENDPOINT_ID.to_owned(),
        host: "127.0.0.1",
        port,
    })
}

fn manager_with_providers<R, E>(
    fixture: &Fixture,
    client: &str,
    roots_provider: R,
    endpoint_provider: E,
) -> (PlanManager, String, u64)
where
    R: Fn() -> ConfigRootInputs + Send + Sync + 'static,
    E: Fn() -> Result<ReadyStableEndpoint, StableEndpointAccessError> + Send + Sync + 'static,
{
    manager_with_providers_and_project_picker(
        fixture,
        client,
        roots_provider,
        endpoint_provider,
        None,
    )
}

fn manager_with_providers_and_project_picker<R, E>(
    fixture: &Fixture,
    client: &str,
    roots_provider: R,
    endpoint_provider: E,
    project_picker: Option<Arc<Mutex<Option<PathBuf>>>>,
) -> (PlanManager, String, u64)
where
    R: Fn() -> ConfigRootInputs + Send + Sync + 'static,
    E: Fn() -> Result<ReadyStableEndpoint, StableEndpointAccessError> + Send + Sync + 'static,
{
    let (package_name, package_version, bin_name, discovery_kind, bin_file) = match client {
        "codex" => (
            "@openai/codex",
            "0.163.0-alpha.10.2",
            "codex",
            "codex",
            "codex.js",
        ),
        "claude_code" => (
            "@anthropic-ai/claude-code",
            "2.1.158",
            "claude",
            "claude_code",
            "claude.js",
        ),
        "opencode_v1" => (
            "opencode-ai",
            "1.19.3",
            "opencode",
            "opencode",
            "opencode.js",
        ),
        _ => panic!("unsupported fixture client"),
    };
    let package_root = fixture.path("npm/node_modules").join(package_name);
    let manifest = format!(
        "{{\"name\":\"{package_name}\",\"version\":\"{package_version}\",\"bin\":{{\"{bin_name}\":\"bin/{bin_file}\"}}}}"
    );
    write_file(&package_root.join("package.json"), manifest.as_bytes());
    write_file(
        &package_root.join("bin").join(bin_file),
        b"fixture metadata only; never execute",
    );
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
        _ => panic!("fixture discovery failed"),
    };
    let selected = scan
        .installations
        .iter()
        .find(|item| item.client_kind == discovery_kind)
        .unwrap_or_else(|| {
            panic!(
                "fixture discovery had no candidate; bounded warnings={:?}",
                scan.warnings
            )
        });
    let manager = PlanManager::with_test_providers(discovery, roots_provider, endpoint_provider)
        .with_ownership_store(
            OwnershipStore::new(&fixture.root, Path::new("state/ownership"))
                .expect("passive ownership store"),
        );
    let manager = match project_picker {
        Some(project_picker) => {
            manager.with_claude_project_picker(move || Ok(project_picker.lock().unwrap().clone()))
        }
        None => manager,
    };
    (
        manager,
        selected.installation_id.clone(),
        scan.scan_generation,
    )
}

fn preview(
    manager: &PlanManager,
    installation_id: String,
    scan_generation: u64,
) -> super::plan::ConnectionPlan {
    preview_with_scope(manager, installation_id, scan_generation, "user_config")
}

fn preview_with_scope(
    manager: &PlanManager,
    installation_id: String,
    scan_generation: u64,
    scope: &str,
) -> super::plan::ConnectionPlan {
    match manager.preview(PreviewRequest {
        schema_version: 1,
        request_id: Uuid::new_v4().to_string(),
        installation_id,
        scan_generation,
        scope: scope.to_owned(),
    }) {
        PlanReply::Preview(reply) => reply.plan,
        PlanReply::Error(_) => panic!("fixture plan should be previewable"),
    }
}

fn select_claude_project(manager: &PlanManager) {
    let reply = manager.select_claude_scope(super::config_io::ClaudeScopeSelectionRequest {
        schema_version: 1,
        request_id: Uuid::new_v4().to_string(),
        scope: "project_local_config".to_owned(),
    });
    assert_eq!(serde_json::to_value(reply).unwrap()["status"], "selected");
}

fn success(reply: ConnectionActionReply) -> super::config_io::ConnectionActionSuccess {
    match reply {
        ConnectionActionReply::Success(value) => value,
        ConnectionActionReply::Error(error) => panic!("expected success, got {}", error.error.code),
    }
}

fn error_code(reply: ConnectionActionReply) -> &'static str {
    match reply {
        ConnectionActionReply::Error(value) => value.error.code,
        ConnectionActionReply::Success(_) => panic!("expected fixed action error"),
    }
}

fn apply(
    manager: &PlanManager,
    plan: &super::plan::ConnectionPlan,
) -> super::config_io::ConnectionActionSuccess {
    success(manager.apply_connection(ApplyConnectionRequest {
        schema_version: 1,
        request_id: Uuid::new_v4().to_string(),
        plan_id: plan.plan_id.clone(),
        expected_config_fingerprint: plan.config_fingerprint.clone(),
    }))
}

fn undo(
    manager: &PlanManager,
    kind: &str,
    owned: &super::config_io::ActionOwnershipSummary,
) -> ConnectionActionReply {
    manager.undo_connection(UndoConnectionRequest {
        schema_version: 1,
        request_id: Uuid::new_v4().to_string(),
        client_kind: kind.to_owned(),
        owner_id: owned.owner_id.clone(),
        expected_revision: owned.revision,
        expected_record_fingerprint: owned.record_fingerprint.clone(),
    })
}

fn exercise_format(client: &str, relative_config: &str, original: &[u8], expected_value: &str) {
    let fixture = Fixture::new();
    let config = fixture.path(relative_config);
    write_file(&config, original);
    let (manager, installation_id, scan_generation) = manager(&fixture, client, config.clone());
    let plan = preview(&manager, installation_id.clone(), scan_generation);
    assert_eq!(plan.status, super::plan::PlanStatus::Preview);
    let applied = apply(&manager, &plan);
    assert_eq!(applied.status, "applied");
    let after = fs::read(&config).unwrap();
    assert!(String::from_utf8_lossy(&after).contains(expected_value));
    let owned = applied.ownership.expect("active ownership summary");
    let kind = match client {
        "codex" => "codex",
        "claude_code" => "claude_code",
        "opencode_v1" => "opencode_v1",
        _ => unreachable!(),
    };
    let released = success(undo(&manager, kind, &owned));
    assert_eq!(released.status, "released");
    assert_eq!(fs::read(&config).unwrap(), original);
}

#[test]
fn claude_project_scope_applies_only_local_leaf_and_undoes_exact_bytes() {
    let fixture = Fixture::new();
    let user_root = fixture.path("claude-user");
    fs::create_dir(&user_root).unwrap();
    let user_settings = user_root.join("settings.json");
    let user_original = br#"{"model":"arbitrary-user-model","effortLevel":"user-effort","apiKeyHelper":"user-helper","env":{"KEEP":"user"}}"#;
    fs::write(&user_settings, user_original).unwrap();

    let project = fixture.path("project-a");
    let project_claude = project.join(".claude");
    fs::create_dir_all(&project_claude).unwrap();
    let shared_settings = project_claude.join("settings.json");
    let shared_original =
        br#"{"model":"arbitrary-shared-model","effortLevel":"xhigh","env":{"ANTHROPIC_API_KEY":"shared-secret"}}"#;
    fs::write(&shared_settings, shared_original).unwrap();
    let project_settings = project_claude.join("settings.local.json");
    let project_original =
        b"{\r\n  \"model\": \"project-model\",\r\n  \"effortLevel\": \"project-effort\",\r\n  \"env\": {\"KEEP\": \"project\"}\r\n}\r\n";
    fs::write(&project_settings, project_original).unwrap();

    let selected_path = Arc::new(Mutex::new(Some(project.clone())));
    let roots = root_inputs("claude_code", &user_settings);
    let roots_for_manager = roots.clone();
    let (manager, installation_id, scan_generation) = manager_with_providers_and_project_picker(
        &fixture,
        "claude_code",
        move || roots_for_manager.clone(),
        || stable_endpoint(41731),
        Some(selected_path),
    );
    select_claude_project(&manager);

    let plan = preview_with_scope(
        &manager,
        installation_id,
        scan_generation,
        "project_local_config",
    );
    assert_eq!(plan.scope, "project_local_config");
    assert_eq!(plan.config_source, super::plan::ConfigSource::ClaudeJson);
    let applied = manager.apply_connection(ApplyConnectionRequest {
        schema_version: 1,
        request_id: Uuid::new_v4().to_string(),
        plan_id: plan.plan_id.clone(),
        expected_config_fingerprint: plan.config_fingerprint.clone(),
    });
    let applied_json = serde_json::to_value(&applied).unwrap();
    assert_eq!(applied_json["ownership"]["scope"], "project_local_config");
    let applied = success(applied);
    let ownership = applied.ownership.expect("project ownership summary");
    assert_eq!(ownership.state, "active");
    let changed = fs::read(&project_settings).unwrap();
    assert!(String::from_utf8_lossy(&changed).contains("http://127.0.0.1:41731\""));
    assert!(String::from_utf8_lossy(&changed).contains("project-model"));
    assert_eq!(fs::read(&user_settings).unwrap(), user_original);
    assert_eq!(fs::read(&shared_settings).unwrap(), shared_original);

    let released = success(undo(&manager, "claude_code", &ownership));
    assert_eq!(released.status, "released");
    assert_eq!(
        serde_json::to_value(released).unwrap()["ownership"]["scope"],
        "project_local_config"
    );
    assert_eq!(fs::read(&project_settings).unwrap(), project_original);
    assert_eq!(fs::read(&user_settings).unwrap(), user_original);
    assert_eq!(fs::read(&shared_settings).unwrap(), shared_original);
}

#[test]
fn claude_project_apply_rejects_changed_non_target_route_layer() {
    let fixture = Fixture::new();
    let user_root = fixture.path("claude-user");
    fs::create_dir(&user_root).unwrap();
    let user_settings = user_root.join("settings.json");
    fs::write(&user_settings, b"{}").unwrap();

    let project = fixture.path("project-a");
    let project_claude = project.join(".claude");
    fs::create_dir_all(&project_claude).unwrap();
    let shared_settings = project_claude.join("settings.json");
    let local_settings = project_claude.join("settings.local.json");
    fs::write(&shared_settings, b"{}").unwrap();
    fs::write(&local_settings, b"{}").unwrap();

    let selected_path = Arc::new(Mutex::new(Some(project.clone())));
    let roots = root_inputs("claude_code", &user_settings);
    let roots_for_manager = roots.clone();
    let (manager, installation_id, scan_generation) = manager_with_providers_and_project_picker(
        &fixture,
        "claude_code",
        move || roots_for_manager.clone(),
        || stable_endpoint(41731),
        Some(selected_path),
    );
    select_claude_project(&manager);
    let plan = preview_with_scope(
        &manager,
        installation_id,
        scan_generation,
        "project_local_config",
    );
    assert_eq!(plan.status, super::plan::PlanStatus::Preview);

    fs::write(
        &shared_settings,
        br#"{"env":{"ANTHROPIC_BASE_URL":"https://gateway.example"}}"#,
    )
    .unwrap();
    let reply = manager.apply_connection(ApplyConnectionRequest {
        schema_version: 1,
        request_id: Uuid::new_v4().to_string(),
        plan_id: plan.plan_id,
        expected_config_fingerprint: plan.config_fingerprint,
    });

    assert_eq!(error_code(reply), "config_conflict");
    assert_eq!(fs::read(&local_settings).unwrap(), b"{}");
    assert_eq!(fs::read(&user_settings).unwrap(), b"{}");
}

#[test]
fn claude_local_managed_route_blocks_preview_and_layer_snapshot_is_redacted() {
    let fixture = Fixture::new();
    let user_root = fixture.path("claude-user");
    fs::create_dir(&user_root).unwrap();
    let user_settings = user_root.join("settings.json");
    fs::write(&user_settings, b"{}").unwrap();

    let project = fixture.path("project-a");
    let project_claude = project.join(".claude");
    fs::create_dir_all(&project_claude).unwrap();
    fs::write(project_claude.join("settings.json"), b"{}").unwrap();
    let target = project_claude.join("settings.local.json");
    fs::write(&target, b"{}").unwrap();
    let managed = fixture.path("managed-settings.json");
    fs::write(
        &managed,
        br#"{"env":{"ANTHROPIC_BASE_URL":"https://user:secret@gateway.example","ANTHROPIC_API_KEY":"private-token"},"model":"private-model"}"#,
    )
    .unwrap();

    let selected_path = Arc::new(Mutex::new(Some(project)));
    let mut roots = root_inputs("claude_code", &user_settings);
    roots.claude_test_managed_settings = Some(managed);
    roots.claude_test_route_environment = Some(Vec::new());
    let roots_for_manager = roots.clone();
    let (manager, installation_id, scan_generation) = manager_with_providers_and_project_picker(
        &fixture,
        "claude_code",
        move || roots_for_manager.clone(),
        || stable_endpoint(41731),
        Some(selected_path),
    );
    select_claude_project(&manager);

    let plan = preview_with_scope(
        &manager,
        installation_id,
        scan_generation,
        "project_local_config",
    );
    assert_eq!(plan.status, super::plan::PlanStatus::Blocked);
    assert!(plan
        .blockers
        .contains(&super::plan::PlanBlocker::CustomGateway));
    assert_eq!(fs::read(&target).unwrap(), b"{}");

    let snapshot =
        manager.claude_settings_snapshot(super::config_io::ClaudeSettingsSnapshotRequest {
            schema_version: 1,
            request_id: Uuid::new_v4().to_string(),
            scope: "project_local_config".to_owned(),
        });
    let encoded = serde_json::to_string(&snapshot).unwrap();
    for secret in [
        "gateway.example",
        "private-token",
        "private-model",
        "https://user:",
    ] {
        assert!(!encoded.contains(secret));
    }
    assert!(encoded.contains("local_managed_settings"));
    assert!(encoded.contains("project_local_config"));
    assert!(encoded.contains("\"model_declared_in_observed_files\":\"present\""));
    assert!(encoded.contains("\"reasoning_declared_in_observed_files\":\"absent\""));
    assert!(encoded.contains("\"auth_declared_in_observed_files\":\"secret_configured\""));
    assert!(encoded.contains("\"hooks_declared_in_observed_files\":\"absent\""));
    assert!(encoded.contains("\"remote_managed_policy\":\"unknown\""));
}

#[test]
fn claude_route_environment_drift_after_preview_conflicts() {
    let fixture = Fixture::new();
    let user_root = fixture.path("claude-user");
    fs::create_dir(&user_root).unwrap();
    let user_settings = user_root.join("settings.json");
    fs::write(&user_settings, b"{}").unwrap();
    let project = fixture.path("project-a");
    fs::create_dir_all(project.join(".claude")).unwrap();
    let target = project.join(".claude/settings.local.json");
    fs::write(&target, b"{}").unwrap();

    let selected_path = Arc::new(Mutex::new(Some(project)));
    let mut roots = root_inputs("claude_code", &user_settings);
    roots.claude_test_route_environment = Some(Vec::new());
    let roots = Arc::new(Mutex::new(roots));
    let roots_for_manager = Arc::clone(&roots);
    let (manager, installation_id, scan_generation) = manager_with_providers_and_project_picker(
        &fixture,
        "claude_code",
        move || roots_for_manager.lock().unwrap().clone(),
        || stable_endpoint(41731),
        Some(selected_path),
    );
    select_claude_project(&manager);
    let plan = preview_with_scope(
        &manager,
        installation_id,
        scan_generation,
        "project_local_config",
    );

    roots.lock().unwrap().claude_test_route_environment = Some(vec![(
        "ANTHROPIC_BASE_URL".to_owned(),
        "https://gateway.example".to_owned(),
    )]);
    let reply = manager.apply_connection(ApplyConnectionRequest {
        schema_version: 1,
        request_id: Uuid::new_v4().to_string(),
        plan_id: plan.plan_id,
        expected_config_fingerprint: plan.config_fingerprint,
    });

    assert_eq!(error_code(reply), "config_conflict");
    assert_eq!(fs::read(&target).unwrap(), b"{}");
}

#[test]
fn claude_project_switch_invalidates_old_lease_and_active_other_project_conflicts() {
    let fixture = Fixture::new();
    let user_root = fixture.path("claude-user");
    fs::create_dir(&user_root).unwrap();
    let user_settings = user_root.join("settings.json");
    fs::write(&user_settings, b"{}").unwrap();
    let project_a = fixture.path("project-a");
    let project_b = fixture.path("project-b");
    for project in [&project_a, &project_b] {
        fs::create_dir_all(project.join(".claude")).unwrap();
        fs::write(project.join(".claude/settings.local.json"), b"{}").unwrap();
    }

    let selected_path = Arc::new(Mutex::new(Some(project_a.clone())));
    let roots = root_inputs("claude_code", &user_settings);
    let roots_for_manager = roots.clone();
    let (manager, installation_id, scan_generation) = manager_with_providers_and_project_picker(
        &fixture,
        "claude_code",
        move || roots_for_manager.clone(),
        || stable_endpoint(41731),
        Some(Arc::clone(&selected_path)),
    );
    let wrong_scope = manager.preview(PreviewRequest {
        schema_version: 1,
        request_id: Uuid::new_v4().to_string(),
        installation_id: installation_id.clone(),
        scan_generation,
        scope: "project_local_config".to_owned(),
    });
    assert_eq!(
        serde_json::to_value(wrong_scope).unwrap()["error"]["code"],
        "unsupported_scope",
        "project preview requires a selected project context"
    );

    select_claude_project(&manager);
    let plan_a = preview_with_scope(
        &manager,
        installation_id.clone(),
        scan_generation,
        "project_local_config",
    );
    let applied_a = apply(&manager, &plan_a);
    let owned_a = applied_a.ownership.expect("first project ownership");

    *selected_path.lock().unwrap() = Some(project_b.clone());
    select_claude_project(&manager);
    assert_eq!(
        error_code(manager.apply_connection(ApplyConnectionRequest {
            schema_version: 1,
            request_id: Uuid::new_v4().to_string(),
            plan_id: plan_a.plan_id,
            expected_config_fingerprint: plan_a.config_fingerprint,
        })),
        "not_found",
        "selection change invalidates leases"
    );
    let plan_b = preview_with_scope(
        &manager,
        installation_id,
        scan_generation,
        "project_local_config",
    );
    assert_eq!(
        error_code(manager.apply_connection(ApplyConnectionRequest {
            schema_version: 1,
            request_id: Uuid::new_v4().to_string(),
            plan_id: plan_b.plan_id,
            expected_config_fingerprint: plan_b.config_fingerprint,
        })),
        "config_conflict",
        "an active record for another project cannot be replaced"
    );
    assert!(String::from_utf8_lossy(
        &fs::read(project_a.join(".claude/settings.local.json")).unwrap()
    )
    .contains("http://127.0.0.1:41731\""));
    assert_eq!(
        fs::read(project_b.join(".claude/settings.local.json")).unwrap(),
        b"{}"
    );
    assert_eq!(owned_a.scope, "project_local_config");
}

#[test]
fn claude_project_undo_works_after_manager_reopen_and_reselection() {
    let fixture = Fixture::new();
    let user_root = fixture.path("claude-user");
    fs::create_dir(&user_root).unwrap();
    let user_settings = user_root.join("settings.json");
    let user_original = b"{\"env\":{\"KEEP\":\"user\"}}";
    fs::write(&user_settings, user_original).unwrap();
    let project = fixture.path("project-a");
    fs::create_dir_all(project.join(".claude")).unwrap();
    let project_settings = project.join(".claude/settings.local.json");
    let project_original = b"{\"env\":{\"KEEP\":\"project\"}}";
    fs::write(&project_settings, project_original).unwrap();
    let selected_path = Arc::new(Mutex::new(Some(project.clone())));
    let roots = root_inputs("claude_code", &user_settings);
    let roots_for_first = roots.clone();
    let (first_manager, installation_id, scan_generation) =
        manager_with_providers_and_project_picker(
            &fixture,
            "claude_code",
            move || roots_for_first.clone(),
            || stable_endpoint(41731),
            Some(Arc::clone(&selected_path)),
        );
    select_claude_project(&first_manager);
    let plan = preview_with_scope(
        &first_manager,
        installation_id,
        scan_generation,
        "project_local_config",
    );
    let ownership = apply(&first_manager, &plan)
        .ownership
        .expect("active project record");
    drop(first_manager);

    let roots_for_second = roots.clone();
    let (second_manager, _, _) = manager_with_providers_and_project_picker(
        &fixture,
        "claude_code",
        move || roots_for_second.clone(),
        || stable_endpoint(41731),
        Some(selected_path),
    );
    select_claude_project(&second_manager);
    let released = success(undo(&second_manager, "claude_code", &ownership));
    assert_eq!(released.status, "released");
    assert_eq!(
        serde_json::to_value(released).unwrap()["ownership"]["scope"],
        "project_local_config"
    );
    assert_eq!(fs::read(&project_settings).unwrap(), project_original);
    assert_eq!(fs::read(&user_settings).unwrap(), user_original);
}

#[test]
fn deleting_selected_claude_project_after_preview_fails_closed() {
    let fixture = Fixture::new();
    let user_root = fixture.path("claude-user");
    fs::create_dir(&user_root).unwrap();
    let user_settings = user_root.join("settings.json");
    fs::write(&user_settings, b"{}").unwrap();
    let project = fixture.path("project-a");
    fs::create_dir_all(project.join(".claude")).unwrap();
    let project_settings = project.join(".claude/settings.local.json");
    let original = b"{\"env\":{\"KEEP\":\"project\"}}";
    fs::write(&project_settings, original).unwrap();
    let selected_path = Arc::new(Mutex::new(Some(project.clone())));
    let roots = root_inputs("claude_code", &user_settings);
    let roots_for_manager = roots.clone();
    let (manager, installation_id, scan_generation) = manager_with_providers_and_project_picker(
        &fixture,
        "claude_code",
        move || roots_for_manager.clone(),
        || stable_endpoint(41731),
        Some(selected_path),
    );
    select_claude_project(&manager);
    let plan = preview_with_scope(
        &manager,
        installation_id.clone(),
        scan_generation,
        "project_local_config",
    );
    fs::remove_dir_all(&project).unwrap();
    let reply = manager.apply_connection(ApplyConnectionRequest {
        schema_version: 1,
        request_id: Uuid::new_v4().to_string(),
        plan_id: plan.plan_id,
        expected_config_fingerprint: plan.config_fingerprint,
    });
    assert!(matches!(reply, ConnectionActionReply::Error(_)));
    assert!(!fixture.path("state").exists());
    assert!(!project_settings.exists());
}

#[test]
fn replacing_selected_missing_claude_project_directory_rejects_old_plan() {
    let fixture = Fixture::new();
    let user_root = fixture.path("claude-user");
    fs::create_dir(&user_root).unwrap();
    let user_settings = user_root.join("settings.json");
    fs::write(&user_settings, b"{}").unwrap();

    let project = fixture.path("project-a");
    fs::create_dir_all(project.join(".claude")).unwrap();
    let project_settings = project.join(".claude/settings.local.json");
    let replacement = fixture.path("project-a-original");
    let selected_path = Arc::new(Mutex::new(Some(project.clone())));
    let roots = root_inputs("claude_code", &user_settings);
    let roots_for_manager = roots.clone();
    let (manager, installation_id, scan_generation) = manager_with_providers_and_project_picker(
        &fixture,
        "claude_code",
        move || roots_for_manager.clone(),
        || stable_endpoint(41731),
        Some(selected_path),
    );
    select_claude_project(&manager);
    let plan = preview_with_scope(
        &manager,
        installation_id.clone(),
        scan_generation,
        "project_local_config",
    );
    assert_eq!(plan.config_presence, super::plan::ConfigPresence::Missing);

    fs::rename(&project, &replacement).unwrap();
    fs::create_dir_all(project.join(".claude")).unwrap();
    let reply = manager.apply_connection(ApplyConnectionRequest {
        schema_version: 1,
        request_id: Uuid::new_v4().to_string(),
        plan_id: plan.plan_id,
        expected_config_fingerprint: plan.config_fingerprint,
    });

    assert!(matches!(reply, ConnectionActionReply::Error(_)));
    assert!(!project_settings.exists());
    assert!(!fixture.path("state").exists());
    assert!(replacement.join(".claude").is_dir());

    select_claude_project(&manager);
    let refreshed_plan = preview_with_scope(
        &manager,
        installation_id,
        scan_generation,
        "project_local_config",
    );
    assert_eq!(
        apply(&manager, &refreshed_plan).status,
        "applied",
        "a new explicit selection binds the replacement directory identity"
    );
    assert!(project_settings.exists());
    assert_eq!(fs::read(&user_settings).unwrap(), b"{}");
}

#[cfg(windows)]
#[test]
fn replacing_selected_claude_project_with_junction_fails_closed() {
    use std::os::windows::ffi::OsStrExt;
    use std::os::windows::io::{FromRawHandle, OwnedHandle, RawHandle};
    use std::ptr::{null, null_mut};
    use windows_sys::Win32::Foundation::GENERIC_WRITE;
    use windows_sys::Win32::Storage::FileSystem::{
        CreateDirectoryW, CreateFileW, RemoveDirectoryW, FILE_FLAG_BACKUP_SEMANTICS,
        FILE_FLAG_OPEN_REPARSE_POINT, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
    };
    use windows_sys::Win32::System::Ioctl::FSCTL_SET_REPARSE_POINT;
    use windows_sys::Win32::System::SystemServices::IO_REPARSE_TAG_MOUNT_POINT;
    use windows_sys::Win32::System::IO::DeviceIoControl;

    let fixture = Fixture::new();
    let user_root = fixture.path("claude-user");
    fs::create_dir(&user_root).unwrap();
    fs::write(user_root.join("settings.json"), b"{}").unwrap();

    let project = fixture.path("project-a");
    fs::create_dir_all(project.join(".claude")).unwrap();
    fs::write(
        project.join(".claude/settings.local.json"),
        b"{\"env\":{\"KEEP\":\"project\"}}",
    )
    .unwrap();
    let replacement = fixture.path("replacement-project");
    fs::create_dir_all(replacement.join(".claude")).unwrap();
    let replacement_settings = replacement.join(".claude/settings.local.json");
    let replacement_original = b"{\"env\":{\"KEEP\":\"replacement\"}}";
    fs::write(&replacement_settings, replacement_original).unwrap();

    let selected_path = Arc::new(Mutex::new(Some(project.clone())));
    let roots = root_inputs("claude_code", &user_root.join("settings.json"));
    let roots_for_manager = roots.clone();
    let (manager, installation_id, scan_generation) = manager_with_providers_and_project_picker(
        &fixture,
        "claude_code",
        move || roots_for_manager.clone(),
        || stable_endpoint(41731),
        Some(selected_path),
    );
    select_claude_project(&manager);
    let plan = preview_with_scope(
        &manager,
        installation_id,
        scan_generation,
        "project_local_config",
    );

    fs::rename(&project, fixture.path("project-a-original")).unwrap();
    let mut link_wide: Vec<u16> = project.as_os_str().encode_wide().chain(Some(0)).collect();
    let target_text = replacement.to_string_lossy();
    let substitute = format!(r"\??\{target_text}");
    let substitute_wide: Vec<u16> = substitute.encode_utf16().collect();
    let print_wide: Vec<u16> = target_text.encode_utf16().collect();
    let mut path_buffer = Vec::new();
    path_buffer.extend(substitute_wide.iter().copied());
    path_buffer.push(0);
    path_buffer.extend(print_wide.iter().copied());
    path_buffer.push(0);
    let substitute_len = (substitute_wide.len() * 2) as u16;
    let print_offset = ((substitute_wide.len() + 1) * 2) as u16;
    let print_len = (print_wide.len() * 2) as u16;
    let mut reparse = Vec::with_capacity(20 + path_buffer.len() * 2);
    reparse.extend_from_slice(&IO_REPARSE_TAG_MOUNT_POINT.to_le_bytes());
    reparse.extend_from_slice(&(8u16 + (path_buffer.len() * 2) as u16).to_le_bytes());
    reparse.extend_from_slice(&0u16.to_le_bytes());
    reparse.extend_from_slice(&0u16.to_le_bytes());
    reparse.extend_from_slice(&substitute_len.to_le_bytes());
    reparse.extend_from_slice(&print_offset.to_le_bytes());
    reparse.extend_from_slice(&print_len.to_le_bytes());
    for unit in path_buffer {
        reparse.extend_from_slice(&unit.to_le_bytes());
    }

    let created = unsafe { CreateDirectoryW(link_wide.as_mut_ptr(), null()) };
    assert_ne!(
        created, 0,
        "fixture junction directory creation is required"
    );
    let raw = unsafe {
        CreateFileW(
            link_wide.as_ptr(),
            GENERIC_WRITE,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            null(),
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT,
            null_mut(),
        )
    };
    assert!(!raw.is_null());
    let junction_handle = unsafe { OwnedHandle::from_raw_handle(raw as RawHandle) };
    let mut returned = 0u32;
    let set = unsafe {
        DeviceIoControl(
            raw,
            FSCTL_SET_REPARSE_POINT,
            reparse.as_ptr().cast(),
            reparse.len() as u32,
            null_mut(),
            0,
            &mut returned,
            null_mut(),
        )
    };
    assert_ne!(set, 0, "fixture mount-point reparse creation is required");

    let reply = manager.apply_connection(ApplyConnectionRequest {
        schema_version: 1,
        request_id: Uuid::new_v4().to_string(),
        plan_id: plan.plan_id,
        expected_config_fingerprint: plan.config_fingerprint,
    });
    drop(junction_handle);
    let removed = unsafe { RemoveDirectoryW(link_wide.as_ptr()) };

    assert_ne!(removed, 0, "remove only the fixture junction itself");
    assert!(matches!(reply, ConnectionActionReply::Error(_)));
    assert_eq!(
        fs::read(&replacement_settings).unwrap(),
        replacement_original
    );
    assert!(!fixture.path("state").exists());
}

#[test]
fn codex_apply_and_undo_commit_only_the_owned_field_and_record() {
    let fixture = Fixture::new();
    let config_dir = fixture.path("codex-home");
    fs::create_dir(&config_dir).unwrap();
    let config = config_dir.join("config.toml");
    let original = b"model = \"fixture-model\"\n\n# keep this comment\n";
    fs::write(&config, original).unwrap();
    let (manager, installation_id, scan_generation) = manager(&fixture, "codex", config.clone());
    let plan = preview(&manager, installation_id, scan_generation);
    assert_eq!(plan.status, super::plan::PlanStatus::Preview);
    assert!(plan.blockers.is_empty());

    let applied = success(manager.apply_connection(ApplyConnectionRequest {
        schema_version: 1,
        request_id: Uuid::new_v4().to_string(),
        plan_id: plan.plan_id,
        expected_config_fingerprint: plan.config_fingerprint,
    }));
    assert_eq!(applied.status, "applied");
    let after = fs::read(&config).unwrap();
    let text = String::from_utf8(after).unwrap();
    assert!(text.contains("model = \"fixture-model\""));
    assert!(text.contains("# keep this comment"));
    assert!(text.contains("openai_base_url = \"http://127.0.0.1:41731/v1\""));
    let owned = applied.ownership.expect("owned record summary");
    assert_eq!(owned.state, "active");
    assert_eq!(owned.endpoint_port, 41731);

    let snapshot = success(manager.ownership_snapshot(OwnershipSnapshotRequest {
        schema_version: 1,
        request_id: Uuid::new_v4().to_string(),
        client_kind: "codex".to_owned(),
    }));
    assert_eq!(
        snapshot.ownership.unwrap().record_fingerprint,
        owned.record_fingerprint
    );

    let undone = success(manager.undo_connection(UndoConnectionRequest {
        schema_version: 1,
        request_id: Uuid::new_v4().to_string(),
        client_kind: "codex".to_owned(),
        owner_id: owned.owner_id,
        expected_revision: owned.revision,
        expected_record_fingerprint: owned.record_fingerprint,
    }));
    assert_eq!(undone.status, "released");
    let restored = fs::read(&config).unwrap();
    assert_eq!(restored, original);
    assert_eq!(undone.ownership.unwrap().state, "released");
}

#[test]
fn same_endpoint_without_existing_ownership_is_a_noop_and_creates_no_store() {
    let fixture = Fixture::new();
    let config_dir = fixture.path("codex-home");
    fs::create_dir(&config_dir).unwrap();
    let config = config_dir.join("config.toml");
    let original = b"model = \"fixture-model\"\nopenai_base_url = \"http://127.0.0.1:41731/v1\"\n";
    fs::write(&config, original).unwrap();
    let (manager, installation_id, scan_generation) = manager(&fixture, "codex", config.clone());
    let plan = preview(&manager, installation_id, scan_generation);
    assert_eq!(plan.status, super::plan::PlanStatus::Preview);
    let reply = success(manager.apply_connection(ApplyConnectionRequest {
        schema_version: 1,
        request_id: Uuid::new_v4().to_string(),
        plan_id: plan.plan_id,
        expected_config_fingerprint: plan.config_fingerprint,
    }));
    assert_eq!(reply.status, "unchanged");
    assert!(reply.ownership.is_none());
    assert_eq!(fs::read(&config).unwrap(), original);
    assert!(!fixture.path("state").exists());
}

#[test]
fn missing_codex_config_is_created_and_leaf_undo_keeps_the_file() {
    let fixture = Fixture::new();
    let config_dir = fixture.path("codex-home");
    fs::create_dir(&config_dir).unwrap();
    let config = config_dir.join("config.toml");
    let (manager, installation_id, scan_generation) = manager(&fixture, "codex", config.clone());
    let plan = preview(&manager, installation_id, scan_generation);
    assert_eq!(plan.config_presence, super::plan::ConfigPresence::Missing);
    let applied = apply(&manager, &plan);
    let active = applied.ownership.unwrap();
    let managed = fs::read(&config).unwrap();
    assert!(String::from_utf8_lossy(&managed).contains("openai_base_url"));
    let released = success(undo(&manager, "codex", &active));
    assert_eq!(released.status, "released");
    assert!(config.is_file());
    assert_eq!(fs::read(&config).unwrap(), b"");
}

#[test]
fn blocked_and_expired_plans_never_patch_or_create_ownership() {
    let fixture = Fixture::new();
    let config_dir = fixture.path("codex-home");
    fs::create_dir(&config_dir).unwrap();
    let config = config_dir.join("config.toml");
    let custom =
        b"model = \"fixture-model\"\nopenai_base_url = \"https://custom.example.invalid\"\n";
    fs::write(&config, custom).unwrap();
    let (manager, installation_id, scan_generation) = manager(&fixture, "codex", config.clone());
    let blocked = preview(&manager, installation_id.clone(), scan_generation);
    assert_eq!(blocked.status, super::plan::PlanStatus::Blocked);
    assert_eq!(
        error_code(manager.apply_connection(ApplyConnectionRequest {
            schema_version: 1,
            request_id: Uuid::new_v4().to_string(),
            plan_id: blocked.plan_id,
            expected_config_fingerprint: blocked.config_fingerprint,
        })),
        "config_conflict"
    );
    assert_eq!(fs::read(&config).unwrap(), custom);
    assert!(!fixture.path("state").exists());

    fs::write(&config, b"model = \"fixture-model\"\n").unwrap();
    let plan = preview(&manager, installation_id, scan_generation);
    assert!(manager.expire_plan_for_test(&plan.plan_id));
    assert_eq!(
        error_code(manager.apply_connection(ApplyConnectionRequest {
            schema_version: 1,
            request_id: Uuid::new_v4().to_string(),
            plan_id: plan.plan_id,
            expected_config_fingerprint: plan.config_fingerprint,
        })),
        "plan_expired"
    );
    assert_eq!(fs::read(&config).unwrap(), b"model = \"fixture-model\"\n");
    assert!(!fixture.path("state").exists());
}

#[test]
fn same_bytes_replacement_conflicts_on_new_file_identity() {
    let fixture = Fixture::new();
    let config_dir = fixture.path("codex-home");
    fs::create_dir(&config_dir).unwrap();
    let config = config_dir.join("config.toml");
    let original = b"model = \"fixture-model\"\n";
    fs::write(&config, original).unwrap();
    let (manager, installation_id, scan_generation) = manager(&fixture, "codex", config.clone());
    let plan = preview(&manager, installation_id, scan_generation);
    let backup = config_dir.join("old-config.toml");
    fs::rename(&config, &backup).unwrap();
    fs::write(&config, original).unwrap();

    assert_eq!(
        error_code(manager.apply_connection(ApplyConnectionRequest {
            schema_version: 1,
            request_id: Uuid::new_v4().to_string(),
            plan_id: plan.plan_id,
            expected_config_fingerprint: plan.config_fingerprint,
        })),
        "config_conflict"
    );
    assert_eq!(fs::read(&config).unwrap(), original);
    assert_eq!(fs::read(&backup).unwrap(), original);
    assert!(!fixture.path("state").exists());
}

#[test]
fn roots_or_discovery_generation_drift_conflicts_before_config_write() {
    let fixture = Fixture::new();
    let config_dir = fixture.path("codex-home");
    let alternate_dir = fixture.path("alternate-codex-home");
    fs::create_dir(&config_dir).unwrap();
    fs::create_dir(&alternate_dir).unwrap();
    let config = config_dir.join("config.toml");
    let alternate = alternate_dir.join("config.toml");
    let original = b"model = \"fixture-model\"\n";
    fs::write(&config, original).unwrap();
    fs::write(&alternate, original).unwrap();
    let roots = Arc::new(Mutex::new(root_inputs("codex", &config)));
    let roots_for_manager = Arc::clone(&roots);
    let (root_manager, installation_id, scan_generation) = manager_with_providers(
        &fixture,
        "codex",
        move || roots_for_manager.lock().unwrap().clone(),
        || stable_endpoint(41731),
    );
    let plan = preview(&root_manager, installation_id.clone(), scan_generation);
    *roots.lock().unwrap() = root_inputs("codex", &alternate);
    assert_eq!(
        error_code(root_manager.apply_connection(ApplyConnectionRequest {
            schema_version: 1,
            request_id: Uuid::new_v4().to_string(),
            plan_id: plan.plan_id,
            expected_config_fingerprint: plan.config_fingerprint,
        })),
        "config_conflict"
    );
    assert_eq!(fs::read(&config).unwrap(), original);
    assert_eq!(fs::read(&alternate).unwrap(), original);
    assert!(!fixture.path("state").exists());

    let (manager, installation_id, scan_generation) = manager(&fixture, "codex", config.clone());
    let plan = preview(&manager, installation_id, scan_generation);
    let rescan = manager.rescan_with_test_inputs(
        Uuid::new_v4().to_string(),
        DiscoveryInputs {
            path_entries: vec![fixture.path("npm")],
            npm_roots: vec![fixture.path("npm")],
            ..DiscoveryInputs::default()
        },
    );
    let super::discovery::DiscoveryReply::Snapshot(rescan) = rescan else {
        panic!("fixture rescan failed");
    };
    assert!(rescan.scan_generation > scan_generation);
    assert_eq!(
        error_code(manager.apply_connection(ApplyConnectionRequest {
            schema_version: 1,
            request_id: Uuid::new_v4().to_string(),
            plan_id: plan.plan_id,
            expected_config_fingerprint: plan.config_fingerprint,
        })),
        "config_conflict"
    );
    assert_eq!(fs::read(&config).unwrap(), original);
}

#[test]
fn endpoint_drift_after_same_endpoint_rollback_is_not_reported_as_success() {
    let fixture = Fixture::new();
    let config_dir = fixture.path("codex-home");
    fs::create_dir(&config_dir).unwrap();
    let config = config_dir.join("config.toml");
    let original = b"model = \"fixture-model\"\nopenai_base_url = \"http://127.0.0.1:41731/v1\"\n";
    fs::write(&config, original).unwrap();
    let roots = root_inputs("codex", &config);
    let calls = Arc::new(AtomicUsize::new(0));
    let calls_for_provider = Arc::clone(&calls);
    let (manager, installation_id, scan_generation) = manager_with_providers(
        &fixture,
        "codex",
        move || roots.clone(),
        move || {
            let call = calls_for_provider.fetch_add(1, Ordering::SeqCst);
            stable_endpoint(if call >= 3 { 41732 } else { 41731 })
        },
    );
    let plan = preview(&manager, installation_id, scan_generation);
    assert_eq!(
        error_code(manager.apply_connection(ApplyConnectionRequest {
            schema_version: 1,
            request_id: Uuid::new_v4().to_string(),
            plan_id: plan.plan_id,
            expected_config_fingerprint: plan.config_fingerprint,
        })),
        "config_conflict"
    );
    assert_eq!(fs::read(&config).unwrap(), original);
    assert!(!fixture.path("state").exists());
}

#[test]
fn opencode_alternate_config_appearing_after_preview_conflicts() {
    let fixture = Fixture::new();
    let directory = fixture.path("xdg-home/opencode");
    fs::create_dir_all(&directory).unwrap();
    let selected = directory.join("opencode.jsonc");
    let alternate = directory.join("opencode.json");
    let original = b"{\n  \"model\": \"openai/fixture-model\",\n  \"provider\": {\"openai\": {\"options\": {}}}\n}\n";
    fs::write(&selected, original).unwrap();
    let (manager, installation_id, scan_generation) =
        manager(&fixture, "opencode_v1", selected.clone());
    let plan = preview(&manager, installation_id, scan_generation);
    assert_eq!(plan.config_source, super::plan::ConfigSource::OpenCodeJsonc);
    let alternate_bytes = b"{}\n";
    fs::write(&alternate, alternate_bytes).unwrap();

    assert_eq!(
        error_code(manager.apply_connection(ApplyConnectionRequest {
            schema_version: 1,
            request_id: Uuid::new_v4().to_string(),
            plan_id: plan.plan_id,
            expected_config_fingerprint: plan.config_fingerprint,
        })),
        "config_conflict"
    );
    assert_eq!(fs::read(&selected).unwrap(), original);
    assert_eq!(fs::read(&alternate).unwrap(), alternate_bytes);
    assert!(!fixture.path("state").exists());
}

#[test]
fn claude_json_and_opencode_jsonc_apply_and_undo_preserve_original_bytes() {
    exercise_format(
        "claude_code",
        "claude-home/settings.json",
        b"{\r\n  \"model\": \"fixture-model\",\r\n  \"env\": {\"ANTHROPIC_API_KEY\": \"sentinel-only\"}\r\n}\r\n",
        "ANTHROPIC_BASE_URL",
    );
    exercise_format(
        "opencode_v1",
        "xdg-home/opencode/opencode.jsonc",
        b"{\n  // keep this comment\n  \"model\": \"openai/fixture-model\",\n  \"provider\": {\"openai\": {\"options\": {}}}\n}\n",
        "baseURL",
    );
}

#[test]
fn stale_config_and_replayed_plan_fail_without_writing_ownership() {
    let fixture = Fixture::new();
    let config_dir = fixture.path("codex-home");
    fs::create_dir(&config_dir).unwrap();
    let config = config_dir.join("config.toml");
    fs::write(&config, b"model = \"before\"\n").unwrap();
    let (manager, installation_id, scan_generation) = manager(&fixture, "codex", config.clone());
    let plan = preview(&manager, installation_id, scan_generation);
    let changed = b"model = \"user edit after preview\"\n";
    fs::write(&config, changed).unwrap();
    let request_id = Uuid::new_v4().to_string();
    let request = ApplyConnectionRequest {
        schema_version: 1,
        request_id: request_id.clone(),
        plan_id: plan.plan_id.clone(),
        expected_config_fingerprint: plan.config_fingerprint.clone(),
    };
    assert_eq!(
        error_code(manager.apply_connection(request.clone())),
        "config_conflict"
    );
    assert_eq!(fs::read(&config).unwrap(), changed);
    assert!(!fixture.path("state").exists());
    assert_eq!(error_code(manager.apply_connection(request)), "not_found");
}

#[test]
fn unrelated_atomic_save_survives_undo_and_managed_field_conflict_is_preserved() {
    let fixture = Fixture::new();
    let config_dir = fixture.path("codex-home");
    fs::create_dir(&config_dir).unwrap();
    let config = config_dir.join("config.toml");
    let original = b"model = \"fixture-model\"\n# original comment\n";
    fs::write(&config, original).unwrap();
    let (manager, installation_id, scan_generation) = manager(&fixture, "codex", config.clone());
    let plan = preview(&manager, installation_id.clone(), scan_generation);
    let applied = apply(&manager, &plan);
    let owned = applied.ownership.unwrap();

    let mut edited = fs::read(&config).unwrap();
    edited.extend_from_slice(b"\n# unrelated user edit\napproval_policy = \"manual\"\n");
    let temp = config_dir.join("config.tmp");
    fs::write(&temp, &edited).unwrap();
    fs::remove_file(&config).unwrap();
    fs::rename(&temp, &config).unwrap();
    let released = success(undo(&manager, "codex", &owned));
    assert_eq!(released.status, "released");
    let after_undo = fs::read(&config).unwrap();
    let text = String::from_utf8(after_undo).unwrap();
    assert!(text.contains("approval_policy = \"manual\""));
    assert!(text.contains("# unrelated user edit"));
    assert!(!text.contains("openai_base_url"));

    let second_plan = preview(&manager, installation_id.clone(), scan_generation);
    let second = apply(&manager, &second_plan);
    let active = second.ownership.unwrap();
    let mut tampered = fs::read(&config).unwrap();
    let text = String::from_utf8(tampered.clone()).unwrap();
    let old = "http://127.0.0.1:41731/v1";
    assert!(text.contains(old));
    tampered = text
        .replace(old, "https://user.example.invalid")
        .into_bytes();
    fs::write(&config, &tampered).unwrap();
    assert_eq!(
        error_code(undo(&manager, "codex", &active)),
        "config_conflict"
    );
    assert_eq!(fs::read(&config).unwrap(), tampered);
    let snapshot = success(manager.ownership_snapshot(OwnershipSnapshotRequest {
        schema_version: 1,
        request_id: Uuid::new_v4().to_string(),
        client_kind: "codex".to_owned(),
    }));
    assert_eq!(snapshot.ownership.as_ref().unwrap().state, "active");
    assert!(!serde_json::to_string(&snapshot)
        .unwrap()
        .contains("user.example.invalid"));
}

#[test]
fn precommit_fault_rolls_back_config_and_ownership_together() {
    let fixture = Fixture::new();
    let config_dir = fixture.path("codex-home");
    fs::create_dir(&config_dir).unwrap();
    let config = config_dir.join("config.toml");
    let original = b"model = \"fixture-model\"\n";
    fs::write(&config, original).unwrap();
    let (manager, installation_id, scan_generation) = manager(&fixture, "codex", config.clone());
    let manager = manager.with_action_commit_fault();
    let plan = preview(&manager, installation_id, scan_generation);
    assert_eq!(
        error_code(manager.apply_connection(ApplyConnectionRequest {
            schema_version: 1,
            request_id: Uuid::new_v4().to_string(),
            plan_id: plan.plan_id,
            expected_config_fingerprint: plan.config_fingerprint,
        })),
        "config_unavailable"
    );
    assert_eq!(fs::read(&config).unwrap(), original);
    let snapshot = success(manager.ownership_snapshot(OwnershipSnapshotRequest {
        schema_version: 1,
        request_id: Uuid::new_v4().to_string(),
        client_kind: "codex".to_owned(),
    }));
    assert!(snapshot.ownership.is_none());
    assert!(!fixture.path("state/ownership/codex.json").exists());
}

#[test]
fn undo_precommit_fault_rolls_back_config_and_ownership_together() {
    let fixture = Fixture::new();
    let config_dir = fixture.path("codex-home");
    fs::create_dir(&config_dir).unwrap();
    let config = config_dir.join("config.toml");
    let original = b"model = \"fixture-model\"\n";
    fs::write(&config, original).unwrap();
    let (manager, installation_id, scan_generation) = manager(&fixture, "codex", config.clone());
    let plan = preview(&manager, installation_id, scan_generation);
    let applied = apply(&manager, &plan);
    let active = applied.ownership.unwrap();
    let applied_bytes = fs::read(&config).unwrap();

    let manager = manager.with_undo_commit_fault();
    assert_eq!(
        error_code(undo(&manager, "codex", &active)),
        "config_unavailable"
    );
    assert_eq!(fs::read(&config).unwrap(), applied_bytes);
    let snapshot = success(manager.ownership_snapshot(OwnershipSnapshotRequest {
        schema_version: 1,
        request_id: Uuid::new_v4().to_string(),
        client_kind: "codex".to_owned(),
    }));
    let retained = snapshot.ownership.expect("active ownership retained");
    assert_eq!(retained.state, "active");
    assert_eq!(retained.owner_id, active.owner_id);
    assert_eq!(retained.record_fingerprint, active.record_fingerprint);
    assert_ne!(applied_bytes, original);
}
