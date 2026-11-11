use super::discovery::{
    claude_code_cli_version_from_digest, codex_cli_version_from_digest,
    opencode_v1_cli_version_from_digest, DiscoveryInputs, DiscoveryManager, DiscoveryReply,
    InstallationSelectionError, RememberInstallationChoiceRequest, SavedInstallationChoiceReply,
    SavedInstallationChoiceRequest,
};
#[cfg(windows)]
use crate::state::choices::{ChoiceClient, ChoiceOrigin, ChoiceStore, ChoiceSurface};
use std::fs;
use std::path::{Component, Path, PathBuf};
use uuid::Uuid;

const REQUEST_ID: &str = "7132bced-3ce0-4c1b-8f48-d93d7b96f7ce";
const RUNS_RELATIVE: &str = "work/proofs/discovery/runs";

#[cfg(windows)]
#[test]
#[ignore = "opt-in read-only inspection of explicitly selected installed signed binaries"]
fn codex_current_installed_binaries_are_recognized_and_pinned() {
    for key in ["HEADROOM_TEST_CODEX_CLI", "HEADROOM_TEST_CODEX_BACKEND"] {
        let path = PathBuf::from(std::env::var_os(key).expect("explicit installed test path"));
        let version =
            super::discovery::verified_codex_cli_version(&path).expect("recognized OpenAI binary");
        assert!(super::codex_binary::cli_config_supported(&version));
        let pin =
            super::discovery::pin_verified_codex_cli(&path, &version).expect("same pinned version");
        assert!(pin.metadata().unwrap().is_file());
        assert!(super::discovery::pin_verified_codex_cli(&path, "0.0.0").is_none());
    }
}

struct Fixture {
    project_root: PathBuf,
    runs_root: PathBuf,
    uuid: String,
    path: PathBuf,
    cleaned: bool,
}

