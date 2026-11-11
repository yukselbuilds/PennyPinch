//! Recovery prototype exercised only against private native fixtures.

use super::{content_hash, hash_config_path_binding, valid_hash, valid_transition};
use super::{OwnershipError, OwnershipRecord, OwnershipState, MAX_RECORD_BYTES};
use crate::integrations::transaction::undo_owned_endpoint;
use crate::integrations::transaction_fs::MAX_TRANSACTION_FILE_BYTES;
use serde::{Deserialize, Serialize};
use std::ffi::CStr;
use std::fs::{self, File, Metadata, OpenOptions};
use std::io::{self, Read, Write};
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;
use uuid::Uuid;

const CONFIG: &CStr = c"config.toml";
const STAGED_CONFIG: &CStr = c"staged-config.toml";
const OWNER: &CStr = c"ownership.json";
const STAGED_OWNER: &CStr = c"staged-ownership.json";
const INTENT: &CStr = c"pending.json";
const MAX_INTENT_BYTES: usize = 4096;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct FileObject {
    device: u64,
    inode: u64,
}

impl From<&Metadata> for FileObject {
    fn from(info: &Metadata) -> Self {
        Self {
            device: info.dev(),
            inode: info.ino(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Version {
    object: FileObject,
    created_seconds: u64,
    created_nanos: u32,
    sha256: String,
}

impl Version {
    fn valid(&self) -> bool {
        self.object.inode != 0 && self.created_nanos < 1_000_000_000 && valid_hash(&self.sha256)
    }
}

struct Snapshot {
    version: Version,
    bytes: Vec<u8>,
}

struct Directory {
    path: PathBuf,
    file: File,
    object: FileObject,
}

impl Directory {
    fn open(path: &Path) -> Result<Self, OwnershipError> {
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(path)
            .map_err(|_| OwnershipError::UnsafePath)?;
        let mut filesystem = std::mem::MaybeUninit::<libc::statfs>::uninit();
        // SAFETY: the live directory fd and writable statfs pointer are valid.
        if unsafe { libc::fstatfs(file.as_raw_fd(), filesystem.as_mut_ptr()) } != 0 {
            return Err(OwnershipError::Unsupported);
        }
        let filesystem = unsafe { filesystem.assume_init() };
        if filesystem.f_flags & (libc::MNT_LOCAL as u32) == 0
            || !filesystem
                .f_fstypename
                .iter()
                .map(|&byte| byte as u8)
                .take(5)
                .eq(b"apfs\0".iter().copied())
        {
            return Err(OwnershipError::Unsupported);
        }
        let info = file.metadata().map_err(|_| OwnershipError::UnsafePath)?;
        let directory = Self {
            path: path.to_owned(),
            file,
            object: FileObject::from(&info),
        };
        directory.verify()?;
        Ok(directory)
    }

    fn verify(&self) -> Result<(), OwnershipError> {
        let info = fs::symlink_metadata(&self.path).map_err(|_| OwnershipError::UnsafePath)?;
        if !info.is_dir()
            || FileObject::from(&info) != self.object
            || info.uid() != unsafe { libc::geteuid() }
            || info.mode() & 0o7777 != 0o700
            || self.path.to_str().is_none()
            || fs::canonicalize(&self.path).ok().as_deref() != Some(self.path.as_path())
        {
            return Err(OwnershipError::UnsafePath);
        }
        Ok(())
    }

    fn open_file(&self, name: &CStr, flags: i32) -> io::Result<File> {
        // SAFETY: callers pass a single constant or UUID-derived component.
        let fd = unsafe {
            libc::openat(
                self.file.as_raw_fd(),
                name.as_ptr(),
                flags | libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK,
                0o600,
            )
        };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(unsafe { File::from_raw_fd(fd) })
    }

    fn snapshot(&self, name: &CStr, limit: usize) -> Result<Option<Snapshot>, OwnershipError> {
        self.verify()?;
        let mut file = match self.open_file(name, libc::O_RDONLY) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err(OwnershipError::UnsafePath),
        };
        let before = file.metadata().map_err(|_| OwnershipError::Unavailable)?;
        if !before.is_file()
            || before.nlink() != 1
            || before.mode() & 0o7777 != 0o600
            || before.uid() != unsafe { libc::geteuid() }
        {
            return Err(OwnershipError::UnsafePath);
        }
        if before.len() > limit as u64 {
            return Err(OwnershipError::TooLarge);
        }
        let mut bytes = Vec::new();
        Read::by_ref(&mut file)
            .take(limit as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| OwnershipError::Unavailable)?;
        let after = file.metadata().map_err(|_| OwnershipError::Unavailable)?;
        let named = self
            .open_file(name, libc::O_RDONLY)
            .and_then(|file| file.metadata())
            .map_err(|_| OwnershipError::Conflict)?;
        self.verify()?;
        if bytes.len() as u64 != before.len()
            || stamp(&before) != stamp(&after)
            || stamp(&before) != stamp(&named)
        {
            return Err(OwnershipError::Conflict);
        }
        let created = before
            .created()
            .ok()
            .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
            .ok_or(OwnershipError::Unsupported)?;
        Ok(Some(Snapshot {
            version: Version {
                object: FileObject::from(&before),
                created_seconds: created.as_secs(),
                created_nanos: created.subsec_nanos(),
                sha256: content_hash(&bytes),
            },
            bytes,
        }))
    }

    fn create(&self, name: &CStr, bytes: &[u8]) -> Result<(), OwnershipError> {
        self.verify()?;
        let mut file = self
            .open_file(name, libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL)
            .map_err(|error| {
                if error.kind() == io::ErrorKind::AlreadyExists {
                    OwnershipError::Conflict
                } else {
                    OwnershipError::Unavailable
                }
            })?;
        file.write_all(bytes)
            .and_then(|_| file.sync_all())
            .and_then(|_| self.file.sync_all())
            .map_err(|_| OwnershipError::Unavailable)
    }

    fn rename(&self, from: &CStr, to: &CStr, flags: u32) -> Result<(), OwnershipError> {
        self.verify()?;
        let fd = self.file.as_raw_fd();
        // SAFETY: the directory fd and both single-component names remain valid.
        if unsafe { libc::renameatx_np(fd, from.as_ptr(), fd, to.as_ptr(), flags) } != 0 {
            return Err(
                if io::Error::last_os_error().kind() == io::ErrorKind::AlreadyExists {
                    OwnershipError::Conflict
                } else {
                    OwnershipError::Unavailable
                },
            );
        }
        self.file
            .sync_all()
            .map_err(|_| OwnershipError::Unavailable)
    }
}

fn stamp(info: &Metadata) -> (FileObject, u32, u32, u64, u64, i64, i64, i64, i64) {
    (
        FileObject::from(info),
        info.uid(),
        info.mode(),
        info.nlink(),
        info.len(),
        info.mtime(),
        info.mtime_nsec(),
        info.ctime(),
        info.ctime_nsec(),
    )
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Intent {
    schema_version: u8,
    config_directory: FileObject,
    owner_directory: FileObject,
    path_binding: String,
    before_config: Version,
    after_config: Version,
    before_owner: Option<Version>,
    after_owner: Version,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RecoveryState {
    Prepared,
    OwnershipPending,
    Recovered,
    Conflict,
}

struct Recovery {
    config: Directory,
    state: Directory,
}

impl Recovery {
    fn open(root: &Path) -> Result<Self, OwnershipError> {
        let config = Directory::open(&root.join("client"))?;
        let state = Directory::open(&root.join("state"))?;
        if config.object == state.object {
            return Err(OwnershipError::UnsafePath);
        }
        Ok(Self { config, state })
    }

    fn lock(&self) -> Result<File, OwnershipError> {
        self.state.verify()?;
        let lock = self
            .state
            .open_file(c"recovery.lock", libc::O_RDWR | libc::O_CREAT)
            .map_err(|_| OwnershipError::Unavailable)?;
        let info = lock.metadata().map_err(|_| OwnershipError::Unavailable)?;
        if !info.is_file()
            || info.nlink() != 1
            || info.mode() & 0o7777 != 0o600
            || info.uid() != unsafe { libc::geteuid() }
        {
            return Err(OwnershipError::UnsafePath);
        }
        // SAFETY: the lock file keeps the descriptor open for the operation.
        if unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
            return Err(OwnershipError::Conflict);
        }
        let named = self
            .state
            .open_file(c"recovery.lock", libc::O_RDONLY)
            .and_then(|file| file.metadata())
            .map_err(|_| OwnershipError::Conflict)?;
        if stamp(&info) != stamp(&named) {
            return Err(OwnershipError::Conflict);
        }
        Ok(lock)
    }

    fn binding(&self) -> Result<String, OwnershipError> {
        hash_config_path_binding(&self.config.path.join("config.toml"))
    }

    fn prepare(&self, next: &OwnershipRecord) -> Result<(), OwnershipError> {
        let _lock = self.lock()?;
        if self.state.snapshot(INTENT, MAX_INTENT_BYTES)?.is_some() {
            return Err(OwnershipError::Conflict);
        }
        let before = self
            .config
            .snapshot(CONFIG, MAX_TRANSACTION_FILE_BYTES)?
            .ok_or(OwnershipError::Conflict)?;
        let after = self
            .config
            .snapshot(STAGED_CONFIG, MAX_TRANSACTION_FILE_BYTES)?
            .ok_or(OwnershipError::Conflict)?;
        let old = self.state.snapshot(OWNER, MAX_RECORD_BYTES)?;
        self.validate_transition(old.as_ref(), next, &before, &after)?;
        self.state.create(STAGED_OWNER, &next.encode()?)?;
        let next = self
            .state
            .snapshot(STAGED_OWNER, MAX_RECORD_BYTES)?
            .ok_or(OwnershipError::Conflict)?;
        let intent = Intent {
            schema_version: 1,
            config_directory: self.config.object,
            owner_directory: self.state.object,
            path_binding: self.binding()?,
            before_config: before.version,
            after_config: after.version,
            before_owner: old.map(|file| file.version),
            after_owner: next.version,
        };
        let bytes = serde_json::to_vec(&intent).map_err(|_| OwnershipError::Invalid)?;
        let name =
            std::ffi::CString::new(format!(".pending-{}.tmp", Uuid::new_v4().simple())).unwrap();
        self.state.create(&name, &bytes)?;
        self.state.rename(&name, INTENT, libc::RENAME_EXCL)?;
        Ok(())
    }

    fn validate_transition(
        &self,
        old: Option<&Snapshot>,
        next: &OwnershipRecord,
        before: &Snapshot,
        after: &Snapshot,
    ) -> Result<(), OwnershipError> {
        next.validate()?;
        if next.config_path_binding_sha256() != self.binding()?
            || before.version.object == after.version.object
            || before.version.sha256 == after.version.sha256
        {
            return Err(OwnershipError::Conflict);
        }
        let previous = old
            .map(|file| OwnershipRecord::decode(&file.bytes))
            .transpose()?;
        if !previous.as_ref().map_or(
            next.state() == OwnershipState::Active && next.revision() == 1,
            |previous| valid_transition(previous, next),
        ) {
            return Err(OwnershipError::Conflict);
        }
        let matches = match next.state() {
            OwnershipState::Active => {
                next.stored.before_config_sha256 == before.version.sha256
                    && next.stored.after_config_sha256 == after.version.sha256
            }
            OwnershipState::Released => undo_owned_endpoint(&next.patch_receipt()?, &before.bytes)
                .is_ok_and(|bytes| bytes == after.bytes),
        };
        if !matches {
            return Err(OwnershipError::Conflict);
        }
        Ok(())
    }

    fn intent(&self) -> Result<(Intent, Version), OwnershipError> {
        let file = self
            .state
            .snapshot(INTENT, MAX_INTENT_BYTES)?
            .ok_or(OwnershipError::Invalid)?;
        let intent: Intent =
            serde_json::from_slice(&file.bytes).map_err(|_| OwnershipError::Invalid)?;
        if intent.schema_version != 1
            || intent.config_directory != self.config.object
            || intent.owner_directory != self.state.object
            || intent.path_binding != self.binding()?
            || [
                &intent.before_config,
                &intent.after_config,
                &intent.after_owner,
            ]
            .iter()
            .any(|version| !version.valid())
            || intent
                .before_owner
                .as_ref()
                .is_some_and(|version| !version.valid())
            || intent.before_config.object == intent.after_config.object
            || intent
                .before_owner
                .as_ref()
                .is_some_and(|version| version.object == intent.after_owner.object)
        {
            return Err(OwnershipError::Invalid);
        }
        Ok((intent, file.version))
    }

    fn inspect(&self, intent: &Intent) -> Result<RecoveryState, OwnershipError> {
        let current = self.config.snapshot(CONFIG, MAX_TRANSACTION_FILE_BYTES)?;
        let staged = self
            .config
            .snapshot(STAGED_CONFIG, MAX_TRANSACTION_FILE_BYTES)?;
        let owner = self.state.snapshot(OWNER, MAX_RECORD_BYTES)?;
        let next_owner = self.state.snapshot(STAGED_OWNER, MAX_RECORD_BYTES)?;
        let old_owner = owner.as_ref().map(|file| &file.version) == intent.before_owner.as_ref();
        let new_owner = owner
            .as_ref()
            .is_some_and(|file| file.version == intent.after_owner);
        let staged_owner = next_owner
            .as_ref()
            .is_some_and(|file| file.version == intent.after_owner);
        let (Some(current), Some(staged)) = (current, staged) else {
            return Ok(RecoveryState::Conflict);
        };
        if current.version == intent.before_config
            && staged.version == intent.after_config
            && old_owner
            && staged_owner
        {
            let next = OwnershipRecord::decode(&next_owner.unwrap().bytes)?;
            self.validate_transition(owner.as_ref(), &next, &current, &staged)?;
            return Ok(RecoveryState::Prepared);
        }
        if current.version != intent.after_config || staged.version != intent.before_config {
            return Ok(RecoveryState::Conflict);
        }
        if new_owner && next_owner.is_none() {
            let record = OwnershipRecord::decode(&owner.unwrap().bytes)?;
            return Ok(
                if record.config_path_binding_sha256() == intent.path_binding {
                    RecoveryState::Recovered
                } else {
                    RecoveryState::Conflict
                },
            );
        }
        if old_owner && staged_owner {
            let next = OwnershipRecord::decode(&next_owner.unwrap().bytes)?;
            self.validate_transition(owner.as_ref(), &next, &staged, &current)?;
            return Ok(RecoveryState::OwnershipPending);
        }
        Ok(RecoveryState::Conflict)
    }

    fn recover(&self) -> Result<RecoveryState, OwnershipError> {
        let _lock = self.lock()?;
        let (intent, version) = self.intent()?;
        let observed = self.inspect(&intent)?;
        if observed != RecoveryState::OwnershipPending {
            return Ok(observed);
        }
        if self.intent()?.1 != version || self.inspect(&intent)? != RecoveryState::OwnershipPending
        {
            return Ok(RecoveryState::Conflict);
        }
        let flags = if intent.before_owner.is_none() {
            libc::RENAME_EXCL
        } else {
            0
        };
        self.state.rename(STAGED_OWNER, OWNER, flags)?;
        if self.intent()?.1 != version {
            return Ok(RecoveryState::Conflict);
        }
        self.inspect(&intent)
    }
}

#[path = "ownership_macos_recovery_tests.rs"]
mod tests;
