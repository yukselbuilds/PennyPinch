use super::*;
use crate::integrations::plan::{
    preview_config_with_presence, ClientKind, ConfigPresence, ConfigScope, ConfigSource,
    PlanContext, PlanStatus, StableEndpoint,
};
use crate::integrations::transaction::{apply_missing_endpoint, undo_owned_endpoint, ApplyResult};
use crate::state::ownership::{
    config_path_binding, config_path_binding_for_claude_project_target, OwnershipError,
    OwnershipRecord, OwnershipSnapshot, OwnershipState, OwnershipStore,
};
use std::ffi::CString;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};
use uuid::Uuid;

const ORIGINAL: &[u8] =
    b"# synthetic config fixture\nmodel = 'fixture-model'\napi_key = 'SYNTHETIC_AUTH_SENTINEL'\n";
const USER_EDIT: &[u8] = b"# unrelated user edit\n";
const AUTH_SENTINEL: &[u8] = b"SYNTHETIC_AUTH_SENTINEL";
const MODEL_SENTINEL: &[u8] = b"fixture-model";
const PROMPT_SENTINEL: &[u8] = b"fixture-prompt";
const EXIT_AFTER_CONFIG: i32 = 42;
const CHILD_TEST: &str = "crash_after_config_publication_before_owner";
const CLAUDE_CHILD_TEST: &str = "crash_after_claude_config_publication_before_owner";
const CONFIG_NAME: &str = "config.toml";
const CLAUDE_SETTINGS_NAME: &str = "settings.local.json";
const CLAUDE_BEFORE: &[u8] = b"{}";
const CLAUDE_USER_FILE: &[u8] = b"{\"api_key\":\"SYNTHETIC_AUTH_SENTINEL\"}";

struct Fixture {
    id: String,
    root: PathBuf,
    client: PathBuf,
    config: PathBuf,
    store: OwnershipStore,
}

struct StagedApply {
    transaction: FileTransaction,
    record: OwnershipRecord,
    after: Vec<u8>,
}

struct ClaudeProjectFixture {
    id: String,
    root: PathBuf,
    project: PathBuf,
    claude_dir: PathBuf,
    config: PathBuf,
    user_file: PathBuf,
    store: OwnershipStore,
}

struct StagedClaudeApply {
    transaction: FileTransaction,
    record: OwnershipRecord,
    after: Vec<u8>,
}

fn proof_root() -> PathBuf {
    let project_work = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("project root")
        .join("work");
    let project_work = fs::canonicalize(project_work).expect("canonical project work");
    let root =
        PathBuf::from(std::env::var_os("HEADROOM_MACOS_PROOF_ROOT").expect("native proof root"));
    let root = fs::canonicalize(root).expect("canonical proof root");
    assert!(root != project_work && root.starts_with(&project_work));
    let metadata = fs::symlink_metadata(&root).expect("proof root metadata");
    assert!(metadata.is_dir() && !metadata.file_type().is_symlink());
    assert_eq!(metadata.uid(), unsafe { libc::geteuid() });
    assert_eq!(metadata.mode() & 0o7777, 0o700);
    Directory::open(&root, true).expect("private fixed-volume proof root");
    root
}

fn create_private_dir(path: &Path) {
    fs::DirBuilder::new()
        .mode(0o700)
        .create(path)
        .expect("create owned fixture directory");
    let metadata = fs::symlink_metadata(path).expect("fixture directory metadata");
    assert!(metadata.is_dir() && !metadata.file_type().is_symlink());
    assert_eq!(metadata.uid(), unsafe { libc::geteuid() });
    assert_eq!(metadata.mode() & 0o7777, 0o700);
}

impl ClaudeProjectFixture {
    fn create() -> Self {
        let id = Uuid::new_v4().to_string();
        let root = proof_root().join(&id);
        create_private_dir(&root);
        let project = root.join("project");
        create_private_dir(&project);
        let user_file = project.join("user-settings.json");
        fs::write(&user_file, CLAUDE_USER_FILE).expect("write synthetic unrelated project file");
        fs::set_permissions(&user_file, fs::Permissions::from_mode(0o600))
            .expect("set unrelated fixture file mode");
        let claude_dir = project.join(".claude");
        let config = claude_dir.join(CLAUDE_SETTINGS_NAME);
        let store = OwnershipStore::new(root.clone(), Path::new("state"))
            .expect("construct Claude project ownership store");
        Self {
            id,
            root,
            project,
            claude_dir,
            config,
            user_file,
            store,
        }
    }

    fn open(id: &str) -> Self {
        let uuid = Uuid::parse_str(id).expect("Claude fixture UUID");
        assert!(!uuid.is_nil() && uuid.to_string() == id);
        let proof = proof_root();
        let root = proof.join(id);
        assert_eq!(root.parent(), Some(proof.as_path()));
        let metadata = fs::symlink_metadata(&root).expect("Claude fixture root metadata");
        assert!(metadata.is_dir() && !metadata.file_type().is_symlink());
        assert_eq!(metadata.uid(), unsafe { libc::geteuid() });
        assert_eq!(metadata.mode() & 0o7777, 0o700);
        let project = root.join("project");
        let claude_dir = project.join(".claude");
        let config = claude_dir.join(CLAUDE_SETTINGS_NAME);
        let user_file = project.join("user-settings.json");
        let store = OwnershipStore::new(root.clone(), Path::new("state"))
            .expect("reopen Claude project ownership store");
        Self {
            id: id.to_owned(),
            root,
            project,
            claude_dir,
            config,
            user_file,
            store,
        }
    }
}

