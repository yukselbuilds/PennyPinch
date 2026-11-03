use super::choices::{
    ChoiceAvailability, ChoiceClient, ChoiceOrigin, ChoiceStore, ChoiceSurface, ClaudeChoiceScope,
    InstallationChoice,
};
use crate::integrations::transaction_fs::FileIdentity;
use std::fs;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle, RawHandle};
use std::path::{Path, PathBuf};
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

struct Fixture {
    root: PathBuf,
    relative_state: PathBuf,
    store: ChoiceStore,
}

impl Fixture {
    fn new() -> Self {
        let project_root = project_root();
        let relative =
            PathBuf::from("work").join(format!(".choices-test-{}", Uuid::new_v4().simple()));
        let root = project_root.join(&relative);
        fs::create_dir_all(root.join("fixtures")).expect("fixture root");
        let relative_state = relative.join("state");
        let store = ChoiceStore::new(project_root, relative_state.clone()).expect("choice store");
        Self {
            root,
            relative_state,
            store,
        }
    }

    fn executable(&self, name: &str) -> PathBuf {
        let path = self.root.join("fixtures").join(name);
        fs::write(&path, b"synthetic executable bytes").expect("synthetic executable");
        path
    }

    fn cleanup(&self) {
        let metadata = match fs::symlink_metadata(&self.root) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return,
            Err(error) => panic!("fixture metadata failed: {error}"),
        };
        assert!(metadata.is_dir() && !is_reparse(&metadata));
        let resolved = normal_canonical(&self.root);
        let project_root = project_root();
        assert!(resolved.starts_with(&project_root));
        assert_fixture_tree_safe(&resolved, &project_root);
        fs::remove_dir_all(resolved).expect("remove owned fixture");
    }
}

fn project_root() -> PathBuf {
    let path = fs::canonicalize(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("project root"),
    )
    .expect("canonical project root");
    normal_canonical(&path)
}

