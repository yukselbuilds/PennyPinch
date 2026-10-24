use super::config_io::{
    read_user_config, ConfigIoError, ConfigRootInputs, PlanManager, PlanReply, PreviewRequest,
};
use super::discovery::{DiscoveryInputs, DiscoveryManager, DiscoveryReply};
use super::plan::{
    AuthDeclaration, ConfigPresence, DeclarationState, EndpointBefore, HookShellEffect,
    PlanBlocker, PlanStatus,
};
use crate::runtime::manager::{ReadyStableEndpoint, StableEndpointAccessError};
use std::fs;
use std::path::{Component, Path, PathBuf};
use uuid::Uuid;

const REQUEST_ID: &str = "c26a9113-8537-4d84-9b62-2ab48dc1dc51";
const ENDPOINT_ID: &str = "79c7fb35-d8ad-46bd-9f24-20e6ae7afcb3";
const RUNS_RELATIVE: &str = "work/proofs/connection-plan/runs";

struct Fixture {
    project_root: PathBuf,
    runs_root: PathBuf,
    uuid: String,
    path: PathBuf,
    cleaned: bool,
}

impl Fixture {
    fn new() -> Self {
        let canonical_project = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .canonicalize()
            .unwrap();
        let project_root = normalize_path(&canonical_project);
        assert_safe_directory(&project_root, &project_root);
        let mut runs = project_root.clone();
        for component in Path::new(RUNS_RELATIVE).components() {
            let Component::Normal(part) = component else {
                panic!("fixture path must be fixed and relative");
            };
            runs.push(part);
            match fs::symlink_metadata(&runs) {
                Ok(_) => assert_safe_directory(&runs, &project_root),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    fs::create_dir(&runs).expect("create checked fixture ancestor");
                    assert_safe_directory(&runs, &project_root);
                }
                Err(error) => panic!("fixture ancestor metadata failed: {error:?}"),
            }
        }
        let runs_root = normalize_path(&runs.canonicalize().unwrap());
        assert!(runs_root.starts_with(&project_root));
        let uuid = Uuid::new_v4().to_string();
        let path = runs_root.join(&uuid);
        assert!(matches!(
            fs::symlink_metadata(&path),
            Err(ref error) if error.kind() == std::io::ErrorKind::NotFound
        ));
        fs::create_dir(&path).unwrap();
        assert_safe_directory(&path, &runs_root);
        Self {
            project_root,
            runs_root,
            uuid,
            path,
            cleaned: false,
        }
    }

    fn path(&self, child: impl AsRef<Path>) -> PathBuf {
        self.path.join(child)
    }

    fn cleanup(&mut self) {
        if self.cleaned {
            return;
        }
        assert_safe_directory(&self.runs_root, &self.project_root);
        let metadata = fs::symlink_metadata(&self.path).expect("fixture UUID metadata");
        assert!(metadata.is_dir());
        assert_no_reparse(&metadata);
        let root = self.path.canonicalize().expect("canonical fixture root");
        let expected = self.runs_root.join(&self.uuid);
        assert!(path_key(&root).starts_with(&path_key(&self.runs_root)));
        assert_eq!(path_key(&root), path_key(&expected));
        let mut pending = vec![root.clone()];
        while let Some(path) = pending.pop() {
            let metadata = fs::symlink_metadata(&path).expect("fixture descendant metadata");
            assert_no_reparse(&metadata);
            assert!(path.canonicalize().unwrap().starts_with(&root));
            if metadata.is_dir() {
                for child in fs::read_dir(&path).unwrap() {
                    pending.push(child.unwrap().path());
                }
            }
        }
        let current = self.path.canonicalize().unwrap();
        assert_eq!(path_key(&current), path_key(&expected));
        fs::remove_dir_all(root).expect("remove only this UUID fixture");
        self.cleaned = true;
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        if !self.cleaned {
            self.cleanup();
        }
    }
}

fn write_file(path: &Path, bytes: &[u8]) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, bytes).unwrap();
}

fn assert_safe_directory(path: &Path, boundary: &Path) {
    let metadata = fs::symlink_metadata(path).unwrap();
    assert!(metadata.is_dir());
    assert_no_reparse(&metadata);
    assert!(path_key(&path.canonicalize().unwrap()).starts_with(&path_key(boundary)));
}

fn assert_no_reparse(metadata: &fs::Metadata) {
    assert!(!metadata.file_type().is_symlink());
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        assert_eq!(metadata.file_attributes() & 0x400, 0);
    }
}

fn path_key(path: &Path) -> String {
    normalize_path(path).to_string_lossy().to_lowercase()
}

