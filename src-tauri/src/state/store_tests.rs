use super::store::{PreferenceStore, PreferencesReply};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Barrier};
use std::thread;
use uuid::Uuid;

const REQUEST_ID: &str = "7132bced-3ce0-4c1b-8f48-d93d7b96f7ce";

#[test]
fn operating_preferences_migrate_without_rewrite_and_legacy_save_preserves_them() {
    use super::store::OperatingPreferences;
    let fixture = Fixture::new();
    snapshot(fixture.store.save_preferences(
        REQUEST_ID.into(),
        "en".into(),
        "dark".into(),
        0,
        None,
    ));
    let path = fixture.store.file_path_for_test();
    let mut legacy: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    legacy["preferences"]
        .as_object_mut()
        .unwrap()
        .remove("operating");
    let original = serde_json::to_vec(&legacy).unwrap();
    fs::write(&path, &original).unwrap();
    let read = snapshot(fixture.store.get_preferences(REQUEST_ID.into()));
    assert_eq!(read.preferences.operating, OperatingPreferences::default());
    assert_eq!(fs::read(&path).unwrap(), original);
    let wanted = OperatingPreferences {
        close_to_tray: false,
        retention_days: 90,
        protected_tools: vec!["ReadExact".into(), "mcp__local__report".into()],
    };
    let saved = snapshot(fixture.store.save_with_operating(
        REQUEST_ID.into(),
        "en".into(),
        "dark".into(),
        Some(wanted.clone()),
        read.revision,
        read.state_fingerprint.clone(),
    ));
    let bytes = fs::read(&path).unwrap();
    assert_eq!(
        error_code(fixture.store.save_with_operating(
            REQUEST_ID.into(),
            "en".into(),
            "dark".into(),
            Some(Default::default()),
            read.revision,
            read.state_fingerprint
        )),
        "config_conflict"
    );
    assert_eq!(fs::read(&path).unwrap(), bytes);
    let reopened =
        PreferenceStore::for_test(fixture.project_root.clone(), fixture.relative_root.clone())
            .unwrap();
    let theme = snapshot(reopened.save_preferences(
        REQUEST_ID.into(),
        "tr".into(),
        "light".into(),
        saved.revision,
        saved.state_fingerprint,
    ));
    assert_eq!(theme.preferences.operating, wanted);
    let mut invalid = wanted;
    invalid.protected_tools.push("bad name".into());
    let before = fs::read(&path).unwrap();
    assert_eq!(
        error_code(reopened.save_with_operating(
            REQUEST_ID.into(),
            "tr".into(),
            "light".into(),
            Some(invalid),
            theme.revision,
            theme.state_fingerprint
        )),
        "validation_failed"
    );
    assert_eq!(fs::read(&path).unwrap(), before);
    fixture.cleanup();
}

#[test]
fn operating_preferences_reject_unbounded_unsorted_or_duplicate_tool_names() {
    use super::store::OperatingPreferences;
    for names in [
        vec!["".into()],
        vec!["b".into(), "a".into()],
        vec!["a".into(), "a".into()],
        vec!["x".repeat(97)],
        vec!["é".into()],
        vec!["a,b".into()],
        (0..33).map(|i| format!("tool{i:02}")).collect(),
    ] {
        assert!(!OperatingPreferences {
            protected_tools: names,
            ..Default::default()
        }
        .is_valid());
    }
    assert!(!OperatingPreferences {
        retention_days: 0,
        ..Default::default()
    }
    .is_valid());
}

struct Fixture {
    project_root: PathBuf,
    relative_root: PathBuf,
    store: PreferenceStore,
}

impl Fixture {
    fn new() -> Self {
        let project_root =
            fs::canonicalize(PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap()).unwrap();
        let relative_root =
            PathBuf::from("work").join(format!(".preferences-test-{}", Uuid::new_v4().simple()));
        let store = PreferenceStore::for_test(project_root.clone(), relative_root.clone()).unwrap();
        Self {
            project_root,
            relative_root,
            store,
        }
    }

    fn root(&self) -> PathBuf {
        self.project_root.join(&self.relative_root)
    }

