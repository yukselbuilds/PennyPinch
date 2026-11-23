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