fn normalize_path(path: &Path) -> PathBuf {
    let text = path.to_string_lossy();
    text.strip_prefix(r"\\?\")
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

fn endpoint() -> ReadyStableEndpoint {
    ReadyStableEndpoint {
        endpoint_id: ENDPOINT_ID.to_owned(),
        host: "127.0.0.1",
        port: 41731,
    }
}

fn scanned_client(
    fixture: &Fixture,
    client_kind: &str,
    executable_name: &str,
) -> (DiscoveryManager, String, u64) {
    let bin = fixture.path(format!("bin-{client_kind}"));
    write_file(&bin.join(executable_name), b"fixture only; never execute");
    let discovery = DiscoveryManager::new();
    let scanned = match discovery.scan_with_inputs(
        REQUEST_ID.to_owned(),
        DiscoveryInputs {
            path_entries: vec![bin],
            ..DiscoveryInputs::default()
        },
    ) {
        DiscoveryReply::Snapshot(value) => value,
        _ => panic!("fixture discovery should return a snapshot"),
    };
    let selected = scanned
        .installations
        .iter()
        .find(|installation| installation.client_kind == client_kind)
        .expect("fixture path entry should be discovered");
    (
        discovery,
        selected.installation_id.clone(),
        scanned.scan_generation,
    )
}

fn request(installation_id: String, scan_generation: u64) -> PreviewRequest {
    request_with_id(REQUEST_ID.to_owned(), installation_id, scan_generation)
}

fn request_with_id(
    request_id: String,
    installation_id: String,
    scan_generation: u64,
) -> PreviewRequest {
    PreviewRequest {
        schema_version: 1,
        request_id,
        installation_id,
        scan_generation,
        scope: "user_config".to_owned(),
    }
}

fn preview_plan(reply: PlanReply) -> super::plan::ConnectionPlan {
    match reply {
        PlanReply::Preview(value) => value.plan,
        PlanReply::Error(_) => panic!("expected preview reply"),
    }
}

fn error_code(reply: PlanReply) -> &'static str {
    match reply {
        PlanReply::Error(value) => value.error.code,
        PlanReply::Preview(_) => panic!("expected error reply"),
    }
}

#[test]
fn missing_config_is_domain_separated_and_does_not_create_directories() {
    let mut fixture = Fixture::new();
    let home = fixture.path("does-not-exist/.codex");
    let inputs = ConfigRootInputs {
        codex_home: Some(home.clone()),
        user_profile: Some(fixture.path("profile")),
        ..ConfigRootInputs::default()
    };

    let first = read_user_config(super::plan::ClientKind::Codex, &inputs).unwrap();
    let second = read_user_config(super::plan::ClientKind::Codex, &inputs).unwrap();
    assert_eq!(first.presence, ConfigPresence::Missing);
    assert!(first.bytes.is_empty());
    assert_eq!(first.fingerprint.len(), 64);
    assert_eq!(first.fingerprint, second.fingerprint);
    assert!(!home.exists());
    fixture.cleanup();
}

#[test]
fn present_toml_is_bounded_fingerprinted_and_read_without_mutation() {
    let mut fixture = Fixture::new();
    let home = fixture.path("codex-home");
    let path = home.join("config.toml");
    let original = b"model = \"fixture-model\"\nmodel_reasoning_effort = \"high\"\n";
    write_file(&path, original);
    let inputs = ConfigRootInputs {
        codex_home: Some(home),
        ..ConfigRootInputs::default()
    };

    let read = read_user_config(super::plan::ClientKind::Codex, &inputs).unwrap();
    assert_eq!(read.presence, ConfigPresence::Present);
    assert_eq!(read.bytes, original);
    assert_eq!(fs::read(&path).unwrap(), original);
    assert_eq!(read.fingerprint.len(), 64);
    fixture.cleanup();
}

#[test]
fn claude_context_request_is_correlated_and_rejects_renderer_paths() {
    use super::config_io::{ClaudeContextSnapshotRequest, ClaudeScopeSelectionRequest};

    let manager = PlanManager::with_test_sources(
        DiscoveryManager::new(),
        ConfigRootInputs::default(),
        Err(StableEndpointAccessError::Unavailable),
    );
    let snapshot = manager.claude_context_snapshot(ClaudeContextSnapshotRequest {
        schema_version: 1,
        request_id: REQUEST_ID.to_owned(),
    });
    let value = serde_json::to_value(snapshot).unwrap();
    assert_eq!(value["status"], "snapshot");
    assert_eq!(value["scope"], "user_config");
    assert_eq!(value["project_directory"], serde_json::Value::Null);
    assert_eq!(value["request_id"], REQUEST_ID);
    assert_eq!(
        value
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect::<std::collections::BTreeSet<_>>(),
        [
            "project_directory",
            "request_id",
            "schema_version",
            "scope",
            "status"
        ]
        .into_iter()
        .collect()
    );

    let invalid = manager.claude_context_snapshot(ClaudeContextSnapshotRequest {
        schema_version: 2,
        request_id: REQUEST_ID.to_owned(),
    });
    let invalid = serde_json::to_value(invalid).unwrap();
    assert_eq!(invalid["status"], "error");
    assert_eq!(invalid["error"]["code"], "validation_failed");

    let raw_path = format!(
        r#"{{"schema_version":1,"request_id":"{REQUEST_ID}","scope":"project_local_config","project_directory":"D:\\private"}}"#
    );
    assert!(serde_json::from_str::<ClaudeScopeSelectionRequest>(&raw_path).is_err());

    let unsupported = manager.select_claude_scope(ClaudeScopeSelectionRequest {
        schema_version: 1,
        request_id: REQUEST_ID.to_owned(),
        scope: "machine_config".to_owned(),
    });
    let unsupported = serde_json::to_value(unsupported).unwrap();
    assert_eq!(unsupported["error"]["code"], "unsupported_scope");
    assert_eq!(unsupported["request_id"], REQUEST_ID);
}

#[cfg(windows)]
#[test]
fn claude_scope_cancel_and_invalid_picker_results_preserve_selected_context() {
    use super::config_io::ClaudeScopeSelectionRequest;
    use std::sync::{Arc, Mutex};

    let mut fixture = Fixture::new();
    let project = fixture.path("project-ø");
    fs::create_dir(&project).unwrap();
    let invalid_file = fixture.path("not-a-directory.txt");
    fs::write(&invalid_file, b"fixture only").unwrap();
    let selected_path = Arc::new(Mutex::new(Some(project.clone())));
    let selected_path_for_picker = Arc::clone(&selected_path);
    let manager = PlanManager::with_test_sources(
        DiscoveryManager::new(),
        ConfigRootInputs::default(),
        Err(StableEndpointAccessError::Unavailable),
    )
    .with_claude_project_picker(move || Ok(selected_path_for_picker.lock().unwrap().clone()));

    let selected = manager.select_claude_scope(ClaudeScopeSelectionRequest {
        schema_version: 1,
        request_id: REQUEST_ID.to_owned(),
        scope: "project_local_config".to_owned(),
    });
    let selected = serde_json::to_value(selected).unwrap();
    assert_eq!(selected["status"], "selected");
    assert_eq!(selected["scope"], "project_local_config");
    assert_eq!(
        path_key(Path::new(selected["project_directory"].as_str().unwrap())),
        path_key(&project)
    );
    assert!(!project.join(".claude").exists());

    for invalid_path in [
        PathBuf::from("relative-project"),
        PathBuf::from(r"\\server\share\project"),
        invalid_file,
    ] {
        *selected_path.lock().unwrap() = Some(invalid_path);
        let invalid = manager.select_claude_scope(ClaudeScopeSelectionRequest {
            schema_version: 1,
            request_id: REQUEST_ID.to_owned(),
            scope: "project_local_config".to_owned(),
        });
        let invalid = serde_json::to_value(invalid).unwrap();
        assert_eq!(invalid["status"], "error");
        assert_eq!(invalid["error"]["code"], "unsafe_project_directory");
    }
    let retained =
        manager.claude_context_snapshot(super::config_io::ClaudeContextSnapshotRequest {
            schema_version: 1,
            request_id: REQUEST_ID.to_owned(),
        });
    let retained = serde_json::to_value(retained).unwrap();
    assert_eq!(retained["scope"], "project_local_config");
    assert_eq!(retained["project_directory"], selected["project_directory"]);

    *selected_path.lock().unwrap() = None;
    let cancelled = manager.select_claude_scope(ClaudeScopeSelectionRequest {
        schema_version: 1,
        request_id: REQUEST_ID.to_owned(),
        scope: "project_local_config".to_owned(),
    });
    let cancelled = serde_json::to_value(cancelled).unwrap();
    assert_eq!(cancelled["status"], "cancelled");
    assert_eq!(cancelled["scope"], "project_local_config");
    assert_eq!(
        cancelled["project_directory"],
        selected["project_directory"]
    );
    fixture.cleanup();
}

#[cfg(windows)]
#[test]
fn claude_scope_changes_invalidate_plans_and_preview_selected_project_local_file() {
    use super::config_io::{ClaudeScopeSelectionRequest, PlanReply};
    use super::plan::PlanStatus;

    let mut fixture = Fixture::new();
    let (discovery, installation_id, scan_generation) =
        scanned_client(&fixture, "claude_code", "claude.cmd");
    let claude_root = fixture.path("claude-context-config");
    write_file(&claude_root.join("settings.json"), b"{}");
    let project = fixture.path("selected-project");
    fs::create_dir(&project).unwrap();
    write_file(&project.join(".claude/settings.local.json"), b"{}");
    let picker_path = std::sync::Arc::new(std::sync::Mutex::new(None));
    let picker_path_for_manager = std::sync::Arc::clone(&picker_path);
    let manager = PlanManager::with_test_sources(
        discovery,
        ConfigRootInputs {
            claude_config_dir: Some(claude_root),
            ..ConfigRootInputs::default()
        },
        Ok(endpoint()),
    )
    .with_claude_project_picker(move || Ok(picker_path_for_manager.lock().unwrap().clone()));

    let first_plan =
        preview_plan(manager.preview(request(installation_id.clone(), scan_generation)));
    assert_eq!(first_plan.status, PlanStatus::Blocked);
    let cancelled = manager.select_claude_scope(ClaudeScopeSelectionRequest {
        schema_version: 1,
        request_id: REQUEST_ID.to_owned(),
        scope: "project_local_config".to_owned(),
    });
    let cancelled = serde_json::to_value(cancelled).unwrap();
    assert_eq!(cancelled["status"], "cancelled");
    assert_eq!(cancelled["scope"], "user_config");
    let retained_plan_action = manager.apply_connection(super::config_io::ApplyConnectionRequest {
        schema_version: 1,
        request_id: REQUEST_ID.to_owned(),
        plan_id: first_plan.plan_id,
        expected_config_fingerprint: first_plan.config_fingerprint,
    });
    let retained_plan_action = serde_json::to_value(retained_plan_action).unwrap();
    assert_eq!(retained_plan_action["error"]["code"], "config_conflict");

    let second_plan =
        preview_plan(manager.preview(request(installation_id.clone(), scan_generation)));
    *picker_path.lock().unwrap() = Some(project);
    let selected = manager.select_claude_scope(ClaudeScopeSelectionRequest {
        schema_version: 1,
        request_id: REQUEST_ID.to_owned(),
        scope: "project_local_config".to_owned(),
    });
    assert_eq!(
        serde_json::to_value(selected).unwrap()["status"],
        "selected"
    );

    let stale_action = manager.apply_connection(super::config_io::ApplyConnectionRequest {
        schema_version: 1,
        request_id: REQUEST_ID.to_owned(),
        plan_id: second_plan.plan_id,
        expected_config_fingerprint: second_plan.config_fingerprint,
    });
    let stale_action = serde_json::to_value(stale_action).unwrap();
    assert_eq!(stale_action["error"]["code"], "not_found");

    let project_preview = manager.preview(PreviewRequest {
        schema_version: 1,
        request_id: Uuid::new_v4().to_string(),
        installation_id: installation_id.clone(),
        scan_generation,
        scope: "project_local_config".to_owned(),
    });
    let PlanReply::Preview(project_preview) = project_preview else {
        panic!("selected project config should be previewable");
    };
    assert_eq!(project_preview.plan.scope, "project_local_config");
    assert_eq!(
        project_preview.plan.config_presence,
        ConfigPresence::Present
    );

    let selected_user = manager.select_claude_scope(ClaudeScopeSelectionRequest {
        schema_version: 1,
        request_id: REQUEST_ID.to_owned(),
        scope: "user_config".to_owned(),
    });
    let selected_user = serde_json::to_value(selected_user).unwrap();
    assert_eq!(selected_user["status"], "selected");
    assert_eq!(selected_user["scope"], "user_config");
    assert_eq!(selected_user["project_directory"], serde_json::Value::Null);
    assert!(matches!(
        manager.preview(request(installation_id, scan_generation)),
        PlanReply::Preview(_)
    ));
    fixture.cleanup();
}

#[cfg(windows)]
#[test]
fn claude_scope_picker_serializes_with_preview_and_mutation_gate() {
    use super::config_io::ClaudeScopeSelectionRequest;
    use std::sync::{mpsc, Arc, Mutex};
    use std::time::Duration;

    let mut fixture = Fixture::new();
    let (discovery, installation_id, scan_generation) =
        scanned_client(&fixture, "claude_code", "claude.cmd");
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::sync_channel(1);
    let release_rx = Arc::new(Mutex::new(release_rx));
    let picker_release = Arc::clone(&release_rx);
    let manager = Arc::new(
        PlanManager::with_test_sources(
            discovery,
            ConfigRootInputs::default(),
            Err(StableEndpointAccessError::Unavailable),
        )
        .with_claude_project_picker(move || {
            entered_tx.send(()).expect("signal picker start");
            picker_release
                .lock()
                .unwrap()
                .recv_timeout(Duration::from_secs(3))
                .expect("bounded picker release");
            Ok(None)
        }),
    );
    let worker_manager = Arc::clone(&manager);
    let worker = std::thread::spawn(move || {
        worker_manager.select_claude_scope(ClaudeScopeSelectionRequest {
            schema_version: 1,
            request_id: "c26a9113-8537-4d84-9b62-2ab48dc1dc52".to_owned(),
            scope: "project_local_config".to_owned(),
        })
    });
    entered_rx
        .recv_timeout(Duration::from_secs(3))
        .expect("scope selection reached picker");

    let busy = manager.preview(request(installation_id, scan_generation));
    assert_eq!(error_code(busy), "busy");
    release_tx.send(()).expect("release picker");
    let cancelled = serde_json::to_value(worker.join().expect("scope worker")).unwrap();
    assert_eq!(cancelled["status"], "cancelled");
    fixture.cleanup();
}

#[test]
fn missing_user_config_plans_report_only_default_declarations() {
    let mut fixture = Fixture::new();

    let (codex_discovery, codex_id, codex_generation) =
        scanned_client(&fixture, "codex", "codex.cmd");
    let codex = PlanManager::with_test_sources(
        codex_discovery,
        ConfigRootInputs {
            codex_home: Some(fixture.path("missing-codex-home")),
            ..ConfigRootInputs::default()
        },
        Ok(endpoint()),
    );
    let codex_plan = preview_plan(codex.preview(request(codex_id, codex_generation)));
    assert_eq!(codex_plan.config_presence, ConfigPresence::Missing);
    assert_eq!(codex_plan.model, DeclarationState::Absent);
    assert_eq!(codex_plan.reasoning, DeclarationState::Absent);
    assert_eq!(codex_plan.auth, AuthDeclaration::Unknown);
    assert_eq!(codex_plan.hook_shell, HookShellEffect::Absent);
    assert_eq!(codex_plan.endpoint_field.before, EndpointBefore::Missing);

    let (claude_discovery, claude_id, claude_generation) =
        scanned_client(&fixture, "claude_code", "claude.cmd");
    let claude = PlanManager::with_test_sources(
        claude_discovery,
        ConfigRootInputs {
            claude_config_dir: Some(fixture.path("missing-claude-home")),
            ..ConfigRootInputs::default()
        },
        Ok(endpoint()),
    );
    let claude_plan = preview_plan(claude.preview(request(claude_id, claude_generation)));
    assert_eq!(claude_plan.config_presence, ConfigPresence::Missing);
    assert_eq!(claude_plan.model, DeclarationState::Absent);
    assert_eq!(claude_plan.reasoning, DeclarationState::Absent);
    assert_eq!(claude_plan.auth, AuthDeclaration::Unknown);
    assert_eq!(claude_plan.hook_shell, HookShellEffect::Absent);
    assert_eq!(claude_plan.endpoint_field.before, EndpointBefore::Missing);

    let (opencode_discovery, opencode_id, opencode_generation) =
        scanned_client(&fixture, "opencode", "opencode.cmd");
    let opencode = PlanManager::with_test_sources(
        opencode_discovery,
        ConfigRootInputs {
            xdg_config_home: Some(fixture.path("missing-xdg-home")),
            ..ConfigRootInputs::default()
        },
        Ok(endpoint()),
    );
    let opencode_plan = preview_plan(opencode.preview(request(opencode_id, opencode_generation)));
    assert_eq!(opencode_plan.config_presence, ConfigPresence::Missing);
    assert_eq!(opencode_plan.model, DeclarationState::Absent);
    assert_eq!(opencode_plan.reasoning, DeclarationState::Unknown);
    assert_eq!(opencode_plan.auth, AuthDeclaration::Unknown);
    assert_eq!(opencode_plan.hook_shell, HookShellEffect::Absent);
    assert_eq!(opencode_plan.endpoint_field.before, EndpointBefore::Unknown);

    fixture.cleanup();
}

#[test]
fn same_bytes_with_a_replaced_file_identity_conflict_during_preview() {
    let mut fixture = Fixture::new();
    let (discovery, installation_id, scan_generation) =
        scanned_client(&fixture, "codex", "codex.cmd");
    let home = fixture.path("codex-identity-home");
    let path = home.join("config.toml");
    let replacement = home.join("replacement.toml");
    let identical = b"model = \"same-bytes\"\n";
    write_file(&path, identical);
    write_file(&replacement, identical);
    let before = read_user_config(
        super::plan::ClientKind::Codex,
        &ConfigRootInputs {
            codex_home: Some(home.clone()),
            ..ConfigRootInputs::default()
        },
    )
    .unwrap()
    .fingerprint;
    let roots = ConfigRootInputs {
        codex_home: Some(home),
        ..ConfigRootInputs::default()
    };
    let old_path = path.clone();
    let replacement_path = replacement.clone();
    let manager = PlanManager::with_test_sources_and_hook(
        discovery,
        roots,
        || Ok(endpoint()),
        move || {
            fs::remove_file(&old_path).unwrap();
            fs::rename(&replacement_path, &old_path).unwrap();
        },
    );
    let reply = manager.preview(request(installation_id, scan_generation));
    assert_eq!(error_code(reply), "config_conflict");
    let after = read_user_config(
        super::plan::ClientKind::Codex,
        &ConfigRootInputs {
            codex_home: Some(path.parent().unwrap().to_path_buf()),
            ..ConfigRootInputs::default()
        },
    )
    .unwrap()
    .fingerprint;
    assert_ne!(before, after);
    assert_eq!(fs::read(&path).unwrap(), identical);
    fixture.cleanup();
}

#[test]
fn opencode_json_to_jsonc_slot_change_with_same_bytes_conflicts() {
    let mut fixture = Fixture::new();
    let (discovery, installation_id, scan_generation) =
        scanned_client(&fixture, "opencode", "opencode.cmd");
    let xdg = fixture.path("xdg-slot-home");
    let json = xdg.join("opencode/opencode.json");
    let jsonc = xdg.join("opencode/opencode.jsonc");
    let identical = b"{\"model\":\"fixture\"}\n";
    write_file(&json, identical);
    let changed_json = json.clone();
    let changed_jsonc = jsonc.clone();
    let manager = PlanManager::with_test_sources_and_hook(
        discovery,
        ConfigRootInputs {
            xdg_config_home: Some(xdg),
            ..ConfigRootInputs::default()
        },
        || Ok(endpoint()),
        move || {
            fs::remove_file(&changed_json).unwrap();
            fs::write(&changed_jsonc, identical).unwrap();
        },
    );
    let reply = manager.preview(request(installation_id, scan_generation));
    assert_eq!(error_code(reply), "config_conflict");
    assert_eq!(fs::read(&jsonc).unwrap(), identical);
    fixture.cleanup();
}

#[cfg(windows)]
#[test]
fn read_handle_rejects_writer_and_parent_pins_block_directory_rename() {
    use super::config_io::pin_parent_directories;
    use std::fs::OpenOptions;

    let mut fixture = Fixture::new();
    let home = fixture.path("locked-codex-home");
    let path = home.join("config.toml");
    write_file(&path, b"model = \"fixture\"\n");

    let read_handle = super::config_io::open_read_shared(&path).unwrap();
    assert!(OpenOptions::new().write(true).open(&path).is_err());
    drop(read_handle);

    let pins = pin_parent_directories(&path).unwrap();
    let renamed = fixture.path("locked-codex-home-renamed");
    assert!(fs::rename(&home, &renamed).is_err());
    drop(pins);
    fixture.cleanup();
}

#[cfg(windows)]
#[test]
fn config_reader_rejects_a_real_junction_without_touching_its_target() {
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

    let mut fixture = Fixture::new();
    let home = fixture.path("reparse-codex-home");
    let link = home.clone();
    let target_dir = fixture.path("target-config-dir");
    let target_config = target_dir.join("config.toml");
    write_file(&target_config, b"model = \"target\"\n");
    fs::create_dir_all(home.parent().unwrap()).unwrap();
    let link_wide: Vec<u16> = link.as_os_str().encode_wide().chain(Some(0)).collect();
    let mut link_wide = link_wide;
    let target_text = target_dir.to_string_lossy();
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
    let _junction_handle = unsafe { OwnedHandle::from_raw_handle(raw as RawHandle) };
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

    let picker_home = home.clone();
    let context_manager = PlanManager::with_test_sources(
        DiscoveryManager::new(),
        ConfigRootInputs::default(),
        Err(StableEndpointAccessError::Unavailable),
    )
    .with_claude_project_picker(move || Ok(Some(picker_home.clone())));
    let context =
        context_manager.select_claude_scope(super::config_io::ClaudeScopeSelectionRequest {
            schema_version: 1,
            request_id: REQUEST_ID.to_owned(),
            scope: "project_local_config".to_owned(),
        });
    assert_eq!(
        serde_json::to_value(context).unwrap()["error"]["code"],
        "unsafe_project_directory"
    );

    assert_eq!(
        read_user_config(
            super::plan::ClientKind::Codex,
            &ConfigRootInputs {
                codex_home: Some(home),
                ..ConfigRootInputs::default()
            },
        )
        .unwrap_err(),
        ConfigIoError::UnsafePath
    );
    assert_eq!(fs::read(&target_config).unwrap(), b"model = \"target\"\n");
    drop(_junction_handle);
    let removed = unsafe { RemoveDirectoryW(link_wide.as_ptr()) };
    assert_ne!(removed, 0, "remove only the fixture junction itself");
    fixture.cleanup();
}

#[test]
fn invalid_override_does_not_fall_back_and_opencode_dual_config_is_conflict() {
    let mut fixture = Fixture::new();
    let invalid_override = ConfigRootInputs {
        codex_home: Some(PathBuf::from("relative-override")),
        user_profile: Some(fixture.path("valid-profile")),
        ..ConfigRootInputs::default()
    };
    assert_eq!(
        read_user_config(super::plan::ClientKind::Codex, &invalid_override).unwrap_err(),
        ConfigIoError::InvalidRoot
    );

    let xdg = fixture.path("xdg");
    write_file(&xdg.join("opencode/opencode.json"), b"{}");
    write_file(&xdg.join("opencode/opencode.jsonc"), b"{}");
    let both = ConfigRootInputs {
        xdg_config_home: Some(xdg),
        ..ConfigRootInputs::default()
    };
    assert_eq!(
        read_user_config(super::plan::ClientKind::OpenCodeV1, &both).unwrap_err(),
        ConfigIoError::Ambiguous
    );
    fixture.cleanup();
}

#[test]
fn relative_parent_ads_and_network_overrides_are_rejected_without_fallback() {
    let mut fixture = Fixture::new();
    let unsafe_roots = [
        ConfigRootInputs {
            codex_home: Some(fixture.path("safe/../codex")),
            user_profile: Some(fixture.path("valid-profile")),
            ..ConfigRootInputs::default()
        },
        ConfigRootInputs {
            codex_home: Some(fixture.path("codex:secret-stream")),
            user_profile: Some(fixture.path("valid-profile")),
            ..ConfigRootInputs::default()
        },
        ConfigRootInputs {
            codex_home: Some(PathBuf::from(r"\\server\share\codex")),
            user_profile: Some(fixture.path("valid-profile")),
            ..ConfigRootInputs::default()
        },
    ];
    for roots in unsafe_roots {
        assert!(matches!(
            read_user_config(super::plan::ClientKind::Codex, &roots),
            Err(ConfigIoError::InvalidRoot | ConfigIoError::UnsafePath)
        ));
    }
    fixture.cleanup();
}

#[test]
fn config_over_limit_is_rejected_before_parser_and_not_echoed() {
    let mut fixture = Fixture::new();
    let home = fixture.path("large-codex");
    write_file(&home.join("config.toml"), &vec![b'x'; 256 * 1024 + 1]);
    let inputs = ConfigRootInputs {
        codex_home: Some(home),
        ..ConfigRootInputs::default()
    };
    assert_eq!(
        read_user_config(super::plan::ClientKind::Codex, &inputs).unwrap_err(),
        ConfigIoError::TooLarge
    );
    fixture.cleanup();
}

#[test]
fn plan_manager_keeps_unknown_version_preview_blocked_and_path_free() {
    let mut fixture = Fixture::new();
    let (discovery, installation_id, scan_generation) =
        scanned_client(&fixture, "codex", "codex.cmd");
    let manager = PlanManager::with_test_sources(
        discovery,
        ConfigRootInputs {
            codex_home: Some(fixture.path("missing-codex-home")),
            ..ConfigRootInputs::default()
        },
        Ok(endpoint()),
    );

    let reply = manager.preview(request(installation_id, scan_generation));
    let envelope = serde_json::to_value(&reply).unwrap();
    let keys: std::collections::BTreeSet<_> = envelope
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(
        keys,
        ["plan", "request_id", "schema_version", "status"]
            .into_iter()
            .collect()
    );
    assert_eq!(envelope["status"], "preview");
    assert_eq!(envelope["plan"]["config_source"], "codex_toml");
    let plan = preview_plan(reply);
    assert_eq!(plan.status, PlanStatus::Blocked);
    assert_eq!(plan.config_presence, ConfigPresence::Missing);
    assert!(plan.blockers.contains(&PlanBlocker::ClientVersionUnknown));
    assert_eq!(
        plan.runtime_effective,
        super::plan::DeclarationState::Unknown
    );
    let serialized = serde_json::to_string(&plan).unwrap();
    assert!(!serialized.contains(&fixture.path.to_string_lossy().to_string()));
    assert!(!serialized.contains("codex.cmd"));
    assert!(!fixture.path("missing-codex-home").exists());
    fixture.cleanup();
}

#[test]
fn claude_json_and_opencode_jsonc_are_read_through_the_real_user_config_resolver() {
    let mut fixture = Fixture::new();

    let (claude_discovery, claude_id, claude_generation) =
        scanned_client(&fixture, "claude_code", "claude.cmd");
    let claude_root = fixture.path("claude-config");
    let claude_path = claude_root.join("settings.json");
    let claude_content = br#"{"model":"fixture-model","effortLevel":"high","env":{"ANTHROPIC_API_KEY":"fixture-secret"}}"#;
    write_file(&claude_path, claude_content);
    let claude_manager = PlanManager::with_test_sources(
        claude_discovery,
        ConfigRootInputs {
            claude_config_dir: Some(claude_root),
            ..ConfigRootInputs::default()
        },
        Ok(endpoint()),
    );
    let claude_reply = claude_manager.preview(request(claude_id, claude_generation));
    let claude_plan = preview_plan(claude_reply);
    let claude_json = serde_json::to_string(&claude_plan).unwrap();
    assert_eq!(claude_plan.config_presence, ConfigPresence::Present);
    assert_eq!(claude_plan.status, PlanStatus::Blocked);
    assert!(claude_json.contains("client_version_unknown"));
    assert!(!claude_json.contains("fixture-secret"));
    assert!(!claude_json.contains("fixture-model"));
    assert_eq!(fs::read(&claude_path).unwrap(), claude_content);

    let (opencode_discovery, opencode_id, opencode_generation) =
        scanned_client(&fixture, "opencode", "opencode.cmd");
    let xdg = fixture.path("xdg-config");
    let opencode_path = xdg.join("opencode/opencode.jsonc");
    let opencode_content = br#"{
      // fixture JSONC
      "model": "openai/fixture-model",
      "provider": { "openai": { "options": { "apiKey": "fixture-secret" } } },
    }"#;
    write_file(&opencode_path, opencode_content);
    let opencode_manager = PlanManager::with_test_sources(
        opencode_discovery,
        ConfigRootInputs {
            xdg_config_home: Some(xdg),
            ..ConfigRootInputs::default()
        },
        Ok(endpoint()),
    );
    let opencode_reply = opencode_manager.preview(request(opencode_id, opencode_generation));
    let opencode_plan = preview_plan(opencode_reply);
    let opencode_json = serde_json::to_string(&opencode_plan).unwrap();
    assert_eq!(opencode_plan.config_presence, ConfigPresence::Present);
    assert_eq!(opencode_plan.status, PlanStatus::Blocked);
    assert!(opencode_json.contains("client_version_unknown"));
    assert!(!opencode_json.contains("fixture-secret"));
    assert!(!opencode_json.contains("fixture-model"));
    assert_eq!(fs::read(&opencode_path).unwrap(), opencode_content);
    fixture.cleanup();
}