impl Fixture {
    fn create() -> Self {
        let id = Uuid::new_v4().to_string();
        let root = proof_root().join(&id);
        create_private_dir(&root);
        let client = root.join("client");
        create_private_dir(&client);
        let config = client.join(CONFIG_NAME);
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o644)
            .open(&config)
            .expect("create synthetic client config");
        file.write_all(ORIGINAL).expect("write synthetic config");
        file.sync_all().expect("sync synthetic config");
        fs::set_permissions(&config, fs::Permissions::from_mode(0o644))
            .expect("set fixture config mode");
        let store = OwnershipStore::new(root.clone(), Path::new("state"))
            .expect("construct fixture ownership store");
        Self {
            id,
            root,
            client,
            config,
            store,
        }
    }

    fn open(id: &str) -> Self {
        let uuid = Uuid::parse_str(id).expect("fixture UUID");
        assert!(!uuid.is_nil() && uuid.to_string() == id);
        let proof = proof_root();
        let root = proof.join(id);
        assert_eq!(root.parent(), Some(proof.as_path()));
        let metadata = fs::symlink_metadata(&root).expect("existing fixture root");
        assert!(metadata.is_dir() && !metadata.file_type().is_symlink());
        assert_eq!(metadata.uid(), unsafe { libc::geteuid() });
        assert_eq!(metadata.mode() & 0o7777, 0o700);
        let client = root.join("client");
        let config = client.join(CONFIG_NAME);
        let store = OwnershipStore::new(root.clone(), Path::new("state"))
            .expect("reopen fixture ownership store");
        Self {
            id: id.to_owned(),
            root,
            client,
            config,
            store,
        }
    }
}

fn plan_for(bytes: &[u8]) -> crate::integrations::plan::ConnectionPlan {
    let context = PlanContext {
        request_id: "019a0000-0000-7000-8000-d00000000001".to_owned(),
        installation_id: "019a0000-0000-7000-8000-d00000000002".to_owned(),
        client_kind: ClientKind::Codex,
        config_source: ConfigSource::CodexToml,
        surface: "cli".to_owned(),
        version: Some("0.163.0-alpha.10.2".to_owned()),
        scope: ConfigScope::UserConfig,
        scan_generation: 1,
        endpoint: StableEndpoint {
            endpoint_id: "019a0000-0000-7000-8000-d00000000003".to_owned(),
            host: "127.0.0.1".to_owned(),
            port: 41731,
        },
        now_unix_seconds: 1_800_000_000,
    };
    let plan = preview_config_with_presence(&context, bytes, ConfigPresence::Present)
        .expect("synthetic plan");
    assert_eq!(
        plan.status,
        PlanStatus::Preview,
        "fixture plan should be eligible"
    );
    plan
}

fn claude_plan_for_missing() -> crate::integrations::plan::ConnectionPlan {
    let context = PlanContext {
        request_id: "019a0000-0000-7000-8000-d00000000011".to_owned(),
        installation_id: "019a0000-0000-7000-8000-d00000000012".to_owned(),
        client_kind: ClientKind::ClaudeCode,
        config_source: ConfigSource::ClaudeJson,
        surface: "cli".to_owned(),
        version: Some("2.1.158".to_owned()),
        scope: ConfigScope::ProjectLocalConfig,
        scan_generation: 1,
        endpoint: StableEndpoint {
            endpoint_id: "019a0000-0000-7000-8000-d00000000013".to_owned(),
            host: "127.0.0.1".to_owned(),
            port: 41731,
        },
        now_unix_seconds: 1_800_000_000,
    };
    let plan = preview_config_with_presence(&context, CLAUDE_BEFORE, ConfigPresence::Missing)
        .expect("synthetic missing Claude project plan");
    assert_eq!(
        plan.status,
        PlanStatus::Preview,
        "Claude fixture plan should be eligible"
    );
    plan
}

fn expected_patched(bytes: &[u8]) -> (Vec<u8>, crate::integrations::transaction::PatchReceipt) {
    match apply_missing_endpoint(&plan_for(bytes), bytes).expect("typed synthetic patch") {
        ApplyResult::Patched { bytes, receipt } => (bytes, receipt),
        ApplyResult::Unchanged => panic!("fixture requires an unpatched endpoint"),
    }
}

fn stage_apply(fixture: &Fixture) -> StagedApply {
    fixture
        .store
        .prepare_for_apply()
        .expect("prepare owned state root");
    let mut transaction = FileTransaction::begin().expect("begin apply transaction");
    let (config_token, config_snapshot) = transaction
        .open_existing(&fixture.config, MAX_TRANSACTION_FILE_BYTES)
        .expect("pin synthetic config");
    let before = config_snapshot.bytes;
    let plan = plan_for(&before);
    let binding = config_path_binding(&fixture.config).expect("bind fixture config path");
    let ownership = fixture
        .store
        .read_in_transaction(&mut transaction, ClientKind::Codex, &binding)
        .expect("read ownership in transaction");
    let revision = ownership.current().map_or(1, |record| {
        assert_eq!(record.state(), OwnershipState::Released);
        record.revision() + 1
    });
    let ApplyResult::Patched {
        bytes: after,
        receipt,
    } = apply_missing_endpoint(&plan, &before).expect("prepare typed patch")
    else {
        panic!("fixture requires a patch");
    };
    let record = OwnershipRecord::active_from_scoped_patch_with_binding(
        &Uuid::new_v4().to_string(),
        &plan.endpoint_id,
        revision,
        1_800_000_000,
        &before,
        &after,
        &receipt,
        ConfigScope::UserConfig,
        &binding,
    )
    .expect("create typed active record");
    transaction
        .replace_contents(config_token, &after)
        .expect("stage config bytes");
    fixture
        .store
        .stage_in_transaction(&mut transaction, ownership, &record)
        .expect("stage ownership bytes");
    StagedApply {
        transaction,
        record,
        after,
    }
}