    fn cleanup(&self) {
        let root = self.root();
        let metadata = match fs::symlink_metadata(&root) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return,
            Err(error) => panic!("fixture cleanup metadata failed: {error}"),
        };
        if metadata.file_type().is_symlink() || is_reparse(&metadata) {
            panic!("refusing to remove a redirected preference fixture root");
        }
        if !root.starts_with(&self.project_root) {
            panic!("refusing to remove an out-of-project fixture root");
        }
        let resolved = fs::canonicalize(&root).expect("fixture root canonicalization");
        if !resolved.starts_with(&self.project_root) {
            panic!("refusing to remove a fixture outside the project root");
        }
        if metadata.is_dir() {
            assert_fixture_tree_safe(&resolved, &self.project_root);
            fs::remove_dir_all(resolved).unwrap();
        } else {
            fs::remove_file(resolved).unwrap();
        }
    }
}

#[cfg(windows)]
fn is_reparse(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    metadata.file_attributes() & 0x400 != 0
}

#[cfg(not(windows))]
fn is_reparse(_metadata: &fs::Metadata) -> bool {
    false
}

fn assert_fixture_tree_safe(root: &Path, project_root: &Path) {
    let resolved = fs::canonicalize(root).expect("fixture subtree canonicalization");
    assert!(resolved.starts_with(project_root));
    for entry in fs::read_dir(root).expect("fixture subtree listing") {
        let entry = entry.expect("fixture subtree entry");
        let metadata = fs::symlink_metadata(entry.path()).expect("fixture subtree metadata");
        assert!(!metadata.file_type().is_symlink() && !is_reparse(&metadata));
        if metadata.is_dir() {
            assert_fixture_tree_safe(&entry.path(), project_root);
        } else {
            let child = fs::canonicalize(entry.path()).expect("fixture file canonicalization");
            assert!(child.starts_with(project_root));
        }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        self.cleanup();
    }
}

fn snapshot(reply: PreferencesReply) -> super::store::PreferencesSnapshot {
    match reply {
        PreferencesReply::Snapshot(value) => value,
        PreferencesReply::Error(error) => {
            panic!("unexpected fixed error code: {}", error.error.code)
        }
    }
}

fn error_code(reply: PreferencesReply) -> &'static str {
    match reply {
        PreferencesReply::Error(error) => error.error.code,
        PreferencesReply::Snapshot(_) => panic!("expected fixed error response"),
    }
}

#[test]
fn missing_state_returns_defaults_without_creating_a_file() {
    let fixture = Fixture::new();
    let value = snapshot(fixture.store.get_preferences(REQUEST_ID.to_owned()));
    assert_eq!(value.revision, 0);
    assert_eq!(value.preferences.language, "tr");
    assert_eq!(value.preferences.theme, "system");
    assert_eq!(value.runtime_manifest.engine_version, "0.41.0");
    assert_eq!(value.runtime_manifest.ccr_ttl_seconds, 7200);
    assert!(value.state_fingerprint.is_none());
    assert!(!fixture.root().exists());
    fixture.cleanup();
}

#[test]
fn save_and_reopen_preserves_only_allowlisted_preferences_and_manifest_reference() {
    let fixture = Fixture::new();
    let saved = snapshot(fixture.store.save_preferences(
        REQUEST_ID.to_owned(),
        "en".to_owned(),
        "dark".to_owned(),
        0,
        None,
    ));
    assert_eq!(saved.revision, 1);
    assert_eq!(saved.preferences.language, "en");
    assert_eq!(saved.preferences.theme, "dark");
    assert!(saved.state_fingerprint.is_some());
    assert_eq!(
        saved.runtime_manifest.manifest_sha256,
        "8e4edf5f1328a7a6323ecf435604feaa48fd80c9648288f05b449584924b7908"
    );
    assert_eq!(saved.runtime_manifest.mode, "cache");
    assert_eq!(saved.runtime_manifest.savings_profile, "coding");

    let reopened =
        PreferenceStore::for_test(fixture.project_root.clone(), fixture.relative_root.clone())
            .unwrap();
    let loaded = snapshot(reopened.get_preferences(REQUEST_ID.to_owned()));
    assert_eq!(loaded.revision, 1);
    assert_eq!(loaded.preferences, saved.preferences);
    assert_eq!(loaded.runtime_manifest, saved.runtime_manifest);
    let raw = fs::read_to_string(reopened.file_path_for_test()).unwrap();
    assert!(!raw.contains("api_key"));
    assert!(!raw.contains("access_token"));
    fixture.cleanup();
}

