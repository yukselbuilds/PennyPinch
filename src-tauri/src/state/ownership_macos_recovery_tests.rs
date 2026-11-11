use super::*;
use crate::integrations::plan::{
    preview_config_with_presence, ClientKind, ConfigPresence, ConfigScope, ConfigSource,
    PlanContext, StableEndpoint,
};
use crate::integrations::transaction::{apply_missing_endpoint, ApplyResult};
use std::os::unix::fs::DirBuilderExt;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

const ORIGINAL: &[u8] = b"# private fixture content\nmodel = 'fixture-model'\n";
const LATER_EDIT: &[u8] = b"# added independently\n";
const EXIT_AFTER_CONFIG: i32 = 42;
const CHILD_CASE: &str = "state::ownership::macos_recovery::tests::crash_after_config_publication";

fn proof_root() -> PathBuf {
    let work = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("work");
    let work = fs::canonicalize(work).expect("project work directory");
    let proof =
        PathBuf::from(std::env::var_os("HEADROOM_MACOS_PROOF_ROOT").expect("native proof root"));
    assert_ne!(proof, work);
    assert!(proof.starts_with(work));
    Directory::open(&proof).expect("private local APFS proof root");
    proof
}

fn fixture() -> (String, Recovery) {
    let id = Uuid::new_v4().to_string();
    let root = proof_root().join(&id);
    for path in [&root, &root.join("client"), &root.join("state")] {
        fs::DirBuilder::new().mode(0o700).create(path).unwrap();
    }
    (id, Recovery::open(&root).unwrap())
}

fn patch(recovery: &Recovery) -> (Vec<u8>, OwnershipRecord) {
    let context = PlanContext {
        request_id: Uuid::new_v4().to_string(),
        installation_id: Uuid::new_v4().to_string(),
        client_kind: ClientKind::Codex,
        config_source: ConfigSource::CodexToml,
        surface: "cli".into(),
        version: Some("0.163.0-alpha.10.2".into()),
        scope: ConfigScope::UserConfig,
        scan_generation: 1,
        endpoint: StableEndpoint {
            endpoint_id: Uuid::new_v4().to_string(),
            host: "127.0.0.1".into(),
            port: 41731,
        },
        now_unix_seconds: 1_800_000_000,
    };
    let plan = preview_config_with_presence(&context, ORIGINAL, ConfigPresence::Present).unwrap();
    let ApplyResult::Patched { bytes, receipt } = apply_missing_endpoint(&plan, ORIGINAL).unwrap()
    else {
        panic!("owned fixture patch expected");
    };
    let owner = OwnershipRecord::active_from_scoped_patch_with_binding(
        &Uuid::new_v4().to_string(),
        &context.endpoint.endpoint_id,
        1,
        context.now_unix_seconds,
        ORIGINAL,
        &bytes,
        &receipt,
        context.scope,
        &recovery.binding().unwrap(),
    )
    .unwrap();
    (bytes, owner)
}

fn prepared_apply() -> (String, Recovery, OwnershipRecord, Vec<u8>) {
    let (id, recovery) = fixture();
    let (patched, owner) = patch(&recovery);
    recovery.config.create(CONFIG, ORIGINAL).unwrap();
    recovery.config.create(STAGED_CONFIG, &patched).unwrap();
    recovery.prepare(&owner).unwrap();
    (id, recovery, owner, patched)
}

fn file_bytes(directory: &Directory, name: &CStr) -> Vec<u8> {
    directory
        .snapshot(name, MAX_TRANSACTION_FILE_BYTES)
        .unwrap()
        .unwrap()
        .bytes
}

fn swap_then_terminate(id: &str) {
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", CHILD_CASE, "--ignored", "--test-threads=1"])
        .env("HEADROOM_MACOS_RECOVERY_FIXTURE", id)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(12);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                assert_eq!(
                    status.code(),
                    Some(EXIT_AFTER_CONFIG),
                    "fixture child did not exit after config publication"
                );
                break;
            }
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(20)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                panic!("fixture child did not finish within its budget");
            }
        }
    }
}

#[test]
#[ignore = "run only as the owned child of the native recovery fixture"]
fn crash_after_config_publication() {
    let id = std::env::var("HEADROOM_MACOS_RECOVERY_FIXTURE").expect("fixture identifier");
    assert!(Uuid::parse_str(&id).is_ok_and(|uuid| !uuid.is_nil() && uuid.to_string() == id));
    let recovery = Recovery::open(&proof_root().join(id)).unwrap();
    let _lock = recovery.lock().unwrap();
    assert_eq!(
        recovery.inspect(&recovery.intent().unwrap().0).unwrap(),
        RecoveryState::Prepared
    );
    recovery
        .config
        .rename(STAGED_CONFIG, CONFIG, libc::RENAME_SWAP)
        .unwrap();
    std::process::exit(EXIT_AFTER_CONFIG);
}

