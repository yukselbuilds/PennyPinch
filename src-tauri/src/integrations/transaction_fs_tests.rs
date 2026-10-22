#[cfg(windows)]
mod tests {
    use super::super::transaction_fs::{
        FileTransaction, TransactionFsError, MAX_TRANSACTION_FILE_BYTES,
    };
    use std::fs::{self, OpenOptions};
    use std::io::Write;
    use std::path::{Component, Path, PathBuf};
    use std::process::{Command, Stdio};
    use std::thread;
    use std::time::{Duration, Instant};
    use uuid::Uuid;
    use windows_sys::Win32::Foundation::GENERIC_WRITE;
    use windows_sys::Win32::Storage::FileSystem::{
        CreateDirectoryW, CreateFileW, DeleteFileW, MoveFileExW, RemoveDirectoryW, ReplaceFileW,
        FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT, FILE_SHARE_READ,
        FILE_SHARE_WRITE, OPEN_EXISTING,
    };
    use windows_sys::Win32::System::Ioctl::FSCTL_SET_REPARSE_POINT;
    use windows_sys::Win32::System::SystemServices::IO_REPARSE_TAG_MOUNT_POINT;
    use windows_sys::Win32::System::IO::DeviceIoControl;

    const RUNS_RELATIVE: &str = "work/proofs/config-transaction/runs";
    const CHILD_ACTION: &str = "HEADROOM_TXF_CHILD_ACTION";
    const CHILD_TARGET: &str = "HEADROOM_TXF_CHILD_TARGET";
    const CHILD_AUX: &str = "HEADROOM_TXF_CHILD_AUX";
    const CHILD_DEST: &str = "HEADROOM_TXF_CHILD_DEST";

    struct Fixture {
        project: PathBuf,
        runs: PathBuf,
        uuid: String,
        root: PathBuf,
    }

    impl Fixture {
        fn new() -> Self {
            let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
            let project = normalize_path(&manifest.parent().unwrap().canonicalize().unwrap());
            let mut runs = project.clone();
            for component in Path::new(RUNS_RELATIVE).components() {
                let Component::Normal(part) = component else {
                    panic!("fixture path must stay relative");
                };
                runs.push(part);
                ensure_safe_directory(&runs, &project);
            }
            let runs = normalize_path(&runs.canonicalize().unwrap());
            assert!(runs.starts_with(&project));
            let uuid = Uuid::new_v4().to_string();
            let root = runs.join(&uuid);
            assert!(matches!(
                fs::symlink_metadata(&root),
                Err(ref error) if error.kind() == std::io::ErrorKind::NotFound
            ));
            fs::create_dir(&root).expect("create unique proof fixture");
            ensure_safe_directory(&root, &runs);
            Self {
                project,
                runs,
                uuid,
                root,
            }
        }

        fn path(&self, relative: impl AsRef<Path>) -> PathBuf {
            self.root.join(relative)
        }

        fn cleanup(&self) -> Result<(), u8> {
            let metadata = match fs::symlink_metadata(&self.root) {
                Ok(metadata) => metadata,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
                Err(_) => return Err(1),
            };
            if !metadata.is_dir() || is_reparse(&metadata) {
                return Err(2);
            }
            let canonical = normalize_path(&self.root.canonicalize().map_err(|_| 3u8)?);
            let expected = self.runs.join(&self.uuid);
            if path_key(&canonical) != path_key(&expected) || !canonical.starts_with(&self.runs) {
                return Err(4);
            }
            let mut pending = vec![canonical.clone()];
            while let Some(path) = pending.pop() {
                let metadata = fs::symlink_metadata(&path).map_err(|_| 5u8)?;
                if is_reparse(&metadata)
                    || !normalize_path(&path.canonicalize().map_err(|_| 6u8)?)
                        .starts_with(&canonical)
                {
                    return Err(6);
                }
                if metadata.is_dir() {
                    for child in fs::read_dir(&path).map_err(|_| 7u8)? {
                        pending.push(child.map_err(|_| 8u8)?.path());
                    }
                }
            }
            if !canonical.starts_with(&self.project) {
                return Err(9);
            }
            fs::remove_dir_all(canonical).map_err(|_| 10)
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            if let Err(stage) = self.cleanup() {
                eprintln!("headroom_txf_fixture_cleanup_stage={stage}");
            }
        }
    }

