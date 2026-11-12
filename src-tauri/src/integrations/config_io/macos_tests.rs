use super::super::*;
use std::ffi::CString;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{FileTypeExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use uuid::Uuid;

struct Fixture {
    path: PathBuf,
    identity: (u64, u64),
}

impl Fixture {
    fn new() -> Self {
        let proof_root = PathBuf::from(
            std::env::var_os("HEADROOM_MACOS_PROOF_ROOT")
                .expect("CI must provide HEADROOM_MACOS_PROOF_ROOT"),
        );
        assert!(proof_root.is_absolute());
        assert_eq!(fs::canonicalize(&proof_root).unwrap(), proof_root);
        let work = fs::canonicalize(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .unwrap()
                .join("work"),
        )
        .unwrap();
        assert!(proof_root.starts_with(&work) && proof_root != work);
        let info = fs::symlink_metadata(&proof_root).unwrap();
        assert!(info.is_dir() && !info.file_type().is_symlink());
        assert_eq!(info.uid(), unsafe { libc::geteuid() });
        assert_eq!(info.mode() & 0o7777, 0o700);

        let id = Uuid::new_v4().simple().to_string();
        let path = proof_root.join(format!("cf-{}", &id[..8]));
        fs::create_dir(&path).expect("create private config fixture");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
        let info = fs::symlink_metadata(&path).unwrap();
        assert!(info.is_dir() && !info.file_type().is_symlink());
        assert_eq!(info.uid(), unsafe { libc::geteuid() });
        assert_eq!(info.mode() & 0o7777, 0o700);
        assert_eq!(fs::canonicalize(&path).unwrap(), path);
        Self {
            path,
            identity: (info.dev(), info.ino()),
        }
    }

    fn child(&self, name: &str) -> PathBuf {
        self.path.join(name)
    }

    fn codex_inputs(&self, home: &Path) -> ConfigRootInputs {
        ConfigRootInputs {
            codex_home: Some(home.to_path_buf()),
            ..ConfigRootInputs::default()
        }
    }

    fn write(&self, path: &Path, bytes: &[u8], mode: u32) {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(mode)
            .open(path)
            .expect("create fixture file exclusively");
        file.write_all(bytes).expect("write synthetic fixture");
        file.sync_all().expect("flush synthetic fixture");
        fs::set_permissions(path, fs::Permissions::from_mode(mode)).unwrap();
    }

    fn mkdir(&self, path: &Path, mode: u32) {
        fs::create_dir(path).expect("create fixture directory");
        fs::set_permissions(path, fs::Permissions::from_mode(mode)).unwrap();
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let Ok(info) = fs::symlink_metadata(&self.path) else {
            return;
        };
        if !info.is_dir()
            || info.file_type().is_symlink()
            || (info.dev(), info.ino()) != self.identity
            || fs::canonicalize(&self.path).ok().as_deref() != Some(self.path.as_path())
        {
            return;
        }
        let Ok(entries) = fs::read_dir(&self.path) else {
            return;
        };
        for entry in entries.flatten() {
            let entry_path = entry.path();
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if kind.is_dir() {
                if let Ok(children) = fs::read_dir(&entry_path) {
                    for child in children.flatten() {
                        if let Ok(child_kind) = child.file_type() {
                            if child_kind.is_file()
                                || child_kind.is_symlink()
                                || child_kind.is_fifo()
                            {
                                let _ = fs::remove_file(child.path());
                            }
                        }
                    }
                }
                let _ = fs::remove_dir(entry_path);
            } else if kind.is_file() || kind.is_symlink() || kind.is_fifo() {
                let _ = fs::remove_file(entry_path);
            }
        }
        let _ = fs::remove_dir(&self.path);
    }
}

fn codex_config(home: &Path) -> PathBuf {
    home.join("config.toml")
}

fn manager_fixture(
    fixture: &Fixture,
    client: &str,
    home: &Path,
) -> (PlanManager, String, u64, ConfigRootInputs) {
    use crate::integrations::discovery::{DiscoveryInputs, DiscoveryManager, DiscoveryReply};
    let (package, version, bin, discovery_kind) = match client {
        "codex" => ("@openai/codex", "0.163.0-alpha.10.2", "codex", "codex"),
        "claude_code" => (
            "@anthropic-ai/claude-code",
            "2.1.158",
            "claude",
            "claude_code",
        ),
        "opencode_v1" => ("opencode-ai", "1.19.3", "opencode", "opencode"),
        _ => panic!("unknown fixture adapter"),
    };
    let npm = fixture.child("npm");
    let root = npm.join("node_modules").join(package);
    fs::create_dir_all(root.join("bin")).unwrap();
    let manifest =
        serde_json::json!({ "name": package, "version": version, "bin": {(bin): "bin/cli.js"} });
    fixture.write(
        &root.join("package.json"),
        &serde_json::to_vec(&manifest).unwrap(),
        0o600,
    );
    fixture.write(
        &root.join("bin/cli.js"),
        b"metadata fixture only; never execute",
        0o600,
    );
    let discovery = DiscoveryManager::with_choice_store(None);
    let DiscoveryReply::Snapshot(scan) = discovery.scan_with_inputs(
        Uuid::new_v4().to_string(),
        DiscoveryInputs {
            npm_roots: vec![npm],
            ..DiscoveryInputs::default()
        },
    ) else {
        panic!("fixture discovery failed");
    };
    let installation = scan
        .installations
        .iter()
        .find(|item| item.client_kind == discovery_kind)
        .expect("native metadata fixture should be discoverable");
    let mut roots = ConfigRootInputs {
        claude_test_managed_settings: Some(fixture.child("managed-missing.json")),
        claude_test_route_environment: Some(Vec::new()),
        ..ConfigRootInputs::default()
    };
    match client {
        "codex" => roots.codex_home = Some(home.to_owned()),
        "claude_code" => roots.claude_config_dir = Some(home.to_owned()),
        "opencode_v1" => roots.xdg_config_home = Some(home.to_owned()),
        _ => unreachable!(),
    }
    let manager = PlanManager::with_test_sources(discovery, roots.clone(), manager_endpoint())
        .with_ownership_store(
            crate::state::ownership::OwnershipStore::new(
                &fixture.path,
                Path::new("state/ownership"),
            )
            .unwrap(),
        );
    (
        manager,
        installation.installation_id.clone(),
        scan.scan_generation,
        roots,
    )
}

fn manager_endpoint() -> Result<
    crate::runtime::manager::ReadyStableEndpoint,
    crate::runtime::manager::StableEndpointAccessError,
> {
    Ok(crate::runtime::manager::ReadyStableEndpoint {
        endpoint_id: "019a0000-0000-7000-8000-d00000000042".to_owned(),
        host: "127.0.0.1",
        port: 41731,
    })
}

fn manager_plan(
    manager: &PlanManager,
    installation_id: String,
    scan_generation: u64,
) -> crate::integrations::plan::ConnectionPlan {
    manager_plan_in_scope(manager, installation_id, scan_generation, "user_config")
}

fn manager_plan_in_scope(
    manager: &PlanManager,
    installation_id: String,
    scan_generation: u64,
    scope: &str,
) -> crate::integrations::plan::ConnectionPlan {
    match manager.preview(PreviewRequest {
        schema_version: 1,
        request_id: Uuid::new_v4().to_string(),
        installation_id,
        scan_generation,
        scope: scope.into(),
    }) {
        PlanReply::Preview(reply) => {
            assert!(reply.plan.blockers.is_empty(), "fixture plan is blocked");
            reply.plan
        }
        PlanReply::Error(_) => panic!("fixture preview failed"),
    }
}

fn manager_success(reply: ConnectionActionReply) -> ConnectionActionSuccess {
    match reply {
        ConnectionActionReply::Success(reply) => reply,
        ConnectionActionReply::Error(reply) => panic!("native action failed: {}", reply.error.code),
    }
}

#[test]
fn native_manager_apply_and_reopened_undo_for_all_adapters() {
    for (client, relative, original) in [
        ("codex", "config.toml", b"model = 'any-main-model'\n# keep reasoning and agents unchanged\n".as_slice()),
        ("claude_code", "settings.json", b"{\"model\":\"any-main-model\",\"env\":{\"KEEP_THIS\":\"yes\"}}".as_slice()),
        ("opencode_v1", "opencode/opencode.json", b"{\"provider\":{\"openai\":{\"npm\":\"@ai-sdk/openai\",\"models\":{\"any-main-model\":{}}}}}".as_slice()),
    ] {
        let fixture = Fixture::new();
        let home = fixture.child("client");
        fixture.mkdir(&home, 0o755);
        let config = home.join(relative);
        if config.parent().unwrap() != home { fixture.mkdir(config.parent().unwrap(), 0o755); }
        fixture.write(&config, original, 0o644);
        let (manager, id, generation, roots) = manager_fixture(&fixture, client, &home);
        let plan = manager_plan(&manager, id, generation);
        let applied = manager_success(manager.apply_connection(ApplyConnectionRequest {
            schema_version: 1, request_id: Uuid::new_v4().to_string(), plan_id: plan.plan_id,
            expected_config_fingerprint: plan.config_fingerprint,
        }));
        assert_eq!(applied.status, "applied");
        let owned = applied.ownership.unwrap();
        let discovery = manager.discovery.clone();
        drop(manager);
        let reopened = PlanManager::with_test_sources(discovery, roots, manager_endpoint())
            .with_ownership_store(crate::state::ownership::OwnershipStore::new(&fixture.path, Path::new("state/ownership")).unwrap());
        let snapshot = manager_success(reopened.ownership_snapshot(OwnershipSnapshotRequest {
            schema_version: 1, request_id: Uuid::new_v4().to_string(), client_kind: client.into(),
        }));
        assert_eq!(snapshot.ownership.unwrap().record_fingerprint, owned.record_fingerprint);
        let released = manager_success(reopened.undo_connection(UndoConnectionRequest {
            schema_version: 1, request_id: Uuid::new_v4().to_string(), client_kind: client.into(),
            owner_id: owned.owner_id, expected_revision: owned.revision, expected_record_fingerprint: owned.record_fingerprint,
        }));
        assert_eq!(released.status, "released");
        assert_eq!(fs::read(&config).unwrap(), original);
        assert_eq!(fs::metadata(&config).unwrap().mode() & 0o7777, 0o644);
    }
}

#[test]
fn native_manager_retained_review_archive_and_reopened_undo_for_all_adapters() {
    for (client, relative, original) in [
        ("codex", "config.toml", b"model = 'any-model'\n".as_slice()),
        (
            "claude_code",
            "settings.json",
            b"{\"model\":\"any-model\"}".as_slice(),
        ),
        (
            "opencode_v1",
            "opencode/opencode.json",
            b"{\"provider\":{\"openai\":{\"npm\":\"@ai-sdk/openai\"}}}".as_slice(),
        ),
    ] {
        let fixture = Fixture::new();
        let home = fixture.child("client");
        fixture.mkdir(&home, 0o755);
        let config = home.join(relative);
        if config.parent().unwrap() != home {
            fixture.mkdir(config.parent().unwrap(), 0o755);
        }
        fixture.write(&config, original, 0o644);
        let (manager, id, generation, roots) = manager_fixture(&fixture, client, &home);
        let read = || RetainedRequest {
            schema_version: 1,
            request_id: Uuid::new_v4().to_string(),
            client_kind: client.into(),
            scope: "user_config".into(),
        };
        let empty = serde_json::to_value(manager.retained_versions(read())).unwrap();
        assert_eq!(empty["active_records"], 0);
        assert_eq!(empty["supported"], true);
        assert!(!fixture.child("state").exists());
        let plan = manager_plan(&manager, id, generation);
        let applied = manager_success(manager.apply_connection(ApplyConnectionRequest {
            schema_version: 1,
            request_id: Uuid::new_v4().to_string(),
            plan_id: plan.plan_id,
            expected_config_fingerprint: plan.config_fingerprint,
        }));
        let owned = applied.ownership.unwrap();
        let current = fs::read(&config).unwrap();
        let review = serde_json::to_value(manager.retained_versions(read())).unwrap();
        assert_eq!(review["selected_records"], 1);
        assert_eq!(review["can_archive"], true);
        let stale =
            serde_json::to_value(manager.archive_retained_versions(ArchiveRetainedRequest {
                schema_version: 1,
                request_id: Uuid::new_v4().to_string(),
                client_kind: client.into(),
                scope: "user_config".into(),
                expected_review_fingerprint: "0".repeat(64),
            }))
            .unwrap();
        assert_eq!(stale["error"]["code"], "conflict");
        let archived =
            serde_json::to_value(manager.archive_retained_versions(ArchiveRetainedRequest {
                schema_version: 1,
                request_id: Uuid::new_v4().to_string(),
                client_kind: client.into(),
                scope: "user_config".into(),
                expected_review_fingerprint: review["review_fingerprint"].as_str().unwrap().into(),
            }))
            .unwrap();
        assert_eq!(archived["status"], "archived");
        assert_eq!(archived["active_records"], 0);
        assert_eq!(archived["completed_archives"], 1);
        assert!(manager.leases.lock().unwrap().is_empty());
        assert_eq!(fs::read(&config).unwrap(), current);
        let discovery = manager.discovery.clone();
        drop(manager);
        let reopened = PlanManager::with_test_sources(discovery, roots, manager_endpoint())
            .with_ownership_store(
                crate::state::ownership::OwnershipStore::new(
                    &fixture.path,
                    Path::new("state/ownership"),
                )
                .unwrap(),
            );
        let reopened_review = serde_json::to_value(reopened.retained_versions(read())).unwrap();
        assert_eq!(reopened_review["completed_archives"], 1);
        let released = manager_success(reopened.undo_connection(UndoConnectionRequest {
            schema_version: 1,
            request_id: Uuid::new_v4().to_string(),
            client_kind: client.into(),
            owner_id: owned.owner_id,
            expected_revision: owned.revision,
            expected_record_fingerprint: owned.record_fingerprint,
        }));
        assert_eq!(released.status, "released");
        assert_eq!(fs::read(&config).unwrap(), original);
    }
}

#[test]
fn native_manager_preserves_unrelated_edit_and_precommit_fault() {
    let fixture = Fixture::new();
    let home = fixture.child("client");
    fixture.mkdir(&home, 0o755);
    let config = home.join("config.toml");
    let original = b"model = 'arbitrary-main-model'\n";
    fixture.write(&config, original, 0o644);
    let (manager, id, generation, _) = manager_fixture(&fixture, "codex", &home);
    let plan = manager_plan(&manager, id.clone(), generation);
    let manager = manager.with_action_commit_fault();
    assert!(matches!(
        manager.apply_connection(ApplyConnectionRequest {
            schema_version: 1,
            request_id: Uuid::new_v4().to_string(),
            plan_id: plan.plan_id,
            expected_config_fingerprint: plan.config_fingerprint,
        }),
        ConnectionActionReply::Error(_)
    ));
    assert_eq!(fs::read(&config).unwrap(), original);
    assert!(!fixture.child("state/ownership/codex.json").exists());
    let discovery = manager.discovery.clone();
    let roots = (manager.roots_provider)();
    drop(manager);
    let manager = PlanManager::with_test_sources(discovery, roots, manager_endpoint())
        .with_ownership_store(
            crate::state::ownership::OwnershipStore::new(
                &fixture.path,
                Path::new("state/ownership"),
            )
            .unwrap(),
        );
    let plan = manager_plan(&manager, id, generation);
    let owned = manager_success(manager.apply_connection(ApplyConnectionRequest {
        schema_version: 1,
        request_id: Uuid::new_v4().to_string(),
        plan_id: plan.plan_id,
        expected_config_fingerprint: plan.config_fingerprint,
    }))
    .ownership
    .unwrap();
    let mut edited = fs::read(&config).unwrap();
    edited.extend_from_slice(b"# unrelated user edit\n");
    fs::write(&config, edited).unwrap();
    let released = manager_success(manager.undo_connection(UndoConnectionRequest {
        schema_version: 1,
        request_id: Uuid::new_v4().to_string(),
        client_kind: "codex".into(),
        owner_id: owned.owner_id,
        expected_revision: owned.revision,
        expected_record_fingerprint: owned.record_fingerprint,
    }));
    assert_eq!(released.status, "released");
    assert_eq!(
        fs::read(&config).unwrap(),
        [original.as_slice(), b"# unrelated user edit\n"].concat()
    );
}

#[test]
#[ignore = "requires actual native panel selections from the bounded AppKit proof"]
fn native_picker_selections_reopen_in_actual_managers() {
    use crate::integrations::discovery::{
        DiscoveryInputs, DiscoveryReply, RememberInstallationChoiceRequest,
        SavedInstallationChoiceReply,
    };
    use crate::state::choices::ChoiceStore;

    let picker_root = PathBuf::from(std::env::var_os("HEADROOM_MACOS_PICKER_PROOF_ROOT").unwrap());
    assert_eq!(picker_root.canonicalize().unwrap(), picker_root);
    let work = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("work");
    assert!(picker_root.starts_with(work.canonicalize().unwrap()));
    let metadata = fs::symlink_metadata(&picker_root).unwrap();
    assert_eq!(metadata.uid(), unsafe { libc::geteuid() });
    assert_eq!(metadata.mode() & 0o7777, 0o700);
    let bytes = fs::read(picker_root.join("selections.json")).unwrap();
    assert!(bytes.len() < 8192);
    let selection: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let executable = PathBuf::from(selection["cli_file"].as_str().unwrap());
    let project = PathBuf::from(selection["project_directory"].as_str().unwrap());
    assert_eq!(executable, picker_root.join("CLI 工具/codex"));
    assert_eq!(project, picker_root.join("Claude 项目"));
    assert_eq!(executable.canonicalize().unwrap(), executable);
    assert_eq!(project.canonicalize().unwrap(), project);

    let fixture = Fixture::new();
    let choices_path = fixture.child("picker-app-data");
    let open_choices = || ChoiceStore::for_app_local_data(Some(choices_path.clone())).unwrap();
    let discovery = DiscoveryManager::with_choice_store(Some(open_choices()));
    let DiscoveryReply::Snapshot(selected) =
        discovery.select_client_file_with(Uuid::new_v4().to_string(), "codex".into(), |_| {
            Ok(Some(executable.clone()))
        })
    else {
        panic!("native panel CLI result was rejected");
    };
    let row = &selected.installations[0];
    assert_eq!(row.version.as_deref(), Some("0.163.0-alpha.11.1"));
    assert!(matches!(
        discovery.remember_installation_choice(RememberInstallationChoiceRequest {
            schema_version: 1,
            request_id: Uuid::new_v4().to_string(),
            scan_generation: selected.scan_generation,
            installation_id: row.installation_id.clone(),
        }),
        SavedInstallationChoiceReply::Snapshot(_)
    ));
    drop(discovery);
    let discovery = DiscoveryManager::with_choice_store(Some(open_choices()));
    let DiscoveryReply::Snapshot(refreshed) =
        discovery.scan_with_inputs(Uuid::new_v4().to_string(), DiscoveryInputs::default())
    else {
        panic!("selected CLI could not be reopened");
    };
    assert_eq!(refreshed.installations.len(), 1);
    assert_eq!(refreshed.installations[0].source, "manual_file");
    assert_eq!(
        refreshed.installations[0].version.as_deref(),
        row.version.as_deref()
    );

    let home = fixture.child("claude-config");
    fixture.mkdir(&home, 0o700);
    let (manager, _, _, roots) = manager_fixture(&fixture, "claude_code", &home);
    let picked_project = project.clone();
    let manager = manager
        .with_claude_choice_store(Some(open_choices()))
        .with_claude_project_picker(move || Ok(Some(picked_project.clone())));
    let reply = manager.select_claude_scope(ClaudeScopeSelectionRequest {
        schema_version: 1,
        request_id: Uuid::new_v4().to_string(),
        scope: "project_local_config".into(),
    });
    assert_eq!(serde_json::to_value(reply).unwrap()["status"], "selected");
    let discovery = manager.discovery.clone();
    drop(manager);
    let reopened = PlanManager::with_test_sources(discovery, roots, manager_endpoint())
        .with_claude_choice_store(Some(open_choices()));
    let (roots, scope) = reopened
        .config_roots_for_context(ClientKind::ClaudeCode)
        .unwrap();
    assert_eq!(scope, ConfigScope::ProjectLocalConfig);
    assert!(matches!(roots.target(ClientKind::ClaudeCode).unwrap(),
        ConfigTarget::Single { path, .. } if path == project.join(".claude/settings.local.json")));
    assert!(fs::read_dir(&project).unwrap().next().is_none());
}

#[test]
fn native_manager_missing_claude_project_directory_reopens_and_preserves_user_additions() {
    let fixture = Fixture::new();
    let home = fixture.child("user-config");
    fixture.mkdir(&home, 0o700);
    let project = fixture.child("project");
    fixture.mkdir(&project, 0o700);
    let (manager, id, generation, roots) = manager_fixture(&fixture, "claude_code", &home);
    let picker_project = project.clone();
    let choices_path = fixture.child("choice-app-data");
    let open_choices = || {
        crate::state::choices::ChoiceStore::for_app_local_data(Some(choices_path.clone())).unwrap()
    };
    let manager = manager
        .with_claude_choice_store(Some(open_choices()))
        .with_claude_project_picker(move || Ok(Some(picker_project.clone())));
    assert!(!choices_path.exists());
    let selection = manager.select_claude_scope(ClaudeScopeSelectionRequest {
        schema_version: 1,
        request_id: Uuid::new_v4().to_string(),
        scope: "project_local_config".into(),
    });
    assert_eq!(
        serde_json::to_value(selection).unwrap()["status"],
        "selected"
    );
    let plan = manager_plan_in_scope(&manager, id, generation, "project_local_config");
    assert_eq!(plan.config_presence, ConfigPresence::Missing);
    assert!(
        !project.join(".claude").exists(),
        "preview must not create a client directory"
    );
    let applied = manager_success(manager.apply_connection(ApplyConnectionRequest {
        schema_version: 1,
        request_id: Uuid::new_v4().to_string(),
        plan_id: plan.plan_id,
        expected_config_fingerprint: plan.config_fingerprint,
    }));
    let owned = applied.ownership.unwrap();
    assert_eq!(owned.scope, "project_local_config");
    let config = project.join(".claude/settings.local.json");
    assert_eq!(
        fs::metadata(config.parent().unwrap()).unwrap().mode() & 0o7777,
        0o700
    );
    assert_eq!(fs::metadata(&config).unwrap().mode() & 0o7777, 0o600);
    let mut user_edit: serde_json::Value =
        serde_json::from_slice(&fs::read(&config).unwrap()).unwrap();
    user_edit["model"] = serde_json::json!("arbitrary-user-model");
    user_edit["env"]["USER_SETTING"] = serde_json::json!("keep");
    fs::write(&config, serde_json::to_vec_pretty(&user_edit).unwrap()).unwrap();
    let note = project.join(".claude/user-note.txt");
    fixture.write(&note, b"user additions stay", 0o600);
    let discovery = manager.discovery.clone();
    drop(manager);
    let reopened = PlanManager::with_test_sources(discovery, roots, manager_endpoint())
        .with_ownership_store(
            crate::state::ownership::OwnershipStore::new(
                &fixture.path,
                Path::new("state/ownership"),
            )
            .unwrap(),
        )
        .with_claude_choice_store(Some(open_choices()));
    let (reopened_roots, reopened_scope) = reopened
        .config_roots_for_context(ClientKind::ClaudeCode)
        .unwrap();
    assert_eq!(reopened_scope, ConfigScope::ProjectLocalConfig);
    assert!(
        matches!(reopened_roots.target(ClientKind::ClaudeCode).unwrap(), ConfigTarget::Single { path, .. } if path == config)
    );
    let released = manager_success(reopened.undo_connection(UndoConnectionRequest {
        schema_version: 1,
        request_id: Uuid::new_v4().to_string(),
        client_kind: "claude_code".into(),
        owner_id: owned.owner_id,
        expected_revision: owned.revision,
        expected_record_fingerprint: owned.record_fingerprint,
    }));
    assert_eq!(released.status, "released");
    let after: serde_json::Value = serde_json::from_slice(&fs::read(&config).unwrap()).unwrap();
    assert_eq!(after["model"], "arbitrary-user-model");
    assert_eq!(after["env"]["USER_SETTING"], "keep");
    assert!(after["env"].get("ANTHROPIC_BASE_URL").is_none());
    assert_eq!(fs::read(note).unwrap(), b"user additions stay");
    assert!(
        !home.join("settings.json").exists(),
        "user scope is unchanged"
    );
}

#[test]
fn native_manager_missing_claude_project_directory_precommit_fault_is_passive() {
    let fixture = Fixture::new();
    let home = fixture.child("user-config");
    fixture.mkdir(&home, 0o700);
    let project = fixture.child("project");
    fixture.mkdir(&project, 0o700);
    let (manager, id, generation, _) = manager_fixture(&fixture, "claude_code", &home);
    let selected_project = project.clone();
    let manager = manager
        .with_claude_project_picker(move || Ok(Some(selected_project.clone())))
        .with_action_commit_fault();
    let selection = manager.select_claude_scope(ClaudeScopeSelectionRequest {
        schema_version: 1,
        request_id: Uuid::new_v4().to_string(),
        scope: "project_local_config".into(),
    });
    assert_eq!(
        serde_json::to_value(selection).unwrap()["status"],
        "selected"
    );
    let plan = manager_plan_in_scope(&manager, id, generation, "project_local_config");
    assert!(matches!(
        manager.apply_connection(ApplyConnectionRequest {
            schema_version: 1,
            request_id: Uuid::new_v4().to_string(),
            plan_id: plan.plan_id,
            expected_config_fingerprint: plan.config_fingerprint,
        }),
        ConnectionActionReply::Error(_)
    ));
    assert!(!project.join(".claude").exists());
    assert!(!fixture.child("state/ownership/claude_code.json").exists());
}

fn assert_supported_directory_chain(path: &Path) {
    for (depth, directory) in path.ancestors().enumerate() {
        if super::open_directory(directory).is_err() {
            let info = fs::symlink_metadata(directory).unwrap();
            let open_errno = OpenOptions::new()
                .read(true)
                .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW_ANY | libc::O_CLOEXEC)
                .open(directory)
                .err()
                .and_then(|error| error.raw_os_error());
            panic!(
                "directory-chain depth={depth} uid={} euid={} mode={:o} open_errno={open_errno:?}",
                info.uid(),
                unsafe { libc::geteuid() },
                info.mode()
            );
        }
    }
}

