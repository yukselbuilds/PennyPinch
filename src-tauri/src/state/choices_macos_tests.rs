use super::*;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{symlink, DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use uuid::Uuid;

struct Fixture {
    root: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let work = fs::canonicalize(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .expect("project root")
                .join("work"),
        )
        .expect("canonical project work");
        let proof = PathBuf::from(
            std::env::var_os("HEADROOM_MACOS_PROOF_ROOT").expect("native proof root"),
        );
        let proof_metadata = fs::symlink_metadata(&proof).expect("proof root metadata");
        assert!(proof_metadata.is_dir() && !proof_metadata.file_type().is_symlink());
        assert_eq!(proof_metadata.uid(), unsafe { libc::geteuid() });
        assert_eq!(proof_metadata.mode() & 0o7777, 0o700);
        let proof = fs::canonicalize(&proof).expect("canonical proof root");
        assert_eq!(
            proof,
            PathBuf::from(std::env::var_os("HEADROOM_MACOS_PROOF_ROOT").unwrap())
        );
        assert!(proof.starts_with(&work) && proof != work);

        let root = proof.join(Uuid::new_v4().to_string());
        private_dir(&root);
        Self { root }
    }

    fn path(&self, name: &str) -> PathBuf {
        self.root.join(name)
    }

    fn executable(&self, name: &str, bytes: &[u8]) -> PathBuf {
        let path = self.path(name);
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o700)
            .open(&path)
            .expect("create synthetic executable");
        file.write_all(bytes).unwrap();
        file.sync_all().unwrap();
        assert_private(&path, 0o700);
        path
    }
}

fn private_dir(path: &Path) {
    fs::DirBuilder::new().mode(0o700).create(path).unwrap();
    assert_private(path, 0o700);
}

fn assert_private(path: &Path, mode: u32) {
    let metadata = fs::symlink_metadata(path).expect("fixture metadata");
    assert!(metadata.file_type().is_file() || metadata.file_type().is_dir());
    assert!(!metadata.file_type().is_symlink());
    assert_eq!(metadata.uid(), unsafe { libc::geteuid() });
    assert_eq!(metadata.mode() & 0o7777, mode);
    if metadata.is_file() {
        assert_eq!(metadata.nlink(), 1);
    }
}

fn choice(client: ChoiceClient, path: PathBuf) -> InstallationChoice {
    ChoiceStore::capture_installation(
        client,
        ChoiceSurface::Cli,
        ChoiceOrigin::Path,
        path,
        Some("fixture-1.0".into()),
    )
    .expect("capture synthetic installation")
}

fn store_path(store: &ChoiceStore) -> PathBuf {
    store.inner.location.root.clone()
}

#[test]
fn app_local_selection_store_is_passive_then_roundtrips_explicit_choices() {
    let fixture = Fixture::new();
    assert!(matches!(
        ChoiceStore::for_app_local_data(None),
        Err(ChoiceError::Unavailable)
    ));

    let app_data = fixture.path("App Support/Ürün/Headroom");
    let store = ChoiceStore::for_app_local_data(Some(app_data.clone())).unwrap();
    assert_eq!(store.read().unwrap().revision, 0);
    assert!(!app_data.exists(), "passive read must not create app data");
    assert!(
        !store_path(&store).exists(),
        "passive read must not create state"
    );

    let codex = choice(
        ChoiceClient::Codex,
        fixture.executable("codex", b"codex-fixture-secretless"),
    );
    let claude = choice(
        ChoiceClient::ClaudeCode,
        fixture.executable("claude", b"claude-fixture-secretless"),
    );
    let opencode = choice(
        ChoiceClient::OpenCode,
        fixture.executable("opencode", b"opencode-fixture-secretless"),
    );
    store.save_installation(codex.clone()).unwrap();
    store.save_installation(claude.clone()).unwrap();
    store.save_installation(opencode.clone()).unwrap();

    let project_path = fixture.path("selected-project");
    private_dir(&project_path);
    let project = ChoiceStore::claude_project_for_test(project_path).unwrap();
    let saved = store.save_claude(project.clone()).unwrap();
    assert_eq!(saved.revision, 4);

    let state_root = store_path(&store);
    let selection_file = state_root.join(CHOICES_FILE);
    assert_private(&state_root, 0o700);
    assert_private(&selection_file, 0o600);
    let bytes = fs::read(&selection_file).unwrap();
    for marker in [
        b"codex-fixture-secretless".as_slice(),
        b"claude-fixture-secretless",
        b"opencode-fixture-secretless",
    ] {
        assert!(!bytes.windows(marker.len()).any(|window| window == marker));
    }

    let reopened = ChoiceStore::for_app_local_data(Some(app_data)).unwrap();
    let snapshot = reopened.read().unwrap();
    assert_eq!(snapshot.revision, 4);
    assert_eq!(
        snapshot.installations,
        vec![codex.clone(), claude, opencode]
    );
    assert_eq!(snapshot.claude, Some(project.clone()));
    assert_eq!(reopened.save_installation(codex).unwrap().revision, 4);
    assert_eq!(reopened.save_claude(project).unwrap().revision, 4);
    assert_eq!(fs::read(&selection_file).unwrap(), bytes);
    let mut legacy: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert!(legacy["installations"][0]
        .as_object_mut()
        .unwrap()
        .remove("macos_change_time")
        .is_some());
    let legacy_bytes = serde_json::to_vec(&legacy).unwrap();
    fs::write(&selection_file, &legacy_bytes).unwrap();
    let loaded = reopened.read().unwrap();
    assert_eq!(
        loaded.installations[0].availability,
        ChoiceAvailability::Unavailable
    );
    assert!(loaded.installations[1..]
        .iter()
        .all(|choice| choice.availability == ChoiceAvailability::Available));
    assert_eq!(fs::read(selection_file).unwrap(), legacy_bytes);
}