fn stage_claude_apply(fixture: &ClaudeProjectFixture) -> StagedClaudeApply {
    fixture
        .store
        .prepare_for_apply()
        .expect("prepare Claude fixture state root");
    let mut transaction = FileTransaction::begin().expect("begin Claude project transaction");
    let (directory_token, _) = transaction
        .create_claude_directory(&fixture.project)
        .expect("stage missing Claude directory");
    let (config_token, _) = transaction
        .create_claude_settings_file(directory_token, MAX_TRANSACTION_FILE_BYTES)
        .expect("stage missing Claude settings file");
    let target = transaction
        .canonical_target(config_token)
        .expect("Claude project target");
    assert_eq!(
        target, fixture.config,
        "target is the fixed project-local leaf"
    );
    assert!(
        !fixture.claude_dir.exists() && !fixture.config.exists(),
        "staging does not expose the missing directory or file"
    );

    let plan = claude_plan_for_missing();
    let binding = config_path_binding_for_claude_project_target(&fixture.project, &target)
        .expect("bind fixed Claude project target");
    let ownership = fixture
        .store
        .read_in_transaction(&mut transaction, ClientKind::ClaudeCode, &binding)
        .expect("read Claude ownership in transaction");
    assert!(
        ownership.current().is_none(),
        "fixture starts without ownership"
    );
    let ApplyResult::Patched {
        bytes: after,
        receipt,
    } = apply_missing_endpoint(&plan, CLAUDE_BEFORE).expect("typed Claude project patch")
    else {
        panic!("missing Claude project target requires a patch");
    };
    let record = OwnershipRecord::active_from_scoped_patch_with_binding(
        &Uuid::new_v4().to_string(),
        &plan.endpoint_id,
        1,
        1_800_000_000,
        CLAUDE_BEFORE,
        &after,
        &receipt,
        ConfigScope::ProjectLocalConfig,
        &binding,
    )
    .expect("create scoped Claude ownership record");
    transaction
        .replace_contents(config_token, &after)
        .expect("stage Claude endpoint bytes");
    fixture
        .store
        .stage_in_transaction(&mut transaction, ownership, &record)
        .expect("stage Claude ownership record");
    assert!(
        !fixture.claude_dir.exists() && !fixture.config.exists(),
        "preview and staging remain invisible before commit"
    );
    StagedClaudeApply {
        transaction,
        record,
        after,
    }
}

fn read_claude_snapshot(fixture: &ClaudeProjectFixture) -> Option<OwnershipSnapshot> {
    let mut transaction = FileTransaction::begin().expect("begin Claude owner snapshot");
    let result = fixture
        .store
        .read_snapshot_in_transaction(&mut transaction, ClientKind::ClaudeCode)
        .expect("read Claude ownership snapshot");
    transaction.rollback().expect("finish Claude snapshot read");
    result
}

fn recover_claude_owner(fixture: &ClaudeProjectFixture) -> OwnershipRecord {
    let mut transaction = FileTransaction::begin().expect("begin explicit Claude recovery action");
    transaction
        .open_existing(&fixture.config, MAX_TRANSACTION_FILE_BYTES)
        .expect("pin published Claude settings file");
    let binding = config_path_binding_for_claude_project_target(&fixture.project, &fixture.config)
        .expect("bind existing Claude project target");
    let ownership = fixture
        .store
        .read_in_transaction(&mut transaction, ClientKind::ClaudeCode, &binding)
        .expect("recover pending Claude owner publication");
    let record = ownership
        .current()
        .expect("active Claude ownership after explicit recovery")
        .clone();
    transaction.rollback().expect("finish Claude recovery read");
    record
}

fn undo_claude(fixture: &ClaudeProjectFixture) -> OwnershipRecord {
    let mut transaction = FileTransaction::begin().expect("begin Claude undo");
    let (config_token, config_snapshot) = transaction
        .open_existing(&fixture.config, MAX_TRANSACTION_FILE_BYTES)
        .expect("pin Claude settings for undo");
    let binding = config_path_binding_for_claude_project_target(&fixture.project, &fixture.config)
        .expect("bind Claude settings for undo");
    let ownership = fixture
        .store
        .read_in_transaction(&mut transaction, ClientKind::ClaudeCode, &binding)
        .expect("read current Claude ownership");
    let record = ownership
        .current()
        .expect("active Claude ownership exists")
        .clone();
    assert_eq!(record.state(), OwnershipState::Active);
    let restored = undo_owned_endpoint(
        &record.patch_receipt().expect("Claude typed receipt"),
        &config_snapshot.bytes,
    )
    .expect("typed Claude endpoint undo");
    transaction
        .replace_contents(config_token, &restored)
        .expect("stage restored Claude settings");
    let released = record
        .released(1_800_000_001)
        .expect("release Claude ownership");
    fixture
        .store
        .stage_in_transaction(&mut transaction, ownership, &released)
        .expect("stage released Claude record");
    transaction.seal().expect("seal Claude undo");
    transaction.commit().expect("commit Claude undo");
    released
}

fn assert_no_claude_state_secrets(fixture: &ClaudeProjectFixture) {
    let state = fixture.root.join("state");
    for entry in fs::read_dir(state).expect("read Claude state directory") {
        let entry = entry.expect("state entry");
        let name = entry.file_name();
        if name.to_string_lossy().ends_with(".json") || name.to_string_lossy().ends_with(".done") {
            let bytes = fs::read(entry.path()).expect("read synthetic transaction metadata");
            assert!(!contains(&bytes, AUTH_SENTINEL));
            assert!(!contains(&bytes, MODEL_SENTINEL));
            assert!(!contains(&bytes, PROMPT_SENTINEL));
        }
    }
}

fn commit_apply(fixture: &Fixture) -> (OwnershipRecord, Vec<u8>) {
    let mut staged = stage_apply(fixture);
    staged.transaction.seal().expect("seal apply");
    staged.transaction.commit().expect("commit apply");
    (staged.record, staged.after)
}

fn read_snapshot(fixture: &Fixture) -> Option<OwnershipSnapshot> {
    let mut transaction = FileTransaction::begin().expect("begin ownership snapshot");
    let result = fixture
        .store
        .read_snapshot_in_transaction(&mut transaction, ClientKind::Codex)
        .expect("read ownership snapshot");
    transaction.rollback().expect("finish snapshot read");
    result
}