    #[test]
    fn transaction_backend_can_open_a_bounded_ntfs_transaction() {
        assert!(FileTransaction::begin().is_ok());
    }

    #[test]
    fn fixture_target_passes_the_same_pinned_path_preflight_as_the_backend() {
        use super::super::config_io;
        let fixture = Fixture::new();
        let target = fixture.path("preflight.json");
        fs::write(&target, b"safe fixture").unwrap();
        assert!(config_io::validate_lexical_path(&target).is_ok(), "lexical");
        assert!(
            config_io::ensure_fixed_volume(&target).is_ok(),
            "fixed_volume"
        );
        let pins = config_io::pin_parent_directories(&target).expect("parent pins");
        assert!(
            config_io::validate_pinned_directories(&pins).is_ok(),
            "pin_identity"
        );
        assert!(
            pins.iter().all(|pin| pin.attributes() & 0x4000 == 0),
            "no_encrypted_parent"
        );
        let parent = target.parent().unwrap();
        let canonical = parent.canonicalize().unwrap();
        assert!(config_io::same_path(parent, &canonical), "canonical_parent");
        drop(pins);
        fixture.cleanup().expect("clean own UUID fixture");
    }

    #[test]
    fn commits_existing_config_and_new_ownership_as_one_transaction() {
        let fixture = Fixture::new();
        let config = fixture.path("config.toml");
        let ownership = fixture.path("ownership.json");
        fs::write(&config, b"model = \"fixture\"\r\n").unwrap();

        let mut transaction = FileTransaction::begin().unwrap();
        let (config_token, config_snapshot) = transaction
            .open_existing(&config, MAX_TRANSACTION_FILE_BYTES)
            .unwrap();
        assert_eq!(config_snapshot.bytes, b"model = \"fixture\"\r\n");
        assert!(config_snapshot.identity.file_index > 0);
        let (ownership_token, ownership_identity) = transaction
            .create_missing(&ownership, MAX_TRANSACTION_FILE_BYTES)
            .unwrap();
        assert!(ownership_identity.file_index > 0);
        transaction
            .replace_contents(
                config_token,
                b"model = \"fixture\"\r\nopenai_base_url = \"http://127.0.0.1:41731/v1\"\r\n",
            )
            .unwrap();
        transaction
            .replace_contents(
                ownership_token,
                b"{\"schema_version\":1,\"owner\":\"fixture\"}",
            )
            .unwrap();

        assert_eq!(fs::read(&config).unwrap(), b"model = \"fixture\"\r\n");
        assert!(!ownership.exists());
        transaction.seal().unwrap();
        transaction.commit().unwrap();
        assert_eq!(
            fs::read(&config).unwrap(),
            b"model = \"fixture\"\r\nopenai_base_url = \"http://127.0.0.1:41731/v1\"\r\n"
        );
        assert_eq!(
            fs::read(&ownership).unwrap(),
            b"{\"schema_version\":1,\"owner\":\"fixture\"}"
        );
        fixture.cleanup().expect("clean own UUID fixture");
    }