#[test]
fn codex_read_preserves_exact_bytes_and_ordinary_user_permissions() {
    const CONFIG: &[u8] = b"# synthetic user configuration\nmodel = \"fixture-model\"\nmodel_reasoning_effort = \"high\"\nopenai_base_url = \"http://127.0.0.1:43127/v1\"\n# auth sentinel must remain opaque\napi_key = \"AUTH_SENTINEL_DO_NOT_LOG\"\n";

    let fixture = Fixture::new();
    let home = fixture.child(".codex");
    fixture.mkdir(&home, 0o755);
    let path = codex_config(&home);
    fixture.write(&path, CONFIG, 0o644);
    let inputs = fixture.codex_inputs(&home);

    assert_supported_directory_chain(&home);
    let read = read_user_config(ClientKind::Codex, &inputs).expect("read user Codex config");
    assert_eq!(read.presence, ConfigPresence::Present);
    assert_eq!(read.bytes, CONFIG);
    let pinned = read_codex_config_pinned(&inputs).expect("pin user Codex config");
    assert_eq!(pinned.presence, ConfigPresence::Present);
    assert_eq!(pinned.bytes, CONFIG);
    assert!(pinned.validate().is_ok());
    assert_eq!(fs::metadata(&home).unwrap().mode() & 0o777, 0o755);
    assert_eq!(fs::metadata(&path).unwrap().mode() & 0o777, 0o644);
    assert_eq!(fs::read(path).unwrap(), CONFIG);
}