fn undo(fixture: &Fixture) -> OwnershipRecord {
    let mut transaction = FileTransaction::begin().expect("begin undo transaction");
    let (config_token, config_snapshot) = transaction
        .open_existing(&fixture.config, MAX_TRANSACTION_FILE_BYTES)
        .expect("pin config for undo");
    let binding = config_path_binding(&fixture.config).expect("bind fixture path");
    let ownership = fixture
        .store
        .read_in_transaction(&mut transaction, ClientKind::Codex, &binding)
        .expect("read current ownership");
    let record = ownership
        .current()
        .expect("active ownership exists")
        .clone();
    assert_eq!(record.state(), OwnershipState::Active);
    let restored = undo_owned_endpoint(
        &record.patch_receipt().expect("typed receipt"),
        &config_snapshot.bytes,
    )
    .expect("typed endpoint undo");
    transaction
        .replace_contents(config_token, &restored)
        .expect("stage restored config");
    let released = record.released(1_800_000_001).expect("release ownership");
    fixture
        .store
        .stage_in_transaction(&mut transaction, ownership, &released)
        .expect("stage released record");
    transaction.seal().expect("seal undo");
    transaction.commit().expect("commit undo");
    released
}

fn assert_no_fixture_secrets_in_state(fixture: &Fixture) {
    let state = fixture.root.join("state");
    for entry in fs::read_dir(state).expect("read fixture state names") {
        let entry = entry.expect("state entry");
        if entry.file_type().expect("state entry type").is_file() {
            let bytes = fs::read(entry.path()).expect("read fixture metadata for assertions");
            assert!(
                !contains(&bytes, AUTH_SENTINEL),
                "auth fixture is absent from state"
            );
            assert!(
                !contains(&bytes, MODEL_SENTINEL),
                "model fixture is absent from state"
            );
            assert!(
                !contains(&bytes, PROMPT_SENTINEL),
                "prompt fixture is absent from state"
            );
        }
    }
}

fn contains(bytes: &[u8], needle: &[u8]) -> bool {
    bytes.windows(needle.len()).any(|window| window == needle)
}

fn set_xattr(path: &Path) {
    let path = CString::new(path.as_os_str().as_bytes()).expect("fixture path CString");
    let name = CString::new("com.headroom.fixture-canary").expect("xattr name");
    let value = b"preserve";
    // SAFETY: both C strings and the bounded value remain live for the call.
    let result = unsafe {
        libc::setxattr(
            path.as_ptr(),
            name.as_ptr(),
            value.as_ptr().cast(),
            value.len(),
            0,
            0,
        )
    };
    assert_eq!(result, 0, "set synthetic metadata canary");
}

fn assert_xattr(path: &Path) {
    let path = CString::new(path.as_os_str().as_bytes()).expect("fixture path CString");
    let name = CString::new("com.headroom.fixture-canary").expect("xattr name");
    let mut value = [0_u8; 16];
    // SAFETY: both C strings and the writable bounded buffer remain live.
    let size = unsafe {
        libc::getxattr(
            path.as_ptr(),
            name.as_ptr(),
            value.as_mut_ptr().cast(),
            value.len(),
            0,
            0,
        )
    };
    assert_eq!(size, 8, "read synthetic metadata canary");
    assert!(&value[..8] == b"preserve", "metadata canary is unchanged");
}

fn assert_config_mode(path: &Path) {
    let metadata = fs::symlink_metadata(path).expect("config metadata");
    assert!(metadata.is_file() && !metadata.file_type().is_symlink());
    assert_eq!(metadata.mode() & 0o7777, 0o644, "config mode is preserved");
}

fn fixture_acl(path: &Path, set: bool) -> Vec<String> {
    if set {
        let mut account = std::mem::MaybeUninit::<libc::passwd>::uninit();
        let mut buffer = [0_i8; 4096];
        let mut found = std::ptr::null_mut();
        // chmod's ACL parser resolves a username, not the decimal UID string.
        let error = unsafe {
            libc::getpwuid_r(
                libc::geteuid(),
                account.as_mut_ptr(),
                buffer.as_mut_ptr(),
                buffer.len(),
                &mut found,
            )
        };
        assert!(
            error == 0 && !found.is_null(),
            "resolve fixture ACL account"
        );
        let account_name = unsafe { CStr::from_ptr((*found).pw_name) }
            .to_str()
            .unwrap();
        let rule = format!("user:{account_name} allow read,write");
        let status = Command::new("/bin/chmod")
            .args(["+a", rule.as_str()])
            .arg(path)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .unwrap();
        assert!(status.success(), "set fixture ACL");
    }
    let output = Command::new("/bin/ls")
        .arg("-le")
        .arg(path)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .unwrap();
    assert!(output.status.success(), "read fixture ACL");
    let entries: Vec<_> = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .skip(1)
        .map(|line| line.trim().to_owned())
        .filter(|line| !line.is_empty())
        .collect();
    assert!(!entries.is_empty(), "fixture ACL is present");
    entries
}

fn spawn_crash_child(id: &str) {
    let executable = std::env::current_exe().expect("test executable");
    let crate_prefix = format!("{}::", env!("CARGO_CRATE_NAME"));
    let module = module_path!()
        .strip_prefix(&crate_prefix)
        .unwrap_or(module_path!());
    let filter = format!("{module}::{CHILD_TEST}");
    let proof = proof_root();
    let mut child = Command::new(executable)
        .args(["--exact", &filter, "--ignored", "--test-threads=1"])
        .env_clear()
        .env("HEADROOM_MACOS_PROOF_ROOT", proof)
        .env("HEADROOM_MACOS_TXN_FIXTURE", id)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn owned crash fixture child");
    let deadline = Instant::now() + Duration::from_secs(12);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                assert_eq!(
                    status.code(),
                    Some(EXIT_AFTER_CONFIG),
                    "child stopped at crash hook"
                );
                break;
            }
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(20)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                panic!("owned crash fixture child exceeded its time budget");
            }
        }
    }
}

