use super::ownership::{
    config_path_binding, OwnershipError, OwnershipRecord, OwnershipState, OwnershipStore,
};
use crate::integrations::plan::{
    preview_config_with_presence, ClientKind, ConfigPresence, ConfigScope, ConfigSource,
    ConnectionPlan, PlanContext, StableEndpoint,
};
use crate::integrations::transaction::{
    apply_missing_endpoint, ApplyResult, PatchReceipt, PatchReceiptMetadata,
};
use crate::integrations::transaction_fs::FileTransaction;
use std::fs;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle, RawHandle};
use std::path::{Component, Path, PathBuf};
use std::ptr::{null, null_mut};
use uuid::Uuid;
use windows_sys::Win32::Foundation::{GetLastError, INVALID_HANDLE_VALUE};
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT, FILE_SHARE_DELETE,
    FILE_SHARE_READ, FILE_SHARE_WRITE, FILE_WRITE_ATTRIBUTES, OPEN_EXISTING,
};
use windows_sys::Win32::System::Ioctl::FSCTL_SET_REPARSE_POINT;
use windows_sys::Win32::System::SystemServices::IO_REPARSE_TAG_MOUNT_POINT;
use windows_sys::Win32::System::IO::DeviceIoControl;

const RUNS_RELATIVE: &str = "work/proofs/config-transaction/runs";

struct Fixture {
    project: PathBuf,
    runs: PathBuf,
    root: PathBuf,
    uuid: String,
    cleaned: bool,
}

impl Fixture {
    fn new() -> Self {
        let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let project = normalize_path(&manifest.parent().unwrap().canonicalize().unwrap());
        let mut runs = project.clone();
        for component in Path::new(RUNS_RELATIVE).components() {
            let Component::Normal(part) = component else {
                panic!("fixture path is not relative");
            };
            runs.push(part);
            ensure_directory(&runs, &project);
        }
        let runs = normalize_path(&runs.canonicalize().unwrap());
        assert!(runs.starts_with(&project));
        assert!(!is_reparse(&fs::symlink_metadata(&runs).unwrap()));
        let uuid = Uuid::new_v4().to_string();
        let root = runs.join(&uuid);
        assert!(matches!(
            fs::symlink_metadata(&root),
            Err(ref error) if error.kind() == std::io::ErrorKind::NotFound
        ));
        fs::create_dir(&root).expect("create new UUID fixture");
        ensure_directory(&root, &runs);
        Self {
            project,
            runs,
            root,
            uuid,
            cleaned: false,
        }
    }

    fn path(&self, name: &str) -> PathBuf {
        self.root.join(name)
    }

    fn store(&self) -> OwnershipStore {
        OwnershipStore::new(&self.root, Path::new("ownership")).expect("passive store construction")
    }

    fn cleanup(&mut self) -> Result<(), u8> {
        let metadata = match fs::symlink_metadata(&self.root) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                self.cleaned = true;
                return Ok(());
            }
            Err(_) => return Err(1),
        };
        if !metadata.is_dir() || is_reparse(&metadata) {
            return Err(2);
        }
        let canonical = normalize_path(&self.root.canonicalize().map_err(|_| 3u8)?);
        let expected = self.runs.join(&self.uuid);
        if path_key(&canonical) != path_key(&expected)
            || !canonical.starts_with(&self.runs)
            || !canonical.starts_with(&self.project)
        {
            return Err(4);
        }
        let mut pending = vec![canonical.clone()];
        while let Some(path) = pending.pop() {
            let metadata = fs::symlink_metadata(&path).map_err(|_| 5u8)?;
            if is_reparse(&metadata)
                || !normalize_path(&path.canonicalize().map_err(|_| 6u8)?).starts_with(&canonical)
            {
                return Err(6);
            }
            if metadata.is_dir() {
                for child in fs::read_dir(&path).map_err(|_| 7u8)? {
                    pending.push(child.map_err(|_| 8u8)?.path());
                }
            }
        }
        fs::remove_dir_all(&canonical).map_err(|_| 9u8)?;
        self.cleaned = true;
        Ok(())
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        if !self.cleaned {
            if let Err(stage) = self.cleanup() {
                eprintln!("headroom_ownership_fixture_cleanup_stage={stage}");
            }
        }
    }
}

fn ensure_directory(path: &Path, boundary: &Path) {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            assert!(metadata.is_dir() && !is_reparse(&metadata));
            let canonical = normalize_path(&path.canonicalize().unwrap());
            assert!(canonical.starts_with(boundary));
            assert_eq!(path_key(&canonical), path_key(path));
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let parent = path.parent().unwrap();
            if parent.exists() {
                ensure_directory(parent, boundary);
            }
            fs::create_dir(path).expect("create project fixture directory");
            ensure_directory(path, boundary);
        }
        Err(_) => panic!("fixture directory metadata unavailable"),
    }
}

fn is_reparse(metadata: &fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        false
    }
}

fn normalize_path(path: &Path) -> PathBuf {
    let value = path.to_string_lossy();
    match value.strip_prefix(r"\\?\") {
        Some(rest) if rest.as_bytes().get(1) == Some(&b':') => PathBuf::from(rest),
        _ => path.to_path_buf(),
    }
}