    #[test]
    fn explicit_rollback_and_drop_keep_original_and_missing_files_absent() {
        let fixture = Fixture::new();
        let original = fixture.path("original.toml");
        let created = fixture.path("created.json");
        fs::write(&original, b"original\r\n").unwrap();
        {
            let mut transaction = FileTransaction::begin().unwrap();
            let (token, _) = transaction
                .open_existing(&original, MAX_TRANSACTION_FILE_BYTES)
                .unwrap();
            let (created_token, _) = transaction
                .create_missing(&created, MAX_TRANSACTION_FILE_BYTES)
                .unwrap();
            transaction.replace_contents(token, b"rolled back").unwrap();
            transaction
                .replace_contents(created_token, b"also rolled back")
                .unwrap();
            transaction.rollback().unwrap();
        }
        assert_eq!(fs::read(&original).unwrap(), b"original\r\n");
        assert!(!created.exists());

        {
            let mut transaction = FileTransaction::begin().unwrap();
            let (token, _) = transaction
                .open_existing(&original, MAX_TRANSACTION_FILE_BYTES)
                .unwrap();
            transaction
                .replace_contents(token, b"drop rolls back")
                .unwrap();
        }
        assert_eq!(fs::read(&original).unwrap(), b"original\r\n");
        fixture.cleanup().expect("clean own UUID fixture");
    }

    #[test]
    fn external_writer_rename_replace_and_delete_are_denied_until_commit() {
        let fixture = Fixture::new();
        let target = fixture.path("config.json");
        let replacement = fixture.path("replacement.json");
        let moved = fixture.path("moved.json");
        fs::write(&target, b"before").unwrap();
        fs::write(&replacement, b"replacement").unwrap();
        let mut transaction = FileTransaction::begin().unwrap();
        let (token, _) = transaction
            .open_existing(&target, MAX_TRANSACTION_FILE_BYTES)
            .unwrap();
        transaction.replace_contents(token, b"after").unwrap();
        transaction.seal().unwrap();

        for action in ["write", "move", "replace", "delete"] {
            run_external_attempt(action, &target, &replacement, &moved);
            assert_eq!(fs::read(&target).unwrap(), b"before");
        }
        transaction.commit().unwrap();
        assert_eq!(fs::read(&target).unwrap(), b"after");
        assert_eq!(fs::read(&replacement).unwrap(), b"replacement");
        assert!(!moved.exists());
        fixture.cleanup().expect("clean own UUID fixture");
    }

    #[test]
    fn reserved_missing_slot_stays_absent_and_blocks_create_until_commit() {
        let fixture = Fixture::new();
        let slot = fixture.path("alternative.jsonc");
        let replacement = fixture.path("unused.txt");
        let moved = fixture.path("unused-moved.txt");
        let mut transaction = FileTransaction::begin().unwrap();
        transaction.reserve_absent(&slot).unwrap();
        assert!(!slot.exists());
        run_external_attempt("create", &slot, &replacement, &moved);
        assert!(!slot.exists());
        transaction.commit().unwrap();
        assert!(!slot.exists());
        fs::write(&slot, b"available after finish").unwrap();
        assert_eq!(fs::read(&slot).unwrap(), b"available after finish");
        fixture.cleanup().expect("clean own UUID fixture");
    }

    #[test]
    fn writer_present_before_transaction_causes_fail_closed_conflict() {
        let fixture = Fixture::new();
        let target = fixture.path("writer-held.json");
        fs::write(&target, b"stable").unwrap();
        let writer = OpenOptions::new().write(true).open(&target).unwrap();
        let mut transaction = FileTransaction::begin().unwrap();
        assert_eq!(
            transaction
                .open_existing(&target, MAX_TRANSACTION_FILE_BYTES)
                .unwrap_err(),
            TransactionFsError::Conflict
        );
        drop(transaction);
        drop(writer);
        assert_eq!(fs::read(&target).unwrap(), b"stable");
        fixture.cleanup().expect("clean own UUID fixture");
    }