#[test]
fn codex_pin_invalidates_after_same_byte_inode_replacement_and_in_place_edit() {
    const CONFIG: &[u8] = b"model = \"fixture-model\"\nmodel_reasoning_effort = \"high\"\n";

    let fixture = Fixture::new();
    let home = fixture.child(".codex");
    fixture.mkdir(&home, 0o700);
    let path = codex_config(&home);
    fixture.write(&path, CONFIG, 0o600);
    let inputs = fixture.codex_inputs(&home);

    let before = read_user_config(ClientKind::Codex, &inputs).unwrap();
    let pinned = read_codex_config_pinned(&inputs).unwrap();
    assert!(pinned.validate().is_ok());
    let replacement = fixture.child("replacement.toml");
    fixture.write(&replacement, CONFIG, 0o600);
    let old_identity = fs::metadata(&path).unwrap();
    fs::rename(&replacement, &path).expect("atomically replace synthetic config");
    let new_identity = fs::metadata(&path).unwrap();
    assert_ne!(
        (old_identity.dev(), old_identity.ino()),
        (new_identity.dev(), new_identity.ino())
    );
    let after = read_user_config(ClientKind::Codex, &inputs).unwrap();
    assert_eq!(before.bytes, after.bytes);
    assert_ne!(before.fingerprint, after.fingerprint);
    assert!(pinned.validate().is_err());

    let current = read_codex_config_pinned(&inputs).unwrap();
    assert!(current.validate().is_ok());
    let mut edited = OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(&path)
        .unwrap();
    edited
        .write_all(b"model = \"edited-model\"\nmodel_reasoning_effort = \"high\"\n")
        .unwrap();
    edited.sync_all().unwrap();
    assert!(current.validate().is_err());

    let before_parent_change = read_codex_config_pinned(&inputs).unwrap();
    let moved = fixture.child("moved-codex");
    fs::rename(&home, &moved).unwrap();
    fixture.mkdir(&home, 0o700);
    fixture.write(&path, CONFIG, 0o600);
    assert!(before_parent_change.validate().is_err());
}

