use super::{ClaudeDirectoryToken, FileIdentity, FileSnapshot, FileToken, TransactionFsError};
use super::{MAX_TRANSACTION_FILES, MAX_TRANSACTION_FILE_BYTES};
use crate::integrations::config_io::{self, ConfigIoError, PinnedDirectoryChain};
use crate::state::ownership::{
    config_path_binding, config_path_binding_for_claude_project_target, validate_macos_publication,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::ffi::{CStr, CString, OsString};
use std::fs::{self, File, Metadata, OpenOptions};
use std::io::{Read, Write};
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, UNIX_EPOCH};
use uuid::Uuid;

const MAX_JOURNALS: usize = 64;
const MAX_JOURNAL_BYTES: usize = 32 * 1024;
const MAX_XATTR_BYTES: usize = 64 * 1024;
const WALL_LIMIT: Duration = Duration::from_secs(10);
static NEXT_TRANSACTION: AtomicU64 = AtomicU64::new(1);

fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn map_path(error: ConfigIoError) -> TransactionFsError {
    match error {
        ConfigIoError::InvalidRoot | ConfigIoError::UnsafePath => TransactionFsError::UnsafePath,
        ConfigIoError::TooLarge => TransactionFsError::TooLarge,
        ConfigIoError::Ambiguous => TransactionFsError::Conflict,
        ConfigIoError::Unavailable => TransactionFsError::Io,
    }
}

fn component(path: &Path) -> Result<CString, TransactionFsError> {
    let value = path.file_name().ok_or(TransactionFsError::UnsafePath)?;
    CString::new(value.as_bytes()).map_err(|_| TransactionFsError::UnsafePath)
}

fn path_binding(path: &Path) -> Result<String, TransactionFsError> {
    if path.file_name() == Some(std::ffi::OsStr::new("settings.local.json"))
        && path.parent().and_then(Path::file_name) == Some(std::ffi::OsStr::new(".claude"))
    {
        config_path_binding_for_claude_project_target(
            path.parent()
                .and_then(Path::parent)
                .ok_or(TransactionFsError::UnsafePath)?,
            path,
        )
        .map_err(|_| TransactionFsError::UnsafePath)
    } else {
        config_path_binding(path).map_err(|_| TransactionFsError::UnsafePath)
    }
}

fn require_absent(path: &Path) -> Result<(), TransactionFsError> {
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        _ => Err(TransactionFsError::Conflict),
    }
}