#[test]
fn unknown_or_corrupt_state_returns_fixed_error_and_preserves_original_bytes() {
    let fixture = Fixture::new();
    snapshot(fixture.store.save_preferences(
        REQUEST_ID.to_owned(),
        "en".to_owned(),
        "light".to_owned(),
        0,
        None,
    ));
    let path = fixture.store.file_path_for_test();
    let original = fs::read(&path).unwrap();
    let mut unknown: serde_json::Value = serde_json::from_slice(&original).unwrap();
    unknown["auth"] = serde_json::json!("fixture-sentinel");
    let unknown_bytes = serde_json::to_vec(&unknown).unwrap();
    fs::write(&path, &unknown_bytes).unwrap();
    let reply = fixture.store.get_preferences(REQUEST_ID.to_owned());
    assert_eq!(error_code(reply), "unsupported_schema");
    let public_error = serde_json::to_string(&fixture.store.get_preferences(REQUEST_ID.to_owned()))
        .expect("fixed public error serializes");
    assert!(!public_error.contains("fixture-sentinel"));
    assert!(!public_error.contains(&fixture.root().to_string_lossy().to_string()));
    assert_eq!(fs::read(&path).unwrap(), unknown_bytes);

    fs::write(&path, b"{invalid-json").unwrap();
    let corrupt_bytes = fs::read(&path).unwrap();
    assert_eq!(
        error_code(fixture.store.get_preferences(REQUEST_ID.to_owned())),
        "unsupported_schema"
    );
    assert_eq!(fs::read(path).unwrap(), corrupt_bytes);
    fixture.cleanup();
}

#[test]
fn failed_atomic_replace_keeps_previous_valid_state_and_removes_its_temp_file() {
    let fixture = Fixture::new();
    let saved = snapshot(fixture.store.save_preferences(
        REQUEST_ID.to_owned(),
        "en".to_owned(),
        "system".to_owned(),
        0,
        None,
    ));
    let path = fixture.store.file_path_for_test();
    let previous = fs::read(&path).unwrap();
    fixture.store.fail_before_next_replace();

    assert_eq!(
        error_code(fixture.store.save_preferences(
            REQUEST_ID.to_owned(),
            "en".to_owned(),
            "dark".to_owned(),
            saved.revision,
            saved.state_fingerprint.clone(),
        )),
        "environment_blocked"
    );
    assert_eq!(fs::read(&path).unwrap(), previous);
    let reopened =
        PreferenceStore::for_test(fixture.project_root.clone(), fixture.relative_root.clone())
            .unwrap();
    let loaded = snapshot(reopened.get_preferences(REQUEST_ID.to_owned()));
    assert_eq!(loaded.revision, saved.revision);
    assert_eq!(loaded.preferences.theme, "system");
    let leftovers = fs::read_dir(fixture.root())
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name().to_string_lossy().ends_with(".tmp"))
        .count();
    assert_eq!(leftovers, 0);
    fixture.cleanup();
}

#[test]
fn stale_revision_and_concurrent_saves_cannot_silently_overwrite_each_other() {
    let fixture = Fixture::new();
    let initial = snapshot(fixture.store.save_preferences(
        REQUEST_ID.to_owned(),
        "tr".to_owned(),
        "system".to_owned(),
        0,
        None,
    ));
    let path = fixture.store.file_path_for_test();
    let before = fs::read(&path).unwrap();
    assert_eq!(
        error_code(fixture.store.save_preferences(
            REQUEST_ID.to_owned(),
            "en".to_owned(),
            "dark".to_owned(),
            0,
            None,
        )),
        "config_conflict"
    );
    assert_eq!(fs::read(&path).unwrap(), before);

    let concurrent = Arc::new(
        PreferenceStore::for_test(fixture.project_root.clone(), fixture.relative_root.clone())
            .unwrap(),
    );
    let barrier = Arc::new(Barrier::new(3));
    let expected_fingerprint = initial.state_fingerprint.clone();
    let mut workers = Vec::new();
    for theme in ["dark", "light"] {
        let store = Arc::clone(&concurrent);
        let barrier = Arc::clone(&barrier);
        let theme = theme.to_owned();
        let expected_fingerprint = expected_fingerprint.clone();
        workers.push(thread::spawn(move || {
            barrier.wait();
            store.save_preferences(
                REQUEST_ID.to_owned(),
                "tr".to_owned(),
                theme,
                1,
                expected_fingerprint,
            )
        }));
    }
    barrier.wait();
    let replies: Vec<_> = workers
        .into_iter()
        .map(|worker| worker.join().unwrap())
        .collect();
    let successes = replies
        .iter()
        .filter(|reply| matches!(reply, PreferencesReply::Snapshot(_)))
        .count();
    let fixed_errors: Vec<_> = replies
        .iter()
        .filter_map(|reply| match reply {
            PreferencesReply::Error(error) => Some(error.error.code),
            PreferencesReply::Snapshot(_) => None,
        })
        .collect();
    assert_eq!(successes, 1);
    assert!(fixed_errors
        .iter()
        .all(|code| *code == "busy" || *code == "config_conflict"));
    let reloaded = snapshot(concurrent.get_preferences(REQUEST_ID.to_owned()));
    assert_eq!(reloaded.revision, 2);
    fixture.cleanup();
}