#[test]
fn config_fingerprint_change_during_preview_returns_fixed_conflict() {
    let mut fixture = Fixture::new();
    let (discovery, installation_id, scan_generation) =
        scanned_client(&fixture, "codex", "codex.cmd");
    let home = fixture.path("codex-drift-home");
    let path = home.join("config.toml");
    write_file(&path, b"model = \"before\"\n");
    let roots = ConfigRootInputs {
        codex_home: Some(home),
        ..ConfigRootInputs::default()
    };
    let changed_path = path.clone();
    let manager = PlanManager::with_test_sources_and_hook(
        discovery,
        roots,
        || Ok(endpoint()),
        move || fs::write(&changed_path, b"model = \"after\"\n").unwrap(),
    );
    let reply = manager.preview(request(installation_id, scan_generation));
    assert_eq!(error_code(reply), "config_conflict");
    assert_eq!(fs::read(&path).unwrap(), b"model = \"after\"\n");
    fixture.cleanup();
}

#[test]
fn concurrent_previews_return_busy_and_release_the_guard_after_conflict() {
    let mut fixture = Fixture::new();
    let (discovery, installation_id, scan_generation) =
        scanned_client(&fixture, "codex", "codex.cmd");
    let (entered_tx, entered_rx) = std::sync::mpsc::channel();
    let (release_tx, release_rx) = std::sync::mpsc::sync_channel(1);
    let release_rx = std::sync::Arc::new(std::sync::Mutex::new(release_rx));
    let hook_release = std::sync::Arc::clone(&release_rx);
    let hook_calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let hook_counter = std::sync::Arc::clone(&hook_calls);
    let endpoint_calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let endpoint_counter = std::sync::Arc::clone(&endpoint_calls);
    let endpoint_provider = move || {
        if endpoint_counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst) == 1 {
            let mut changed = endpoint();
            changed.port = 41732;
            Ok(changed)
        } else {
            Ok(endpoint())
        }
    };
    let manager = std::sync::Arc::new(PlanManager::with_test_sources_and_hook(
        discovery,
        ConfigRootInputs {
            codex_home: Some(fixture.path("preview-gate-home")),
            ..ConfigRootInputs::default()
        },
        endpoint_provider,
        move || {
            if hook_counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst) == 0 {
                entered_tx
                    .send(())
                    .expect("signal first preview entered its read phase");
                hook_release
                    .lock()
                    .unwrap()
                    .recv_timeout(std::time::Duration::from_secs(3))
                    .expect("bounded release for gated preview");
            }
        },
    ));

    let first_manager = std::sync::Arc::clone(&manager);
    let first_installation = installation_id.clone();
    let first = std::thread::spawn(move || {
        first_manager.preview(request_with_id(
            "c26a9113-8537-4d84-9b62-2ab48dc1dc52".to_owned(),
            first_installation,
            scan_generation,
        ))
    });
    entered_rx
        .recv_timeout(std::time::Duration::from_secs(3))
        .expect("first preview reached the controlled read phase");

    let busy = manager.preview(request_with_id(
        "c26a9113-8537-4d84-9b62-2ab48dc1dc53".to_owned(),
        installation_id.clone(),
        scan_generation,
    ));
    let busy_json = serde_json::to_value(&busy).unwrap();
    assert_eq!(
        busy_json["request_id"],
        "c26a9113-8537-4d84-9b62-2ab48dc1dc53"
    );
    assert_eq!(busy_json["error"]["code"], "busy");
    release_tx.send(()).expect("release the first preview");
    let first_result = first.join().expect("first preview thread");
    assert_eq!(error_code(first_result), "config_conflict");

    let after_release = manager.preview(request_with_id(
        "c26a9113-8537-4d84-9b62-2ab48dc1dc54".to_owned(),
        installation_id,
        scan_generation,
    ));
    assert!(matches!(after_release, PlanReply::Preview(_)));
    fixture.cleanup();
}