    #[test]
    fn file_count_and_byte_limits_fail_closed_and_preserve_targets() {
        let fixture = Fixture::new();
        let paths: Vec<_> = (0..5)
            .map(|index| fixture.path(format!("bounded-{index}.json")))
            .collect();
        for path in &paths {
            fs::write(path, b"original").unwrap();
        }

        let mut transaction = FileTransaction::begin().unwrap();
        for path in &paths[..4] {
            let (_token, _) = transaction
                .open_existing(path, MAX_TRANSACTION_FILE_BYTES)
                .unwrap();
        }
        assert_eq!(
            transaction
                .open_existing(&paths[4], MAX_TRANSACTION_FILE_BYTES)
                .unwrap_err(),
            TransactionFsError::TooManyFiles
        );
        drop(transaction);
        for path in &paths {
            assert_eq!(fs::read(path).unwrap(), b"original");
        }

        let mut transaction = FileTransaction::begin().unwrap();
        let (token, _) = transaction
            .open_existing(&paths[0], MAX_TRANSACTION_FILE_BYTES)
            .unwrap();
        assert_eq!(
            transaction.replace_contents(token, &vec![b'x'; MAX_TRANSACTION_FILE_BYTES + 1]),
            Err(TransactionFsError::TooLarge)
        );
        drop(transaction);

        let mut transaction = FileTransaction::begin().unwrap();
        assert_eq!(
            transaction.create_missing(&paths[0], MAX_TRANSACTION_FILE_BYTES),
            Err(TransactionFsError::Conflict)
        );
        drop(transaction);
        assert_eq!(fs::read(&paths[0]).unwrap(), b"original");
        fixture.cleanup().expect("clean own UUID fixture");
    }

    #[test]
    fn reservation_interleavings_keep_tokens_bound_to_file_entries() {
        let fixture = Fixture::new();
        let reserved_first = fixture.path("reserved-first.jsonc");
        let config_first = fixture.path("config-first.toml");
        let owner_first = fixture.path("owner-first.json");
        let reserved_middle = fixture.path("reserved-middle.jsonc");
        let config_middle = fixture.path("config-middle.toml");
        let owner_middle = fixture.path("owner-middle.json");
        fs::write(&config_first, b"old-config-first").unwrap();
        fs::write(&owner_first, b"old-owner-first").unwrap();
        fs::write(&config_middle, b"old-config-middle").unwrap();
        fs::write(&owner_middle, b"old-owner-middle").unwrap();

        let mut first = FileTransaction::begin().unwrap();
        first.reserve_absent(&reserved_first).unwrap();
        let (config_token, _) = first
            .open_existing(&config_first, MAX_TRANSACTION_FILE_BYTES)
            .unwrap();
        let (owner_token, _) = first
            .open_existing(&owner_first, MAX_TRANSACTION_FILE_BYTES)
            .unwrap();
        first
            .replace_contents(config_token, b"new-config-first")
            .unwrap();
        first
            .replace_contents(owner_token, b"new-owner-first")
            .unwrap();
        first.commit().unwrap();

        let mut second = FileTransaction::begin().unwrap();
        let (config_token, _) = second
            .open_existing(&config_middle, MAX_TRANSACTION_FILE_BYTES)
            .unwrap();
        second.reserve_absent(&reserved_middle).unwrap();
        let (owner_token, _) = second
            .open_existing(&owner_middle, MAX_TRANSACTION_FILE_BYTES)
            .unwrap();
        second
            .replace_contents(config_token, b"new-config-middle")
            .unwrap();
        second
            .replace_contents(owner_token, b"new-owner-middle")
            .unwrap();
        second.commit().unwrap();

        assert_eq!(fs::read(config_first).unwrap(), b"new-config-first");
        assert_eq!(fs::read(owner_first).unwrap(), b"new-owner-first");
        assert_eq!(fs::read(config_middle).unwrap(), b"new-config-middle");
        assert_eq!(fs::read(owner_middle).unwrap(), b"new-owner-middle");
        assert!(!reserved_first.exists());
        assert!(!reserved_middle.exists());
        fixture.cleanup().expect("clean own UUID fixture");
    }