impl Fixture {
    fn new() -> Self {
        let project_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .canonicalize()
            .unwrap();
        assert_safe_directory(&project_root, &project_root);
        let mut runs = project_root.clone();
        for component in Path::new(RUNS_RELATIVE).components() {
            let Component::Normal(part) = component else {
                panic!("fixture path must be a fixed relative path");
            };
            runs.push(part);
            match fs::symlink_metadata(&runs) {
                Ok(_) => {
                    assert_safe_directory(&runs, &project_root);
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    // The already-checked parent is the only location used by this create.
                    fs::create_dir(&runs).expect("create one checked fixture ancestor");
                    assert_safe_directory(&runs, &project_root);
                }
                Err(error) => panic!("fixture ancestor metadata failed: {error:?}"),
            }
        }
        let runs_root = runs.canonicalize().expect("canonical runs root");
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

    fn at(&self, relative: impl AsRef<Path>) -> PathBuf {
        let path = self.path.join(relative);
        let raw = path.as_os_str().to_string_lossy();
        raw.strip_prefix(r"\\?\")
            .filter(|local| {
                let bytes = local.as_bytes();
                bytes.len() >= 3
                    && bytes[0].is_ascii_alphabetic()
                    && bytes[1] == b':'
                    && matches!(bytes[2], b'\\' | b'/')
            })
            .map(PathBuf::from)
            .unwrap_or(path)
    }

    fn cleanup(&mut self) {
        if self.cleaned {
            return;
        }
        assert_safe_runs_chain(&self.project_root, &self.runs_root);
        let original_metadata = fs::symlink_metadata(&self.path).expect("fixture UUID metadata");
        assert!(original_metadata.is_dir());
        assert!(!original_metadata.file_type().is_symlink());
        assert_no_reparse(&original_metadata);
        let root = self.path.canonicalize().expect("fixture root exists");
        let expected_root = self.runs_root.join(&self.uuid);
        assert!(root.starts_with(&self.runs_root));
        assert_eq!(test_path_key(&root), test_path_key(&expected_root));
        let mut pending = vec![root.clone()];
        while let Some(path) = pending.pop() {
            let metadata = fs::symlink_metadata(&path).expect("fixture metadata");
            assert_no_reparse(&metadata);
            assert!(path.canonicalize().unwrap().starts_with(&root));
            if metadata.is_dir() {
                for child in fs::read_dir(&path).expect("fixture descendants") {
                    pending.push(child.expect("fixture entry").path());
                }
            }
        }
        assert_safe_runs_chain(&self.project_root, &self.runs_root);
        let original_metadata = fs::symlink_metadata(&self.path).expect("fixture UUID metadata");
        assert!(original_metadata.is_dir());
        assert_no_reparse(&original_metadata);
        let confirmed_root = self
            .path
            .canonicalize()
            .expect("fixture UUID canonical path");
        assert_eq!(
            test_path_key(&confirmed_root),
            test_path_key(&expected_root)
        );
        assert!(confirmed_root.starts_with(&self.runs_root));
        fs::remove_dir_all(&root).expect("remove only this UUID fixture");
        self.cleaned = true;
    }
}

fn assert_safe_runs_chain(project_root: &Path, runs_root: &Path) {
    let mut current = project_root.to_path_buf();
    assert_safe_directory(&current, project_root);
    for component in Path::new(RUNS_RELATIVE).components() {
        let Component::Normal(part) = component else {
            panic!("fixture path must be a fixed relative path");
        };
        current.push(part);
        assert_safe_directory(&current, project_root);
    }
    let canonical_runs = current.canonicalize().expect("canonical runs root");
    assert_eq!(test_path_key(&canonical_runs), test_path_key(runs_root));
}

fn assert_safe_directory(path: &Path, boundary: &Path) {
    let metadata = fs::symlink_metadata(path).expect("fixture directory metadata");
    assert!(metadata.is_dir());
    assert_no_reparse(&metadata);
    let canonical = path
        .canonicalize()
        .expect("fixture directory canonical path");
    assert!(canonical.starts_with(boundary));
}

fn assert_no_reparse(metadata: &fs::Metadata) {
    assert!(!metadata.file_type().is_symlink());
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        assert_eq!(
            metadata.file_attributes() & 0x400,
            0,
            "fixture contains reparse point"
        );
    }
}

fn test_path_key(path: &Path) -> String {
    path.to_string_lossy().to_lowercase()
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

#[cfg(target_os = "macos")]
fn make_executable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
}

fn snapshot(reply: DiscoveryReply) -> super::discovery::DiscoverySnapshot {
    match reply {
        DiscoveryReply::Snapshot(value) => value,
        DiscoveryReply::Error(_) => panic!("expected discovery snapshot"),
        DiscoveryReply::Cancelled(_) => panic!("expected discovery snapshot"),
    }
}

#[test]
fn recovery_detects_updated_and_missing_installations_without_executing_them() {
    let fixture = Fixture::new();
    let package = fixture.at("npm/node_modules/@openai/codex");
    write_file(&package.join("package.json"), br#"{"name":"@openai/codex","version":"0.163.0-alpha.10.2","bin":{"codex":"bin/codex.js"}}"#);
    let binary = package.join("bin/codex.js");
    write_file(&binary, b"never execute this fixture");
    let manager = DiscoveryManager::new();
    assert!(manager.recovery_scan().is_none());
    let inputs = || DiscoveryInputs {
        npm_roots: vec![fixture.at("npm")],
        ..Default::default()
    };
    snapshot(manager.scan_with_inputs(REQUEST_ID.into(), inputs()));
    let first = manager.recovery_scan().unwrap();
    assert_eq!(first.installations.len(), 1);
    assert_eq!(first.installations[0].1, "current");
    write_file(&binary, b"changed executable metadata; never execute");
    assert_eq!(manager.recovery_scan().unwrap().installations[0].1, "stale");
    snapshot(manager.scan_with_inputs(REQUEST_ID.into(), inputs()));
    assert_eq!(
        manager.recovery_scan().unwrap().installations[0].1,
        "current"
    );
    fs::remove_file(&binary).unwrap();
    assert_eq!(manager.recovery_scan().unwrap().installations[0].1, "stale");
}

#[test]
fn compatibility_updated_installation_requires_revalidation_without_global_version_promises() {
    use crate::compatibility::catalog::{CatalogManager, Request};
    use crate::integrations::config_io::PlanManager;
    use crate::runtime::manager::RuntimeManager;
    let fixture = Fixture::new();
    let package = fixture.at("npm/node_modules/@openai/codex");
    let binary = package.join("bin/codex.js");
    write_file(&package.join("package.json"),br#"{"name":"@openai/codex","version":"0.163.0-alpha.10.2","bin":{"codex":"bin/codex.js"}}"#);
    write_file(&binary, b"never execute this fixture");
    let discovery = DiscoveryManager::new();
    let runtime = RuntimeManager::new(None, None);
    let plans = PlanManager::new(discovery.clone(), runtime.clone());
    let catalog = CatalogManager::default();
    let inputs = || DiscoveryInputs {
        npm_roots: vec![fixture.at("npm")],
        ..Default::default()
    };
    let read = || {
        serde_json::to_value(catalog.read(
            Request {
                schema_version: 1,
                request_id: Uuid::new_v4().to_string(),
            },
            &runtime,
            &discovery,
            &plans,
        ))
        .unwrap()
    };
    snapshot(discovery.scan_with_inputs(REQUEST_ID.into(), inputs()));
    let first = read();
    assert_eq!(first["installations"][0]["freshness"], "current");
    assert_eq!(first["installations"][0]["config_supported"], true);
    assert_eq!(
        first["installations"][0]["historical_evidence_ids"],
        serde_json::json!(["codex-cli-responses-api-fixture"])
    );
    write_file(
        &binary,
        b"changed binary metadata, never execute this fixture",
    );
    let stale = read();
    assert_eq!(stale["installations"][0]["freshness"], "stale");
    assert_eq!(stale["installations"][0]["config_supported"], false);
    assert!(stale["installations"][0]["historical_evidence_ids"]
        .as_array()
        .unwrap()
        .is_empty());
    assert!(stale["context_fingerprint"].is_null());
    write_file(
        &package.join("package.json"),
        br#"{"name":"@openai/codex","version":"0.167.0","bin":{"codex":"bin/codex.js"}}"#,
    );
    snapshot(discovery.scan_with_inputs(REQUEST_ID.into(), inputs()));
    let updated = read();
    assert_eq!(updated["installations"][0]["version"], "0.167.0");
    assert_eq!(updated["installations"][0]["config_supported"], true);
    assert!(updated["installations"][0]["historical_evidence_ids"]
        .as_array()
        .unwrap()
        .is_empty());
    assert_eq!(
        fs::read(&binary).unwrap(),
        b"changed binary metadata, never execute this fixture"
    );
}

#[test]
fn initial_state_is_not_scanned_and_absence_is_not_unsupported() {
    let manager = DiscoveryManager::new();
    let initial = snapshot(manager.list_snapshot(REQUEST_ID.to_owned()));
    assert_eq!(initial.status, "not_scanned");
    assert!(initial.installations.is_empty());
    assert!(initial
        .warnings
        .iter()
        .any(|warning| *warning == "scan_not_run"));

    let scanned =
        snapshot(manager.scan_with_inputs(REQUEST_ID.to_owned(), DiscoveryInputs::default()));
    assert_eq!(scanned.status, "partial");
    assert!(scanned.installations.is_empty());
    assert!(scanned
        .warnings
        .iter()
        .any(|warning| *warning == "scope_partial"));
    assert!(scanned
        .warnings
        .iter()
        .any(|warning| *warning == "absence_is_not_unsupported"));
}

#[cfg(windows)]
#[test]
fn remembered_manual_installation_rebinds_to_fresh_scan_ids() {
    let mut fixture = Fixture::new();
    let executable = fixture.at("manual/codex.exe");
    write_file(&executable, b"stable synthetic Codex executable");
    let store = ChoiceStore::new(&fixture.path, "app-state/onboarding").unwrap();
    let first_manager = DiscoveryManager::with_choice_store(Some(store.clone()));
    let first = snapshot(first_manager.select_client_file_with(
        REQUEST_ID.to_owned(),
        "codex".to_owned(),
        |_| Ok(Some(executable)),
    ));
    assert_eq!(first.installations.len(), 1);
    assert_eq!(first.installations[0].source, "manual_file");
    let first_id = first.installations[0].installation_id.clone();
    let remembered =
        first_manager.remember_installation_choice(RememberInstallationChoiceRequest {
            schema_version: 1,
            request_id: Uuid::new_v4().to_string(),
            scan_generation: first.scan_generation,
            installation_id: first_id.clone(),
        });
    let SavedInstallationChoiceReply::Snapshot(remembered) = remembered else {
        panic!("expected saved-choice response");
    };
    assert_eq!(remembered.status, "saved");

    let reopened_manager = DiscoveryManager::with_choice_store(Some(store));
    let reopened = snapshot(
        reopened_manager.scan_with_inputs(REQUEST_ID.to_owned(), DiscoveryInputs::default()),
    );
    assert_eq!(reopened.installations.len(), 1);
    assert_ne!(reopened.installations[0].installation_id, first_id);
    let saved = reopened_manager.saved_installation_choice(SavedInstallationChoiceRequest {
        schema_version: 1,
        request_id: Uuid::new_v4().to_string(),
        client_kind: "codex".to_owned(),
    });
    let SavedInstallationChoiceReply::Snapshot(saved) = saved else {
        panic!("expected saved-choice snapshot");
    };
    assert_eq!(saved.status, "available");
    assert_eq!(
        saved.installation_id.as_deref(),
        Some(reopened.installations[0].installation_id.as_str())
    );
    fixture.cleanup();
}

#[cfg(windows)]
#[test]
fn replaced_remembered_manual_installation_does_not_select_another_candidate() {
    let mut fixture = Fixture::new();
    let executable = fixture.at("manual/codex.exe");
    write_file(&executable, b"remembered synthetic executable");
    let store = ChoiceStore::new(&fixture.path, "app-state/onboarding").unwrap();
    let choice = ChoiceStore::capture_installation(
        ChoiceClient::Codex,
        ChoiceSurface::Cli,
        ChoiceOrigin::ManualFile,
        executable.clone(),
        None,
    )
    .unwrap();
    store.save_installation(choice).unwrap();

    let moved = fixture.at("manual/old-codex.exe");
    fs::rename(&executable, &moved).unwrap();
    write_file(&executable, b"replacement synthetic executable");
    let alternate = fixture.at("path/codex.exe");
    write_file(&alternate, b"different discovered executable");
    let mut inputs = DiscoveryInputs::default();
    inputs
        .path_entries
        .push(alternate.parent().unwrap().to_path_buf());

    let manager = DiscoveryManager::with_choice_store(Some(store));
    let scanned = snapshot(manager.scan_with_inputs(REQUEST_ID.to_owned(), inputs));
    assert_eq!(scanned.installations.len(), 1);
    assert_ne!(
        test_path_key(
            &manager
                .installation_path(&scanned.installations[0].installation_id)
                .unwrap()
        ),
        test_path_key(&executable)
    );
    let saved = manager.saved_installation_choice(SavedInstallationChoiceRequest {
        schema_version: 1,
        request_id: Uuid::new_v4().to_string(),
        client_kind: "codex".to_owned(),
    });
    let SavedInstallationChoiceReply::Snapshot(saved) = saved else {
        panic!("expected saved-choice snapshot");
    };
    assert_eq!(saved.status, "unavailable");
    assert_eq!(saved.installation_id, None);
    fixture.cleanup();
}

#[cfg(windows)]
#[test]
fn remember_rejects_executable_identity_drift_after_scan() {
    let mut fixture = Fixture::new();
    let bin = fixture.at("path");
    let executable = bin.join("codex.exe");
    write_file(&executable, b"scanned synthetic executable");
    let store = ChoiceStore::new(&fixture.path, "app-state/onboarding").unwrap();
    let manager = DiscoveryManager::with_choice_store(Some(store.clone()));
    let mut inputs = DiscoveryInputs::default();
    inputs.path_entries.push(bin);
    let scanned = snapshot(manager.scan_with_inputs(REQUEST_ID.to_owned(), inputs));
    let installation = scanned
        .installations
        .iter()
        .find(|item| item.client_kind == "codex")
        .unwrap();

    let stale = manager.remember_installation_choice(RememberInstallationChoiceRequest {
        schema_version: 1,
        request_id: Uuid::new_v4().to_string(),
        scan_generation: scanned.scan_generation + 1,
        installation_id: installation.installation_id.clone(),
    });
    assert!(matches!(stale, SavedInstallationChoiceReply::Error(_)));
    assert!(store.read().unwrap().installations.is_empty());

    let unknown_field =
        serde_json::from_value::<RememberInstallationChoiceRequest>(serde_json::json!({
            "schema_version": 1,
            "request_id": Uuid::new_v4().to_string(),
            "scan_generation": scanned.scan_generation,
            "installation_id": installation.installation_id,
            "executable_path": "ignored"
        }));
    assert!(unknown_field.is_err());

    let moved = fixture.at("path/old-codex.exe");
    fs::rename(&executable, &moved).unwrap();
    write_file(&executable, b"replacement executable after scan");
    let reply = manager.remember_installation_choice(RememberInstallationChoiceRequest {
        schema_version: 1,
        request_id: Uuid::new_v4().to_string(),
        scan_generation: scanned.scan_generation,
        installation_id: installation.installation_id.clone(),
    });
    assert!(matches!(reply, SavedInstallationChoiceReply::Error(_)));
    assert!(store.read().unwrap().installations.is_empty());
    fixture.cleanup();
}

#[test]
fn installation_selection_is_bound_to_scan_generation_and_returns_no_path() {
    let mut fixture = Fixture::new();
    let bin = fixture.at("Selected Codex");
    let executable = bin.join(if cfg!(target_os = "macos") {
        "codex"
    } else {
        "codex.cmd"
    });
    write_file(&executable, b"fixture command stub");
    #[cfg(target_os = "macos")]
    make_executable(&executable);
    let manager = DiscoveryManager::new();
    let scanned = snapshot(manager.scan_with_inputs(
        REQUEST_ID.to_owned(),
        DiscoveryInputs {
            path_entries: vec![bin],
            ..DiscoveryInputs::default()
        },
    ));
    let candidate = scanned
        .installations
        .iter()
        .find(|installation| installation.client_kind == "codex")
        .expect("fixture PATH candidate");

    let selected = manager
        .select_installation(&candidate.installation_id, scanned.scan_generation)
        .expect("current opaque installation should resolve to a summary");
    assert_eq!(selected.installation_id, candidate.installation_id);
    assert_eq!(selected.client_kind, "codex");
    assert_eq!(selected.source, "path");
    let serialized = serde_json::to_string(&selected).unwrap();
    assert!(!serialized.contains(&executable.to_string_lossy().to_string()));

    assert!(matches!(
        manager.select_installation("invalid-id", scanned.scan_generation),
        Err(InstallationSelectionError::InvalidInstallationId)
    ));
    assert!(matches!(
        manager.select_installation(&candidate.installation_id, scanned.scan_generation + 1),
        Err(InstallationSelectionError::StaleScanGeneration)
    ));
    assert!(matches!(
        manager.select_installation(&candidate.installation_id, 0),
        Err(InstallationSelectionError::InvalidScanGeneration)
    ));
    let missing_id = Uuid::new_v4().to_string();
    assert!(matches!(
        manager.select_installation(&missing_id, scanned.scan_generation),
        Err(InstallationSelectionError::InstallationNotFound)
    ));
    assert!(matches!(
        DiscoveryManager::new().select_installation(&candidate.installation_id, 1),
        Err(InstallationSelectionError::NotScanned)
    ));
    fixture.cleanup();
}

#[cfg(windows)]
#[test]
fn path_keeps_multiple_installations_and_deduplicates_case_insensitive_same_file() {
    let mut fixture = Fixture::new();
    let first = fixture.at("Bin One");
    let second = fixture.at("Îşlemci 工具");
    write_file(&first.join("codex.cmd"), b"test shim marker");
    write_file(&second.join("codex.cmd"), b"another installation marker");
    let inputs = DiscoveryInputs {
        path_entries: vec![
            first.clone(),
            first.with_file_name("bin one"),
            second.clone(),
        ],
        ..DiscoveryInputs::default()
    };
    let manager = DiscoveryManager::new();
    let result = snapshot(manager.scan_with_inputs(REQUEST_ID.to_owned(), inputs));
    let codex: Vec<_> = result
        .installations
        .iter()
        .filter(|item| item.client_kind == "codex")
        .collect();
    assert_eq!(codex.len(), 2);
    assert!(codex.iter().all(|item| item.version.is_none()));
    assert!(codex
        .iter()
        .all(|item| item.connection_status == "not_verified"));
    assert!(codex.iter().all(|item| item.status == "detected"));
    let serialized = serde_json::to_string(&result).unwrap();
    assert!(!serialized.contains(&fixture.path.to_string_lossy().to_string()));
    let owned_path = manager
        .installation_path(&codex[0].installation_id)
        .unwrap();
    assert!(owned_path.starts_with(fixture.at("")));
    fixture.cleanup();
}

#[test]
fn npm_metadata_is_separate_from_unverified_path_wrappers_and_scan_is_read_only() {
    let mut fixture = Fixture::new();
    let npm = fixture.at("npm");
    let codex = npm.join("node_modules/@openai/codex");
    let claude = npm.join("node_modules/@anthropic-ai/claude-code");
    let opencode = npm.join("node_modules/opencode-ai");
    let codex_manifest = br#"{"name":"@openai/codex","version":"0.163.0-alpha.10.2","bin":{"codex":"bin/codex.js"},"scripts":{"postinstall":"must not run"}}"#;
    let claude_manifest =
        br#"{"name":"@anthropic-ai/claude-code","version":"latest","bin":{"claude":"cli.js"}}"#;
    let oversized_manifest = format!(
        "{{\"name\":\"opencode-ai\",\"version\":\"1.2.3\",\"bin\":{{\"opencode\":\"cli.js\"}},\"padding\":\"{}\"}}",
        "x".repeat(16 * 1024)
    );
    write_file(&codex.join("package.json"), codex_manifest);
    write_file(&codex.join("bin/codex.js"), b"fixture executable data");
    let wrapper_name = if cfg!(target_os = "macos") {
        "codex"
    } else {
        "codex.cmd"
    };
    let wrapper = npm.join(wrapper_name);
    write_file(&wrapper, b"independent PATH wrapper");
    #[cfg(target_os = "macos")]
    make_executable(&wrapper);
    write_file(&claude.join("package.json"), claude_manifest);
    write_file(&claude.join("cli.js"), b"fixture executable data");
    write_file(
        &opencode.join("package.json"),
        oversized_manifest.as_bytes(),
    );

    let manager = DiscoveryManager::new();
    let result = snapshot(manager.scan_with_inputs(
        REQUEST_ID.to_owned(),
        DiscoveryInputs {
            path_entries: vec![npm.clone()],
            npm_roots: vec![npm.clone()],
            ..DiscoveryInputs::default()
        },
    ));
    let codex_entries: Vec<_> = result
        .installations
        .iter()
        .filter(|item| item.client_kind == "codex")
        .collect();
    assert_eq!(codex_entries.len(), 2);
    let package_entry = codex_entries
        .iter()
        .find(|item| item.source == "user_local_npm")
        .unwrap();
    let path_entry = codex_entries
        .iter()
        .find(|item| item.source == "path")
        .unwrap();
    let claude_entry = result
        .installations
        .iter()
        .find(|item| item.client_kind == "claude_code")
        .unwrap();
    assert_eq!(package_entry.version.as_deref(), Some("0.163.0-alpha.10.2"));
    assert_eq!(path_entry.version, None);
    let path_executable = manager
        .installation_path(&path_entry.installation_id)
        .unwrap();
    assert!(path_executable.ends_with(wrapper_name));
    assert_eq!(claude_entry.version, None);
    assert!(!result
        .installations
        .iter()
        .any(|item| item.client_kind == "opencode"));
    assert!(result
        .warnings
        .iter()
        .any(|warning| *warning == "package_metadata_untrusted"));
    assert_eq!(
        fs::read(codex.join("package.json")).unwrap(),
        codex_manifest
    );
    assert_eq!(
        fs::read(claude.join("package.json")).unwrap(),
        claude_manifest
    );
    assert_eq!(
        fs::read(codex.join("bin/codex.js")).unwrap(),
        b"fixture executable data"
    );
    assert_eq!(fs::read(wrapper).unwrap(), b"independent PATH wrapper");
    fixture.cleanup();
}

#[cfg(windows)]
#[test]
fn candidate_scan_is_bounded() {
    let mut fixture = Fixture::new();
    let mut path_entries = Vec::new();
    for index in 0..57 {
        let bin = fixture.at(format!("candidate-{index}"));
        for name in ["codex.exe", "claude.exe", "opencode.exe"] {
            write_file(&bin.join(name), b"fixture only");
        }
        path_entries.push(bin);
    }
    let result = snapshot(DiscoveryManager::new().scan_with_inputs(
        REQUEST_ID.to_owned(),
        DiscoveryInputs {
            path_entries,
            ..DiscoveryInputs::default()
        },
    ));
    assert!(result
        .warnings
        .iter()
        .any(|warning| *warning == "candidate_limit_reached"));
    assert!(result.installations.len() <= 512);
    fixture.cleanup();
}

#[test]
fn path_and_metadata_limits_are_reported_and_network_candidates_are_skipped() {
    let mut fixture = Fixture::new();
    let local = fixture.at("local-bin");
    let executable = local.join(if cfg!(target_os = "macos") {
        "opencode"
    } else {
        "opencode.exe"
    });
    write_file(&executable, b"fixture only");
    #[cfg(target_os = "macos")]
    make_executable(&executable);
    let mut path_entries = vec![local.clone()];
    path_entries.extend((0..80).map(|_| PathBuf::from(r"\\nonexistent.invalid\share")));
    let manager = DiscoveryManager::new();
    let result = snapshot(manager.scan_with_inputs(
        REQUEST_ID.to_owned(),
        DiscoveryInputs {
            path_entries,
            ..DiscoveryInputs::default()
        },
    ));
    assert!(result
        .installations
        .iter()
        .any(|item| item.client_kind == "opencode"));
    assert!(result
        .warnings
        .iter()
        .any(|warning| *warning == "path_entry_limit_reached"));
    assert!(result
        .warnings
        .iter()
        .any(|warning| *warning == "network_location_skipped"));
    assert!(result
        .installations
        .iter()
        .all(|item| item.version.is_none()));
    fixture.cleanup();
}

#[test]
fn fixed_error_dto_rejects_invalid_request_and_preserves_correlation() {
    let manager = DiscoveryManager::new();
    let reply = manager.list_snapshot("invalid".to_owned());
    match reply {
        DiscoveryReply::Error(error) => {
            assert_eq!(error.schema_version, 1);
            assert_eq!(error.status, "error");
            assert_eq!(error.request_id, "");
            assert_eq!(error.error.code, "validation_failed");
            assert_eq!(error.error.retryable, false);
            assert_eq!(error.error.message_key, "discovery.validation_failed");
            assert_eq!(error.error.diagnostic_id, None);
        }
        DiscoveryReply::Snapshot(_) | DiscoveryReply::Cancelled(_) => {
            panic!("invalid request must fail closed")
        }
    }
}

fn select_fixture(manager: &DiscoveryManager, client_kind: &str, path: PathBuf) -> DiscoveryReply {
    manager.select_client_file_with(REQUEST_ID.to_owned(), client_kind.to_owned(), move |_| {
        Ok(Some(path))
    })
}

fn discovery_error_code(reply: &DiscoveryReply) -> &str {
    match reply {
        DiscoveryReply::Error(error) => error.error.code,
        _ => panic!("expected fixed discovery error"),
    }
}

#[test]
fn picker_cancel_keeps_snapshot_generation_and_returns_correlated_cancelled_envelope() {
    let manager = DiscoveryManager::new();
    let before = snapshot(manager.list_snapshot(REQUEST_ID.to_owned()));
    let reply =
        manager.select_client_file_with(REQUEST_ID.to_owned(), "codex".to_owned(), |_| Ok(None));
    assert!(matches!(reply, DiscoveryReply::Cancelled(_)));
    let cancelled = serde_json::to_value(reply).unwrap();
    assert_eq!(cancelled["schema_version"], 1);
    assert_eq!(cancelled["request_id"], REQUEST_ID);
    assert_eq!(cancelled["status"], "cancelled");
    let after = snapshot(manager.list_snapshot(REQUEST_ID.to_owned()));
    assert_eq!(before.status, after.status);
    assert_eq!(before.scan_generation, after.scan_generation);
    assert_eq!(before.installations.len(), after.installations.len());
    assert_eq!(before.warnings, after.warnings);
}

#[test]
fn picker_failure_keeps_previous_snapshot_and_uses_only_fixed_error_metadata() {
    let manager = DiscoveryManager::new();
    let before = snapshot(manager.list_snapshot(REQUEST_ID.to_owned()));
    let failed =
        manager.select_client_file_with(REQUEST_ID.to_owned(), "codex".to_owned(), |_| Err(()));
    assert_eq!(discovery_error_code(&failed), "environment_blocked");
    let serialized = serde_json::to_string(&failed).unwrap();
    assert!(serialized.contains("discovery.environment_blocked"));
    assert!(!serialized.contains("path"));
    let after = snapshot(manager.list_snapshot(REQUEST_ID.to_owned()));
    assert_eq!(before.status, after.status);
    assert_eq!(before.scan_generation, after.scan_generation);
    assert!(after.installations.is_empty());
}

#[test]
fn picker_rejects_invalid_request_client_mismatch_wrong_name_and_unc_without_state_change() {
    let mut fixture = Fixture::new();
    let codex = fixture.at("codex.cmd");
    let claude = fixture.at("claude.exe");
    let wrong_basename = fixture.at("other.exe");
    let wrong_extension = fixture.at("codex.ps1");
    write_file(&codex, b"fixture command stub");
    write_file(&claude, b"fixture executable data");
    write_file(&wrong_basename, b"fixture executable data");
    write_file(&wrong_extension, b"fixture only");
    let manager = DiscoveryManager::new();
    let before = snapshot(manager.list_snapshot(REQUEST_ID.to_owned()));

    let invalid_id =
        manager.select_client_file_with("invalid".to_owned(), "codex".to_owned(), |_| {
            panic!("invalid request must not open the picker")
        });
    assert_eq!(discovery_error_code(&invalid_id), "validation_failed");
    let invalid_kind =
        manager.select_client_file_with(REQUEST_ID.to_owned(), "Codex".to_owned(), |_| {
            panic!("invalid client kind must not open the picker")
        });
    assert_eq!(discovery_error_code(&invalid_kind), "validation_failed");

    let mismatched = select_fixture(&manager, "codex", claude);
    assert_eq!(discovery_error_code(&mismatched), "validation_failed");
    let wrong_name = select_fixture(&manager, "codex", wrong_basename);
    assert_eq!(discovery_error_code(&wrong_name), "validation_failed");
    let wrong_name = select_fixture(&manager, "codex", wrong_extension);
    assert_eq!(discovery_error_code(&wrong_name), "validation_failed");
    let network = select_fixture(
        &manager,
        "codex",
        PathBuf::from(r"\\nonexistent.invalid\share\codex.exe"),
    );
    assert_eq!(discovery_error_code(&network), "validation_failed");

    let after = snapshot(manager.list_snapshot(REQUEST_ID.to_owned()));
    assert_eq!(before.status, after.status);
    assert_eq!(before.scan_generation, after.scan_generation);
    assert!(after.installations.is_empty());
    fixture.cleanup();
}

#[test]
fn manual_picker_candidate_is_unverified_canonical_and_session_memory_only() {
    let mut fixture = Fixture::new();
    let executable = fixture.at("Selected 工具/codex.CMD");
    write_file(&executable, b"fixture command stub");
    let manager = DiscoveryManager::new();
    let result = snapshot(select_fixture(&manager, "codex", executable.clone()));
    assert_eq!(result.status, "partial");
    assert_eq!(result.scan_generation, 1);
    assert_eq!(result.installations.len(), 1);
    let install = &result.installations[0];
    assert_eq!(install.client_kind, "codex");
    assert_eq!(install.surface, "cli");
    assert_eq!(install.version, None);
    assert_eq!(install.source, "manual_file");
    assert_eq!(install.location_label, "Selected file");
    assert_eq!(install.connection_status, "not_verified");
    let path = manager.installation_path(&install.installation_id).unwrap();
    let canonical = executable.canonicalize().unwrap();
    let canonical_text = canonical.to_string_lossy();
    let expected = PathBuf::from(
        canonical_text
            .strip_prefix(r"\\?\")
            .unwrap_or(canonical_text.as_ref()),
    );
    assert_eq!(path, expected);
    let serialized = serde_json::to_string(&result).unwrap();
    assert!(!serialized.contains(&executable.to_string_lossy().to_string()));
    assert!(result
        .warnings
        .iter()
        .any(|warning| *warning == "absence_is_not_unsupported"));
    fixture.cleanup();
}

#[cfg(windows)]
#[test]
fn selecting_a_file_already_found_by_scan_does_not_duplicate_or_advance_generation() {
    let mut fixture = Fixture::new();
    let bin = fixture.at("auto-bin");
    let executable = bin.join("codex.cmd");
    write_file(&executable, b"fixture command stub");
    let manager = DiscoveryManager::new();
    let scanned = snapshot(manager.scan_with_inputs(
        REQUEST_ID.to_owned(),
        DiscoveryInputs {
            path_entries: vec![bin],
            ..DiscoveryInputs::default()
        },
    ));
    let selected = snapshot(select_fixture(&manager, "codex", executable));
    assert_eq!(scanned.scan_generation, selected.scan_generation);
    assert_eq!(selected.installations.len(), 1);
    assert_eq!(selected.installations[0].source, "path");
    let rescanned =
        snapshot(manager.scan_with_inputs(REQUEST_ID.to_owned(), DiscoveryInputs::default()));
    assert_eq!(rescanned.installations.len(), 1);
    assert_eq!(rescanned.installations[0].source, "manual_file");
    fixture.cleanup();
}

#[test]
fn manual_candidates_survive_rescan_and_disappeared_files_are_dropped_with_warning() {
    let mut fixture = Fixture::new();
    let executable = fixture.at("Chosen/opencode.exe");
    write_file(&executable, b"fixture executable data");
    let manager = DiscoveryManager::new();
    let selected = snapshot(select_fixture(&manager, "opencode", executable.clone()));
    assert_eq!(selected.installations.len(), 1);
    let rescanned =
        snapshot(manager.scan_with_inputs(REQUEST_ID.to_owned(), DiscoveryInputs::default()));
    assert_eq!(rescanned.installations.len(), 1);
    assert_eq!(rescanned.installations[0].source, "manual_file");
    assert_eq!(rescanned.scan_generation, selected.scan_generation + 1);

    fs::remove_file(&executable).unwrap();
    let after_gone =
        snapshot(manager.scan_with_inputs(REQUEST_ID.to_owned(), DiscoveryInputs::default()));
    assert!(after_gone.installations.is_empty());
    assert!(after_gone
        .warnings
        .iter()
        .any(|warning| *warning == "candidate_location_unavailable"));
    fixture.cleanup();
}

#[test]
fn local_codex_directory_name_does_not_infer_binary_version() {
    let mut fixture = Fixture::new();
    let local_app_data = fixture.at("Local App Data");
    let executable = local_app_data.join("OpenAI/Codex/bin/0.163.0-alpha.11.1/codex.exe");
    write_file(&executable, b"small unknown executable fixture");

    let result = snapshot(DiscoveryManager::new().scan_with_inputs(
        REQUEST_ID.to_owned(),
        DiscoveryInputs {
            local_app_data: Some(local_app_data),
            ..DiscoveryInputs::default()
        },
    ));

    let codex = result
        .installations
        .iter()
        .find(|installation| installation.source == "user_local_native")
        .expect("bounded local Codex installation should be detected");
    assert_eq!(codex.client_kind, "codex");
    assert_eq!(codex.surface, "cli");
    assert_eq!(
        codex.version, None,
        "directory name is not version evidence"
    );
    fixture.cleanup();
}

#[test]
fn local_codex_scan_is_bounded_to_sixteen_direct_version_directories() {
    let mut fixture = Fixture::new();
    let local_app_data = fixture.at("Local App Data");
    for index in 0..17 {
        let executable = local_app_data
            .join("OpenAI/Codex/bin")
            .join(format!("slot-{index}"))
            .join("codex.exe");
        write_file(&executable, b"small unknown executable fixture");
    }

    let result = snapshot(DiscoveryManager::new().scan_with_inputs(
        REQUEST_ID.to_owned(),
        DiscoveryInputs {
            local_app_data: Some(local_app_data),
            ..DiscoveryInputs::default()
        },
    ));

    assert_eq!(
        result
            .installations
            .iter()
            .filter(|installation| installation.source == "user_local_native")
            .count(),
        16
    );
    assert!(result.warnings.contains(&"candidate_limit_reached"));
    fixture.cleanup();
}

#[test]
fn bundled_codex_version_requires_both_exact_size_and_sha256() {
    const HASH: &str = "e757246b4de87359e660647fa9514955a0971ff6a30affdd8c145d1d5ece7b84";
    const NEXT_HASH: &str = "c81ed464e5e7cafd09a5fa26af390c480bed8e9fab2a87a54b285fbd56e7122a";
    assert_eq!(
        codex_cli_version_from_digest(336_011_568, HASH).as_deref(),
        Some("0.163.0-alpha.11.1")
    );
    assert_eq!(
        codex_cli_version_from_digest(336_134_960, NEXT_HASH).as_deref(),
        Some("0.163.0-alpha.11.2")
    );
    assert_eq!(
        codex_cli_version_from_digest(336_011_567, HASH),
        None,
        "matching digest with a different size must not identify a version"
    );
    assert_eq!(
        codex_cli_version_from_digest(
            336_011_568,
            "f757246b4de87359e660647fa9514955a0971ff6a30affdd8c145d1d5ece7b84"
        ),
        None,
        "matching size with a different digest must remain unknown"
    );
    assert_eq!(
        codex_cli_version_from_digest(336_134_959, NEXT_HASH),
        None,
        "the new digest at a different size must remain unknown"
    );
    assert_eq!(
        codex_cli_version_from_digest(
            336_134_960,
            "d81ed464e5e7cafd09a5fa26af390c480bed8e9fab2a87a54b285fbd56e7122a"
        ),
        None,
        "the new size with a different digest must remain unknown"
    );
}

#[test]
fn bundled_opencode_v1_version_requires_both_exact_size_and_sha256() {
    const HASH: &str = "cab0c9a8480f71fbb7cbc88ad91057712ecb500ab41fb9c01dd586cb9d8ea375";
    assert_eq!(
        opencode_v1_cli_version_from_digest(180_982_664, HASH).as_deref(),
        Some("1.19.3")
    );
    assert_eq!(
        opencode_v1_cli_version_from_digest(180_982_663, HASH),
        None,
        "the exact digest at a different size must remain unknown"
    );
    assert_eq!(
        opencode_v1_cli_version_from_digest(
            180_982_664,
            "dbb0c9a8480f71fbb7cbc88ad91057712ecb500ab41fb9c01dd586cb9d8ea375"
        ),
        None,
        "the exact size with a different digest must remain unknown"
    );
}

#[cfg(windows)]
#[test]
fn unknown_opencode_exes_remain_unversioned_on_path_native_and_manual_sources() {
    let mut fixture = Fixture::new();
    let path_bin = fixture.at("path-bin");
    let path_exe = path_bin.join("opencode.exe");
    let profile = fixture.at("profile");
    let native_exe = profile.join(".local/bin/opencode.exe");
    let manual_exe = fixture.at("manual/opencode.exe");
    for path in [&path_exe, &native_exe, &manual_exe] {
        write_file(path, b"small unknown executable fixture");
    }

    let path_snapshot = snapshot(DiscoveryManager::new().scan_with_inputs(
        REQUEST_ID.to_owned(),
        DiscoveryInputs {
            path_entries: vec![path_bin],
            ..DiscoveryInputs::default()
        },
    ));
    let path_candidate = path_snapshot
        .installations
        .iter()
        .find(|item| item.client_kind == "opencode")
        .expect("PATH OpenCode executable is detected");
    assert_eq!(path_candidate.source, "path");
    assert_eq!(path_candidate.version, None);

    let native_snapshot = snapshot(DiscoveryManager::new().scan_with_inputs(
        REQUEST_ID.to_owned(),
        DiscoveryInputs {
            user_profile: Some(profile),
            ..DiscoveryInputs::default()
        },
    ));
    let native_candidate = native_snapshot
        .installations
        .iter()
        .find(|item| item.client_kind == "opencode")
        .expect("user-local native OpenCode executable is detected");
    assert_eq!(native_candidate.source, "user_local_native");
    assert_eq!(native_candidate.version, None);

    let manual_snapshot = snapshot(select_fixture(
        &DiscoveryManager::new(),
        "opencode",
        manual_exe,
    ));
    assert_eq!(manual_snapshot.installations.len(), 1);
    assert_eq!(manual_snapshot.installations[0].source, "manual_file");
    assert_eq!(manual_snapshot.installations[0].version, None);
    fixture.cleanup();
}

#[test]
fn small_manual_codex_executable_stays_unknown_across_rescan() {
    let mut fixture = Fixture::new();
    let executable = fixture.at("Chosen/codex.exe");
    write_file(&executable, b"not the pinned Codex executable");
    let manager = DiscoveryManager::new();

    let selected = snapshot(select_fixture(&manager, "codex", executable));
    assert_eq!(selected.installations.len(), 1);
    assert_eq!(selected.installations[0].source, "manual_file");
    assert_eq!(selected.installations[0].version, None);

    let rescanned =
        snapshot(manager.scan_with_inputs(REQUEST_ID.to_owned(), DiscoveryInputs::default()));
    assert_eq!(rescanned.installations.len(), 1);
    assert_eq!(rescanned.installations[0].source, "manual_file");
    assert_eq!(rescanned.installations[0].version, None);
    fixture.cleanup();
}

#[test]
fn zero_byte_cmd_picker_candidate_survives_rescan() {
    let mut fixture = Fixture::new();
    let executable = fixture.at("Selected/opencode.cmd");
    write_file(&executable, b"");
    assert_eq!(fs::metadata(&executable).unwrap().len(), 0);

    let manager = DiscoveryManager::new();
    let selected = snapshot(select_fixture(&manager, "opencode", executable.clone()));
    assert_eq!(selected.status, "partial");
    assert_eq!(selected.installations.len(), 1);
    assert_eq!(selected.installations[0].source, "manual_file");
    assert_eq!(selected.installations[0].version, None);
    assert_eq!(selected.installations[0].location_label, "Selected file");
    let selected_path = manager
        .installation_path(&selected.installations[0].installation_id)
        .unwrap();

    let rescanned =
        snapshot(manager.scan_with_inputs(REQUEST_ID.to_owned(), DiscoveryInputs::default()));
    assert_eq!(rescanned.status, "partial");
    assert_eq!(rescanned.scan_generation, selected.scan_generation + 1);
    assert_eq!(rescanned.installations.len(), 1);
    assert_eq!(rescanned.installations[0].client_kind, "opencode");
    assert_eq!(rescanned.installations[0].source, "manual_file");
    assert_eq!(rescanned.installations[0].version, None);
    assert_eq!(rescanned.installations[0].location_label, "Selected file");
    let rescanned_path = manager
        .installation_path(&rescanned.installations[0].installation_id)
        .unwrap();
    assert_eq!(
        test_path_key(&rescanned_path),
        test_path_key(&selected_path)
    );

    fixture.cleanup();
}

#[test]
#[ignore = "Opt-in: runs only bounded local installation metadata discovery; launches no client."]
fn real_environment_scan_retains_zero_byte_cmd_picker_candidate() {
    let mut fixture = Fixture::new();
    let executable = fixture.at("Selected/opencode.cmd");
    write_file(&executable, b"");
    assert_eq!(fs::metadata(&executable).unwrap().len(), 0);

    let manager = DiscoveryManager::new();
    let selected = snapshot(select_fixture(&manager, "opencode", executable));
    assert_eq!(selected.installations.len(), 1);
    assert_eq!(selected.installations[0].source, "manual_file");
    let selected_path = manager
        .installation_path(&selected.installations[0].installation_id)
        .unwrap();

    let rescanned = snapshot(manager.scan(REQUEST_ID.to_owned()));
    let matching: Vec<_> = rescanned
        .installations
        .iter()
        .filter(|installation| {
            manager
                .installation_path(&installation.installation_id)
                .is_some_and(|path| test_path_key(&path) == test_path_key(&selected_path))
        })
        .collect();
    let candidate_location_unavailable = rescanned
        .warnings
        .iter()
        .any(|warning| *warning == "candidate_location_unavailable");
    println!(
        "bounded discovery metadata: before_count={} after_count={} generation={} retained_count={} retained_source={} candidate_location_unavailable={}",
        selected.installations.len(),
        rescanned.installations.len(),
        rescanned.scan_generation,
        matching.len(),
        matching.first().map(|item| item.source).unwrap_or("absent"),
        candidate_location_unavailable,
    );
    assert_eq!(matching.len(), 1, "selected candidate was not retained");
    assert_eq!(matching[0].source, "manual_file");
    assert_eq!(rescanned.scan_generation, selected.scan_generation + 1);

    fixture.cleanup();
}

#[test]
fn manual_candidate_count_is_bounded() {
    let mut fixture = Fixture::new();
    let manager = DiscoveryManager::new();
    for index in 0..32 {
        let path = fixture.at(format!("manual-{index}/claude.exe"));
        write_file(&path, b"fixture executable data");
        let result = select_fixture(&manager, "claude_code", path);
        assert!(matches!(result, DiscoveryReply::Snapshot(_)));
    }
    let overflow = fixture.at("manual-overflow/claude.exe");
    write_file(&overflow, b"fixture executable data");
    let rejected = select_fixture(&manager, "claude_code", overflow);
    assert_eq!(discovery_error_code(&rejected), "validation_failed");
    let final_snapshot = snapshot(manager.list_snapshot(REQUEST_ID.to_owned()));
    assert_eq!(final_snapshot.installations.len(), 32);
    fixture.cleanup();
}

#[test]
fn scan_and_picker_share_the_mutation_lock_while_snapshot_reads_remain_available() {
    use std::sync::mpsc;
    use std::time::Duration;

    let manager = DiscoveryManager::new();
    let picker_manager = manager.clone();
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let picker = std::thread::spawn(move || {
        picker_manager.select_client_file_with(REQUEST_ID.to_owned(), "codex".to_owned(), |_| {
            entered_tx.send(()).unwrap();
            let _ = release_rx.recv_timeout(Duration::from_secs(3));
            Ok(None)
        })
    });
    if entered_rx.recv_timeout(Duration::from_secs(3)).is_err() {
        let _ = release_tx.send(());
        let _ = picker.join();
        panic!("picker selector was not reached");
    }

    let busy = manager.scan_with_inputs(REQUEST_ID.to_owned(), DiscoveryInputs::default());
    assert_eq!(discovery_error_code(&busy), "busy");
    let readable = snapshot(manager.list_snapshot(REQUEST_ID.to_owned()));
    assert_eq!(readable.status, "not_scanned");
    release_tx.send(()).unwrap();
    assert!(matches!(
        picker.join().unwrap(),
        DiscoveryReply::Cancelled(_)
    ));
    let after = snapshot(manager.list_snapshot(REQUEST_ID.to_owned()));
    assert_eq!(after.status, "not_scanned");
    assert_eq!(after.scan_generation, 0);
}

#[test]
fn bundled_claude_version_requires_both_exact_size_and_sha256() {
    const HASH: &str = "10fa305545b20baf6e074a086762bd252b89dfe7035848a5d41385503b1a6c74";
    assert_eq!(
        claude_code_cli_version_from_digest(236_306_080, HASH).as_deref(),
        Some("2.1.158")
    );
    assert_eq!(
        claude_code_cli_version_from_digest(236_306_079, HASH),
        None,
        "matching digest at any other size must remain unknown"
    );
    assert_eq!(
        claude_code_cli_version_from_digest(
            236_306_080,
            "20fa305545b20baf6e074a086762bd252b89dfe7035848a5d41385503b1a6c74"
        ),
        None,
        "matching size with any other digest must remain unknown"
    );
    assert_eq!(
        claude_code_cli_version_from_digest(0, HASH),
        None,
        "an unknown size must not trigger version identification"
    );
}

#[test]
fn unknown_claude_exe_stays_unversioned_on_path_and_manual_selection() {
    let mut fixture = Fixture::new();
    let path_dir = fixture.at("path-install");
    let path_exe = path_dir.join("claude.exe");
    write_file(&path_exe, b"small synthetic Claude executable");
    let mut inputs = DiscoveryInputs::default();
    inputs.path_entries.push(path_dir);
    let path_result =
        snapshot(DiscoveryManager::new().scan_with_inputs(REQUEST_ID.to_owned(), inputs));
    let path_install = path_result
        .installations
        .iter()
        .find(|item| item.client_kind == "claude_code")
        .expect("the synthetic PATH candidate should be discovered");
    assert_eq!(path_install.source, "path");
    assert_eq!(path_install.version, None);

    let manual_exe = fixture.at("manual/claude.exe");
    write_file(&manual_exe, b"small synthetic Claude executable");
    let manual_result = snapshot(select_fixture(
        &DiscoveryManager::new(),
        "claude_code",
        manual_exe,
    ));
    assert_eq!(manual_result.installations.len(), 1);
    assert_eq!(manual_result.installations[0].source, "manual_file");
    assert_eq!(manual_result.installations[0].version, None);
    fixture.cleanup();
}
