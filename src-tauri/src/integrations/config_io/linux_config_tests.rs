use super::*;
use crate::state::ownership::config_path_binding;
use std::ffi::{CString, OsString};
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::os::unix::fs::{symlink, DirBuilderExt, MetadataExt, PermissionsExt};

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let work = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("work")
            .canonicalize()
            .unwrap();
        let root = PathBuf::from(env::var_os("HEADROOM_LINUX_PROOF_ROOT").unwrap());
        assert_eq!(root.canonicalize().unwrap(), root);
        assert!(root.starts_with(&work) && root != work);
        let info = fs::symlink_metadata(&root).unwrap();
        assert_eq!(info.uid(), unsafe { libc::geteuid() });
        assert_eq!(info.mode() & 0o777, 0o700);
        pin_existing_directory_chain(&root).unwrap();
        let path = root.join(Uuid::new_v4().simple().to_string());
        fs::DirBuilder::new().mode(0o700).create(&path).unwrap();
        Self(path)
    }

    fn directory(&self, name: &str) -> PathBuf {
        let path = self.0.join(name);
        fs::DirBuilder::new().mode(0o700).create(&path).unwrap();
        path
    }

    fn codex_roots(&self) -> ConfigRootInputs {
        ConfigRootInputs {
            codex_home: Some(self.0.clone()),
            ..ConfigRootInputs::default()
        }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
#[ignore = "requires an isolated HEADROOM_LINUX_PROOF_ROOT"]
fn reads_user_configs_without_changing_bytes_or_creating_missing_paths() {
    let fixture = Fixture::new();
    let home = fixture.directory("home");
    let xdg = fixture.directory("xdg");
    let roots = ConfigRootInputs {
        user_profile: Some(home.clone()),
        xdg_config_home: Some(xdg.clone()),
        ..ConfigRootInputs::default()
    };
    assert_eq!(
        ConfigRootInputs::from_environment().user_profile,
        env::var_os("HOME").map(PathBuf::from)
    );
    assert!(matches!(
        read_user_config(ClientKind::Codex, &roots)
            .unwrap()
            .presence,
        ConfigPresence::Missing
    ));
    assert!(!home.join(".codex").exists());

    let configs: [(ClientKind, PathBuf, &[u8]); 3] = [
        (
            ClientKind::Codex,
            home.join(".codex/config.toml"),
            b"model = 'fixture'\r\n",
        ),
        (
            ClientKind::ClaudeCode,
            home.join(".claude/settings.json"),
            b"{\"model\": \"fixture\"}\n",
        ),
        (
            ClientKind::OpenCodeV1,
            xdg.join("opencode/opencode.jsonc"),
            b"{\n  // fixture\n}\n",
        ),
    ];
    for (kind, path, bytes) in configs {
        fs::create_dir(path.parent().unwrap()).unwrap();
        fs::write(&path, bytes).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        let read = read_user_config(kind, &roots).unwrap();
        assert_eq!(read.presence, ConfigPresence::Present);
        assert_eq!(read.bytes, bytes);
        assert_eq!(fs::read(&path).unwrap(), bytes);
        assert_eq!(fs::metadata(path).unwrap().mode() & 0o777, 0o644);
    }
    fs::write(xdg.join("opencode/opencode.json"), "{}\n").unwrap();
    assert!(matches!(
        read_user_config(ClientKind::OpenCodeV1, &roots),
        Err(ConfigIoError::Ambiguous)
    ));
}

#[test]
#[ignore = "requires an isolated HEADROOM_LINUX_PROOF_ROOT"]
fn rejects_unsafe_paths_and_file_modes() {
    let fixture = Fixture::new();
    let config = fixture.0.join("config.toml");
    let original = fixture.0.join("original.toml");
    fs::write(&original, "model = 'fixture'\n").unwrap();
    let roots = fixture.codex_roots();
    let assert_unsafe = || {
        assert!(matches!(
            read_user_config(ClientKind::Codex, &roots),
            Err(ConfigIoError::UnsafePath)
        ));
    };

    symlink(&original, &config).unwrap();
    assert_unsafe();
    fs::remove_file(&config).unwrap();
    fs::hard_link(&original, &config).unwrap();
    assert_unsafe();
    fs::remove_file(&config).unwrap();
    let fifo = CString::new(config.as_os_str().as_bytes()).unwrap();
    // SAFETY: fifo points to a NUL-terminated name in the test directory.
    assert_eq!(unsafe { libc::mkfifo(fifo.as_ptr(), 0o600) }, 0);
    assert_unsafe();
    fs::remove_file(&config).unwrap();

    fs::write(&config, "model = 'fixture'\n").unwrap();
    fs::set_permissions(&config, fs::Permissions::from_mode(0o660)).unwrap();
    assert_unsafe();
    fs::set_permissions(&config, fs::Permissions::from_mode(0o600)).unwrap();
    fs::set_permissions(&fixture.0, fs::Permissions::from_mode(0o770)).unwrap();
    assert_unsafe();
    fs::set_permissions(&fixture.0, fs::Permissions::from_mode(0o700)).unwrap();

    let directory = fixture.directory("root");
    symlink(&directory, fixture.0.join("alias")).unwrap();
    let path = fixture.0.join("alias/missing/config.toml");
    assert_eq!(ensure_fixed_volume(&path), Err(ConfigIoError::UnsafePath));
    assert_eq!(
        validate_lexical_path(&fixture.0.join("root/../config.toml")),
        Err(ConfigIoError::UnsafePath)
    );
    assert_eq!(
        ensure_fixed_volume(Path::new("/proc/self")),
        Err(ConfigIoError::InvalidRoot)
    );
}

#[test]
#[ignore = "requires an isolated HEADROOM_LINUX_PROOF_ROOT"]
fn pins_reject_later_creation_and_replacement() {
    let fixture = Fixture::new();
    let home = fixture.directory("home");
    let config = home.join("config.toml");
    let roots = ConfigRootInputs {
        codex_home: Some(home.clone()),
        ..ConfigRootInputs::default()
    };
    let absent = read_codex_config_pinned(&roots).unwrap();
    assert_eq!(absent.presence, ConfigPresence::Missing);
    absent.validate().unwrap();
    let original = b"model = 'fixture'\n";
    fs::write(&config, original).unwrap();
    assert!(absent.validate().is_err());

    let before = read_user_config(ClientKind::Codex, &roots).unwrap();
    let pinned = read_codex_config_pinned(&roots).unwrap();
    pinned.validate().unwrap();
    let replacement = home.join("replaced.toml");
    fs::write(&replacement, original).unwrap();
    fs::rename(&replacement, &config).unwrap();
    assert!(pinned.validate().is_err());
    let after = read_user_config(ClientKind::Codex, &roots).unwrap();
    assert_eq!(before.bytes, after.bytes);
    assert_ne!(before.fingerprint, after.fingerprint);

    let file_pin = read_codex_config_pinned(&roots).unwrap();
    fs::write(&config, b"model = 'fixture2'\n").unwrap();
    assert!(file_pin.validate().is_err());
    let directory_pin = pin_existing_directory_chain(&home).unwrap();
    fs::rename(&home, fixture.0.join("old")).unwrap();
    fixture.directory("home");
    assert!(directory_pin.validate().is_err());
}

#[test]
#[ignore = "requires an isolated HEADROOM_LINUX_PROOF_ROOT"]
fn ownership_uses_private_app_state_and_preserves_distinct_path_bytes() {
    let fixture = Fixture::new();
    let app_data = fixture.0.join("app");
    let store = ownership_store_for_app_local_data(Some(app_data.clone())).unwrap();
    assert!(!app_data.exists());
    store.prepare_for_apply().unwrap();
    let metadata = fs::symlink_metadata(app_data.join("ownership")).unwrap();
    assert_eq!(metadata.mode() & 0o777, 0o700);
    assert_eq!(metadata.uid(), unsafe { libc::geteuid() });
    let ownership = app_data.join("ownership");
    fs::set_permissions(&ownership, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(ownership_store_for_app_local_data(Some(app_data)).is_err());
    assert_eq!(
        fs::symlink_metadata(ownership).unwrap().mode() & 0o777,
        0o755
    );
    assert_ne!(
        config_path_binding(&fixture.0.join("Config.toml")).unwrap(),
        config_path_binding(&fixture.0.join("config.toml")).unwrap()
    );

    let missing_config = |last_byte| {
        read_user_config(
            ClientKind::Codex,
            &ConfigRootInputs {
                codex_home: Some(fixture.0.join(OsString::from_vec(vec![last_byte]))),
                ..ConfigRootInputs::default()
            },
        )
        .unwrap()
    };
    assert_ne!(
        missing_config(0xff).fingerprint,
        missing_config(0xfe).fingerprint
    );
}

#[test]
#[ignore = "requires an isolated HEADROOM_LINUX_PROOF_ROOT"]
fn config_transaction_reads_the_same_native_file_as_the_config_reader() {
    use crate::integrations::transaction_fs::{FileTransaction, MAX_TRANSACTION_FILE_BYTES};

    let fixture = Fixture::new();
    let config = fixture.0.join("config.toml");
    let original = b"model = 'fixture'\n";
    fs::write(&config, original).unwrap();
    let observed = read_user_config(ClientKind::Codex, &fixture.codex_roots()).unwrap();
    let mut transaction = FileTransaction::begin().expect("begin Linux configuration transaction");
    let (token, pinned) = transaction
        .open_existing(&config, MAX_TRANSACTION_FILE_BYTES)
        .unwrap();
    let identity = pinned.identity;
    let target = transaction.canonical_target(token).unwrap();
    let fingerprint = super::super::plan::fingerprint_config(
        ClientKind::Codex,
        observed.source,
        ConfigPresence::Present,
        &pinned.bytes,
        Some((
            identity.volume_serial,
            (identity.file_index >> 32) as u32,
            identity.file_index as u32,
        )),
        Some(&target),
    );
    assert_eq!(target, config);
    assert_eq!(pinned.bytes, original);
    assert_eq!(fingerprint, observed.fingerprint);
    transaction.rollback().unwrap();
    assert_eq!(fs::read(config).unwrap(), original);
}

struct StopRuntime(RuntimeManager);

impl Drop for StopRuntime {
    fn drop(&mut self) {
        let _ = self.0.shutdown();
    }
}

#[test]
#[ignore = "requires the pinned Linux runtime and a local Responses fixture"]
fn managed_engine_routes_an_isolated_connection_and_undoes_all_client_configs() {
    let fixture = Fixture::new();
    let app = fixture.directory("app");
    let codex = fixture.directory("codex");
    let claude = fixture.directory("claude");
    let xdg = fixture.directory("xdg");
    fs::DirBuilder::new()
        .mode(0o700)
        .create(xdg.join("opencode"))
        .unwrap();
    let upstream = env::var("HEADROOM_LINUX_FIXTURE_ORIGIN").unwrap();
    assert!(upstream
        .strip_prefix("http://127.0.0.1:")
        .and_then(|port| port.parse::<u16>().ok())
        .is_some_and(|port| port != 0));
    let workspace = app.join("runtime/workspace");
    fs::DirBuilder::new()
        .mode(0o700)
        .recursive(true)
        .create(&workspace)
        .unwrap();
    let settings = workspace.join("settings.json");
    fs::write(
        &settings,
        serde_json::json!({"openai_base_url": upstream}).to_string(),
    )
    .unwrap();
    fs::set_permissions(&settings, fs::Permissions::from_mode(0o600)).unwrap();
    let resources = PathBuf::from(env::var_os("HEADROOM_LINUX_RUNTIME_ROOT").unwrap());
    let runtime = RuntimeManager::new(
        Some(resources.parent().unwrap().to_owned()),
        Some(app.clone()),
    );
    let _stop = StopRuntime(runtime.clone());
    let reply = serde_json::to_value(runtime.start_engine(Uuid::new_v4().to_string())).unwrap();
    assert_eq!(
        reply["engine"]["status"], "ready",
        "engine start: {}",
        reply["error"]["code"]
    );
    let endpoint = runtime
        .ready_stable_endpoint()
        .expect("owned stable endpoint");
    let root_url = format!("http://127.0.0.1:{}", endpoint.port);

    let discovery = DiscoveryManager::with_choice_store(None);
    let marker = fixture.0.join("client-metadata");
    fs::write(&marker, b"synthetic metadata only").unwrap();
    let scan = discovery.fixture_cli_scan(&marker);
    let roots = ConfigRootInputs {
        codex_home: Some(codex.clone()),
        claude_config_dir: Some(claude.clone()),
        xdg_config_home: Some(xdg.clone()),
        claude_test_managed_settings: Some(fixture.0.join("missing-managed-settings.json")),
        claude_test_route_environment: Some(Vec::new()),
        ..ConfigRootInputs::default()
    };
    let mut manager = PlanManager::new(discovery, runtime.clone()).with_ownership_store(
        ownership_store_for_app_local_data(Some(app)).expect("private ownership store"),
    );
    manager.roots_provider = Arc::new(move || roots.clone());
    for (kind, client_kind, config, original) in [
        (
            "codex",
            "codex",
            codex.join("config.toml"),
            b"model = 'gpt-5.2-codex'\nmodel_reasoning_effort = 'medium'\n# preserve settings\n"
                .as_slice(),
        ),
        (
            "claude_code",
            "claude_code",
            claude.join("settings.json"),
            br#"{"model":"fixture-claude","env":{"KEEP_THIS":"yes"}}"#.as_slice(),
        ),
        (
            "opencode",
            "opencode_v1",
            xdg.join("opencode/opencode.json"),
            br#"{"provider":{"openai":{"npm":"@ai-sdk/openai","models":{"gpt-5.2-codex":{}}}}}"#
                .as_slice(),
        ),
    ] {
        fs::write(&config, original).unwrap();
        fs::set_permissions(&config, fs::Permissions::from_mode(0o640)).unwrap();
        let installation = scan
            .installations
            .iter()
            .find(|item| item.client_kind == kind)
            .unwrap();
        let plan = match manager.preview(PreviewRequest {
            schema_version: 1,
            request_id: Uuid::new_v4().to_string(),
            scope: "user_config".into(),
            installation_id: installation.installation_id.clone(),
            scan_generation: scan.scan_generation,
        }) {
            PlanReply::Preview(reply) => reply.plan,
            PlanReply::Error(reply) => panic!("fixture preview failed: {}", reply.error.code),
        };
        assert!(
            plan.blockers.is_empty(),
            "fixture plan is blocked: {:?}",
            plan.blockers
        );
        let applied = match manager.apply_connection(ApplyConnectionRequest {
            schema_version: 1,
            request_id: Uuid::new_v4().to_string(),
            expected_config_fingerprint: plan.config_fingerprint,
            plan_id: plan.plan_id,
        }) {
            ConnectionActionReply::Success(reply) => reply,
            ConnectionActionReply::Error(reply) => {
                panic!("fixture apply failed: {}", reply.error.code)
            }
        };
        assert_eq!(applied.status, "applied");
        let owned = applied.ownership.unwrap();
        let persisted = fs::read_to_string(&config).unwrap();
        assert!(persisted.contains(&root_url));
        if kind == "codex" {
            request_from_codex_fixture(&persisted, &root_url);
        }
        let undone = match manager.undo_connection(UndoConnectionRequest {
            schema_version: 1,
            request_id: Uuid::new_v4().to_string(),
            client_kind: client_kind.into(),
            owner_id: owned.owner_id,
            expected_revision: owned.revision,
            expected_record_fingerprint: owned.record_fingerprint,
        }) {
            ConnectionActionReply::Success(reply) => reply,
            ConnectionActionReply::Error(reply) => {
                panic!("fixture undo failed: {}", reply.error.code)
            }
        };
        assert_eq!(undone.status, "released");
        assert_eq!(fs::read(&config).unwrap(), original);
        assert_eq!(fs::metadata(&config).unwrap().mode() & 0o777, 0o640);
    }
    assert!(runtime.shutdown());
}

fn request_from_codex_fixture(config: &str, root_url: &str) {
    let saved: toml::Table = toml::from_str(config).unwrap();
    let model = saved["model"].as_str().unwrap();
    let effort = saved["model_reasoning_effort"].as_str().unwrap();
    let base = saved["openai_base_url"].as_str().unwrap();
    assert_eq!(base, format!("{root_url}/v1"));
    let nonce = env::var("HEADROOM_LINUX_FIXTURE_NONCE").unwrap();
    let key = env::var("HEADROOM_LINUX_FIXTURE_KEY").unwrap();
    let body = serde_json::json!({
        "model": model, "reasoning": {"effort": effort}, "stream": true,
        "input": format!("Return the local fixture marker. {nonce}"),
    });
    let authorization = format!("Authorization: Bearer {key}");
    let request = std::process::Command::new("/usr/bin/curl")
        .args([
            "--disable",
            "--fail",
            "--silent",
            "--max-time",
            "25",
            "--noproxy",
            "*",
            "--header",
            &authorization,
            "--header",
            "Content-Type: application/json",
            "--data",
            &body.to_string(),
            &format!("{base}/responses"),
        ])
        .env_clear()
        .output()
        .expect("run loopback request through the saved config");
    assert!(
        request.status.success(),
        "local Responses request failed: {}",
        request.status
    );
    let output = String::from_utf8(request.stdout).unwrap();
    let completed: Vec<_> = output
        .lines()
        .filter_map(|line| line.strip_prefix("data: "))
        .filter_map(|data| serde_json::from_str::<serde_json::Value>(data).ok())
        .filter(|event| event["type"] == "response.completed")
        .collect();
    assert_eq!(completed.len(), 1);
    let response = &completed[0]["response"];
    assert_eq!(response["model"], model);
    assert_eq!(response["usage"]["input_tokens"], 11);
    assert_eq!(response["usage"]["output_tokens"], 7);
    assert!(response["output"][0]["content"][0]["text"]
        .as_str()
        .unwrap()
        .contains(&format!("HEADROOM_FIXTURE_RESULT:{nonce}")));
}