#[test]
fn owned_apply_recovers_metadata_after_process_exit_and_reopens() {
    let (id, recovery, owner, patched) = prepared_apply();
    let mut editor = recovery
        .config
        .open_file(CONFIG, libc::O_WRONLY | libc::O_APPEND)
        .unwrap();
    assert_eq!(recovery.recover().unwrap(), RecoveryState::Prepared);
    assert!(recovery
        .state
        .snapshot(OWNER, MAX_RECORD_BYTES)
        .unwrap()
        .is_none());
    assert!(!file_bytes(&recovery.state, INTENT)
        .windows(b"private fixture content".len())
        .any(|bytes| bytes == b"private fixture content"));
    swap_then_terminate(&id);
    assert_eq!(file_bytes(&recovery.config, CONFIG), patched);
    assert_eq!(file_bytes(&recovery.config, STAGED_CONFIG), ORIGINAL);
    assert!(recovery
        .state
        .snapshot(OWNER, MAX_RECORD_BYTES)
        .unwrap()
        .is_none());
    let reopened = Recovery::open(&proof_root().join(&id)).unwrap();
    assert_eq!(reopened.recover().unwrap(), RecoveryState::Recovered);
    assert_eq!(
        OwnershipRecord::decode(&file_bytes(&reopened.state, OWNER)).unwrap(),
        owner
    );
    assert_eq!(reopened.recover().unwrap(), RecoveryState::Recovered);
    assert_eq!(file_bytes(&reopened.config, CONFIG), patched);
    assert_eq!(file_bytes(&reopened.config, STAGED_CONFIG), ORIGINAL);
    editor.write_all(LATER_EDIT).unwrap();
    editor.sync_all().unwrap();
    assert_eq!(reopened.recover().unwrap(), RecoveryState::Conflict);
    assert_eq!(file_bytes(&reopened.config, CONFIG), patched);
    assert_eq!(
        file_bytes(&reopened.config, STAGED_CONFIG),
        [ORIGINAL, LATER_EDIT].concat()
    );
}

#[test]
fn late_writer_is_preserved_and_blocks_claiming_ownership() {
    let (id, recovery, _, patched) = prepared_apply();
    let mut editor = recovery
        .config
        .open_file(CONFIG, libc::O_WRONLY | libc::O_APPEND)
        .unwrap();
    swap_then_terminate(&id);
    editor.write_all(LATER_EDIT).unwrap();
    editor.sync_all().unwrap();
    assert_eq!(recovery.recover().unwrap(), RecoveryState::Conflict);
    assert!(recovery
        .state
        .snapshot(OWNER, MAX_RECORD_BYTES)
        .unwrap()
        .is_none());
    assert_eq!(file_bytes(&recovery.config, CONFIG), patched);
    assert_eq!(
        file_bytes(&recovery.config, STAGED_CONFIG),
        [ORIGINAL, LATER_EDIT].concat()
    );
    assert!(recovery
        .state
        .snapshot(INTENT, MAX_INTENT_BYTES)
        .unwrap()
        .is_some());
}

#[test]
fn identical_bytes_in_replacement_inode_do_not_prove_ownership() {
    let (id, recovery, _, patched) = prepared_apply();
    swap_then_terminate(&id);
    recovery.config.create(c"foreign.toml", &patched).unwrap();
    recovery.config.rename(c"foreign.toml", CONFIG, 0).unwrap();
    assert_eq!(recovery.recover().unwrap(), RecoveryState::Conflict);
    assert!(recovery
        .state
        .snapshot(OWNER, MAX_RECORD_BYTES)
        .unwrap()
        .is_none());
    assert_eq!(file_bytes(&recovery.config, CONFIG), patched);
    assert_eq!(file_bytes(&recovery.config, STAGED_CONFIG), ORIGINAL);
}

#[test]
fn owned_undo_recovers_release_and_preserves_later_settings() {
    let (id, recovery) = fixture();
    let (mut patched, owner) = patch(&recovery);
    patched.extend_from_slice(LATER_EDIT);
    let reverted = undo_owned_endpoint(&owner.patch_receipt().unwrap(), &patched).unwrap();
    assert_eq!(reverted, [ORIGINAL, LATER_EDIT].concat());
    let released = owner.released(1_800_000_001).unwrap();
    recovery.config.create(CONFIG, &patched).unwrap();
    recovery.config.create(STAGED_CONFIG, &reverted).unwrap();
    recovery
        .state
        .create(OWNER, &owner.encode().unwrap())
        .unwrap();
    recovery.prepare(&released).unwrap();
    swap_then_terminate(&id);
    assert_eq!(
        OwnershipRecord::decode(&file_bytes(&recovery.state, OWNER)).unwrap(),
        owner
    );
    let reopened = Recovery::open(&proof_root().join(&id)).unwrap();
    assert_eq!(reopened.recover().unwrap(), RecoveryState::Recovered);
    assert_eq!(
        OwnershipRecord::decode(&file_bytes(&reopened.state, OWNER)).unwrap(),
        released
    );
    assert_eq!(file_bytes(&reopened.config, CONFIG), reverted);
    assert_eq!(file_bytes(&reopened.config, STAGED_CONFIG), patched);
}
