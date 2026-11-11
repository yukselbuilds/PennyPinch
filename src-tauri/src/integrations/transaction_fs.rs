const MAX_TRANSACTION_FILES: usize = 4;
pub(crate) const MAX_TRANSACTION_FILE_BYTES: usize = 256 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TransactionFsError {
    Unsupported,
    UnsafePath,
    UnsafeFile,
    Conflict,
    TooLarge,
    TooManyFiles,
    Expired,
    Io,
    CommitFailed,
    RollbackFailed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct FileToken {
    transaction_id: u64,
    entry_index: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ClaudeDirectoryToken {
    transaction_id: u64,
    entry_index: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct FileIdentity {
    pub(crate) volume_serial: u32,
    pub(crate) file_index: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FileSnapshot {
    pub(crate) bytes: Vec<u8>,
    pub(crate) identity: FileIdentity,
}

#[cfg(windows)]
mod windows_impl {
    use super::{
        ClaudeDirectoryToken, FileIdentity, FileSnapshot, FileToken, TransactionFsError,
        MAX_TRANSACTION_FILES, MAX_TRANSACTION_FILE_BYTES,
    };
    use crate::integrations::config_io::{self, ConfigIoError, DirectoryPins};
    use std::fs;
    use std::os::windows::ffi::OsStrExt;
    use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle, RawHandle};
    use std::path::{Path, PathBuf};
    use std::ptr::{null, null_mut};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{Duration, Instant};
    use windows_sys::Win32::Foundation::{
        GetLastError, ERROR_ALREADY_EXISTS, ERROR_FILE_EXISTS, ERROR_FILE_NOT_FOUND,
        ERROR_LOCK_VIOLATION, ERROR_PATH_NOT_FOUND, ERROR_SHARING_VIOLATION,
        ERROR_TRANSACTIONAL_CONFLICT, GENERIC_READ, GENERIC_WRITE, HANDLE, INVALID_HANDLE_VALUE,
    };
    use windows_sys::Win32::Storage::FileSystem::{
        CommitTransaction, CreateDirectoryTransactedW, CreateFileTransactedW, CreateTransaction,
        FlushFileBuffers, GetFileInformationByHandle, GetFileSizeEx, GetVolumeInformationByHandleW,
        ReadFile, RollbackTransaction, SetEndOfFile, SetFilePointerEx, WriteFile,
        BY_HANDLE_FILE_INFORMATION, CREATE_NEW, DELETE, FILE_ADD_FILE, FILE_ATTRIBUTE_DIRECTORY,
        FILE_ATTRIBUTE_ENCRYPTED, FILE_ATTRIBUTE_NORMAL, FILE_ATTRIBUTE_READONLY,
        FILE_ATTRIBUTE_REPARSE_POINT, FILE_BEGIN, FILE_FLAG_BACKUP_SEMANTICS,
        FILE_FLAG_DELETE_ON_CLOSE, FILE_FLAG_OPEN_REPARSE_POINT, FILE_LIST_DIRECTORY,
        FILE_READ_ATTRIBUTES, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING, SYNCHRONIZE,
        TRANSACTION_DO_NOT_PROMOTE,
    };
    use windows_sys::Win32::System::SystemServices::FILE_SUPPORTS_TRANSACTIONS;

    const TRANSACTION_TIMEOUT_MS: u32 = 10_000;
    const TRANSACTION_WALL_LIMIT: Duration = Duration::from_secs(10);
    const MAX_PATH_UNITS: usize = 32_767;
    const MAX_PARENT_COMPONENTS: usize = 256;
    static NEXT_TRANSACTION_ID: AtomicU64 = AtomicU64::new(1);
    struct PreparedTarget {
        canonical_path: PathBuf,
        pins: DirectoryPins,
        volume_serial: u32,
    }

    struct Entry {
        handle: Option<OwnedHandle>,
        max_bytes: usize,
        canonical_path: PathBuf,
    }

    struct DirectoryEntry {
        handle: Option<OwnedHandle>,
        canonical_path: PathBuf,
        identity: FileIdentity,
        volume_serial: u32,
    }

    pub(crate) struct FileTransaction {
        transaction_id: u64,
        transaction: Option<OwnedHandle>,
        started: Instant,
        entries: Vec<Entry>,
        directories: Vec<DirectoryEntry>,
        reservations: Vec<PathBuf>,
        pinned_directories: Vec<DirectoryPins>,
        paths: Vec<PathBuf>,
        volume_serial: Option<u32>,
        sealed: bool,
        poisoned: bool,
        finished: bool,
    }

    impl FileTransaction {
        pub(crate) fn begin() -> Result<Self, TransactionFsError> {
            let transaction_id = NEXT_TRANSACTION_ID
                .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                    current.checked_add(1)
                })
                .map_err(|_| TransactionFsError::Unsupported)?;
            let description: Vec<u16> = "Headroom config write"
                .encode_utf16()
                .chain(Some(0))
                .collect();
            let raw = unsafe {
                CreateTransaction(
                    null_mut(),
                    null_mut(),
                    TRANSACTION_DO_NOT_PROMOTE,
                    0,
                    0,
                    TRANSACTION_TIMEOUT_MS,
                    description.as_ptr(),
                )
            };
            if raw.is_null() || raw == INVALID_HANDLE_VALUE {
                return Err(TransactionFsError::Unsupported);
            }
            Ok(Self {
                transaction_id,
                transaction: Some(unsafe { OwnedHandle::from_raw_handle(raw as RawHandle) }),
                started: Instant::now(),
                entries: Vec::new(),
                directories: Vec::new(),
                reservations: Vec::new(),
                pinned_directories: Vec::new(),
                paths: Vec::new(),
                volume_serial: None,
                sealed: false,
                poisoned: false,
                finished: false,
            })
        }

        pub(crate) fn open_existing(
            &mut self,
            path: &Path,
            max_bytes: usize,
        ) -> Result<(FileToken, FileSnapshot), TransactionFsError> {
            let result = self.open_existing_inner(path, max_bytes);
            if result.is_err() {
                self.poisoned = true;
            }
            result
        }

        fn open_existing_inner(
            &mut self,
            path: &Path,
            max_bytes: usize,
        ) -> Result<(FileToken, FileSnapshot), TransactionFsError> {
            self.ensure_mutable()?;
            validate_max_bytes(max_bytes)?;
            let target = prepare_target(path)?;
            self.check_target(&target)?;
            match fs::symlink_metadata(path) {
                Ok(metadata) => {
                    if !metadata.is_file() || metadata_is_unsafe_file(&metadata) {
                        return Err(TransactionFsError::UnsafeFile);
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    return Err(TransactionFsError::Conflict)
                }
                Err(_) => return Err(TransactionFsError::Io),
            }

            let wide = wide_path(path)?;
            let raw = unsafe {
                CreateFileTransactedW(
                    wide.as_ptr(),
                    GENERIC_READ | GENERIC_WRITE,
                    FILE_SHARE_READ,
                    null(),
                    OPEN_EXISTING,
                    FILE_ATTRIBUTE_NORMAL | FILE_FLAG_OPEN_REPARSE_POINT,
                    null_mut(),
                    self.transaction_handle(),
                    null(),
                    null(),
                )
            };
            if is_invalid_handle(raw) {
                return Err(map_open_error(unsafe { GetLastError() }));
            }
            let handle = unsafe { OwnedHandle::from_raw_handle(raw as RawHandle) };
            let info = file_information(handle.as_raw_handle() as HANDLE)?;
            let identity = validate_file_info(&info, target.volume_serial)?;
            let bytes = read_bounded(handle.as_raw_handle() as HANDLE, max_bytes)?;
            verify_target_after_open(path, &target)?;

            let token = self.register_entry(target, handle, max_bytes)?;
            Ok((token, FileSnapshot { bytes, identity }))
        }

        pub(crate) fn create_missing(
            &mut self,
            path: &Path,
            max_bytes: usize,
        ) -> Result<(FileToken, FileIdentity), TransactionFsError> {
            let result = self.create_missing_inner(path, max_bytes);
            if result.is_err() {
                self.poisoned = true;
            }
            result
        }

        fn create_missing_inner(
            &mut self,
            path: &Path,
            max_bytes: usize,
        ) -> Result<(FileToken, FileIdentity), TransactionFsError> {
            self.ensure_mutable()?;
            validate_max_bytes(max_bytes)?;
            let target = prepare_target(path)?;
            self.check_target(&target)?;
            match fs::symlink_metadata(path) {
                Ok(_) => return Err(TransactionFsError::Conflict),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(_) => return Err(TransactionFsError::Io),
            }

            let wide = wide_path(path)?;
            let raw = unsafe {
                CreateFileTransactedW(
                    wide.as_ptr(),
                    GENERIC_READ | GENERIC_WRITE,
                    FILE_SHARE_READ,
                    null(),
                    CREATE_NEW,
                    FILE_ATTRIBUTE_NORMAL | FILE_FLAG_OPEN_REPARSE_POINT,
                    null_mut(),
                    self.transaction_handle(),
                    null(),
                    null(),
                )
            };
            if is_invalid_handle(raw) {
                return Err(map_create_error(unsafe { GetLastError() }));
            }
            let handle = unsafe { OwnedHandle::from_raw_handle(raw as RawHandle) };
            let info = file_information(handle.as_raw_handle() as HANDLE)?;
            let identity = validate_file_info(&info, target.volume_serial)?;
            verify_target_after_open(path, &target)?;
            let token = self.register_entry(target, handle, max_bytes)?;
            Ok((token, identity))
        }

        pub(crate) fn create_claude_directory(
            &mut self,
            project_root: &Path,
        ) -> Result<(ClaudeDirectoryToken, FileIdentity), TransactionFsError> {
            let result = self.create_claude_directory_inner(project_root);
            if result.is_err() {
                self.poisoned = true;
            }
            result
        }

        fn create_claude_directory_inner(
            &mut self,
            project_root: &Path,
        ) -> Result<(ClaudeDirectoryToken, FileIdentity), TransactionFsError> {
            self.ensure_mutable()?;
            let target = prepare_target(&project_root.join(".claude"))?;
            self.check_target(&target)?;
            match fs::symlink_metadata(&target.canonical_path) {
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Ok(metadata) if metadata.is_dir() && !metadata_is_reparse(&metadata) => {
                    return Err(TransactionFsError::Conflict);
                }
                Ok(_) => return Err(TransactionFsError::UnsafePath),
                Err(_) => return Err(TransactionFsError::Io),
            }

            let wide = wide_path(&target.canonical_path)?;
            let created = unsafe {
                CreateDirectoryTransactedW(null(), wide.as_ptr(), null(), self.transaction_handle())
            };
            if created == 0 {
                return Err(map_create_error(unsafe { GetLastError() }));
            }

            let handle =
                open_transacted_directory(&target.canonical_path, self.transaction_handle())?;
            let identity = validate_directory_info(
                &file_information(handle.as_raw_handle() as HANDLE)?,
                target.volume_serial,
            )?;
            verify_target_after_open(&target.canonical_path, &target)?;
            let token = self.register_directory(target, handle, identity)?;
            Ok((token, identity))
        }

        pub(crate) fn create_claude_settings_file(
            &mut self,
            directory: ClaudeDirectoryToken,
            max_bytes: usize,
        ) -> Result<(FileToken, FileIdentity), TransactionFsError> {
            let result = self.create_claude_settings_file_inner(directory, max_bytes);
            if result.is_err() {
                self.poisoned = true;
            }
            result
        }

        fn create_claude_settings_file_inner(
            &mut self,
            directory: ClaudeDirectoryToken,
            max_bytes: usize,
        ) -> Result<(FileToken, FileIdentity), TransactionFsError> {
            self.ensure_mutable()?;
            validate_max_bytes(max_bytes)?;
            if directory.transaction_id != self.transaction_id {
                return Err(TransactionFsError::Conflict);
            }
            let directory_entry = self
                .directories
                .get(directory.entry_index)
                .ok_or(TransactionFsError::Conflict)?;
            let directory_path = directory_entry.canonical_path.clone();
            let directory_identity = directory_entry.identity;
            let directory_volume = directory_entry.volume_serial;
            let directory_handle = directory_entry
                .handle
                .as_ref()
                .ok_or(TransactionFsError::Conflict)?;
            if validate_directory_info(
                &file_information(directory_handle.as_raw_handle() as HANDLE)?,
                directory_volume,
            )? != directory_identity
            {
                return Err(TransactionFsError::Conflict);
            }
            self.validate_all_pins()?;

            let path = directory_path.join("settings.local.json");
            self.check_child_path(&path, directory_volume)?;
            let wide = wide_path(&path)?;
            let raw = unsafe {
                CreateFileTransactedW(
                    wide.as_ptr(),
                    GENERIC_READ | GENERIC_WRITE,
                    FILE_SHARE_READ,
                    null(),
                    CREATE_NEW,
                    FILE_ATTRIBUTE_NORMAL | FILE_FLAG_OPEN_REPARSE_POINT,
                    null_mut(),
                    self.transaction_handle(),
                    null(),
                    null(),
                )
            };
            if is_invalid_handle(raw) {
                return Err(map_create_error(unsafe { GetLastError() }));
            }
            let handle = unsafe { OwnedHandle::from_raw_handle(raw as RawHandle) };
            let identity = validate_file_info(
                &file_information(handle.as_raw_handle() as HANDLE)?,
                directory_volume,
            )?;
            let current_directory = self
                .directories
                .get(directory.entry_index)
                .and_then(|entry| entry.handle.as_ref())
                .ok_or(TransactionFsError::Conflict)?;
            if validate_directory_info(
                &file_information(current_directory.as_raw_handle() as HANDLE)?,
                directory_volume,
            )? != directory_identity
            {
                return Err(TransactionFsError::Conflict);
            }
            self.validate_all_pins()?;
            let token = self.register_child_entry(path, handle, max_bytes, directory_volume)?;
            Ok((token, identity))
        }

        pub(crate) fn reserve_absent(&mut self, path: &Path) -> Result<(), TransactionFsError> {
            let result = self.reserve_absent_inner(path);
            if result.is_err() {
                self.poisoned = true;
            }
            result
        }

        fn reserve_absent_inner(&mut self, path: &Path) -> Result<(), TransactionFsError> {
            self.ensure_mutable()?;
            let target = prepare_target(path)?;
            self.check_target(&target)?;
            match fs::symlink_metadata(path) {
                Ok(_) => return Err(TransactionFsError::Conflict),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(_) => return Err(TransactionFsError::Io),
            }

            let wide = wide_path(path)?;
            let raw = unsafe {
                CreateFileTransactedW(
                    wide.as_ptr(),
                    DELETE | GENERIC_READ | GENERIC_WRITE,
                    FILE_SHARE_READ | FILE_SHARE_WRITE,
                    null(),
                    CREATE_NEW,
                    FILE_ATTRIBUTE_NORMAL
                        | FILE_FLAG_OPEN_REPARSE_POINT
                        | FILE_FLAG_DELETE_ON_CLOSE,
                    null_mut(),
                    self.transaction_handle(),
                    null(),
                    null(),
                )
            };
            if is_invalid_handle(raw) {
                return Err(map_create_error(unsafe { GetLastError() }));
            }
            let handle = unsafe { OwnedHandle::from_raw_handle(raw as RawHandle) };
            let info = file_information(handle.as_raw_handle() as HANDLE)?;
            validate_file_info(&info, target.volume_serial)?;
            verify_target_after_open(path, &target)?;

            let canonical_path = target.canonical_path.clone();
            self.register_target_path(target)?;
            self.reservations.push(canonical_path);
            drop(handle);
            Ok(())
        }

        pub(crate) fn replace_contents(
            &mut self,
            token: FileToken,
            bytes: &[u8],
        ) -> Result<(), TransactionFsError> {
            let result = self.replace_contents_inner(token, bytes);
            if result.is_err() {
                self.poisoned = true;
            }
            result
        }

        pub(crate) fn canonical_target(
            &self,
            token: FileToken,
        ) -> Result<PathBuf, TransactionFsError> {
            if token.transaction_id != self.transaction_id || self.finished {
                return Err(TransactionFsError::Conflict);
            }
            self.entries
                .get(token.entry_index)
                .map(|entry| entry.canonical_path.clone())
                .ok_or(TransactionFsError::Conflict)
        }

        fn replace_contents_inner(
            &mut self,
            token: FileToken,
            bytes: &[u8],
        ) -> Result<(), TransactionFsError> {
            self.ensure_mutable()?;
            if token.transaction_id != self.transaction_id {
                return Err(TransactionFsError::Conflict);
            }
            let entry = self
                .entries
                .get_mut(token.entry_index)
                .ok_or(TransactionFsError::Conflict)?;
            if bytes.len() > entry.max_bytes || bytes.len() > MAX_TRANSACTION_FILE_BYTES {
                return Err(TransactionFsError::TooLarge);
            }
            let handle = entry
                .handle
                .as_ref()
                .ok_or(TransactionFsError::Conflict)?
                .as_raw_handle() as HANDLE;
            write_bounded(handle, bytes)
        }

        pub(crate) fn seal(&mut self) -> Result<(), TransactionFsError> {
            let result = self.seal_inner();
            if result.is_err() {
                self.poisoned = true;
            }
            result
        }

        fn seal_inner(&mut self) -> Result<(), TransactionFsError> {
            if self.poisoned
                || self.finished
                || self.entries.is_empty()
                    && self.directories.is_empty()
                    && self.reservations.is_empty()
            {
                return Err(TransactionFsError::Conflict);
            }
            self.check_deadline()?;
            self.validate_all_pins()?;
            for entry in &mut self.entries {
                entry.handle.take();
            }
            for directory in &mut self.directories {
                directory.handle.take();
            }
            self.validate_all_pins()?;
            self.sealed = true;
            Ok(())
        }

        pub(crate) fn commit(mut self) -> Result<(), TransactionFsError> {
            if self.poisoned || self.finished {
                return Err(TransactionFsError::Conflict);
            }
            if !self.sealed {
                self.seal()?;
            }
            self.check_deadline()?;
            self.validate_all_pins()?;
            let handle = self.transaction_handle();
            if unsafe { CommitTransaction(handle) } == 0 {
                return Err(TransactionFsError::CommitFailed);
            }
            self.finished = true;
            self.transaction.take();
            Ok(())
        }

        pub(crate) fn rollback(mut self) -> Result<(), TransactionFsError> {
            for entry in &mut self.entries {
                entry.handle.take();
            }
            for directory in &mut self.directories {
                directory.handle.take();
            }
            let handle = self.transaction_handle();
            if unsafe { RollbackTransaction(handle) } == 0 {
                return Err(TransactionFsError::RollbackFailed);
            }
            self.finished = true;
            self.transaction.take();
            Ok(())
        }

        fn register_entry(
            &mut self,
            target: PreparedTarget,
            handle: OwnedHandle,
            max_bytes: usize,
        ) -> Result<FileToken, TransactionFsError> {
            let entry_index = self.entries.len();
            let canonical_path = target.canonical_path.clone();
            self.register_target_path(target)?;
            self.entries.push(Entry {
                handle: Some(handle),
                max_bytes,
                canonical_path,
            });
            Ok(FileToken {
                transaction_id: self.transaction_id,
                entry_index,
            })
        }

        fn register_directory(
            &mut self,
            target: PreparedTarget,
            handle: OwnedHandle,
            identity: FileIdentity,
        ) -> Result<ClaudeDirectoryToken, TransactionFsError> {
            let entry_index = self.directories.len();
            let canonical_path = target.canonical_path.clone();
            let volume_serial = target.volume_serial;
            self.register_target_path(target)?;
            self.directories.push(DirectoryEntry {
                handle: Some(handle),
                canonical_path,
                identity,
                volume_serial,
            });
            Ok(ClaudeDirectoryToken {
                transaction_id: self.transaction_id,
                entry_index,
            })
        }

        fn check_child_path(
            &self,
            path: &Path,
            volume_serial: u32,
        ) -> Result<(), TransactionFsError> {
            self.check_deadline()?;
            if self.paths.len() >= MAX_TRANSACTION_FILES {
                return Err(TransactionFsError::TooManyFiles);
            }
            if self
                .paths
                .iter()
                .any(|existing| config_io::same_path(existing, path))
            {
                return Err(TransactionFsError::Conflict);
            }
            if self
                .volume_serial
                .is_some_and(|serial| serial != volume_serial)
            {
                return Err(TransactionFsError::Unsupported);
            }
            Ok(())
        }

        fn register_child_entry(
            &mut self,
            canonical_path: PathBuf,
            handle: OwnedHandle,
            max_bytes: usize,
            volume_serial: u32,
        ) -> Result<FileToken, TransactionFsError> {
            self.check_child_path(&canonical_path, volume_serial)?;
            let entry_index = self.entries.len();
            self.paths.push(canonical_path.clone());
            self.volume_serial = Some(volume_serial);
            self.entries.push(Entry {
                handle: Some(handle),
                max_bytes,
                canonical_path,
            });
            Ok(FileToken {
                transaction_id: self.transaction_id,
                entry_index,
            })
        }

        fn register_target_path(
            &mut self,
            target: PreparedTarget,
        ) -> Result<(), TransactionFsError> {
            if self.paths.len() >= MAX_TRANSACTION_FILES {
                return Err(TransactionFsError::TooManyFiles);
            }
            if self
                .paths
                .iter()
                .any(|path| config_io::same_path(path, &target.canonical_path))
            {
                return Err(TransactionFsError::Conflict);
            }
            if self
                .volume_serial
                .is_some_and(|serial| serial != target.volume_serial)
            {
                return Err(TransactionFsError::Unsupported);
            }
            self.paths.push(target.canonical_path);
            self.volume_serial = Some(target.volume_serial);
            self.pinned_directories.push(target.pins);
            Ok(())
        }

        fn check_target(&self, target: &PreparedTarget) -> Result<(), TransactionFsError> {
            self.check_deadline()?;
            if self.paths.len() >= MAX_TRANSACTION_FILES {
                return Err(TransactionFsError::TooManyFiles);
            }
            if self
                .paths
                .iter()
                .any(|path| config_io::same_path(path, &target.canonical_path))
            {
                return Err(TransactionFsError::Conflict);
            }
            if self
                .volume_serial
                .is_some_and(|serial| serial != target.volume_serial)
            {
                return Err(TransactionFsError::Unsupported);
            }
            Ok(())
        }

        fn validate_all_pins(&self) -> Result<(), TransactionFsError> {
            for pins in &self.pinned_directories {
                config_io::validate_pinned_directories(pins).map_err(map_config_io_error)?;
            }
            for directory in &self.directories {
                if let Some(handle) = directory.handle.as_ref() {
                    let information = file_information(handle.as_raw_handle() as HANDLE)?;
                    let identity = validate_directory_info(&information, directory.volume_serial)?;
                    if identity != directory.identity {
                        return Err(TransactionFsError::Conflict);
                    }
                }
            }
            Ok(())
        }

        fn ensure_mutable(&self) -> Result<(), TransactionFsError> {
            if self.poisoned || self.finished || self.sealed {
                return Err(TransactionFsError::Conflict);
            }
            self.check_deadline()
        }

        fn check_deadline(&self) -> Result<(), TransactionFsError> {
            if self.started.elapsed() >= TRANSACTION_WALL_LIMIT {
                Err(TransactionFsError::Expired)
            } else {
                Ok(())
            }
        }

        fn transaction_handle(&self) -> HANDLE {
            self.transaction
                .as_ref()
                .expect("active transaction handle")
                .as_raw_handle() as HANDLE
        }
    }

    impl Drop for FileTransaction {
        fn drop(&mut self) {
            if self.finished {
                return;
            }
            for entry in &mut self.entries {
                entry.handle.take();
            }
            for directory in &mut self.directories {
                directory.handle.take();
            }
            if let Some(transaction) = self.transaction.take() {
                let _ = unsafe { RollbackTransaction(transaction.as_raw_handle() as HANDLE) };
                drop(transaction);
            }
            self.finished = true;
        }
    }

    fn prepare_target(path: &Path) -> Result<PreparedTarget, TransactionFsError> {
        config_io::validate_lexical_path(path).map_err(map_config_io_error)?;
        config_io::ensure_fixed_volume(path).map_err(map_config_io_error)?;
        let parent = path.parent().ok_or(TransactionFsError::UnsafePath)?;
        let filename = path.file_name().ok_or(TransactionFsError::UnsafePath)?;
        if parent
            .components()
            .filter(|component| matches!(component, std::path::Component::Normal(_)))
            .take(MAX_PARENT_COMPONENTS + 1)
            .count()
            > MAX_PARENT_COMPONENTS
        {
            return Err(TransactionFsError::UnsafePath);
        }
        let pins = config_io::pin_parent_directories(path).map_err(map_config_io_error)?;
        config_io::validate_pinned_directories(&pins).map_err(map_config_io_error)?;
        let canonical_parent =
            fs::canonicalize(parent).map_err(|_| TransactionFsError::UnsafePath)?;
        if !config_io::same_path(parent, &canonical_parent) {
            return Err(TransactionFsError::UnsafePath);
        }
        config_io::validate_pinned_directories(&pins).map_err(map_config_io_error)?;
        let volume_serial = check_ntfs_transaction_capability(&pins)?;
        Ok(PreparedTarget {
            canonical_path: canonical_parent.join(filename),
            pins,
            volume_serial,
        })
    }

    fn check_ntfs_transaction_capability(pins: &DirectoryPins) -> Result<u32, TransactionFsError> {
        let Some(root) = pins.first() else {
            return Err(TransactionFsError::UnsafePath);
        };
        if pins.iter().any(|pin| {
            pin.attributes() & (FILE_ATTRIBUTE_REPARSE_POINT | FILE_ATTRIBUTE_ENCRYPTED) != 0
        }) {
            return Err(TransactionFsError::UnsafePath);
        }
        let mut filesystem = [0u16; 32];
        let mut serial = 0u32;
        let mut flags = 0u32;
        let ok = unsafe {
            GetVolumeInformationByHandleW(
                root.raw_handle() as HANDLE,
                null_mut(),
                0,
                &mut serial,
                null_mut(),
                &mut flags,
                filesystem.as_mut_ptr(),
                filesystem.len() as u32,
            )
        };
        if ok == 0 {
            return Err(TransactionFsError::Unsupported);
        }
        let end = filesystem
            .iter()
            .position(|unit| *unit == 0)
            .unwrap_or(filesystem.len());
        if !String::from_utf16_lossy(&filesystem[..end]).eq_ignore_ascii_case("NTFS")
            || flags & FILE_SUPPORTS_TRANSACTIONS == 0
        {
            return Err(TransactionFsError::Unsupported);
        }
        Ok(serial)
    }

    fn verify_target_after_open(
        path: &Path,
        target: &PreparedTarget,
    ) -> Result<(), TransactionFsError> {
        let parent = path.parent().ok_or(TransactionFsError::UnsafePath)?;
        let filename = path.file_name().ok_or(TransactionFsError::UnsafePath)?;
        let canonical = fs::canonicalize(parent)
            .map_err(|_| TransactionFsError::UnsafePath)?
            .join(filename);
        if !config_io::same_path(&canonical, &target.canonical_path) {
            return Err(TransactionFsError::UnsafePath);
        }
        config_io::validate_pinned_directories(&target.pins).map_err(map_config_io_error)
    }

    fn open_transacted_directory(
        path: &Path,
        transaction: HANDLE,
    ) -> Result<OwnedHandle, TransactionFsError> {
        let wide = wide_path(path)?;
        let raw = unsafe {
            CreateFileTransactedW(
                wide.as_ptr(),
                FILE_LIST_DIRECTORY | FILE_ADD_FILE | FILE_READ_ATTRIBUTES | SYNCHRONIZE,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                null(),
                OPEN_EXISTING,
                FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT,
                null_mut(),
                transaction,
                null(),
                null(),
            )
        };
        if is_invalid_handle(raw) {
            return Err(map_open_error(unsafe { GetLastError() }));
        }
        Ok(unsafe { OwnedHandle::from_raw_handle(raw as RawHandle) })
    }

    fn validate_directory_info(
        info: &BY_HANDLE_FILE_INFORMATION,
        expected_volume: u32,
    ) -> Result<FileIdentity, TransactionFsError> {
        let attributes = info.dwFileAttributes;
        if attributes & FILE_ATTRIBUTE_REPARSE_POINT != 0
            || attributes & FILE_ATTRIBUTE_DIRECTORY == 0
            || info.nNumberOfLinks != 1
        {
            return Err(TransactionFsError::UnsafePath);
        }
        if attributes & FILE_ATTRIBUTE_ENCRYPTED != 0 {
            return Err(TransactionFsError::Unsupported);
        }
        file_identity_from_info(info, expected_volume)
    }

    fn file_identity_from_info(
        info: &BY_HANDLE_FILE_INFORMATION,
        expected_volume: u32,
    ) -> Result<FileIdentity, TransactionFsError> {
        if info.dwVolumeSerialNumber != expected_volume {
            return Err(TransactionFsError::Unsupported);
        }
        Ok(FileIdentity {
            volume_serial: expected_volume,
            file_index: ((info.nFileIndexHigh as u64) << 32) | info.nFileIndexLow as u64,
        })
    }

    fn validate_file_info(
        info: &BY_HANDLE_FILE_INFORMATION,
        expected_volume: u32,
    ) -> Result<FileIdentity, TransactionFsError> {
        let attributes = info.dwFileAttributes;
        if attributes
            & (FILE_ATTRIBUTE_DIRECTORY
                | FILE_ATTRIBUTE_REPARSE_POINT
                | FILE_ATTRIBUTE_READONLY
                | FILE_ATTRIBUTE_ENCRYPTED)
            != 0
            || info.nNumberOfLinks != 1
            || info.dwVolumeSerialNumber != expected_volume
        {
            return Err(TransactionFsError::UnsafeFile);
        }
        Ok(FileIdentity {
            volume_serial: info.dwVolumeSerialNumber,
            file_index: ((info.nFileIndexHigh as u64) << 32) | info.nFileIndexLow as u64,
        })
    }

    fn file_information(handle: HANDLE) -> Result<BY_HANDLE_FILE_INFORMATION, TransactionFsError> {
        let mut info = BY_HANDLE_FILE_INFORMATION::default();
        if unsafe { GetFileInformationByHandle(handle, &mut info) } == 0 {
            return Err(TransactionFsError::Io);
        }
        Ok(info)
    }

    fn read_bounded(handle: HANDLE, max_bytes: usize) -> Result<Vec<u8>, TransactionFsError> {
        let mut size = 0i64;
        if unsafe { GetFileSizeEx(handle, &mut size) } == 0 || size < 0 {
            return Err(TransactionFsError::Io);
        }
        if size as usize > max_bytes || size as usize > MAX_TRANSACTION_FILE_BYTES {
            return Err(TransactionFsError::TooLarge);
        }
        if unsafe { SetFilePointerEx(handle, 0, null_mut(), FILE_BEGIN) } == 0 {
            return Err(TransactionFsError::Io);
        }
        let mut result = Vec::with_capacity(size as usize);
        let mut remaining = max_bytes.saturating_add(1);
        while remaining > 0 {
            let chunk_len = remaining.min(16 * 1024);
            let mut chunk = vec![0u8; chunk_len];
            let mut read = 0u32;
            let ok = unsafe {
                ReadFile(
                    handle,
                    chunk.as_mut_ptr(),
                    chunk_len as u32,
                    &mut read,
                    null_mut(),
                )
            };
            if ok == 0 {
                return Err(TransactionFsError::Io);
            }
            if read == 0 {
                break;
            }
            result.extend_from_slice(&chunk[..read as usize]);
            if result.len() > max_bytes || result.len() > MAX_TRANSACTION_FILE_BYTES {
                return Err(TransactionFsError::TooLarge);
            }
            remaining = remaining.saturating_sub(read as usize);
        }
        Ok(result)
    }

    fn write_bounded(handle: HANDLE, bytes: &[u8]) -> Result<(), TransactionFsError> {
        if bytes.len() > MAX_TRANSACTION_FILE_BYTES {
            return Err(TransactionFsError::TooLarge);
        }
        if unsafe { SetFilePointerEx(handle, 0, null_mut(), FILE_BEGIN) } == 0 {
            return Err(TransactionFsError::Io);
        }
        let mut cursor = 0usize;
        while cursor < bytes.len() {
            let end = (cursor + 16 * 1024).min(bytes.len());
            let mut written = 0u32;
            let ok = unsafe {
                WriteFile(
                    handle,
                    bytes[cursor..end].as_ptr(),
                    (end - cursor) as u32,
                    &mut written,
                    null_mut(),
                )
            };
            if ok == 0 || written == 0 {
                return Err(TransactionFsError::Io);
            }
            cursor += written as usize;
        }
        if unsafe { SetEndOfFile(handle) } == 0 || unsafe { FlushFileBuffers(handle) } == 0 {
            return Err(TransactionFsError::Io);
        }
        Ok(())
    }

    fn wide_path(path: &Path) -> Result<Vec<u16>, TransactionFsError> {
        let encoded: Vec<u16> = path.as_os_str().encode_wide().collect();
        if encoded.is_empty() || encoded.len() >= MAX_PATH_UNITS || encoded.contains(&0) {
            return Err(TransactionFsError::UnsafePath);
        }
        Ok(encoded.into_iter().chain(Some(0)).collect())
    }

    fn validate_max_bytes(max_bytes: usize) -> Result<(), TransactionFsError> {
        if max_bytes == 0 || max_bytes > MAX_TRANSACTION_FILE_BYTES {
            Err(TransactionFsError::TooLarge)
        } else {
            Ok(())
        }
    }

    fn metadata_is_reparse(metadata: &fs::Metadata) -> bool {
        use std::os::windows::fs::MetadataExt;
        metadata.file_type().is_symlink()
            || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    }

    fn metadata_is_unsafe_file(metadata: &fs::Metadata) -> bool {
        use std::os::windows::fs::MetadataExt;
        metadata_is_reparse(metadata)
            || metadata.file_attributes() & (FILE_ATTRIBUTE_READONLY | FILE_ATTRIBUTE_ENCRYPTED)
                != 0
    }

    fn is_invalid_handle(handle: HANDLE) -> bool {
        handle.is_null() || handle == INVALID_HANDLE_VALUE
    }

    fn map_open_error(code: u32) -> TransactionFsError {
        match code {
            ERROR_FILE_NOT_FOUND | ERROR_PATH_NOT_FOUND => TransactionFsError::Conflict,
            ERROR_SHARING_VIOLATION | ERROR_LOCK_VIOLATION | ERROR_TRANSACTIONAL_CONFLICT => {
                TransactionFsError::Conflict
            }
            _ => TransactionFsError::Io,
        }
    }

    fn map_create_error(code: u32) -> TransactionFsError {
        match code {
            ERROR_FILE_EXISTS | ERROR_ALREADY_EXISTS => TransactionFsError::Conflict,
            ERROR_SHARING_VIOLATION | ERROR_LOCK_VIOLATION | ERROR_TRANSACTIONAL_CONFLICT => {
                TransactionFsError::Conflict
            }
            _ => TransactionFsError::Io,
        }
    }

    fn map_config_io_error(error: ConfigIoError) -> TransactionFsError {
        match error {
            ConfigIoError::InvalidRoot | ConfigIoError::UnsafePath => {
                TransactionFsError::UnsafePath
            }
            ConfigIoError::TooLarge => TransactionFsError::TooLarge,
            ConfigIoError::Ambiguous => TransactionFsError::Conflict,
            ConfigIoError::Unavailable => TransactionFsError::Io,
        }
    }
}

#[cfg(windows)]
pub(crate) use windows_impl::FileTransaction;

#[cfg(target_os = "macos")]
#[path = "transaction_macos.rs"]
mod macos_impl;
#[cfg(target_os = "macos")]
pub(crate) use macos_impl::FileTransaction;

#[cfg(not(any(windows, target_os = "macos")))]
pub(crate) struct FileTransaction;

#[cfg(not(any(windows, target_os = "macos")))]
impl FileTransaction {
    pub(crate) fn begin() -> Result<Self, TransactionFsError> {
        Err(TransactionFsError::Unsupported)
    }

    pub(crate) fn open_existing(
        &mut self,
        _path: &std::path::Path,
        _max_bytes: usize,
    ) -> Result<(FileToken, FileSnapshot), TransactionFsError> {
        Err(TransactionFsError::Unsupported)
    }

    pub(crate) fn create_missing(
        &mut self,
        _path: &std::path::Path,
        _max_bytes: usize,
    ) -> Result<(FileToken, FileIdentity), TransactionFsError> {
        Err(TransactionFsError::Unsupported)
    }

    pub(crate) fn create_claude_directory(
        &mut self,
        _project_root: &std::path::Path,
    ) -> Result<(ClaudeDirectoryToken, FileIdentity), TransactionFsError> {
        Err(TransactionFsError::Unsupported)
    }

    pub(crate) fn create_claude_settings_file(
        &mut self,
        _directory: ClaudeDirectoryToken,
        _max_bytes: usize,
    ) -> Result<(FileToken, FileIdentity), TransactionFsError> {
        Err(TransactionFsError::Unsupported)
    }

    pub(crate) fn canonical_target(
        &self,
        _token: FileToken,
    ) -> Result<std::path::PathBuf, TransactionFsError> {
        Err(TransactionFsError::Unsupported)
    }

    pub(crate) fn reserve_absent(
        &mut self,
        _path: &std::path::Path,
    ) -> Result<(), TransactionFsError> {
        Err(TransactionFsError::Unsupported)
    }

    pub(crate) fn replace_contents(
        &mut self,
        _token: FileToken,
        _bytes: &[u8],
    ) -> Result<(), TransactionFsError> {
        Err(TransactionFsError::Unsupported)
    }

    pub(crate) fn seal(&mut self) -> Result<(), TransactionFsError> {
        Err(TransactionFsError::Unsupported)
    }

    pub(crate) fn commit(self) -> Result<(), TransactionFsError> {
        Err(TransactionFsError::Unsupported)
    }

    pub(crate) fn rollback(self) -> Result<(), TransactionFsError> {
        Err(TransactionFsError::Unsupported)
    }
}