    #[test]
    fn file_tokens_cannot_cross_transactions_and_post_seal_error_cannot_commit() {
        let fixture = Fixture::new();
        let first_path = fixture.path("first.json");
        let second_path = fixture.path("second.json");
        fs::write(&first_path, b"first-original").unwrap();
        fs::write(&second_path, b"second-original").unwrap();

        let mut first = FileTransaction::begin().unwrap();
        let (first_token, _) = first
            .open_existing(&first_path, MAX_TRANSACTION_FILE_BYTES)
            .unwrap();
        let mut second = FileTransaction::begin().unwrap();
        let (_second_token, _) = second
            .open_existing(&second_path, MAX_TRANSACTION_FILE_BYTES)
            .unwrap();
        assert_eq!(
            second.replace_contents(first_token, b"cross-transaction"),
            Err(TransactionFsError::Conflict)
        );
        drop(second);
        first
            .replace_contents(first_token, b"first-pending")
            .unwrap();
        first.commit().unwrap();
        assert_eq!(fs::read(&first_path).unwrap(), b"first-pending");
        assert_eq!(fs::read(&second_path).unwrap(), b"second-original");

        let mut sealed = FileTransaction::begin().unwrap();
        let (sealed_token, _) = sealed
            .open_existing(&second_path, MAX_TRANSACTION_FILE_BYTES)
            .unwrap();
        sealed
            .replace_contents(sealed_token, b"pending-before-seal")
            .unwrap();
        sealed.seal().unwrap();
        assert_eq!(
            sealed.replace_contents(sealed_token, b"forbidden-after-seal"),
            Err(TransactionFsError::Conflict)
        );
        assert_eq!(sealed.commit(), Err(TransactionFsError::Conflict));
        assert_eq!(fs::read(&second_path).unwrap(), b"second-original");
        fixture.cleanup().expect("clean own UUID fixture");
    }

    #[test]
    fn hardlink_readonly_oversize_and_reparse_targets_are_rejected() {
        let fixture = Fixture::new();
        let hardlink = fixture.path("linked.json");
        let source = fixture.path("source.json");
        fs::write(&source, b"linked").unwrap();
        fs::hard_link(&source, &hardlink).unwrap();
        let mut transaction = FileTransaction::begin().unwrap();
        assert_eq!(
            transaction
                .open_existing(&hardlink, MAX_TRANSACTION_FILE_BYTES)
                .unwrap_err(),
            TransactionFsError::UnsafeFile
        );
        drop(transaction);

        let readonly = fixture.path("readonly.json");
        fs::write(&readonly, b"readonly").unwrap();
        let mut permissions = fs::metadata(&readonly).unwrap().permissions();
        permissions.set_readonly(true);
        fs::set_permissions(&readonly, permissions).unwrap();
        let mut transaction = FileTransaction::begin().unwrap();
        assert_eq!(
            transaction
                .open_existing(&readonly, MAX_TRANSACTION_FILE_BYTES)
                .unwrap_err(),
            TransactionFsError::UnsafeFile
        );
        drop(transaction);
        let mut permissions = fs::metadata(&readonly).unwrap().permissions();
        permissions.set_readonly(false);
        fs::set_permissions(&readonly, permissions).unwrap();

        let large = fixture.path("large.json");
        fs::write(&large, vec![b'x'; MAX_TRANSACTION_FILE_BYTES + 1]).unwrap();
        let mut transaction = FileTransaction::begin().unwrap();
        assert_eq!(
            transaction
                .open_existing(&large, MAX_TRANSACTION_FILE_BYTES)
                .unwrap_err(),
            TransactionFsError::TooLarge
        );
        drop(transaction);

        let target = fixture.path("junction-target");
        fs::create_dir(&target).unwrap();
        fs::write(target.join("config.json"), b"untouched").unwrap();
        let junction = fixture.path("junction-parent");
        create_junction(&junction, &target);
        let mut transaction = FileTransaction::begin().unwrap();
        assert_eq!(
            transaction
                .open_existing(&junction.join("config.json"), MAX_TRANSACTION_FILE_BYTES)
                .unwrap_err(),
            TransactionFsError::UnsafePath
        );
        drop(transaction);
        remove_junction(&junction);
        assert_eq!(fs::read(target.join("config.json")).unwrap(), b"untouched");
        fixture.cleanup().expect("clean own UUID fixture");
    }