#[test]
fn transaction_apply_reopen_and_typed_undo_preserve_user_metadata() {
    let fixture = Fixture::create();
    set_xattr(&fixture.config);
    let acl = fixture_acl(&fixture.config, true);
    let (active, patched) = commit_apply(&fixture);
    assert_eq!(fixture_acl(&fixture.config, false), acl);
    assert!(
        fs::read(&fixture.config)
            .expect("patched fixture")
            .as_slice()
            == patched
    );
    assert_config_mode(&fixture.config);

    let reopened = Fixture::open(&fixture.id);
    let snapshot = read_snapshot(&reopened).expect("active snapshot after reopen");
    assert_eq!(snapshot.record.state(), OwnershipState::Active);
    assert_eq!(snapshot.record.owner_id(), active.owner_id());
    assert_no_fixture_secrets_in_state(&reopened);

    let mut editor = OpenOptions::new()
        .append(true)
        .open(&reopened.config)
        .expect("open fixture as independent editor");
    editor.write_all(USER_EDIT).expect("append unrelated edit");
    editor.sync_all().expect("sync unrelated edit");
    let released = undo(&reopened);
    assert_eq!(fixture_acl(&reopened.config, false), acl);
    assert_eq!(released.state(), OwnershipState::Released);
    let expected = [ORIGINAL, USER_EDIT].concat();
    assert!(
        fs::read(&reopened.config)
            .expect("restored fixture")
            .as_slice()
            == expected,
        "typed undo removes only the owned endpoint"
    );
    assert_config_mode(&reopened.config);
    assert_xattr(&reopened.config);
    assert_no_fixture_secrets_in_state(&reopened);
    assert_eq!(
        read_snapshot(&reopened)
            .expect("released snapshot")
            .record
            .state(),
        OwnershipState::Released
    );
}

#[test]
fn rollback_and_sealed_drop_leave_config_and_abort_without_replay() {
    let fixture = Fixture::create();
    stage_apply(&fixture)
        .transaction
        .rollback()
        .expect("pre-seal rollback");
    assert!(
        fs::read(&fixture.config)
            .expect("config after rollback")
            .as_slice()
            == ORIGINAL
    );
    assert!(
        read_snapshot(&fixture).is_none(),
        "rollback leaves no owner record"
    );

    let mut sealed = stage_apply(&fixture);
    sealed.transaction.seal().expect("prepare sealed intent");
    drop(sealed);
    assert!(
        fs::read(&fixture.config)
            .expect("config before recovery")
            .as_slice()
            == ORIGINAL
    );

    let mut fresh_action = FileTransaction::begin().expect("begin explicit recovery action");
    fresh_action
        .open_existing(&fixture.config, MAX_TRANSACTION_FILE_BYTES)
        .expect("pin unchanged config");
    let binding = config_path_binding(&fixture.config).expect("bind fixture path");
    let ownership = fixture
        .store
        .read_in_transaction(&mut fresh_action, ClientKind::Codex, &binding)
        .expect("fresh action aborts old prepared plan");
    assert!(
        ownership.current().is_none(),
        "aborted plan never publishes ownership"
    );
    fresh_action.rollback().expect("finish recovery read");
    assert!(
        fs::read(&fixture.config)
            .expect("config after recovery")
            .as_slice()
            == ORIGINAL
    );
    assert!(
        read_snapshot(&fixture).is_none(),
        "aborted plan remains unowned"
    );
}

#[test]
fn fresh_action_recovers_owner_after_process_exit_between_publications() {
    let fixture = Fixture::create();
    let (expected, _) = expected_patched(ORIGINAL);
    spawn_crash_child(&fixture.id);
    assert!(
        fs::read(&fixture.config)
            .expect("config after child publication")
            .as_slice()
            == expected,
        "child publishes only the prepared synthetic config"
    );
    assert!(
        !fixture.root.join("state").join("codex.json").exists(),
        "child exits before ownership publication"
    );

    let reopened = Fixture::open(&fixture.id);
    let mut transaction = FileTransaction::begin().expect("begin recovery action");
    transaction
        .open_existing(&reopened.config, MAX_TRANSACTION_FILE_BYTES)
        .expect("pin published config");
    let binding = config_path_binding(&reopened.config).expect("bind fixture config");
    let current = reopened
        .store
        .read_in_transaction(&mut transaction, ClientKind::Codex, &binding)
        .expect("recover staged owner metadata on explicit action");
    let recovered = current.current().expect("recovered active record");
    assert_eq!(recovered.state(), OwnershipState::Active);
    assert_eq!(recovered.endpoint_port(), 41731);
    transaction.rollback().expect("finish recovered read");
    assert_no_fixture_secrets_in_state(&reopened);
}

#[test]
fn late_write_through_displaced_fd_conflicts_without_losing_either_version() {
    let fixture = Fixture::create();
    let mut late_writer = OpenOptions::new()
        .append(true)
        .open(&fixture.config)
        .expect("hold original config descriptor");
    let (active, patched) = commit_apply(&fixture);
    late_writer
        .write_all(USER_EDIT)
        .expect("write through displaced original descriptor");
    late_writer.sync_all().expect("sync delayed user write");
    assert!(
        fs::read(&fixture.config)
            .expect("current patched config")
            .as_slice()
            == patched,
        "late write does not alter the new config version"
    );

    let mut undo_attempt = FileTransaction::begin().expect("begin conflicting undo read");
    undo_attempt
        .open_existing(&fixture.config, MAX_TRANSACTION_FILE_BYTES)
        .expect("pin current endpoint config");
    let binding = config_path_binding(&fixture.config).expect("bind config path");
    let result = fixture
        .store
        .read_in_transaction(&mut undo_attempt, ClientKind::Codex, &binding);
    assert!(
        matches!(result, Err(OwnershipError::Conflict)),
        "late write invalidates journal ownership evidence"
    );
    drop(undo_attempt);

    let displaced: Vec<Vec<u8>> = fs::read_dir(&fixture.client)
        .expect("list fixture config versions")
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            name.starts_with(".apfs-") && name.ends_with(".config")
        })
        .map(|entry| fs::read(entry.path()).expect("read displaced fixture version"))
        .collect();
    let expected_displaced = [ORIGINAL, USER_EDIT].concat();
    assert!(
        displaced.len() == 1 && displaced[0].as_slice() == expected_displaced,
        "displaced original and its late edit are preserved"
    );
    assert!(read_snapshot(&fixture).is_some_and(|snapshot| {
        snapshot.record.state() == OwnershipState::Active
            && snapshot.record.owner_id() == active.owner_id()
    }));
}