#[test]
fn input_and_resolved_root_escape_are_rejected_before_writing() {
    let fixture = Fixture::new();
    assert_eq!(
        error_code(fixture.store.save_preferences(
            REQUEST_ID.to_owned(),
            "fr".to_owned(),
            "system".to_owned(),
            0,
            None,
        )),
        "validation_failed"
    );
    assert!(!fixture.root().exists());
    assert!(PreferenceStore::for_test(
        fixture.project_root.clone(),
        Path::new("work")
            .join("..")
            .join("outside-preferences")
            .to_path_buf(),
    )
    .is_err());
    assert!(!fixture.project_root.join("outside-preferences").exists());
    fixture.cleanup();
}

#[test]
fn root_type_drift_is_rejected_before_a_write() {
    let fixture = Fixture::new();
    fs::write(fixture.root(), b"unexpected file at state directory").unwrap();
    assert_eq!(
        error_code(fixture.store.save_preferences(
            REQUEST_ID.to_owned(),
            "en".to_owned(),
            "dark".to_owned(),
            0,
            None,
        )),
        "environment_blocked"
    );
    assert_eq!(
        fs::read(fixture.root()).unwrap(),
        b"unexpected file at state directory"
    );
    fixture.cleanup();
}

#[test]
fn same_revision_external_edit_is_rejected_by_state_fingerprint() {
    let fixture = Fixture::new();
    let saved = snapshot(fixture.store.save_preferences(
        REQUEST_ID.to_owned(),
        "en".to_owned(),
        "system".to_owned(),
        0,
        None,
    ));
    let path = fixture.store.file_path_for_test();
    let mut external: serde_json::Value =
        serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    external["preferences"]["theme"] = serde_json::json!("light");
    let external_bytes = serde_json::to_vec(&external).unwrap();
    fs::write(&path, &external_bytes).unwrap();

    assert_eq!(
        error_code(fixture.store.save_preferences(
            REQUEST_ID.to_owned(),
            "en".to_owned(),
            "dark".to_owned(),
            saved.revision,
            saved.state_fingerprint,
        )),
        "config_conflict"
    );
    assert_eq!(fs::read(&path).unwrap(), external_bytes);
    fixture.cleanup();
}

#[test]
fn revisions_stay_inside_the_javascript_safe_integer_range() {
    const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;
    let fixture = Fixture::new();
    snapshot(fixture.store.save_preferences(
        REQUEST_ID.to_owned(),
        "tr".to_owned(),
        "system".to_owned(),
        0,
        None,
    ));
    let path = fixture.store.file_path_for_test();
    let mut state: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    state["revision"] = serde_json::json!(MAX_SAFE_INTEGER);
    fs::write(&path, serde_json::to_vec(&state).unwrap()).unwrap();

    let at_limit = snapshot(fixture.store.get_preferences(REQUEST_ID.to_owned()));
    assert_eq!(at_limit.revision, MAX_SAFE_INTEGER);
    assert_eq!(
        error_code(fixture.store.save_preferences(
            REQUEST_ID.to_owned(),
            "en".to_owned(),
            "dark".to_owned(),
            MAX_SAFE_INTEGER,
            at_limit.state_fingerprint,
        )),
        "config_conflict"
    );

    state["revision"] = serde_json::json!(MAX_SAFE_INTEGER + 1);
    let beyond_limit = serde_json::to_vec(&state).unwrap();
    fs::write(&path, &beyond_limit).unwrap();
    assert_eq!(
        error_code(fixture.store.get_preferences(REQUEST_ID.to_owned())),
        "unsupported_schema"
    );
    assert_eq!(fs::read(&path).unwrap(), beyond_limit);
    fixture.cleanup();
}