    #[test]
    #[ignore = "spawned only as an owned child for external filesystem probes"]
    fn external_filesystem_operation_child() {
        let Ok(action) = std::env::var(CHILD_ACTION) else {
            return;
        };
        let target = PathBuf::from(std::env::var_os(CHILD_TARGET).unwrap());
        let auxiliary = PathBuf::from(std::env::var_os(CHILD_AUX).unwrap());
        let destination = PathBuf::from(std::env::var_os(CHILD_DEST).unwrap());
        let succeeded = match action.as_str() {
            "write" => OpenOptions::new()
                .write(true)
                .open(&target)
                .and_then(|mut file| file.write_all(b"external"))
                .is_ok(),
            "create" => OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&target)
                .is_ok(),
            "delete" => unsafe { DeleteFileW(wide(&target).as_ptr()) != 0 },
            "move" => unsafe {
                MoveFileExW(wide(&target).as_ptr(), wide(&destination).as_ptr(), 0) != 0
            },
            "replace" => unsafe {
                ReplaceFileW(
                    wide(&target).as_ptr(),
                    wide(&auxiliary).as_ptr(),
                    std::ptr::null(),
                    0,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                ) != 0
            },
            _ => panic!("fixed fixture action only"),
        };
        assert!(
            !succeeded,
            "external operation must not mutate a locked target"
        );
    }

    fn run_external_attempt(action: &str, target: &Path, auxiliary: &Path, destination: &Path) {
        let mut child = Command::new(std::env::current_exe().unwrap())
            .arg("--exact")
            .arg("integrations::transaction_fs_tests::tests::external_filesystem_operation_child")
            .arg("--ignored")
            .arg("--nocapture")
            .env(CHILD_ACTION, action)
            .env(CHILD_TARGET, target)
            .env(CHILD_AUX, auxiliary)
            .env(CHILD_DEST, destination)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("start only owned test child");
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            if let Some(status) = child.try_wait().expect("poll owned test child") {
                assert!(status.success(), "external operation was not denied");
                return;
            }
            if Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                panic!("external operation child exceeded fixed deadline");
            }
            thread::sleep(Duration::from_millis(10));
        }
    }

    fn ensure_safe_directory(path: &Path, boundary: &Path) {
        match fs::symlink_metadata(path) {
            Ok(metadata) => {
                assert!(metadata.is_dir());
                assert_no_reparse(&metadata);
                assert!(normalize_path(&path.canonicalize().unwrap()).starts_with(boundary));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let parent = path.parent().unwrap();
                if parent.exists() {
                    assert_safe_existing_directory(parent, boundary);
                }
                fs::create_dir(path).expect("create one checked fixture component");
                assert_safe_existing_directory(path, boundary);
            }
            Err(_) => panic!("fixture directory metadata unavailable"),
        }
    }

    fn assert_safe_existing_directory(path: &Path, boundary: &Path) {
        let metadata = fs::symlink_metadata(path).unwrap();
        assert!(metadata.is_dir());
        assert_no_reparse(&metadata);
        assert!(normalize_path(&path.canonicalize().unwrap()).starts_with(boundary));
    }

    fn assert_no_reparse(metadata: &fs::Metadata) {
        assert!(!is_reparse(metadata));
    }

    fn is_reparse(metadata: &fs::Metadata) -> bool {
        use std::os::windows::fs::MetadataExt;
        metadata.file_type().is_symlink() || metadata.file_attributes() & 0x400 != 0
    }

    fn normalize_path(path: &Path) -> PathBuf {
        let text = path.to_string_lossy();
        text.strip_prefix(r"\\?\")
            .filter(|local| {
                let bytes = local.as_bytes();
                bytes.len() >= 3
                    && bytes[0].is_ascii_alphabetic()
                    && bytes[1] == b':'
                    && matches!(bytes[2], b'\\' | b'/')
            })
            .map(PathBuf::from)
            .unwrap_or_else(|| path.to_path_buf())
    }

    fn path_key(path: &Path) -> String {
        let text = path.to_string_lossy();
        text.strip_prefix(r"\\?\")
            .filter(|local| {
                let bytes = local.as_bytes();
                bytes.len() >= 3
                    && bytes[0].is_ascii_alphabetic()
                    && bytes[1] == b':'
                    && matches!(bytes[2], b'\\' | b'/')
            })
            .unwrap_or(&text)
            .to_lowercase()
    }

    fn wide(path: &Path) -> Vec<u16> {
        use std::os::windows::ffi::OsStrExt;
        path.as_os_str().encode_wide().chain(Some(0)).collect()
    }

    fn create_junction(link: &Path, target: &Path) {
        use std::os::windows::io::{FromRawHandle, OwnedHandle, RawHandle};
        use std::ptr::null;
        use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
        let link_wide = wide(link);
        let target_text = target.to_string_lossy();
        let substitute = format!(r"\??\{target_text}");
        let substitute_wide: Vec<u16> = substitute.encode_utf16().collect();
        let print_wide: Vec<u16> = target_text.encode_utf16().collect();
        let mut path_buffer = Vec::new();
        path_buffer.extend(substitute_wide.iter().copied());
        path_buffer.push(0);
        path_buffer.extend(print_wide.iter().copied());
        path_buffer.push(0);
        let substitute_len = (substitute_wide.len() * 2) as u16;
        let print_offset = ((substitute_wide.len() + 1) * 2) as u16;
        let print_len = (print_wide.len() * 2) as u16;
        let mut reparse = Vec::with_capacity(20 + path_buffer.len() * 2);
        reparse.extend_from_slice(&IO_REPARSE_TAG_MOUNT_POINT.to_le_bytes());
        reparse.extend_from_slice(&(8u16 + (path_buffer.len() * 2) as u16).to_le_bytes());
        reparse.extend_from_slice(&0u16.to_le_bytes());
        reparse.extend_from_slice(&0u16.to_le_bytes());
        reparse.extend_from_slice(&substitute_len.to_le_bytes());
        reparse.extend_from_slice(&print_offset.to_le_bytes());
        reparse.extend_from_slice(&print_len.to_le_bytes());
        for unit in path_buffer {
            reparse.extend_from_slice(&unit.to_le_bytes());
        }
        assert_ne!(unsafe { CreateDirectoryW(link_wide.as_ptr(), null()) }, 0);
        let raw = unsafe {
            CreateFileW(
                link_wide.as_ptr(),
                GENERIC_WRITE,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                null(),
                OPEN_EXISTING,
                FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT,
                std::ptr::null_mut(),
            )
        };
        assert_ne!(raw, INVALID_HANDLE_VALUE);
        let handle = unsafe { OwnedHandle::from_raw_handle(raw as RawHandle) };
        let mut returned = 0u32;
        assert_ne!(
            unsafe {
                DeviceIoControl(
                    raw,
                    FSCTL_SET_REPARSE_POINT,
                    reparse.as_ptr().cast(),
                    reparse.len() as u32,
                    std::ptr::null_mut(),
                    0,
                    &mut returned,
                    std::ptr::null_mut(),
                )
            },
            0
        );
        drop(handle);
    }

    fn remove_junction(path: &Path) {
        assert_ne!(unsafe { RemoveDirectoryW(wide(path).as_ptr()) }, 0);
    }
}