#[test]
#[ignore = "owned child invoked by the macOS config transaction recovery test"]
fn crash_after_config_publication_before_owner() {
    let id = std::env::var("HEADROOM_MACOS_TXN_FIXTURE").expect("owned fixture id");
    let fixture = Fixture::open(&id);
    let StagedApply {
        mut transaction, ..
    } = stage_apply(&fixture);
    transaction.seal().expect("seal child apply");
    transaction
        .commit_inner(|| std::process::exit(EXIT_AFTER_CONFIG))
        .expect("commit hook is only reached after successful publication");
}

fn spawn_claude_crash_child(id: &str) {
    let executable = std::env::current_exe().expect("test executable");
    let crate_prefix = format!("{}::", env!("CARGO_CRATE_NAME"));
    let module = module_path!()
        .strip_prefix(&crate_prefix)
        .unwrap_or(module_path!());
    let filter = format!("{module}::{CLAUDE_CHILD_TEST}");
    let proof = proof_root();
    let mut child = Command::new(executable)
        .args(["--exact", &filter, "--ignored", "--test-threads=1"])
        .env_clear()
        .env("HEADROOM_MACOS_PROOF_ROOT", proof)
        .env("HEADROOM_MACOS_CLAUDE_FIXTURE", id)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn owned Claude transaction child");
    let deadline = Instant::now() + Duration::from_secs(12);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                assert_eq!(status.code(), Some(EXIT_AFTER_CONFIG));
                break;
            }
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(20)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                panic!("Claude fixture child exceeded its time budget");
            }
        }
    }
}

#[test]
fn missing_claude_project_apply_and_typed_undo_keep_user_additions() {
    let fixture = ClaudeProjectFixture::create();
    let unrelated_before = fs::read(&fixture.user_file).expect("unrelated file bytes");
    let StagedClaudeApply {
        mut transaction,
        record,
        after,
    } = stage_claude_apply(&fixture);
    assert!(!fixture.claude_dir.exists() && !fixture.config.exists());
    transaction.seal().expect("seal Claude directory and file");
    transaction.commit().expect("commit Claude project apply");

    let directory_metadata =
        fs::symlink_metadata(&fixture.claude_dir).expect("published directory");
    assert!(
        directory_metadata.ino() > 0,
        "published directory has a real inode"
    );
    assert_eq!(directory_metadata.mode() & 0o7777, 0o700);
    let config_metadata = fs::symlink_metadata(&fixture.config).expect("published settings");
    assert!(
        config_metadata.ino() > 0,
        "published settings have a real inode"
    );
    assert_eq!(config_metadata.mode() & 0o7777, 0o600);
    assert_eq!(config_metadata.uid(), unsafe { libc::geteuid() });
    assert_eq!(fs::read(&fixture.config).unwrap(), after);
    assert_eq!(
        read_claude_snapshot(&fixture)
            .expect("active ownership snapshot")
            .record
            .state(),
        OwnershipState::Active
    );

    let mut edited: serde_json::Value = serde_json::from_slice(&after).expect("patched JSON");
    let object = edited.as_object_mut().expect("settings object");
    object.insert(
        "userSetting".to_owned(),
        serde_json::Value::String("keep this value".to_owned()),
    );
    object.insert(
        "apiKey".to_owned(),
        serde_json::Value::String("SYNTHETIC_AUTH_SENTINEL".to_owned()),
    );
    let edited_bytes = serde_json::to_vec(&edited).expect("edited synthetic settings");
    let mut editor = OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(&fixture.config)
        .expect("edit synthetic settings in place");
    editor.write_all(&edited_bytes).unwrap();
    editor.sync_all().unwrap();
    fs::write(&fixture.project.join("later-user-file.txt"), USER_EDIT)
        .expect("add unrelated project file");

    let released = undo_claude(&fixture);
    assert_eq!(released.state(), OwnershipState::Released);
    let restored: serde_json::Value =
        serde_json::from_slice(&fs::read(&fixture.config).unwrap()).expect("restored settings");
    assert!(
        restored.get("env").is_none(),
        "owned endpoint ancestor is removed"
    );
    assert_eq!(restored["userSetting"], "keep this value");
    assert_eq!(restored["apiKey"], "SYNTHETIC_AUTH_SENTINEL");
    assert!(
        fixture.claude_dir.is_dir(),
        "Undo keeps the project directory"
    );
    assert_eq!(
        fs::read(&fixture.project.join("later-user-file.txt")).unwrap(),
        USER_EDIT
    );
    assert_eq!(fs::read(&fixture.user_file).unwrap(), unrelated_before);
    assert_eq!(
        fs::read(&fixture.user_file).unwrap(),
        CLAUDE_USER_FILE,
        "an existing user file stays byte-exact"
    );
    assert_no_claude_state_secrets(&fixture);
    assert_eq!(record.state(), OwnershipState::Active);
}