fn metadata_hash(file: &File) -> Result<String, TransactionFsError> {
    let fd = file.as_raw_fd();
    // SAFETY: null requests only the required bounded buffer size.
    let length = unsafe { libc::flistxattr(fd, std::ptr::null_mut(), 0, 0) };
    if length < 0 || length as usize > MAX_XATTR_BYTES {
        return Err(TransactionFsError::UnsafeFile);
    }
    let mut names = vec![0u8; length as usize];
    if length > 0
        && unsafe { libc::flistxattr(fd, names.as_mut_ptr().cast(), names.len(), 0) } != length
    {
        return Err(TransactionFsError::Conflict);
    }
    let mut keys: Vec<_> = names
        .split(|byte| *byte == 0)
        .filter(|key| !key.is_empty())
        .collect();
    keys.sort_unstable();
    let mut digest = Sha256::new();
    let mut total = names.len();
    for key in keys {
        let name = CString::new(key).map_err(|_| TransactionFsError::UnsafeFile)?;
        let size = unsafe { libc::fgetxattr(fd, name.as_ptr(), std::ptr::null_mut(), 0, 0, 0) };
        if size < 0 || size as usize > MAX_XATTR_BYTES.saturating_sub(total) {
            return Err(TransactionFsError::UnsafeFile);
        }
        let mut value = vec![0; size as usize];
        if unsafe {
            libc::fgetxattr(
                fd,
                name.as_ptr(),
                value.as_mut_ptr().cast(),
                value.len(),
                0,
                0,
            )
        } != size
        {
            return Err(TransactionFsError::Conflict);
        }
        total += value.len();
        digest.update((key.len() as u64).to_le_bytes());
        digest.update(key);
        digest.update((value.len() as u64).to_le_bytes());
        digest.update(&value);
    }
    Ok(format!("{:x}", digest.finalize()))
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Object {
    device: u64,
    inode: u64,
}

impl From<&Metadata> for Object {
    fn from(value: &Metadata) -> Self {
        Self {
            device: value.dev(),
            inode: value.ino(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Version {
    object: Object,
    birth_seconds: u64,
    birth_nanos: u32,
    mode: u32,
    gid: u32,
    sha256: String,
    metadata_sha256: String,
}

struct Snapshot {
    file: File,
    version: Version,
    bytes: Vec<u8>,
}

fn stamp(info: &Metadata) -> (Object, u32, u32, u32, u64, u64, i64, i64, i64, i64) {
    (
        Object::from(info),
        info.uid(),
        info.gid(),
        info.mode(),
        info.nlink(),
        info.len(),
        info.mtime(),
        info.mtime_nsec(),
        info.ctime(),
        info.ctime_nsec(),
    )
}

struct Directory {
    path: PathBuf,
    file: File,
    object: Object,
    pins: PinnedDirectoryChain,
}

impl Directory {
    fn open(path: &Path, private: bool) -> Result<Self, TransactionFsError> {
        let pins = config_io::pin_existing_directory_chain(path).map_err(map_path)?;
        config_io::ensure_fixed_volume(&path.join(".headroom-volume")).map_err(map_path)?;
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW_ANY | libc::O_CLOEXEC)
            .open(path)
            .map_err(|_| TransactionFsError::UnsafePath)?;
        let info = file.metadata().map_err(|_| TransactionFsError::Io)?;
        if info.uid() != unsafe { libc::geteuid() }
            || info.mode() & 0o022 != 0
            || (private && info.mode() & 0o7777 != 0o700)
        {
            return Err(TransactionFsError::UnsafePath);
        }
        let directory = Self {
            path: path.to_owned(),
            file,
            object: Object::from(&info),
            pins,
        };
        directory.verify()?;
        Ok(directory)
    }

    fn verify(&self) -> Result<(), TransactionFsError> {
        self.pins.validate().map_err(map_path)?;
        let named = fs::symlink_metadata(&self.path).map_err(|_| TransactionFsError::UnsafePath)?;
        if !named.is_dir()
            || named.file_type().is_symlink()
            || Object::from(&named) != self.object
            || Object::from(&self.file.metadata().map_err(|_| TransactionFsError::Io)?)
                != self.object
        {
            return Err(TransactionFsError::UnsafePath);
        }
        Ok(())
    }

    fn open_file(&self, name: &CStr, flags: i32) -> Result<File, TransactionFsError> {
        self.verify()?;
        // SAFETY: every caller supplies one checked leaf name and a held directory fd.
        let fd = unsafe {
            libc::openat(
                self.file.as_raw_fd(),
                name.as_ptr(),
                flags | libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC,
                0o600,
            )
        };
        if fd < 0 {
            return Err(match std::io::Error::last_os_error().kind() {
                std::io::ErrorKind::AlreadyExists => TransactionFsError::Conflict,
                _ => TransactionFsError::Io,
            });
        }
        Ok(unsafe { File::from_raw_fd(fd) })
    }

    fn snapshot(&self, name: &CStr, limit: usize) -> Result<Option<Snapshot>, TransactionFsError> {
        self.verify()?;
        let path = self.path.join(OsString::from_vec(name.to_bytes().to_vec()));
        match fs::symlink_metadata(&path) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Ok(info) if info.is_file() && !info.file_type().is_symlink() => {}
            _ => return Err(TransactionFsError::UnsafeFile),
        }
        let mut file = self.open_file(name, libc::O_RDONLY)?;
        let before = file.metadata().map_err(|_| TransactionFsError::Io)?;
        if !before.is_file()
            || before.nlink() != 1
            || before.uid() != unsafe { libc::geteuid() }
            || before.mode() & 0o7022 != 0
        {
            return Err(TransactionFsError::UnsafeFile);
        }
        if before.len() > limit as u64 {
            return Err(TransactionFsError::TooLarge);
        }
        let metadata_sha256 = metadata_hash(&file)?;
        let mut bytes = Vec::new();
        Read::by_ref(&mut file)
            .take(limit as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| TransactionFsError::Io)?;
        let after = file.metadata().map_err(|_| TransactionFsError::Io)?;
        let named = self
            .open_file(name, libc::O_RDONLY)?
            .metadata()
            .map_err(|_| TransactionFsError::Io)?;
        self.verify()?;
        if bytes.len() as u64 != before.len()
            || stamp(&before) != stamp(&after)
            || stamp(&before) != stamp(&named)
            || metadata_hash(&file)? != metadata_sha256
        {
            return Err(TransactionFsError::Conflict);
        }
        let birth = before
            .created()
            .ok()
            .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
            .ok_or(TransactionFsError::Unsupported)?;
        Ok(Some(Snapshot {
            version: Version {
                object: Object::from(&before),
                birth_seconds: birth.as_secs(),
                birth_nanos: birth.subsec_nanos(),
                mode: before.mode(),
                gid: before.gid(),
                sha256: hash(&bytes),
                metadata_sha256,
            },
            file,
            bytes,
        }))
    }

    fn create(
        &self,
        name: &CStr,
        bytes: &[u8],
        original: Option<&Snapshot>,
    ) -> Result<Snapshot, TransactionFsError> {
        let mut file = self.open_file(name, libc::O_RDWR | libc::O_CREAT | libc::O_EXCL)?;
        let created = Object::from(&file.metadata().map_err(|_| TransactionFsError::Io)?);
        file.write_all(bytes).map_err(|_| TransactionFsError::Io)?;
        if let Some(original) = original {
            if original.version.mode & 0o200 == 0 {
                return Err(TransactionFsError::UnsafeFile);
            }
            // Copy metadata from the held source, including ACLs and extended attributes.
            if unsafe {
                libc::fcopyfile(
                    original.file.as_raw_fd(),
                    file.as_raw_fd(),
                    std::ptr::null_mut(),
                    libc::COPYFILE_METADATA,
                )
            } != 0
            {
                return Err(TransactionFsError::UnsafeFile);
            }
        }
        file.sync_all()
            .and_then(|_| self.file.sync_all())
            .map_err(|_| TransactionFsError::Io)?;
        let staged = self
            .snapshot(name, MAX_TRANSACTION_FILE_BYTES)?
            .ok_or(TransactionFsError::Conflict)?;
        if staged.version.object != created
            || staged.bytes != bytes
            || original.is_some_and(|original| {
                staged.version.mode != original.version.mode
                    || staged.version.gid != original.version.gid
                    || staged.version.metadata_sha256 != original.version.metadata_sha256
            })
        {
            return Err(TransactionFsError::Conflict);
        }
        Ok(staged)
    }

    fn create_claude_stage(&self, id: &str) -> Result<Directory, TransactionFsError> {
        self.verify()?;
        let leaf = name(id, "claude")?;
        if unsafe { libc::mkdirat(self.file.as_raw_fd(), leaf.as_ptr(), 0o700) } != 0 {
            return Err(TransactionFsError::Conflict);
        }
        self.file.sync_all().map_err(|_| TransactionFsError::Io)?;
        self.verify()?;
        Directory::open(
            &self.path.join(OsString::from_vec(leaf.to_bytes().to_vec())),
            true,
        )
    }

    fn publish(
        &self,
        stage: &CStr,
        target: &CStr,
        existed: bool,
    ) -> Result<(), TransactionFsError> {
        self.verify()?;
        let flags = if existed {
            libc::RENAME_SWAP
        } else {
            libc::RENAME_EXCL
        };
        if unsafe {
            libc::renameatx_np(
                self.file.as_raw_fd(),
                stage.as_ptr(),
                self.file.as_raw_fd(),
                target.as_ptr(),
                flags,
            )
        } != 0
        {
            return Err(TransactionFsError::Conflict);
        }
        self.file
            .sync_all()
            .map_err(|_| TransactionFsError::CommitFailed)?;
        self.verify().map_err(|_| TransactionFsError::CommitFailed)
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Journal {
    schema_version: u8,
    id: String,
    config_path: Vec<u8>,
    binding: String,
    config_directory: Object,
    owner_directory: Object,
    owner_filename: String,
    before_config: Option<Version>,
    after_config: Version,
    before_owner: Option<Version>,
    after_owner: Version,
    absent_guards: Vec<Vec<u8>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    claude_project: Option<Object>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum Disposition {
    Committed,
    Aborted,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Marker {
    schema_version: u8,
    journal_sha256: String,
    disposition: Disposition,
}

fn name(id: &str, suffix: &str) -> Result<CString, TransactionFsError> {
    if Uuid::parse_str(id)
        .ok()
        .map(|uuid| uuid.to_string())
        .as_deref()
        != Some(id)
    {
        return Err(TransactionFsError::Conflict);
    }
    CString::new(format!(".apfs-{id}.{suffix}")).map_err(|_| TransactionFsError::Conflict)
}

fn config_stage(id: &str) -> Result<CString, TransactionFsError> {
    name(id, "config")
}

fn version(file: &Option<Snapshot>) -> Option<&Version> {
    file.as_ref().map(|file| &file.version)
}

impl Journal {
    fn target(&self) -> Result<PathBuf, TransactionFsError> {
        let path = PathBuf::from(OsString::from_vec(self.config_path.clone()));
        config_io::validate_lexical_path(&path).map_err(map_path)?;
        if self.schema_version != 1
            || !matches!(
                self.owner_filename.as_str(),
                "codex.json" | "claude_code.json" | "opencode_v1.json"
            )
            || self.absent_guards.len() > 1
        {
            return Err(TransactionFsError::Conflict);
        }
        name(&self.id, "json")?;
        if self.claude_project.is_some()
            && (path.file_name() != Some(std::ffi::OsStr::new("settings.local.json"))
                || path.parent().and_then(Path::file_name) != Some(std::ffi::OsStr::new(".claude"))
                || self.owner_filename != "claude_code.json"
                || self.before_config.is_some()
                || !self.absent_guards.is_empty())
        {
            return Err(TransactionFsError::Conflict);
        }
        for raw in &self.absent_guards {
            let guard = PathBuf::from(OsString::from_vec(raw.clone()));
            config_io::validate_lexical_path(&guard).map_err(map_path)?;
            if guard.parent() != path.parent()
                || guard == path
                || !matches!(
                    guard.file_name().and_then(|value| value.to_str()),
                    Some("opencode.json" | "opencode.jsonc")
                )
            {
                return Err(TransactionFsError::UnsafePath);
            }
        }
        Ok(path)
    }

    fn check_guards(&self) -> Result<(), TransactionFsError> {
        for raw in &self.absent_guards {
            match fs::symlink_metadata(PathBuf::from(OsString::from_vec(raw.clone()))) {
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                _ => return Err(TransactionFsError::Conflict),
            }
        }
        Ok(())
    }

    fn marker(
        &self,
        state: &Directory,
        digest: &str,
    ) -> Result<Option<Disposition>, TransactionFsError> {
        let Some(snapshot) = state.snapshot(&name(&self.id, "done")?, MAX_JOURNAL_BYTES)? else {
            return Ok(None);
        };
        let marker: Marker =
            serde_json::from_slice(&snapshot.bytes).map_err(|_| TransactionFsError::Conflict)?;
        if marker.schema_version != 1 || marker.journal_sha256 != digest {
            return Err(TransactionFsError::Conflict);
        }
        Ok(Some(marker.disposition))
    }

    fn mark(
        &self,
        state: &Directory,
        digest: &str,
        disposition: Disposition,
    ) -> Result<(), TransactionFsError> {
        let bytes = serde_json::to_vec(&Marker {
            schema_version: 1,
            journal_sha256: digest.to_owned(),
            disposition,
        })
        .map_err(|_| TransactionFsError::Io)?;
        let temporary = name(&self.id, "mark")?;
        match state.snapshot(&temporary, MAX_JOURNAL_BYTES)? {
            Some(staged) if staged.bytes == bytes => {}
            Some(_) => return Err(TransactionFsError::Conflict),
            None => {
                state.create(&temporary, &bytes, None)?;
            }
        }
        state.publish(&temporary, &name(&self.id, "done")?, false)?;
        Ok(())
    }

    fn recover(
        &self,
        state: &Directory,
        digest: &str,
        expected_path: &Path,
    ) -> Result<(), TransactionFsError> {
        let path = self.target()?;
        if path != expected_path
            || path_binding(&path).ok().as_deref() != Some(self.binding.as_str())
        {
            return Err(TransactionFsError::Conflict);
        }
        if self.owner_directory != state.object {
            return Err(TransactionFsError::Conflict);
        }
        let disposition = self.marker(state, digest)?;
        if let Some(expected_project) = &self.claude_project {
            let project = Directory::open(
                path.parent()
                    .and_then(Path::parent)
                    .ok_or(TransactionFsError::UnsafePath)?,
                false,
            )?;
            if &project.object != expected_project {
                return Err(TransactionFsError::Conflict);
            }
            if disposition == Some(Disposition::Aborted) {
                return Ok(());
            }
            if fs::symlink_metadata(path.parent().unwrap())
                .is_err_and(|error| error.kind() == std::io::ErrorKind::NotFound)
            {
                if disposition.is_some() {
                    return Err(TransactionFsError::Conflict);
                }
                let staged = Directory::open(
                    &project.path.join(OsString::from_vec(
                        name(&self.id, "claude")?.to_bytes().to_vec(),
                    )),
                    true,
                )?;
                let config = staged.snapshot(c"settings.local.json", MAX_TRANSACTION_FILE_BYTES)?;
                let owner_name = CString::new(self.owner_filename.as_str())
                    .map_err(|_| TransactionFsError::UnsafePath)?;
                let owner = state.snapshot(&owner_name, MAX_TRANSACTION_FILE_BYTES)?;
                let staged_owner =
                    state.snapshot(&name(&self.id, "owner")?, MAX_TRANSACTION_FILE_BYTES)?;
                if staged.object != self.config_directory
                    || version(&config) != Some(&self.after_config)
                    || version(&owner) != self.before_owner.as_ref()
                    || version(&staged_owner) != Some(&self.after_owner)
                {
                    return Err(TransactionFsError::Conflict);
                }
                validate_macos_publication(
                    &path,
                    &self.owner_filename,
                    owner.as_ref().map(|file| file.bytes.as_slice()),
                    &staged_owner.as_ref().unwrap().bytes,
                    None,
                    &config.as_ref().unwrap().bytes,
                )
                .map_err(|_| TransactionFsError::Conflict)?;
                require_absent(path.parent().unwrap())?;
                return self.mark(state, digest, Disposition::Aborted);
            }
        }
        let config = Directory::open(
            path.parent().ok_or(TransactionFsError::UnsafePath)?,
            self.claude_project.is_some(),
        )?;
        if self.config_directory != config.object || self.owner_directory != state.object {
            return Err(TransactionFsError::Conflict);
        }
        let old = config.snapshot(&config_stage(&self.id)?, MAX_TRANSACTION_FILE_BYTES)?;
        if let Some(disposition) = disposition {
            return if disposition == Disposition::Aborted
                || version(&old) == self.before_config.as_ref()
            {
                Ok(())
            } else {
                Err(TransactionFsError::Conflict)
            };
        }
        self.check_guards()?;
        let current = config.snapshot(&component(&path)?, MAX_TRANSACTION_FILE_BYTES)?;
        let owner_name = CString::new(self.owner_filename.as_str())
            .map_err(|_| TransactionFsError::UnsafePath)?;
        let owner = state.snapshot(&owner_name, MAX_TRANSACTION_FILE_BYTES)?;
        let staged_owner = state.snapshot(&name(&self.id, "owner")?, MAX_TRANSACTION_FILE_BYTES)?;
        let prepared = version(&current) == self.before_config.as_ref()
            && version(&old) == Some(&self.after_config)
            && version(&owner) == self.before_owner.as_ref()
            && version(&staged_owner) == Some(&self.after_owner);
        if prepared {
            // A sealed lease is not authority to publish a config after a restart.
            return self.mark(state, digest, Disposition::Aborted);
        }
        if version(&current) != Some(&self.after_config)
            || version(&old) != self.before_config.as_ref()
        {
            return Err(TransactionFsError::Conflict);
        }
        if version(&owner) == Some(&self.after_owner)
            && version(&staged_owner) == self.before_owner.as_ref()
        {
            return self.mark(state, digest, Disposition::Committed);
        }
        if version(&owner) != self.before_owner.as_ref()
            || version(&staged_owner) != Some(&self.after_owner)
        {
            return Err(TransactionFsError::Conflict);
        }
        validate_macos_publication(
            &path,
            &self.owner_filename,
            owner.as_ref().map(|value| value.bytes.as_slice()),
            &staged_owner.as_ref().unwrap().bytes,
            old.as_ref().map(|value| value.bytes.as_slice()),
            &current.as_ref().unwrap().bytes,
        )
        .map_err(|_| TransactionFsError::Conflict)?;
        // Recheck all four names immediately before the remaining metadata publication.
        if version(&config.snapshot(&component(&path)?, MAX_TRANSACTION_FILE_BYTES)?)
            != Some(&self.after_config)
            || version(&config.snapshot(&config_stage(&self.id)?, MAX_TRANSACTION_FILE_BYTES)?)
                != self.before_config.as_ref()
            || version(&state.snapshot(&owner_name, MAX_TRANSACTION_FILE_BYTES)?)
                != self.before_owner.as_ref()
            || version(&state.snapshot(&name(&self.id, "owner")?, MAX_TRANSACTION_FILE_BYTES)?)
                != Some(&self.after_owner)
        {
            return Err(TransactionFsError::Conflict);
        }
        state.publish(
            &name(&self.id, "owner")?,
            &owner_name,
            self.before_owner.is_some(),
        )?;
        if version(&state.snapshot(&owner_name, MAX_TRANSACTION_FILE_BYTES)?)
            != Some(&self.after_owner)
            || version(&state.snapshot(&name(&self.id, "owner")?, MAX_TRANSACTION_FILE_BYTES)?)
                != self.before_owner.as_ref()
            || version(&config.snapshot(&config_stage(&self.id)?, MAX_TRANSACTION_FILE_BYTES)?)
                != self.before_config.as_ref()
            || version(&config.snapshot(&component(&path)?, MAX_TRANSACTION_FILE_BYTES)?)
                != Some(&self.after_config)
        {
            return Err(TransactionFsError::Conflict);
        }
        self.mark(state, digest, Disposition::Committed)
    }
}

fn journals(state: &Directory) -> Result<Vec<(Journal, String)>, TransactionFsError> {
    state.verify()?;
    let archived = retained_versions::archived_ids(state)?;
    let mut found = Vec::new();
    for (count, entry) in fs::read_dir(&state.path)
        .map_err(|_| TransactionFsError::Io)?
        .enumerate()
    {
        if count >= retained_versions::MAX_ROOT_ENTRIES {
            return Err(TransactionFsError::TooManyFiles);
        }
        let entry = entry.map_err(|_| TransactionFsError::Io)?;
        let filename = entry.file_name();
        let Some(filename) = filename.to_str() else {
            continue;
        };
        let Some(id) = filename
            .strip_prefix(".apfs-")
            .and_then(|name| name.strip_suffix(".json"))
        else {
            continue;
        };
        if archived.contains(id) {
            continue;
        }
        let file = state
            .snapshot(&name(id, "json")?, MAX_JOURNAL_BYTES)?
            .ok_or(TransactionFsError::Conflict)?;
        let journal: Journal =
            serde_json::from_slice(&file.bytes).map_err(|_| TransactionFsError::Conflict)?;
        if journal.id != id {
            return Err(TransactionFsError::Conflict);
        }
        journal.target()?;
        found.push((journal, hash(&file.bytes)));
        if found.len() > MAX_JOURNALS {
            return Err(TransactionFsError::TooManyFiles);
        }
    }
    state.verify()?;
    Ok(found)
}

fn check_history(
    state: &Directory,
    owner: &Path,
    path: &Path,
    except: Option<&str>,
) -> Result<(), TransactionFsError> {
    let binding = path_binding(path)?;
    for (journal, digest) in journals(state)? {
        if Some(journal.id.as_str()) == except
            || journal.binding != binding
            || owner.file_name().and_then(|name| name.to_str())
                != Some(journal.owner_filename.as_str())
        {
            continue;
        }
        if journal.marker(state, &digest)?.is_none() {
            return Err(TransactionFsError::Conflict);
        }
        journal.recover(state, &digest, path)?;
    }
    Ok(())
}

fn lock(state: &Directory, create: bool) -> Result<Option<File>, TransactionFsError> {
    if !create && state.snapshot(c".apfs.lock", 0)?.is_none() {
        return Ok(None);
    }
    let file = state.open_file(
        c".apfs.lock",
        libc::O_RDWR | if create { libc::O_CREAT } else { 0 },
    )?;
    let info = file.metadata().map_err(|_| TransactionFsError::Io)?;
    if !info.is_file()
        || info.uid() != unsafe { libc::geteuid() }
        || info.nlink() != 1
        || info.mode() & 0o7777 != 0o600
    {
        return Err(TransactionFsError::UnsafeFile);
    }
    if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
        return Err(TransactionFsError::Conflict);
    }
    let named = state
        .snapshot(c".apfs.lock", 0)?
        .ok_or(TransactionFsError::Conflict)?;
    if Object::from(&info) != named.version.object {
        return Err(TransactionFsError::Conflict);
    }
    Ok(Some(file))
}

struct Entry {
    path: PathBuf,
    directory: Directory,
    before: Option<Snapshot>,
    after: Option<Vec<u8>>,
    limit: usize,
    new_claude_directory: bool,
}

struct Prepared {
    state: Directory,
    journal: Journal,
    digest: String,
}

pub(crate) struct FileTransaction {
    id: u64,
    started: Instant,
    entries: Vec<Entry>,
    absent: Vec<PathBuf>,
    owner: Option<PathBuf>,
    owner_directory: Option<Directory>,
    lock: Option<File>,
    prepared: Option<Prepared>,
    claude_project: Option<Directory>,
}

impl FileTransaction {
    pub(crate) fn begin() -> Result<Self, TransactionFsError> {
        let id = NEXT_TRANSACTION.fetch_add(1, Ordering::Relaxed);
        if id == u64::MAX {
            return Err(TransactionFsError::Expired);
        }
        Ok(Self {
            id,
            started: Instant::now(),
            entries: Vec::new(),
            absent: Vec::new(),
            owner: None,
            owner_directory: None,
            lock: None,
            prepared: None,
            claude_project: None,
        })
    }

    fn check(&self) -> Result<(), TransactionFsError> {
        if self.started.elapsed() > WALL_LIMIT {
            return Err(TransactionFsError::Expired);
        }
        if let Some(directory) = &self.owner_directory {
            directory.verify()?;
        }
        Ok(())
    }

    fn token(&self, token: FileToken) -> Result<usize, TransactionFsError> {
        self.check()?;
        if token.transaction_id != self.id || token.entry_index >= self.entries.len() {
            return Err(TransactionFsError::Conflict);
        }
        Ok(token.entry_index)
    }

    fn add(
        &mut self,
        path: &Path,
        limit: usize,
        missing: bool,
    ) -> Result<(FileToken, Option<FileSnapshot>), TransactionFsError> {
        self.check()?;
        if self.prepared.is_some()
            || self.entries.iter().any(|entry| entry.path == path)
            || self.absent.iter().any(|guard| guard == path)
        {
            return Err(TransactionFsError::Conflict);
        }
        if self.entries.len() + self.absent.len() >= MAX_TRANSACTION_FILES {
            return Err(TransactionFsError::TooManyFiles);
        }
        if limit > MAX_TRANSACTION_FILE_BYTES {
            return Err(TransactionFsError::TooLarge);
        }
        config_io::validate_lexical_path(path).map_err(map_path)?;
        let directory = Directory::open(
            path.parent().ok_or(TransactionFsError::UnsafePath)?,
            self.owner.as_deref() == Some(path),
        )?;
        let before = directory.snapshot(&component(path)?, limit)?;
        if before.is_none() != missing {
            return Err(TransactionFsError::Conflict);
        }
        if self.owner.as_deref() == Some(path)
            && before
                .as_ref()
                .is_some_and(|file| file.version.mode & 0o7777 != 0o600)
        {
            return Err(TransactionFsError::UnsafeFile);
        }
        let public = before
            .as_ref()
            .map(|snapshot| -> Result<_, TransactionFsError> {
                Ok(FileSnapshot {
                    bytes: snapshot.bytes.clone(),
                    identity: FileIdentity {
                        volume_serial: snapshot
                            .version
                            .object
                            .device
                            .try_into()
                            .map_err(|_| TransactionFsError::UnsafeFile)?,
                        file_index: snapshot.version.object.inode,
                    },
                })
            })
            .transpose()?;
        let token = FileToken {
            transaction_id: self.id,
            entry_index: self.entries.len(),
        };
        self.entries.push(Entry {
            path: path.to_owned(),
            directory,
            before,
            after: None,
            limit,
            new_claude_directory: false,
        });
        Ok((token, public))
    }

    pub(crate) fn open_existing(
        &mut self,
        path: &Path,
        max_bytes: usize,
    ) -> Result<(FileToken, FileSnapshot), TransactionFsError> {
        let (token, snapshot) = self.add(path, max_bytes, false)?;
        Ok((token, snapshot.ok_or(TransactionFsError::Conflict)?))
    }

    pub(crate) fn create_missing(
        &mut self,
        path: &Path,
        max_bytes: usize,
    ) -> Result<(FileToken, FileIdentity), TransactionFsError> {
        let (token, _) = self.add(path, max_bytes, true)?;
        // Missing configs are fingerprinted with None; no inode is published before commit.
        Ok((
            token,
            FileIdentity {
                volume_serial: 0,
                file_index: 0,
            },
        ))
    }

    pub(crate) fn create_claude_directory(
        &mut self,
        root: &Path,
    ) -> Result<(ClaudeDirectoryToken, FileIdentity), TransactionFsError> {
        self.check()?;
        if self.claude_project.is_some()
            || !self.entries.is_empty()
            || !self.absent.is_empty()
            || self.prepared.is_some()
        {
            return Err(TransactionFsError::Conflict);
        }
        let project = Directory::open(root, false)?;
        require_absent(&root.join(".claude"))?;
        self.claude_project = Some(project);
        Ok((
            ClaudeDirectoryToken {
                transaction_id: self.id,
                entry_index: 0,
            },
            FileIdentity {
                volume_serial: 0,
                file_index: 0,
            },
        ))
    }

    pub(crate) fn create_claude_settings_file(
        &mut self,
        directory: ClaudeDirectoryToken,
        max_bytes: usize,
    ) -> Result<(FileToken, FileIdentity), TransactionFsError> {
        self.check()?;
        if directory.transaction_id != self.id
            || directory.entry_index != 0
            || !self.entries.is_empty()
            || self.prepared.is_some()
        {
            return Err(TransactionFsError::Conflict);
        }
        if max_bytes > MAX_TRANSACTION_FILE_BYTES {
            return Err(TransactionFsError::TooLarge);
        }
        let project = self
            .claude_project
            .as_ref()
            .ok_or(TransactionFsError::Conflict)?;
        project.verify()?;
        require_absent(&project.path.join(".claude"))?;
        let path = project.path.join(".claude/settings.local.json");
        config_io::validate_lexical_path(&path).map_err(map_path)?;
        let directory = Directory::open(&project.path, false)?;
        let token = FileToken {
            transaction_id: self.id,
            entry_index: 0,
        };
        self.entries.push(Entry {
            path,
            directory,
            before: None,
            after: None,
            limit: max_bytes,
            new_claude_directory: true,
        });
        Ok((
            token,
            FileIdentity {
                volume_serial: 0,
                file_index: 0,
            },
        ))
    }

    pub(crate) fn canonical_target(&self, token: FileToken) -> Result<PathBuf, TransactionFsError> {
        let index = self.token(token)?;
        self.entries[index].directory.verify()?;
        Ok(self.entries[index].path.clone())
    }

    pub(crate) fn reserve_absent(&mut self, path: &Path) -> Result<(), TransactionFsError> {
        self.check()?;
        if self.prepared.is_some()
            || self.entries.len() + self.absent.len() >= MAX_TRANSACTION_FILES
        {
            return Err(TransactionFsError::TooManyFiles);
        }
        config_io::validate_lexical_path(path).map_err(map_path)?;
        Directory::open(path.parent().ok_or(TransactionFsError::UnsafePath)?, false)?;
        match fs::symlink_metadata(path) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                self.absent.push(path.to_owned());
                Ok(())
            }
            _ => Err(TransactionFsError::Conflict),
        }
    }

    pub(crate) fn replace_contents(
        &mut self,
        token: FileToken,
        bytes: &[u8],
    ) -> Result<(), TransactionFsError> {
        let index = self.token(token)?;
        if self.prepared.is_some() {
            return Err(TransactionFsError::Conflict);
        }
        if bytes.len() > self.entries[index].limit {
            return Err(TransactionFsError::TooLarge);
        }
        self.entries[index].after = Some(bytes.to_vec());
        Ok(())
    }

    fn ensure_lock(&mut self, state: &Directory, create: bool) -> Result<(), TransactionFsError> {
        if let Some(file) = &self.lock {
            let named = state
                .snapshot(c".apfs.lock", 0)?
                .ok_or(TransactionFsError::Conflict)?;
            if Object::from(&file.metadata().map_err(|_| TransactionFsError::Io)?)
                != named.version.object
            {
                return Err(TransactionFsError::Conflict);
            }
        } else {
            self.lock = lock(state, create)?;
        }
        Ok(())
    }

    pub(crate) fn bind_ownership_path(
        &mut self,
        path: &Path,
        binding: Option<&str>,
    ) -> Result<(), TransactionFsError> {
        self.check()?;
        if self.owner.as_ref().is_some_and(|current| current != path) {
            return Err(TransactionFsError::Conflict);
        }
        self.owner = Some(path.to_owned());
        let root = path.parent().ok_or(TransactionFsError::UnsafePath)?;
        match fs::symlink_metadata(root) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Ok(_) => {}
            Err(_) => return Err(TransactionFsError::UnsafePath),
        }
        let state = Directory::open(root, true)?;
        let history = journals(&state)?;
        if !history.is_empty() {
            self.ensure_lock(&state, false)?;
            if self.lock.is_none() {
                return Err(TransactionFsError::Conflict);
            }
        }
        let target = self
            .entries
            .iter()
            .find(|entry| entry.path != path)
            .map(|entry| entry.path.clone());
        for (journal, digest) in history {
            if path.file_name().and_then(|value| value.to_str())
                != Some(journal.owner_filename.as_str())
            {
                continue;
            }
            if let (Some(binding), Some(target)) = (binding, target.as_deref()) {
                if journal.binding == binding {
                    journal.recover(&state, &digest, target)?;
                    continue;
                }
            }
            // Passive snapshots never follow stored paths or perform recovery writes.
            if journal.marker(&state, &digest)?.is_none() {
                return Err(TransactionFsError::Conflict);
            }
        }
        self.owner_directory = Some(state);
        Ok(())
    }

    fn validate_entries(&self) -> Result<(), TransactionFsError> {
        self.check()?;
        for entry in &self.entries {
            if entry.new_claude_directory {
                entry.directory.verify()?;
                require_absent(entry.path.parent().ok_or(TransactionFsError::UnsafePath)?)?;
                continue;
            }
            let current = entry
                .directory
                .snapshot(&component(&entry.path)?, entry.limit)?;
            if version(&current) != version(&entry.before) {
                return Err(TransactionFsError::Conflict);
            }
        }
        for guard in &self.absent {
            match fs::symlink_metadata(guard) {
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                _ => return Err(TransactionFsError::Conflict),
            }
        }
        Ok(())
    }

    pub(crate) fn seal(&mut self) -> Result<(), TransactionFsError> {
        self.validate_entries()?;
        if self.prepared.is_some() || self.entries.len() != 2 {
            return Err(TransactionFsError::Unsupported);
        }
        let owner_path = self.owner.clone().ok_or(TransactionFsError::Unsupported)?;
        let state = Directory::open(
            owner_path.parent().ok_or(TransactionFsError::UnsafePath)?,
            true,
        )?;
        self.ensure_lock(&state, true)?;
        let history = journals(&state)?;
        if history.len() >= MAX_JOURNALS {
            return Err(TransactionFsError::RetainedVersionsLimit);
        }
        let owner_index = self
            .entries
            .iter()
            .position(|entry| entry.path == owner_path)
            .ok_or(TransactionFsError::Unsupported)?;
        let config_index = 1 - owner_index;
        let config = &self.entries[config_index];
        let owner = &self.entries[owner_index];
        check_history(&state, &owner_path, &config.path, None)?;
        let after_config = config
            .after
            .as_ref()
            .ok_or(TransactionFsError::Unsupported)?;
        let after_owner = owner
            .after
            .as_ref()
            .ok_or(TransactionFsError::Unsupported)?;
        let owner_filename = owner_path
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or(TransactionFsError::UnsafePath)?;
        validate_macos_publication(
            &config.path,
            owner_filename,
            owner.before.as_ref().map(|file| file.bytes.as_slice()),
            after_owner,
            config.before.as_ref().map(|file| file.bytes.as_slice()),
            after_config,
        )
        .map_err(|_| TransactionFsError::Conflict)?;
        let id = Uuid::new_v4().to_string();
        let (staged_config, config_directory) = if config.new_claude_directory {
            let staged = config.directory.create_claude_stage(&id)?;
            (
                staged.create(c"settings.local.json", after_config, None)?,
                staged.object,
            )
        } else {
            (
                config.directory.create(
                    &config_stage(&id)?,
                    after_config,
                    config.before.as_ref(),
                )?,
                config.directory.object.clone(),
            )
        };
        let staged_owner = state.create(&name(&id, "owner")?, after_owner, None)?;
        self.validate_entries()?;
        let journal = Journal {
            schema_version: 1,
            id,
            config_path: config.path.as_os_str().as_bytes().to_vec(),
            binding: path_binding(&config.path)?,
            config_directory,
            owner_directory: state.object.clone(),
            owner_filename: owner_filename.to_owned(),
            before_config: version(&config.before).cloned(),
            after_config: staged_config.version,
            before_owner: version(&owner.before).cloned(),
            after_owner: staged_owner.version,
            absent_guards: self
                .absent
                .iter()
                .map(|path| path.as_os_str().as_bytes().to_vec())
                .collect(),
            claude_project: config
                .new_claude_directory
                .then(|| config.directory.object.clone()),
        };
        journal.target()?;
        let bytes = serde_json::to_vec(&journal).map_err(|_| TransactionFsError::Io)?;
        if bytes.len() > MAX_JOURNAL_BYTES {
            return Err(TransactionFsError::TooLarge);
        }
        let temporary = name(&journal.id, "intent")?;
        state.create(&temporary, &bytes, None)?;
        state.publish(&temporary, &name(&journal.id, "json")?, false)?;
        self.prepared = Some(Prepared {
            state,
            journal,
            digest: hash(&bytes),
        });
        Ok(())
    }

    pub(crate) fn commit(self) -> Result<(), TransactionFsError> {
        self.commit_inner(|| {})
    }

    fn commit_inner(mut self, after_config: impl FnOnce()) -> Result<(), TransactionFsError> {
        self.validate_entries()?;
        let prepared = self
            .prepared
            .take()
            .ok_or(TransactionFsError::CommitFailed)?;
        let journal_file = prepared
            .state
            .snapshot(&name(&prepared.journal.id, "json")?, MAX_JOURNAL_BYTES)?
            .ok_or(TransactionFsError::Conflict)?;
        if hash(&journal_file.bytes) != prepared.digest {
            return Err(TransactionFsError::Conflict);
        }
        let path = prepared.journal.target()?;
        let owner_path = self.owner.as_ref().ok_or(TransactionFsError::Conflict)?;
        check_history(
            &prepared.state,
            owner_path,
            &path,
            Some(&prepared.journal.id),
        )?;
        let config = self
            .entries
            .iter()
            .find(|entry| entry.path == path)
            .ok_or(TransactionFsError::Conflict)?;
        if config.new_claude_directory {
            let stage_name = name(&prepared.journal.id, "claude")?;
            let staged = Directory::open(
                &config
                    .directory
                    .path
                    .join(OsString::from_vec(stage_name.to_bytes().to_vec())),
                true,
            )?;
            if staged.object != prepared.journal.config_directory
                || version(&staged.snapshot(c"settings.local.json", MAX_TRANSACTION_FILE_BYTES)?)
                    != Some(&prepared.journal.after_config)
            {
                return Err(TransactionFsError::Conflict);
            }
            config.directory.publish(&stage_name, c".claude", false)?;
        } else {
            if version(&config.directory.snapshot(
                &config_stage(&prepared.journal.id)?,
                MAX_TRANSACTION_FILE_BYTES,
            )?) != Some(&prepared.journal.after_config)
            {
                return Err(TransactionFsError::Conflict);
            }
            config.directory.publish(
                &config_stage(&prepared.journal.id)?,
                &component(&path)?,
                config.before.is_some(),
            )?;
        }
        after_config();
        let outcome = (|| {
            check_history(
                &prepared.state,
                owner_path,
                &path,
                Some(&prepared.journal.id),
            )?;
            prepared
                .journal
                .recover(&prepared.state, &prepared.digest, &path)?;
            let published = Directory::open(
                path.parent().ok_or(TransactionFsError::UnsafePath)?,
                config.new_claude_directory,
            )?;
            if prepared.journal.marker(&prepared.state, &prepared.digest)?
                != Some(Disposition::Committed)
                || version(&published.snapshot(&component(&path)?, MAX_TRANSACTION_FILE_BYTES)?)
                    != Some(&prepared.journal.after_config)
                || version(
                    &prepared
                        .state
                        .snapshot(&component(owner_path)?, MAX_TRANSACTION_FILE_BYTES)?,
                ) != Some(&prepared.journal.after_owner)
            {
                return Err(TransactionFsError::Conflict);
            }
            prepared.journal.check_guards()?;
            check_history(&prepared.state, owner_path, &path, None)
        })();
        outcome.map_err(|error| {
            if error == TransactionFsError::Conflict {
                error
            } else {
                TransactionFsError::CommitFailed
            }
        })
    }

    pub(crate) fn rollback(self) -> Result<(), TransactionFsError> {
        // A sealed intent is reconciled by the next explicit action; Drop never writes.
        if self.prepared.is_some() {
            return Err(TransactionFsError::RollbackFailed);
        }
        self.check()
    }
}

#[cfg(test)]
#[path = "transaction_macos_tests.rs"]
mod tests;

#[path = "transaction_macos_retained.rs"]
pub(crate) mod retained_versions;