#[test]
fn executable_replacement_and_same_size_edit_make_saved_choices_unavailable() {
    let fixture = Fixture::new();
    let store = ChoiceStore::new(&fixture.root, "onboarding").unwrap();
    let codex_path = fixture.executable("codex", b"codex-fixture-a");
    let claude_path = fixture.executable("claude", b"claude-fixture-a");
    let codex = choice(ChoiceClient::Codex, codex_path.clone());
    let claude = choice(ChoiceClient::ClaudeCode, claude_path.clone());
    store.save_installation(codex.clone()).unwrap();
    store.save_installation(claude.clone()).unwrap();
    let file = store_path(&store).join(CHOICES_FILE);
    let saved_bytes = fs::read(&file).unwrap();

    fs::rename(&codex_path, fixture.path("codex-original")).unwrap();
    let replacement = fixture.executable("codex", b"codex-fixture-b");
    assert_eq!(fs::metadata(replacement).unwrap().len(), codex.size_bytes);
    let mut in_place = OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(&claude_path)
        .unwrap();
    in_place.write_all(b"claude-fixture-b").unwrap();
    in_place.sync_all().unwrap();
    drop(in_place);

    let snapshot = store.read().unwrap();
    assert_eq!(snapshot.installations.len(), 2);
    assert!(snapshot
        .installations
        .iter()
        .all(|entry| entry.availability == ChoiceAvailability::Unavailable));
    assert!(matches!(
        store.save_installation(codex),
        Err(ChoiceError::Conflict)
    ));
    assert!(matches!(
        store.save_installation(claude),
        Err(ChoiceError::Conflict)
    ));
    assert_eq!(fs::read(file).unwrap(), saved_bytes);
}

#[test]
fn malformed_and_oversized_selection_files_are_rejected_without_rewrite() {
    let fixture = Fixture::new();
    let store = ChoiceStore::new(&fixture.root, "onboarding").unwrap();
    let codex = choice(
        ChoiceClient::Codex,
        fixture.executable("codex", b"codex-fixture"),
    );
    store.save_installation(codex.clone()).unwrap();
    let path = store_path(&store).join(CHOICES_FILE);

    for invalid in [
        br#"{"schema_version":99,"revision":1,"installations":[],"claude":null}"#.to_vec(),
        br#"{"schema_version":1,"revision":1,"installations":[],"claude":null,"extra":true}"#
            .to_vec(),
        vec![b'x'; MAX_CHOICES_BYTES + 1],
    ] {
        fs::write(&path, &invalid).unwrap();
        assert!(store.read().is_err());
        assert!(store.save_installation(codex.clone()).is_err());
        assert_eq!(fs::read(&path).unwrap(), invalid);
    }
}

#[test]
fn unsafe_selection_roots_and_files_fail_closed_without_touching_targets() {
    let fixture = Fixture::new();
    let store = ChoiceStore::new(&fixture.root, "onboarding").unwrap();
    let codex = choice(
        ChoiceClient::Codex,
        fixture.executable("codex", b"codex-fixture"),
    );
    store.save_installation(codex).unwrap();
    let root = store_path(&store);
    let path = root.join(CHOICES_FILE);
    let original = fs::read(&path).unwrap();

    let hardlink = fixture.path("selection-hardlink");
    fs::hard_link(&path, &hardlink).unwrap();
    assert!(store.read().is_err());
    assert_eq!(fs::read(&path).unwrap(), original);
    fs::remove_file(hardlink).unwrap();

    fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
    assert!(store.read().is_err());
    assert_eq!(fs::read(&path).unwrap(), original);
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();

    let sentinel = fixture.path("sentinel");
    fs::write(&sentinel, b"sentinel-safe").unwrap();
    fs::set_permissions(&sentinel, fs::Permissions::from_mode(0o600)).unwrap();
    let backup = root.join("selection-owned-backup");
    fs::rename(&path, &backup).unwrap();
    symlink(&sentinel, &path).unwrap();
    assert!(store.read().is_err());
    assert_eq!(fs::read(&sentinel).unwrap(), b"sentinel-safe");
    fs::remove_file(&path).unwrap();
    fs::rename(backup, &path).unwrap();

    fs::set_permissions(&root, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(store.read().is_err());
    assert_eq!(fs::read(&path).unwrap(), original);
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();

    let target = fixture.path("boundary-target");
    private_dir(&target);
    let link = fixture.path("boundary-link");
    symlink(&target, &link).unwrap();
    assert!(ChoiceStore::new(&link, "onboarding").is_err());
    assert!(!target.join("onboarding").exists());
}