#[test]
fn missing_claude_rollback_and_new_action_abort_do_not_publish_old_plan() {
    let fixture = ClaudeProjectFixture::create();
    let StagedClaudeApply { transaction, .. } = stage_claude_apply(&fixture);
    transaction.rollback().expect("rollback before seal");
    assert!(!fixture.claude_dir.exists() && !fixture.config.exists());
    assert!(!fixture.root.join("state/claude_code.json").exists());

    let StagedClaudeApply {
        mut transaction, ..
    } = stage_claude_apply(&fixture);
    transaction.seal().expect("seal staged Claude lease");
    drop(transaction);
    assert!(!fixture.claude_dir.exists() && !fixture.config.exists());

    // A fresh explicit operation binds the same missing target and aborts the
    // abandoned intent instead of publishing its old config/ownership lease.
    let StagedClaudeApply { transaction, .. } = stage_claude_apply(&fixture);
    transaction
        .rollback()
        .expect("finish explicit recovery action");
    assert!(!fixture.claude_dir.exists() && !fixture.config.exists());
    assert!(!fixture.root.join("state/claude_code.json").exists());
    assert!(
        fs::read_dir(fixture.root.join("state"))
            .unwrap()
            .any(|entry| {
                entry
                    .ok()
                    .is_some_and(|entry| entry.file_name().to_string_lossy().ends_with(".done"))
            }),
        "fresh action records the abandoned plan as aborted"
    );
}

#[test]
fn competing_claude_directory_and_replaced_project_are_preserved() {
    let token_fixture = ClaudeProjectFixture::create();
    let mut original_transaction = FileTransaction::begin().unwrap();
    let (directory_token, _) = original_transaction
        .create_claude_directory(&token_fixture.project)
        .unwrap();
    let mut unrelated_transaction = FileTransaction::begin().unwrap();
    assert!(matches!(
        unrelated_transaction
            .create_claude_settings_file(directory_token, MAX_TRANSACTION_FILE_BYTES),
        Err(TransactionFsError::Conflict)
    ));
    assert!(!token_fixture.claude_dir.exists());
    original_transaction.rollback().unwrap();

    let fixture = ClaudeProjectFixture::create();
    let StagedClaudeApply {
        mut transaction, ..
    } = stage_claude_apply(&fixture);
    create_private_dir(&fixture.claude_dir);
    let competitor = fixture.config.clone();
    fs::write(&competitor, b"{\"competitor\":true}").expect("write competing settings");
    let result = match transaction.seal() {
        Ok(()) => transaction.commit(),
        Err(error) => Err(error),
    };
    assert!(
        result.is_err(),
        "competing .claude creation blocks publication"
    );
    assert_eq!(fs::read(&competitor).unwrap(), b"{\"competitor\":true}");
    assert!(!fixture.root.join("state/claude_code.json").exists());

    let drift = ClaudeProjectFixture::create();
    let StagedClaudeApply {
        mut transaction, ..
    } = stage_claude_apply(&drift);
    let moved_project = drift.root.join("project-moved");
    fs::rename(&drift.project, &moved_project).expect("move pinned project directory");
    create_private_dir(&drift.project);
    let result = match transaction.seal() {
        Ok(()) => transaction.commit(),
        Err(error) => Err(error),
    };
    assert!(
        result.is_err(),
        "replacement project identity blocks publication"
    );
    assert!(!drift.claude_dir.exists() && !drift.config.exists());
    assert!(!moved_project.join(".claude").exists());
    assert!(!drift.root.join("state/claude_code.json").exists());
}

#[test]
fn claude_directory_publication_crash_recovers_owner_on_explicit_action() {
    let fixture = ClaudeProjectFixture::create();
    let (_, expected) = {
        let plan = claude_plan_for_missing();
        match apply_missing_endpoint(&plan, CLAUDE_BEFORE).unwrap() {
            ApplyResult::Patched { bytes, .. } => (plan, bytes),
            ApplyResult::Unchanged => panic!("Claude fixture requires a patch"),
        }
    };
    spawn_claude_crash_child(&fixture.id);
    assert!(
        fixture.claude_dir.is_dir(),
        "commit publishes the staged directory"
    );
    assert_eq!(fs::read(&fixture.config).unwrap(), expected);
    assert!(
        !fixture.root.join("state/claude_code.json").exists(),
        "child exits between config and ownership publication"
    );

    let reopened = ClaudeProjectFixture::open(&fixture.id);
    let recovered = recover_claude_owner(&reopened);
    assert_eq!(recovered.state(), OwnershipState::Active);
    assert_eq!(
        recovered.config_scope(),
        Some(ConfigScope::ProjectLocalConfig)
    );
    assert_eq!(recovered.config_source(), Some(ConfigSource::ClaudeJson));
    assert_no_claude_state_secrets(&reopened);
}

#[test]
#[ignore = "owned child invoked by Claude project transaction recovery test"]
fn crash_after_claude_config_publication_before_owner() {
    let id = std::env::var("HEADROOM_MACOS_CLAUDE_FIXTURE").expect("owned Claude fixture id");
    let fixture = ClaudeProjectFixture::open(&id);
    let StagedClaudeApply {
        mut transaction, ..
    } = stage_claude_apply(&fixture);
    transaction.seal().expect("seal child Claude transaction");
    transaction
        .commit_inner(|| std::process::exit(EXIT_AFTER_CONFIG))
        .expect("child exit hook follows directory and config publication");
}

#[test]
fn retained_review_is_passive_and_archive_keeps_displaced_edits_after_reopen() {
    let fixture = Fixture::create();
    let mut late = OpenOptions::new()
        .append(true)
        .open(&fixture.config)
        .unwrap();
    let (_, patched) = commit_apply(&fixture);
    late.write_all(USER_EDIT).unwrap();
    late.sync_all().unwrap();
    let state = fixture.root.join("state");
    let names_before: std::collections::BTreeSet<_> = fs::read_dir(&state)
        .unwrap()
        .flatten()
        .map(|entry| entry.file_name())
        .collect();
    let review = retained_versions::inspect(&state, &fixture.config, "codex.json").unwrap();
    assert_eq!(review.active_records, 1);
    assert_eq!(review.selected_records, 1);
    assert_eq!(review.changed_versions, 1);
    assert!(review.can_archive);
    assert_eq!(
        names_before,
        fs::read_dir(&state)
            .unwrap()
            .flatten()
            .map(|entry| entry.file_name())
            .collect()
    );
    let archived = retained_versions::archive(
        &state,
        &fixture.config,
        "codex.json",
        &review.review_fingerprint,
    )
    .unwrap();
    assert_eq!(archived.active_records, 0);
    assert_eq!(archived.completed_archives, 1);
    assert_eq!(fs::read(&fixture.config).unwrap(), patched);
    late.write_all(b"# another delayed write\n").unwrap();
    late.sync_all().unwrap();
    let reopened = Fixture::open(&fixture.id);
    undo(&reopened);
    assert_eq!(fs::read(&reopened.config).unwrap(), ORIGINAL);
    assert!(fs::read_dir(&reopened.client)
        .unwrap()
        .flatten()
        .any(|entry| entry
            .file_name()
            .to_str()
            .is_some_and(|name| name.ends_with(".config"))
            && fs::read(entry.path()).unwrap()
                == [ORIGINAL, USER_EDIT, b"# another delayed write\n"].concat()));
}

