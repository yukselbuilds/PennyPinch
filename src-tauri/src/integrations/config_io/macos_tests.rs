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

fn assert_supported_directory_chain(path: &Path) {
    for (depth, directory) in path.ancestors().enumerate() {
        if super::open_directory(directory).is_err() {
            let info = fs::symlink_metadata(directory).unwrap();
            let open_errno = OpenOptions::new()
                .read(true)
                .custom_flags(
                    libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_NOFOLLOW_ANY | libc::O_CLOEXEC,
                )
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