#[test]
fn stable_endpoint_drift_during_preview_is_not_returned_as_a_plan() {
    let mut fixture = Fixture::new();
    let (discovery, installation_id, scan_generation) =
        scanned_client(&fixture, "codex", "codex.cmd");
    let home = fixture.path("endpoint-drift-home");
    write_file(&home.join("config.toml"), b"model = \"fixture\"\n");
    let roots = ConfigRootInputs {
        codex_home: Some(home),
        ..ConfigRootInputs::default()
    };
    let calls = std::sync::atomic::AtomicUsize::new(0);
    let manager = PlanManager::with_test_sources_fn(discovery, roots, move || {
        let port = if calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst) == 0 {
            41731
        } else {
            41732
        };
        Ok(ReadyStableEndpoint {
            endpoint_id: ENDPOINT_ID.to_owned(),
            host: "127.0.0.1",
            port,
        })
    });
    let reply = manager.preview(request(installation_id, scan_generation));
    assert_eq!(error_code(reply), "config_conflict");
    fixture.cleanup();
}

#[test]
fn malformed_request_and_stale_installation_do_not_return_a_plan() {
    let discovery = DiscoveryManager::new();
    let manager = PlanManager::with_test_sources(
        discovery,
        ConfigRootInputs::default(),
        Err(StableEndpointAccessError::Unavailable),
    );
    let reply = manager.preview(PreviewRequest {
        schema_version: 1,
        request_id: "not-a-uuid".to_owned(),
        installation_id: "not-a-uuid".to_owned(),
        scan_generation: 1,
        scope: "user_config".to_owned(),
    });
    let serialized = serde_json::to_value(&reply).unwrap();
    assert_eq!(
        serialized["request_id"],
        "00000000-0000-0000-0000-000000000000"
    );
    assert_eq!(serialized["status"], "error");
    assert_eq!(serialized["error"]["code"], "validation_failed");
    assert_eq!(
        serialized["error"]["diagnostic_id"],
        serde_json::Value::Null
    );
    assert!(serialized.get("plan").is_none());
    let envelope_keys: std::collections::BTreeSet<_> = serialized
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(
        envelope_keys,
        ["error", "request_id", "schema_version", "status"]
            .into_iter()
            .collect()
    );
    let error_keys: std::collections::BTreeSet<_> = serialized["error"]
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(
        error_keys,
        ["code", "diagnostic_id", "message_key", "retryable"]
            .into_iter()
            .collect()
    );
}

#[test]
fn preview_request_rejects_renderer_supplied_path_or_endpoint_fields() {
    let raw = format!(
        r#"{{"schema_version":1,"request_id":"{REQUEST_ID}","installation_id":"{ENDPOINT_ID}","scan_generation":1,"scope":"user_config","path":"D:\\private\\settings.json","endpoint":"http://127.0.0.1:1"}}"#
    );
    assert!(serde_json::from_str::<PreviewRequest>(&raw).is_err());
}