fn normal_canonical(path: &Path) -> PathBuf {
    let path = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    #[cfg(windows)]
    {
        let text = path.to_str().expect("Unicode fixture path");
        PathBuf::from(text.strip_prefix(r"\\?\").unwrap_or(text))
    }
    #[cfg(not(windows))]
    {
        path
    }
}

#[cfg(windows)]
fn is_reparse(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    metadata.file_attributes() & 0x400 != 0
}

#[cfg(not(windows))]
fn is_reparse(metadata: &fs::Metadata) -> bool {
    metadata.file_type().is_symlink()
}

fn assert_fixture_tree_safe(path: &Path, project_root: &Path) {
    for entry in fs::read_dir(path).expect("fixture walk") {
        let entry = entry.expect("fixture entry");
        let metadata = fs::symlink_metadata(entry.path()).expect("fixture child metadata");
        assert!(!is_reparse(&metadata));
        let resolved = normal_canonical(&entry.path());
        assert!(resolved.starts_with(project_root));
        if metadata.is_dir() {
            assert_fixture_tree_safe(&resolved, project_root);
        }
    }
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
            || !target_dos
                .as_bytes()
                .get(1)
                .is_some_and(|byte| *byte == b':')
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
        let input_length = (16usize + path_units * 2) as u32;
        let succeeded = unsafe {
            DeviceIoControl(
                handle.as_raw_handle() as _,
                FSCTL_SET_REPARSE_POINT,
                (&*buffer as *const MountPointReparseBuffer).cast(),
                input_length,
                null_mut(),
                0,
                &mut bytes_returned,
                null_mut(),
            )
        };
        if succeeded == 0 {
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

fn choice(client: ChoiceClient, path: PathBuf) -> InstallationChoice {
    ChoiceStore::installation_for_test(client, path).expect("inspect synthetic executable")
}

#[test]
fn missing_store_is_passive_and_installation_choices_roundtrip_per_client() {
    let fixture = Fixture::new();
    assert_eq!(fixture.store.read().unwrap().revision, 0);
    assert!(!fixture.root.join("state").exists());

    for client in [
        ChoiceClient::Codex,
        ChoiceClient::ClaudeCode,
        ChoiceClient::OpenCode,
    ] {
        let executable = fixture.executable(match client {
            ChoiceClient::Codex => "codex.exe",
            ChoiceClient::ClaudeCode => "claude.exe",
            ChoiceClient::OpenCode => "opencode.exe",
        });
        fixture
            .store
            .save_installation(choice(client, executable))
            .unwrap();
    }
    let project = fixture.root.join("project");
    fs::create_dir(&project).unwrap();
    let claude = ChoiceStore::claude_project_for_test(project).unwrap();
    fixture.store.save_claude(claude).unwrap();

    let project_root = project_root();
    let reopened = ChoiceStore::new(project_root, fixture.relative_state.clone()).unwrap();
    let snapshot = reopened.read().unwrap();
    assert_eq!(snapshot.installations.len(), 3);
    assert_eq!(
        snapshot.claude.as_ref().unwrap().scope,
        ClaudeChoiceScope::ProjectLocalConfig
    );
    assert_eq!(
        snapshot.claude.as_ref().unwrap().availability,
        ChoiceAvailability::Available
    );
    assert_eq!(
        snapshot
            .installations
            .iter()
            .map(|item| item.client)
            .collect::<Vec<_>>(),
        vec![
            ChoiceClient::Codex,
            ChoiceClient::ClaudeCode,
            ChoiceClient::OpenCode
        ]
    );
    fixture.cleanup();
}

#[test]
fn stale_other_client_choice_is_preserved_but_does_not_block_explicit_save() {
    let fixture = Fixture::new();
    let project = fixture.root.join("claude-project");
    fs::create_dir(&project).unwrap();
    let claude = ChoiceStore::claude_project_for_test(project.clone()).unwrap();
    fixture.store.save_claude(claude.clone()).unwrap();

    let moved = fixture.root.join("claude-project-moved");
    fs::rename(&project, &moved).unwrap();
    fs::create_dir(&project).unwrap();
    let stale = fixture.store.read().unwrap();
    assert_eq!(
        stale.claude.as_ref().unwrap().availability,
        ChoiceAvailability::Unavailable
    );

    let executable = fixture.executable("opencode.exe");
    let saved = fixture
        .store
        .save_installation(choice(ChoiceClient::OpenCode, executable))
        .unwrap();
    assert_eq!(saved.installations.len(), 1);
    assert_eq!(
        saved.claude.as_ref().unwrap().project_root,
        claude.project_root
    );
    assert_eq!(
        saved.claude.as_ref().unwrap().availability,
        ChoiceAvailability::Unavailable
    );
    fixture.cleanup();
}

#[test]
fn malformed_or_oversized_store_is_rejected_without_rewriting_bytes() {
    let fixture = Fixture::new();
    let executable = fixture.executable("codex.exe");
    fixture
        .store
        .save_installation(choice(ChoiceClient::Codex, executable))
        .unwrap();
    let path = fixture.root.join("state").join("selections.json");

    for bytes in [
        br#"{"schema_version":99,"revision":1,"installations":[],"claude":null}"#.to_vec(),
        br#"{"schema_version":1,"revision":1,"installations":[],"claude":null,"extra":true}"#
            .to_vec(),
        vec![b'x'; 16 * 1024 + 1],
    ] {
        fs::write(&path, &bytes).unwrap();
        assert!(fixture.store.read().is_err());
        assert_eq!(fs::read(&path).unwrap(), bytes);
    }
    fixture.cleanup();
}

#[test]
fn replaced_executable_is_unavailable_and_invalid_save_preserves_choice_file() {
    let fixture = Fixture::new();
    let executable = fixture.executable("codex.exe");
    fixture
        .store
        .save_installation(choice(ChoiceClient::Codex, executable.clone()))
        .unwrap();

    let store_path = fixture.root.join("state").join("selections.json");
    let saved_bytes = fs::read(&store_path).unwrap();
    let moved = fixture.root.join("fixtures").join("codex-old.exe");
    fs::rename(&executable, &moved).unwrap();
    fs::write(&executable, b"synthetic executable bytes").unwrap();

    let snapshot = fixture.store.read().unwrap();
    assert_eq!(snapshot.installations.len(), 1);
    assert_eq!(
        snapshot.installations[0].availability,
        ChoiceAvailability::Unavailable
    );
    assert_eq!(fs::read(&store_path).unwrap(), saved_bytes);

    let invalid = InstallationChoice {
        client: ChoiceClient::OpenCode,
        surface: ChoiceSurface::Cli,
        origin: ChoiceOrigin::ManualFile,
        executable_path: PathBuf::from("relative.exe"),
        identity: FileIdentity {
            volume_serial: 1,
            file_index: 1,
        },
        size_bytes: 1,
        version: None,
        availability: ChoiceAvailability::Available,
    };
    assert!(fixture.store.save_installation(invalid).is_err());
    assert_eq!(fs::read(&store_path).unwrap(), saved_bytes);
    fixture.cleanup();
}

#[test]
fn boundary_replacement_before_save_does_not_create_state_under_new_directory() {
    let fixture = Fixture::new();
    let boundary = fixture.root.join("owned-boundary");
    fs::create_dir(&boundary).unwrap();
    let store = ChoiceStore::new(&boundary, "state").unwrap();

    let moved_boundary = fixture.root.join("original-boundary");
    fs::rename(&boundary, &moved_boundary).unwrap();
    fs::create_dir(&boundary).unwrap();

    let executable = fixture.executable("codex.exe");
    let result = store.save_installation(choice(ChoiceClient::Codex, executable));
    assert!(result.is_err());
    assert!(
        !boundary.join("state").exists(),
        "replaced boundary must remain untouched"
    );
    fixture.cleanup();
}

#[test]
fn reparse_boundary_is_rejected_before_its_target_can_be_used_as_store_root() {
    let fixture = Fixture::new();
    let target = fixture.root.join("boundary-target");
    let link = fixture.root.join("boundary-link");
    fs::create_dir(&target).unwrap();
    create_directory_junction(&link, &target).expect("create synthetic boundary junction");

    assert!(ChoiceStore::new(&link, "onboarding").is_err());
    assert!(!target.join("onboarding").exists());
    fs::remove_dir(&link).unwrap();
    fixture.cleanup();
}