#[test]
fn missing_codex_config_is_read_only_and_pin_detects_later_creation() {
    let fixture = Fixture::new();
    let absent_home = fixture.child("not-created").join(".codex");
    let absent_inputs = fixture.codex_inputs(&absent_home);
    let absent = read_user_config(ClientKind::Codex, &absent_inputs).unwrap();
    assert_eq!(absent.presence, ConfigPresence::Missing);
    assert!(!absent_home.exists());

    let home = fixture.child(".codex");
    fixture.mkdir(&home, 0o700);
    let inputs = fixture.codex_inputs(&home);
    let missing = read_user_config(ClientKind::Codex, &inputs).unwrap();
    assert_eq!(missing.presence, ConfigPresence::Missing);
    let pinned = read_codex_config_pinned(&inputs).expect("pin missing leaf under existing parent");
    assert_eq!(pinned.presence, ConfigPresence::Missing);
    assert!(pinned.validate().is_ok());
    let created = codex_config(&home);
    fixture.write(&created, b"model = \"created-later\"\n", 0o600);
    assert!(pinned.validate().is_err());
}

#[test]
fn codex_reader_rejects_symlink_hardlink_and_fifo_entries_without_blocking() {
    let fixture = Fixture::new();
    let real_home = fixture.child("real-codex");
    fixture.mkdir(&real_home, 0o700);
    let target = fixture.child("target.toml");
    fixture.write(&target, b"model = \"target\"\n", 0o600);

    let symlink_home = fixture.child("symlink-codex");
    std::os::unix::fs::symlink(&real_home, &symlink_home).unwrap();
    let nested = real_home.join("nested");
    fixture.mkdir(&nested, 0o700);
    assert!(super::open_directory(&symlink_home.join("nested")).is_err());
    fs::remove_dir(nested).unwrap();
    let symlink_inputs = fixture.codex_inputs(&symlink_home);
    assert!(matches!(
        read_user_config(ClientKind::Codex, &symlink_inputs),
        Err(ConfigIoError::UnsafePath)
    ));

    let symlink_leaf = codex_config(&real_home);
    std::os::unix::fs::symlink(&target, &symlink_leaf).unwrap();
    let real_inputs = fixture.codex_inputs(&real_home);
    assert!(matches!(
        read_codex_config_pinned(&real_inputs),
        Err(ConfigIoError::UnsafePath)
    ));
    fs::remove_file(&symlink_leaf).unwrap();

    let hardlinked = codex_config(&real_home);
    fixture.write(&hardlinked, b"model = \"linked\"\n", 0o600);
    let second_link = fixture.child("second-link.toml");
    fs::hard_link(&hardlinked, &second_link).unwrap();
    assert!(matches!(
        read_user_config(ClientKind::Codex, &real_inputs),
        Err(ConfigIoError::UnsafePath)
    ));
    fs::remove_file(second_link).unwrap();
    fs::remove_file(&hardlinked).unwrap();

    let fifo = codex_config(&real_home);
    let fifo_c = CString::new(fifo.as_os_str().as_bytes()).unwrap();
    assert_eq!(unsafe { libc::mkfifo(fifo_c.as_ptr(), 0o600) }, 0);
    let start = std::time::Instant::now();
    assert!(matches!(
        read_user_config(ClientKind::Codex, &real_inputs),
        Err(ConfigIoError::UnsafePath)
    ));
    assert!(start.elapsed() < std::time::Duration::from_secs(1));
}