#[test]
fn retained_review_rejects_changed_config_and_cross_target_acceptance() {
    let fixture = Fixture::create();
    commit_apply(&fixture);
    let state = fixture.root.join("state");
    let review = retained_versions::inspect(&state, &fixture.config, "codex.json").unwrap();
    let other = Fixture::create();
    assert_eq!(
        retained_versions::archive(
            &state,
            &other.config,
            "codex.json",
            &review.review_fingerprint
        )
        .unwrap_err(),
        TransactionFsError::Conflict
    );
    let mut file = OpenOptions::new()
        .append(true)
        .open(&fixture.config)
        .unwrap();
    file.write_all(USER_EDIT).unwrap();
    file.sync_all().unwrap();
    assert_eq!(
        retained_versions::archive(
            &state,
            &fixture.config,
            "codex.json",
            &review.review_fingerprint
        )
        .unwrap_err(),
        TransactionFsError::Conflict
    );
    assert_eq!(
        retained_versions::inspect(&state, &fixture.config, "codex.json")
            .unwrap()
            .active_records,
        1
    );
}

#[test]
fn retained_record_capacity_can_be_archived_without_discarding_config_versions() {
    let fixture = Fixture::create();
    for _ in 0..32 {
        commit_apply(&fixture);
        undo(&fixture);
    }
    let state = fixture.root.join("state");
    let review = retained_versions::inspect(&state, &fixture.config, "codex.json").unwrap();
    assert_eq!(review.active_records, 64);
    assert_eq!(review.selected_records, 64);
    let mut blocked = stage_apply(&fixture).transaction;
    assert!(blocked.seal().is_err());
    drop(blocked);
    let after = retained_versions::archive(
        &state,
        &fixture.config,
        "codex.json",
        &review.review_fingerprint,
    )
    .unwrap();
    assert_eq!(after.active_records, 0);
    assert_eq!(after.completed_archives, 1);
    commit_apply(&fixture);
    undo(&fixture);
    assert_eq!(fs::read(&fixture.config).unwrap(), ORIGINAL);
}

#[test]
fn publication_rechecks_replacement_inode_and_late_write_before_claiming_owner() {
    let fixture = Fixture::create();
    let mut staged = stage_apply(&fixture);
    staged.transaction.seal().unwrap();
    let replacement = fixture.client.join("replacement.toml");
    fs::write(&replacement, ORIGINAL).unwrap();
    fs::set_permissions(&replacement, fs::Permissions::from_mode(0o644)).unwrap();
    let replacement_inode = fs::metadata(&replacement).unwrap().ino();
    fs::rename(replacement, &fixture.config).unwrap();
    assert_eq!(
        staged.transaction.commit(),
        Err(TransactionFsError::Conflict)
    );
    assert_eq!(
        fs::metadata(&fixture.config).unwrap().ino(),
        replacement_inode
    );
    assert_eq!(fs::read(&fixture.config).unwrap(), ORIGINAL);
    assert!(!fixture.root.join("state/codex.json").exists());

    let fixture = Fixture::create();
    let mut writer = OpenOptions::new()
        .append(true)
        .open(&fixture.config)
        .unwrap();
    let mut staged = stage_apply(&fixture);
    let expected = staged.after.clone();
    staged.transaction.seal().unwrap();
    let result = staged.transaction.commit_inner(|| {
        writer.write_all(USER_EDIT).unwrap();
        writer.sync_all().unwrap();
    });
    assert_eq!(result, Err(TransactionFsError::Conflict));
    assert_eq!(fs::read(&fixture.config).unwrap(), expected);
    assert!(!fixture.root.join("state/codex.json").exists());
    let saved: Vec<_> = fs::read_dir(&fixture.client)
        .unwrap()
        .flatten()
        .filter(|entry| {
            entry
                .file_name()
                .to_str()
                .is_some_and(|name| name.ends_with(".config"))
        })
        .map(|entry| fs::read(entry.path()).unwrap())
        .collect();
    assert_eq!(saved, vec![[ORIGINAL, USER_EDIT].concat()]);
    assert_no_fixture_secrets_in_state(&fixture);

    let fixture = Fixture::create();
    let mut staged = stage_apply(&fixture);
    staged.transaction.seal().unwrap();
    let result = staged.transaction.commit_inner(|| {
        let displaced = fs::read_dir(&fixture.client)
            .unwrap()
            .flatten()
            .find(|entry| {
                entry
                    .file_name()
                    .to_str()
                    .is_some_and(|name| name.ends_with(".config"))
            })
            .unwrap();
        let target = CString::new(fixture.config.as_os_str().as_bytes()).unwrap();
        let displaced = CString::new(displaced.path().as_os_str().as_bytes()).unwrap();
        assert_eq!(
            unsafe {
                libc::renameatx_np(
                    libc::AT_FDCWD,
                    target.as_ptr(),
                    libc::AT_FDCWD,
                    displaced.as_ptr(),
                    libc::RENAME_SWAP,
                )
            },
            0
        );
    });
    assert_eq!(
        result,
        Err(TransactionFsError::Conflict),
        "a reverted publication cannot report success"
    );
    assert_eq!(fs::read(&fixture.config).unwrap(), ORIGINAL);
    assert!(!fixture.root.join("state/codex.json").exists());
}