fn path_key(path: &Path) -> String {
    let value = path.to_string_lossy();
    value
        .strip_prefix(r"\\?\")
        .unwrap_or(&value)
        .trim_end_matches(['\\', '/'])
        .to_owned()
}

fn config_source(kind: ClientKind, presence: ConfigPresence) -> ConfigSource {
    match kind {
        ClientKind::Codex => ConfigSource::CodexToml,
        ClientKind::ClaudeCode => ConfigSource::ClaudeJson,
        ClientKind::OpenCodeV1 => match presence {
            ConfigPresence::Missing => ConfigSource::OpenCodeMissing,
            ConfigPresence::Present => ConfigSource::OpenCodeJson,
        },
    }
}

fn preview(kind: ClientKind, presence: ConfigPresence, bytes: &[u8]) -> ConnectionPlan {
    preview_with_scope(kind, presence, bytes, ConfigScope::UserConfig)
}

fn preview_with_scope(
    kind: ClientKind,
    presence: ConfigPresence,
    bytes: &[u8],
    scope: ConfigScope,
) -> ConnectionPlan {
    preview_config_with_presence(
        &PlanContext {
            request_id: "019a0000-0000-7000-8000-d00000000001".to_owned(),
            installation_id: "019a0000-0000-7000-8000-d00000000002".to_owned(),
            client_kind: kind,
            config_source: config_source(kind, presence),
            surface: "cli".to_owned(),
            version: Some(
                match kind {
                    ClientKind::Codex => "0.163.0-alpha.10.2",
                    ClientKind::ClaudeCode => "2.1.158",
                    ClientKind::OpenCodeV1 => "1.19.3",
                }
                .to_owned(),
            ),
            scope,
            scan_generation: 7,
            endpoint: StableEndpoint {
                endpoint_id: "019a0000-0000-7000-8000-d00000000003".to_owned(),
                host: "127.0.0.1".to_owned(),
                port: 41731,
            },
            now_unix_seconds: 1_800_000_000,
        },
        bytes,
        presence,
    )
    .expect("fixture plan")
}

fn patched(kind: ClientKind, presence: ConfigPresence, original: &[u8]) -> (Vec<u8>, PatchReceipt) {
    match apply_missing_endpoint(&preview(kind, presence, original), original).unwrap() {
        ApplyResult::Patched { bytes, receipt } => (bytes, receipt),
        ApplyResult::Unchanged => panic!("fixture requires a missing endpoint"),
    }
}

fn patched_with_scope(
    kind: ClientKind,
    presence: ConfigPresence,
    original: &[u8],
    scope: ConfigScope,
) -> (Vec<u8>, PatchReceipt) {
    match apply_missing_endpoint(
        &preview_with_scope(kind, presence, original, scope),
        original,
    )
    .unwrap()
    {
        ApplyResult::Patched { bytes, receipt } => (bytes, receipt),
        ApplyResult::Unchanged => panic!("fixture requires a missing endpoint"),
    }
}

fn active_record(
    owner_id: &str,
    endpoint_id: &str,
    config_path: &Path,
    before: &[u8],
    after: &[u8],
    receipt: &PatchReceipt,
    revision: u64,
    updated_at: u64,
) -> OwnershipRecord {
    OwnershipRecord::active_from_patch(
        owner_id,
        endpoint_id,
        config_path,
        revision,
        updated_at,
        before,
        after,
        receipt,
    )
    .expect("valid legacy user-config fixture ownership record")
}

fn active_record_with_scope(
    owner_id: &str,
    endpoint_id: &str,
    config_path: &Path,
    before: &[u8],
    after: &[u8],
    receipt: &PatchReceipt,
    revision: u64,
    updated_at: u64,
    scope: ConfigScope,
) -> OwnershipRecord {
    OwnershipRecord::active_from_scoped_patch(
        owner_id,
        endpoint_id,
        config_path,
        revision,
        updated_at,
        before,
        after,
        receipt,
        scope,
    )
    .expect("valid fixture ownership record")
}

#[repr(C)]
struct MountPointReparseBuffer {
    tag: u32,
    data_length: u16,
    reserved: u16,
    substitute_offset: u16,
    substitute_length: u16,
    print_offset: u16,
    print_length: u16,
    path: [u16; 2048],
}

fn create_directory_junction(link: &Path, target: &Path) -> Result<(), u32> {
    fs::create_dir(link).map_err(|error| error.raw_os_error().unwrap_or(1) as u32)?;
    let result = (|| {
        let target = target
            .canonicalize()
            .map_err(|error| error.raw_os_error().unwrap_or(1) as u32)?;
        let target_text = target.to_string_lossy();
        let target_dos = target_text.strip_prefix(r"\\?\").unwrap_or(&target_text);
        if target_dos.starts_with(r"\\")
            || !target_dos.as_bytes().get(1).is_some_and(|b| *b == b':')
        {
            return Err(2);
        }
        let substitute: Vec<u16> = format!(r"\??\{target_dos}").encode_utf16().collect();
        let print: Vec<u16> = target_dos.encode_utf16().collect();
        let path_units = substitute.len() + 1 + print.len() + 1;
        let data_length = 8usize + path_units * std::mem::size_of::<u16>();
        if path_units > 2048 || data_length > u16::MAX as usize {
            return Err(3);
        }
        let mut buffer = Box::new(MountPointReparseBuffer {
            tag: IO_REPARSE_TAG_MOUNT_POINT,
            data_length: data_length as u16,
            reserved: 0,
            substitute_offset: 0,
            substitute_length: (substitute.len() * 2) as u16,
            print_offset: ((substitute.len() + 1) * 2) as u16,
            print_length: (print.len() * 2) as u16,
            path: [0; 2048],
        });
        let mut cursor = 0;
        for unit in substitute
            .iter()
            .copied()
            .chain([0])
            .chain(print.iter().copied())
            .chain([0])
        {
            buffer.path[cursor] = unit;
            cursor += 1;
        }
        let mut wide_link: Vec<u16> = link.as_os_str().encode_wide().collect();
        wide_link.push(0);
        let raw = unsafe {
            CreateFileW(
                wide_link.as_ptr(),
                FILE_WRITE_ATTRIBUTES,
                FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                null(),
                OPEN_EXISTING,
                FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS,
                null_mut(),
            )
        };
        if raw == INVALID_HANDLE_VALUE {
            return Err(unsafe { GetLastError() });
        }
        let handle = unsafe { OwnedHandle::from_raw_handle(raw as RawHandle) };
        let mut bytes_returned = 0u32;
        let input_length = 16usize + path_units * 2;
        let ok = unsafe {
            DeviceIoControl(
                handle.as_raw_handle() as _,
                FSCTL_SET_REPARSE_POINT,
                (&*buffer as *const MountPointReparseBuffer).cast(),
                input_length as u32,
                null_mut(),
                0,
                &mut bytes_returned,
                null_mut(),
            )
        };
        if ok == 0 {
            Err(unsafe { GetLastError() })
        } else {
            Ok(())
        }
    })();
    if result.is_err() {
        let _ = fs::remove_dir(link);
    }
    result
}

fn commit_active(
    fixture: &Fixture,
    store: &OwnershipStore,
    kind: ClientKind,
    original: &[u8],
) -> OwnershipRecord {
    commit_active_with_scope(fixture, store, kind, original, ConfigScope::UserConfig)
}

fn commit_active_with_scope(
    fixture: &Fixture,
    store: &OwnershipStore,
    kind: ClientKind,
    original: &[u8],
    scope: ConfigScope,
) -> OwnershipRecord {
    let config_path = fixture.path("client-config.json");
    fs::write(&config_path, original).unwrap();
    let binding = config_path_binding(&config_path).unwrap();
    store.prepare_for_apply().unwrap();
    let mut transaction = FileTransaction::begin().unwrap();
    let (config_token, snapshot) = transaction.open_existing(&config_path, 256 * 1024).unwrap();
    let (after, receipt) =
        patched_with_scope(kind, ConfigPresence::Present, &snapshot.bytes, scope);
    let ownership_read = store
        .read_in_transaction(&mut transaction, kind, &binding)
        .unwrap();
    let record = if scope == ConfigScope::UserConfig {
        active_record(
            "019a0000-0000-7000-8000-d00000000011",
            "019a0000-0000-7000-8000-d00000000012",
            &config_path,
            &snapshot.bytes,
            &after,
            &receipt,
            1,
            1_800_000_000,
        )
    } else {
        active_record_with_scope(
            "019a0000-0000-7000-8000-d00000000011",
            "019a0000-0000-7000-8000-d00000000012",
            &config_path,
            &snapshot.bytes,
            &after,
            &receipt,
            1,
            1_800_000_000,
            scope,
        )
    };
    store
        .stage_in_transaction(&mut transaction, ownership_read, &record)
        .unwrap();
    transaction.replace_contents(config_token, &after).unwrap();
    transaction.seal().unwrap();
    transaction.commit().unwrap();
    record
}

#[test]
fn store_construction_and_missing_read_are_passive() {
    let mut fixture = Fixture::new();
    let config_path = fixture.path("client-config.json");
    fs::write(&config_path, br#"{"model":"fixture"}"#).unwrap();
    let binding = config_path_binding(&config_path).unwrap();
    let store = fixture.store();
    assert!(!fixture.path("ownership").exists());
    let mut transaction = FileTransaction::begin().unwrap();
    let result = store
        .read_in_transaction(&mut transaction, ClientKind::ClaudeCode, &binding)
        .unwrap();
    assert!(result.current().is_none());
    assert!(!fixture.path("ownership").exists());
    drop(transaction);
    fixture.cleanup().unwrap();
}

#[test]
fn transaction_can_create_a_fixed_slot_under_the_prepared_store_root() {
    let mut fixture = Fixture::new();
    let store = fixture.store();
    store.prepare_for_apply().unwrap();
    let mut transaction = FileTransaction::begin().unwrap();
    let path = store.file_path_for_test(ClientKind::ClaudeCode);
    let created = transaction.create_missing(&path, 16 * 1024);
    assert!(created.is_ok(), "fixed slot create result: {created:?}");
    drop(transaction);
    assert!(!path.exists());
    fixture.cleanup().unwrap();
}

#[test]
fn nested_store_ancestors_are_created_with_pinned_parent_chain() {
    let mut fixture = Fixture::new();
    let store = OwnershipStore::new(&fixture.root, Path::new("app-state/ownership"))
        .expect("bounded nested relative store root");
    let result = store.prepare_for_apply();
    assert!(result.is_ok(), "nested store prepare rejected: {result:?}");
    assert!(fixture.path("app-state/ownership").is_dir());
    let mut transaction = FileTransaction::begin().unwrap();
    let slot = store.file_path_for_test(ClientKind::ClaudeCode);
    let created = transaction.create_missing(&slot, 16 * 1024);
    assert!(created.is_ok(), "nested slot create rejected: {created:?}");
    drop(transaction);
    assert!(!slot.exists());
    fixture.cleanup().unwrap();
}

#[test]
fn reparse_directory_component_fails_closed() {
    let mut fixture = Fixture::new();
    let target = fixture.path("reparse-target");
    fs::create_dir(&target).unwrap();
    let reparse = fixture.path("app-state");
    if let Err(code) = create_directory_junction(&reparse, &target) {
        panic!("fixture-only junction creation failed with Win32 code {code}");
    }
    assert!(matches!(
        OwnershipStore::new(&fixture.root, Path::new("app-state/ownership")),
        Err(OwnershipError::UnsafePath)
    ));
    fs::remove_dir(&reparse).unwrap();
    fixture.cleanup().unwrap();
}

#[test]
fn state_parent_stays_pinned_while_the_owned_store_directory_is_created() {
    let mut fixture = Fixture::new();
    let store = fixture.store();
    let moved = fixture.runs.join(format!("{}-swap", fixture.uuid));
    assert!(!moved.exists());
    let mut attempted = false;
    store
        .prepare_for_apply_with_hook(|_| {
            attempted = true;
            if let Err(error) = fs::rename(&fixture.root, &moved) {
                assert_eq!(error.raw_os_error(), Some(32));
                assert!(!moved.exists());
            } else {
                fs::rename(&moved, &fixture.root).expect("restore only this fixture root");
                panic!("pinned state parent was renamed");
            }
        })
        .unwrap();
    assert!(attempted);
    assert!(fixture.path("ownership").is_dir());
    assert!(!moved.exists());
    fixture.cleanup().unwrap();
}

#[test]
fn config_and_secret_free_ownership_commit_and_reload_together() {
    let mut fixture = Fixture::new();
    let original = br#"{"model":"fixture-only-secret-marker"}"#;
    let store = fixture.store();
    let record = commit_active(&fixture, &store, ClientKind::ClaudeCode, original);
    let config_path = fixture.path("client-config.json");
    assert_ne!(fs::read(&config_path).unwrap(), original);
    let binding = config_path_binding(&config_path).unwrap();
    let ownership_path = store.file_path_for_test(ClientKind::ClaudeCode);
    let encoded = fs::read(&ownership_path).unwrap();
    assert!(encoded.len() <= 16 * 1024);
    assert!(!encoded
        .windows(b"fixture-only-secret-marker".len())
        .any(|window| window == b"fixture-only-secret-marker"));
    assert!(!encoded
        .windows(config_path.to_string_lossy().len())
        .any(|window| window == config_path.to_string_lossy().as_bytes()));

    let reloaded = fixture.store();
    let mut transaction = FileTransaction::begin().unwrap();
    let read = reloaded
        .read_in_transaction(&mut transaction, ClientKind::ClaudeCode, &binding)
        .unwrap();
    let current = read.current().unwrap();
    assert_eq!(current.state(), OwnershipState::Active);
    assert_eq!(current.owner_id(), record.owner_id());
    assert_eq!(current.endpoint_id(), record.endpoint_id());
    assert_eq!(current.endpoint_port(), 41731);
    assert_eq!(current.revision(), 1);
    assert_eq!(current.config_scope(), Some(ConfigScope::UserConfig));
    let legacy_shape: serde_json::Value = serde_json::from_slice(&encoded).unwrap();
    assert_eq!(legacy_shape["schema_version"], 1);
    assert_eq!(legacy_shape["scope"], "user_config");
    assert_eq!(
        current.patch_receipt().unwrap().metadata(),
        record.patch_receipt().unwrap().metadata()
    );
    drop(transaction);
    fixture.cleanup().unwrap();
}

#[test]
fn opencode_multi_route_ownership_persists_only_bounded_route_receipt_metadata() {
    let mut fixture = Fixture::new();
    let original = r#"{
  "model": "lab-a/selector",
  "provider": {
    "lab-a": {"npm": "@ai-sdk/openai-compatible", "models": {"selector": {"id": "wire-a-private"}}, "options": {"apiKey": "fixture-auth-secret", "baseURL": "https:\/\/api.openai.com\/v1"}},
    "openai": {"npm": "@ai-sdk/openai", "models": {"responses": {"id": "wire-responses-private"}}}
  }
}"#
    .as_bytes();
    let store = fixture.store();
    let record = commit_active(&fixture, &store, ClientKind::OpenCodeV1, original);
    let config_path = fixture.path("client-config.json");
    let binding = config_path_binding(&config_path).unwrap();
    let slot = store.file_path_for_test(ClientKind::OpenCodeV1);
    let encoded = fs::read(&slot).unwrap();
    assert!(encoded.len() <= 16 * 1024);
    for private in [
        b"lab-a".as_slice(),
        b"wire-a-private".as_slice(),
        b"wire-responses-private".as_slice(),
        b"fixture-auth-secret".as_slice(),
    ] {
        assert!(!encoded
            .windows(private.len())
            .any(|window| window == private));
    }
    let value: serde_json::Value = serde_json::from_slice(&encoded).unwrap();
    let rows = value["patch_receipt"]["opencode_routes"]
        .as_array()
        .expect("multi-route receipt is persisted");
    assert_eq!(rows.len(), 2);
    let mut masks: Vec<u8> = rows
        .iter()
        .map(|row| row["created_ancestor_mask"].as_u64().unwrap() as u8)
        .collect();
    masks.sort_unstable();
    assert_eq!(masks, [0, 4]);

    let reloaded_store = fixture.store();
    let mut transaction = FileTransaction::begin().unwrap();
    let reloaded = reloaded_store
        .read_in_transaction(&mut transaction, ClientKind::OpenCodeV1, &binding)
        .unwrap()
        .current()
        .cloned()
        .unwrap();
    let restored_metadata = reloaded.patch_receipt().unwrap().metadata();
    assert_eq!(
        restored_metadata,
        record.patch_receipt().unwrap().metadata()
    );
    drop(transaction);
    fixture.cleanup().unwrap();
}

#[test]
fn claude_project_local_ownership_roundtrips_and_release_preserves_scope() {
    let mut fixture = Fixture::new();
    let original = br#"{"model":"fixture-project-only-secret"}"#;
    let store = fixture.store();
    let record = commit_active_with_scope(
        &fixture,
        &store,
        ClientKind::ClaudeCode,
        original,
        ConfigScope::ProjectLocalConfig,
    );
    assert_eq!(record.config_scope(), Some(ConfigScope::ProjectLocalConfig));

    let config_path = fixture.path("client-config.json");
    let binding = config_path_binding(&config_path).unwrap();
    let slot = store.file_path_for_test(ClientKind::ClaudeCode);
    let encoded = fs::read(&slot).unwrap();
    let stored: serde_json::Value = serde_json::from_slice(&encoded).unwrap();
    assert_eq!(stored["schema_version"], 1);
    assert_eq!(stored["scope"], "project_local_config");
    assert!(!encoded
        .windows(original.len())
        .any(|window| window == original));
    assert!(!encoded
        .windows(config_path.to_string_lossy().len())
        .any(|window| window == config_path.to_string_lossy().as_bytes()));

    let reloaded_store = fixture.store();
    let mut read_transaction = FileTransaction::begin().unwrap();
    let reloaded = reloaded_store
        .read_in_transaction(&mut read_transaction, ClientKind::ClaudeCode, &binding)
        .unwrap()
        .current()
        .cloned()
        .unwrap();
    assert_eq!(
        reloaded.config_scope(),
        Some(ConfigScope::ProjectLocalConfig)
    );
    drop(read_transaction);

    let mut conflict_transaction = FileTransaction::begin().unwrap();
    let current = reloaded_store
        .read_in_transaction(&mut conflict_transaction, ClientKind::ClaudeCode, &binding)
        .unwrap();
    let config_after = fs::read(&config_path).unwrap();
    let user_scoped = active_record_with_scope(
        reloaded.owner_id(),
        reloaded.endpoint_id(),
        &config_path,
        original,
        &config_after,
        &reloaded.patch_receipt().unwrap(),
        reloaded.revision() + 1,
        1_800_000_001,
        ConfigScope::UserConfig,
    );
    assert!(matches!(
        reloaded_store.stage_in_transaction(&mut conflict_transaction, current, &user_scoped),
        Err(OwnershipError::Conflict)
    ));
    drop(conflict_transaction);

    let mut release_transaction = FileTransaction::begin().unwrap();
    let current = reloaded_store
        .read_in_transaction(&mut release_transaction, ClientKind::ClaudeCode, &binding)
        .unwrap();
    let released = current.current().unwrap().released(1_800_000_001).unwrap();
    assert_eq!(
        released.config_scope(),
        Some(ConfigScope::ProjectLocalConfig)
    );
    reloaded_store
        .stage_in_transaction(&mut release_transaction, current, &released)
        .unwrap();
    release_transaction.seal().unwrap();
    release_transaction.commit().unwrap();

    let mut final_transaction = FileTransaction::begin().unwrap();
    let final_record = reloaded_store
        .read_in_transaction(&mut final_transaction, ClientKind::ClaudeCode, &binding)
        .unwrap()
        .current()
        .cloned()
        .unwrap();
    assert_eq!(final_record.state(), OwnershipState::Released);
    assert_eq!(
        final_record.config_scope(),
        Some(ConfigScope::ProjectLocalConfig)
    );
    assert_eq!(final_record.revision(), 2);
    drop(final_transaction);
    fixture.cleanup().unwrap();
}

#[test]
fn project_local_ownership_scope_is_rejected_for_other_clients() {
    let mut fixture = Fixture::new();
    for (kind, original) in [
        (ClientKind::Codex, b"model = \"fixture\"\n".as_slice()),
        (
            ClientKind::OpenCodeV1,
            br#"{"model":"openai/fixture"}"#.as_slice(),
        ),
    ] {
        let config_path = fixture.path(match kind {
            ClientKind::Codex => "codex-config.toml",
            ClientKind::OpenCodeV1 => "opencode-config.json",
            ClientKind::ClaudeCode => unreachable!(),
        });
        fs::write(&config_path, original).unwrap();
        let (after, receipt) = patched(kind, ConfigPresence::Present, original);
        let result = OwnershipRecord::active_from_scoped_patch(
            "019a0000-0000-7000-8000-d00000000071",
            "019a0000-0000-7000-8000-d00000000072",
            &config_path,
            1,
            1_800_000_000,
            original,
            &after,
            &receipt,
            ConfigScope::ProjectLocalConfig,
        );
        assert!(matches!(result, Err(OwnershipError::Invalid)), "{kind:?}");
    }

    let codex_store = fixture.store();
    let codex_original = b"model = \"fixture\"\n";
    commit_active(&fixture, &codex_store, ClientKind::Codex, codex_original);
    let codex_path = fixture.path("client-config.json");
    let binding = config_path_binding(&codex_path).unwrap();
    let slot = codex_store.file_path_for_test(ClientKind::Codex);
    let mut stored: serde_json::Value = serde_json::from_slice(&fs::read(&slot).unwrap()).unwrap();
    stored["scope"] = serde_json::json!("project_local_config");
    fs::write(&slot, serde_json::to_vec(&stored).unwrap()).unwrap();
    let mut transaction = FileTransaction::begin().unwrap();
    assert!(matches!(
        codex_store.read_in_transaction(&mut transaction, ClientKind::Codex, &binding),
        Err(OwnershipError::Invalid)
    ));
    drop(transaction);
    fixture.cleanup().unwrap();
}

#[test]
fn active_record_does_not_follow_another_config_path() {
    let mut fixture = Fixture::new();
    let original = br#"{"model":"fixture"}"#;
    let store = fixture.store();
    commit_active(&fixture, &store, ClientKind::ClaudeCode, original);
    let second_path = fixture.path("other-config.json");
    fs::write(&second_path, original).unwrap();
    let second_binding = config_path_binding(&second_path).unwrap();
    let mut transaction = FileTransaction::begin().unwrap();
    let result =
        store.read_in_transaction(&mut transaction, ClientKind::ClaudeCode, &second_binding);
    assert!(matches!(result, Err(OwnershipError::Conflict)));
    drop(transaction);
    fixture.cleanup().unwrap();
}

#[test]
fn linked_ownership_slot_fails_closed() {
    let mut fixture = Fixture::new();
    let config_path = fixture.path("client-config.json");
    fs::write(&config_path, br#"{"model":"fixture"}"#).unwrap();
    let binding = config_path_binding(&config_path).unwrap();
    let store = fixture.store();
    store.prepare_for_apply().unwrap();
    let slot = store.file_path_for_test(ClientKind::ClaudeCode);
    let link = fixture.path("slot-link.json");
    fs::write(&slot, b"{}").unwrap();
    fs::hard_link(&slot, &link).unwrap();
    let mut transaction = FileTransaction::begin().unwrap();
    assert!(matches!(
        store.read_in_transaction(&mut transaction, ClientKind::ClaudeCode, &binding),
        Err(OwnershipError::UnsafePath)
    ));
    drop(transaction);
    assert!(slot.exists() && link.exists());
    fixture.cleanup().unwrap();
}

#[test]
fn active_owner_cannot_be_replaced_by_another_owner() {
    let mut fixture = Fixture::new();
    let original = br#"{"model":"fixture"}"#;
    let store = fixture.store();
    let initial = commit_active(&fixture, &store, ClientKind::ClaudeCode, original);
    let config_path = fixture.path("client-config.json");
    let after = fs::read(&config_path).unwrap();
    let receipt = initial.patch_receipt().unwrap();
    let conflicting = active_record(
        "019a0000-0000-7000-8000-d00000000021",
        initial.endpoint_id(),
        &config_path,
        original,
        &after,
        &receipt,
        2,
        1_800_000_001,
    );
    let ownership_path = store.file_path_for_test(ClientKind::ClaudeCode);
    let before = fs::read(&ownership_path).unwrap();
    let binding = config_path_binding(&config_path).unwrap();
    let mut transaction = FileTransaction::begin().unwrap();
    let current = store
        .read_in_transaction(&mut transaction, ClientKind::ClaudeCode, &binding)
        .unwrap();
    assert!(matches!(
        store.stage_in_transaction(&mut transaction, current, &conflicting),
        Err(OwnershipError::Conflict)
    ));
    drop(transaction);
    assert_eq!(fs::read(&ownership_path).unwrap(), before);
    fixture.cleanup().unwrap();
}

#[test]
fn staging_config_and_ownership_rolls_back_together_on_drop() {
    let mut fixture = Fixture::new();
    let original = br#"{"model":"fixture"}"#;
    let config_path = fixture.path("client-config.json");
    fs::write(&config_path, original).unwrap();
    let binding = config_path_binding(&config_path).unwrap();
    let store = fixture.store();
    store.prepare_for_apply().unwrap();
    {
        let mut transaction = FileTransaction::begin().unwrap();
        let (config_token, snapshot) = transaction.open_existing(&config_path, 256 * 1024).unwrap();
        let (after, receipt) = patched(
            ClientKind::ClaudeCode,
            ConfigPresence::Present,
            &snapshot.bytes,
        );
        let current = store
            .read_in_transaction(&mut transaction, ClientKind::ClaudeCode, &binding)
            .unwrap();
        let record = active_record(
            "019a0000-0000-7000-8000-d00000000031",
            "019a0000-0000-7000-8000-d00000000032",
            &config_path,
            &snapshot.bytes,
            &after,
            &receipt,
            1,
            1_800_000_000,
        );
        store
            .stage_in_transaction(&mut transaction, current, &record)
            .unwrap();
        transaction.replace_contents(config_token, &after).unwrap();
        transaction.seal().unwrap();
    }
    assert_eq!(fs::read(&config_path).unwrap(), original);
    assert!(!store.file_path_for_test(ClientKind::ClaudeCode).exists());
    fixture.cleanup().unwrap();
}

#[test]
fn released_record_preserves_owner_and_advances_safe_revision() {
    let mut fixture = Fixture::new();
    let store = fixture.store();
    let record = commit_active(
        &fixture,
        &store,
        ClientKind::Codex,
        b"model = \"fixture\"\n",
    );
    let released = record.released(1_800_000_001).unwrap();
    assert_eq!(released.state(), OwnershipState::Released);
    assert_eq!(released.revision(), 2);
    assert_eq!(released.owner_id(), record.owner_id());
    assert_eq!(released.endpoint_id(), record.endpoint_id());
    fixture.cleanup().unwrap();
}

#[test]
fn opencode_receipts_preserve_supported_presence_and_source_masks() {
    let mut fixture = Fixture::new();
    let present_path = fixture.path("opencode.json");
    let present_before = br#"{"model":"openai/gpt-5-codex"}"#;
    fs::write(&present_path, present_before).unwrap();
    let (present_after, present_receipt) = patched(
        ClientKind::OpenCodeV1,
        ConfigPresence::Present,
        present_before,
    );
    let present = active_record(
        "019a0000-0000-7000-8000-d00000000051",
        "019a0000-0000-7000-8000-d00000000052",
        &present_path,
        present_before,
        &present_after,
        &present_receipt,
        1,
        1_800_000_000,
    );
    assert_eq!(present.config_source(), Some(ConfigSource::OpenCodeJson));
    assert_eq!(present.config_was_missing(), false);

    let missing_path = fixture.path("opencode-missing.json");
    let missing_before = b"{}";
    // The planner intentionally blocks OpenCodeMissing. This codec-only
    // fixture uses the receipt allowlist directly rather than claiming a plan
    // can currently produce a patch for that source.
    let missing_after = br#"{"fixture":"managed endpoint leaf"}"#;
    let missing_receipt = PatchReceipt::from_metadata(PatchReceiptMetadata {
        client_kind: ClientKind::OpenCodeV1,
        config_source: ConfigSource::OpenCodeMissing,
        endpoint_port: 41731,
        created_ancestor_mask: 7,
        config_was_missing: true,
        opencode_routes: None,
    })
    .expect("valid codec-only missing-source receipt");
    assert!(!missing_path.exists());
    let missing = active_record(
        "019a0000-0000-7000-8000-d00000000061",
        "019a0000-0000-7000-8000-d00000000062",
        &missing_path,
        missing_before,
        missing_after,
        &missing_receipt,
        1,
        1_800_000_000,
    );
    assert_eq!(missing.config_source(), Some(ConfigSource::OpenCodeMissing));
    assert!(missing.config_was_missing());
    fixture.cleanup().unwrap();
}

#[test]
fn malformed_and_unknown_slot_fields_fail_closed() {
    let mut fixture = Fixture::new();
    let config_path = fixture.path("client-config.json");
    fs::write(&config_path, br#"{"model":"fixture"}"#).unwrap();
    let binding = config_path_binding(&config_path).unwrap();
    let store = fixture.store();
    store.prepare_for_apply().unwrap();
    let slot = store.file_path_for_test(ClientKind::ClaudeCode);
    fs::write(
        &slot,
        br#"{"schema_version":1,"owner_tag":"headroom.connection_ownership.v1","state":"active","owner_id":"019a0000-0000-7000-8000-d00000000041","endpoint_id":"019a0000-0000-7000-8000-d00000000042","client_kind":"claude_code","scope":"user_config","config_source":"claude_json","config_presence":"present","config_path_binding_sha256":"0000000000000000000000000000000000000000000000000000000000000000","endpoint_host":"127.0.0.1","endpoint_port":41731,"revision":1,"updated_at_unix_seconds":1800000000,"before_config_sha256":"0000000000000000000000000000000000000000000000000000000000000000","after_config_sha256":"1111111111111111111111111111111111111111111111111111111111111111","patch_receipt":{"client_kind":"claude_code","config_source":"claude_json","endpoint_port":41731,"created_ancestor_mask":0,"config_was_missing":false},"unexpected":"must reject"}"#,
    )
    .unwrap();
    let mut transaction = FileTransaction::begin().unwrap();
    assert!(matches!(
        store.read_in_transaction(&mut transaction, ClientKind::ClaudeCode, &binding),
        Err(OwnershipError::Invalid)
    ));
    drop(transaction);
    fixture.cleanup().unwrap();
}

#[test]
fn invalid_schema_ids_hashes_masks_and_duplicate_keys_fail_closed() {
    let mut fixture = Fixture::new();
    let original = br#"{"model":"fixture"}"#;
    let store = fixture.store();
    commit_active(&fixture, &store, ClientKind::ClaudeCode, original);
    let config_path = fixture.path("client-config.json");
    let binding = config_path_binding(&config_path).unwrap();
    let slot = store.file_path_for_test(ClientKind::ClaudeCode);
    let valid_bytes = fs::read(&slot).unwrap();
    let valid_value: serde_json::Value = serde_json::from_slice(&valid_bytes).unwrap();

    let mut invalid_cases = Vec::new();
    for (field, value) in [
        ("schema_version", serde_json::json!(2)),
        ("scope", serde_json::json!("unknown_scope")),
        ("owner_id", serde_json::json!("not-a-uuid")),
        (
            "config_path_binding_sha256",
            serde_json::json!("AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"),
        ),
        ("revision", serde_json::json!(9_007_199_254_740_992u64)),
        (
            "updated_at_unix_seconds",
            serde_json::json!(9_007_199_254_740_992u64),
        ),
    ] {
        let mut value_doc = valid_value.clone();
        value_doc[field] = value;
        invalid_cases.push(serde_json::to_vec(&value_doc).unwrap());
    }
    let mut bad_receipt = valid_value.clone();
    bad_receipt["patch_receipt"]["created_ancestor_mask"] = serde_json::json!(2);
    invalid_cases.push(serde_json::to_vec(&bad_receipt).unwrap());
    let mut bad_source = valid_value.clone();
    bad_source["config_source"] = serde_json::json!("opencode_missing");
    invalid_cases.push(serde_json::to_vec(&bad_source).unwrap());
    let mut duplicate = valid_bytes.clone();
    duplicate.pop();
    duplicate.extend_from_slice(b",\"schema_version\":1}");
    invalid_cases.push(duplicate);

    for bytes in invalid_cases {
        fs::write(&slot, bytes).unwrap();
        let mut transaction = FileTransaction::begin().unwrap();
        assert!(matches!(
            store.read_in_transaction(&mut transaction, ClientKind::ClaudeCode, &binding),
            Err(OwnershipError::Invalid)
        ));
        drop(transaction);
    }
    fs::write(&slot, valid_bytes).unwrap();
    fixture.cleanup().unwrap();
}

#[test]
fn active_record_path_binding_is_canonical_and_domain_separated() {
    let mut fixture = Fixture::new();
    let first = fixture.path("client-config.json");
    let second = fixture.path("different-config.json");
    fs::write(&first, b"one").unwrap();
    fs::write(&second, b"one").unwrap();
    let first_hash = config_path_binding(&first).unwrap();
    let canonical_hash = config_path_binding(&first.canonicalize().unwrap()).unwrap();
    let second_hash = config_path_binding(&second).unwrap();
    assert_eq!(first_hash.len(), 64);
    assert!(first_hash
        .bytes()
        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)));
    assert_ne!(first_hash, second_hash);
    assert_eq!(first_hash, canonical_hash);
    fixture.cleanup().unwrap();
}

#[test]
fn no_read_or_codec_path_accepts_oversized_slot() {
    let mut fixture = Fixture::new();
    let config_path = fixture.path("client-config.json");
    fs::write(&config_path, br#"{"model":"fixture"}"#).unwrap();
    let binding = config_path_binding(&config_path).unwrap();
    let store = fixture.store();
    store.prepare_for_apply().unwrap();
    let slot = store.file_path_for_test(ClientKind::ClaudeCode);
    fs::write(&slot, vec![b' '; 16 * 1024 + 1]).unwrap();
    let mut transaction = FileTransaction::begin().unwrap();
    assert!(matches!(
        store.read_in_transaction(&mut transaction, ClientKind::ClaudeCode, &binding),
        Err(OwnershipError::TooLarge)
    ));
    drop(transaction);
    fixture.cleanup().unwrap();
}