#[test]
fn codex_reader_rejects_group_or_world_writable_config_and_directory() {
    let fixture = Fixture::new();
    let home = fixture.child(".codex");
    fixture.mkdir(&home, 0o755);
    let path = codex_config(&home);
    fixture.write(&path, b"model = \"fixture\"\n", 0o666);
    let inputs = fixture.codex_inputs(&home);
    assert!(matches!(
        read_user_config(ClientKind::Codex, &inputs),
        Err(ConfigIoError::UnsafePath)
    ));
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    fs::set_permissions(&home, fs::Permissions::from_mode(0o777)).unwrap();
    assert!(matches!(
        read_codex_config_pinned(&inputs),
        Err(ConfigIoError::UnsafePath)
    ));
}

#[test]
fn opencode_dual_files_are_ambiguous_and_claude_nested_env_is_read_only() {
    let fixture = Fixture::new();
    let opencode = fixture.child("opencode");
    fixture.mkdir(&opencode, 0o755);
    fixture.write(
        &opencode.join("opencode.json"),
        b"{\"model\":\"fixture\"}\n",
        0o600,
    );
    fixture.write(
        &opencode.join("opencode.jsonc"),
        b"{\"model\":\"other\"}\n",
        0o600,
    );
    let opencode_inputs = ConfigRootInputs {
        xdg_config_home: Some(fixture.path.clone()),
        ..ConfigRootInputs::default()
    };
    assert!(matches!(
        read_user_config(ClientKind::OpenCodeV1, &opencode_inputs),
        Err(ConfigIoError::Ambiguous)
    ));

    let claude = fixture.child("claude");
    fixture.mkdir(&claude, 0o755);
    const SETTINGS: &[u8] = b"{\n  \"env\": {\n    \"ANTHROPIC_BASE_URL\": \"http://127.0.0.1:43127\"\n  },\n  \"model\": \"fixture-model\"\n}\n";
    let settings = claude.join("settings.json");
    fixture.write(&settings, SETTINGS, 0o644);
    let claude_inputs = ConfigRootInputs {
        claude_config_dir: Some(claude),
        ..ConfigRootInputs::default()
    };
    let read = read_user_config(ClientKind::ClaudeCode, &claude_inputs).unwrap();
    assert_eq!(read.presence, ConfigPresence::Present);
    assert_eq!(read.bytes, SETTINGS);
    assert_eq!(fs::read(&settings).unwrap(), SETTINGS);
}

#[test]
fn ownership_factory_uses_the_application_data_root_without_creating_it_during_discovery() {
    let fixture = Fixture::new();
    let root = fixture.child("ownership");
    let store = ownership_store_for_app_local_data(Some(fixture.path.clone())).unwrap();
    assert!(!root.exists());
    store.prepare_for_apply().unwrap();
    assert!(root.is_dir());
    let config = fixture.child("config.toml");
    fixture.write(&config, b"model = 'fixture'\n", 0o600);
    let binding = crate::state::ownership::config_path_binding(&config).unwrap();
    assert_eq!(binding.len(), 64);
    assert_eq!(fs::read(&config).unwrap(), b"model = 'fixture'\n");
}
