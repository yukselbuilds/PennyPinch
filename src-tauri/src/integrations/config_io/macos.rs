use super::{ConfigIoError, FileIdentity};
use std::ffi::CString;
use std::fs::{self, File, Metadata, OpenOptions};
use std::io;
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Component, Path, PathBuf};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct FileStamp {
    length: u64,
    mode: u32,
    uid: u32,
    links: u64,
    modified_seconds: i64,
    modified_nanos: i64,
    changed_seconds: i64,
    changed_nanos: i64,
}

impl From<&Metadata> for FileStamp {
    fn from(info: &Metadata) -> Self {
        Self {
            length: info.len(),
            mode: info.mode(),
            uid: info.uid(),
            links: info.nlink(),
            modified_seconds: info.mtime(),
            modified_nanos: info.mtime_nsec(),
            changed_seconds: info.ctime(),
            changed_nanos: info.ctime_nsec(),
        }
    }
}

pub(super) struct DirectoryPin {
    path: PathBuf,
    file: File,
    device: u64,
    inode: u64,
}

pub(super) type DirectoryPins = Vec<DirectoryPin>;

pub(crate) fn validate_lexical_path(path: &Path) -> Result<(), ConfigIoError> {
    let bytes = path.as_os_str().as_bytes();
    if !path.is_absolute() {
        return Err(ConfigIoError::InvalidRoot);
    }
    if bytes.len() >= libc::PATH_MAX as usize
        || bytes.contains(&0)
        || bytes
            .split(|byte| *byte == b'/')
            .any(|part| part == b"." || part == b"..")
        || path
            .components()
            .any(|part| !matches!(part, Component::RootDir | Component::Normal(_)))
    {
        return Err(ConfigIoError::UnsafePath);
    }
    Ok(())
}

fn open_directory(path: &Path) -> Result<DirectoryPin, ConfigIoError> {
    let file = OpenOptions::new()
        .read(true)
        // Darwin rejects combining O_NOFOLLOW with O_NOFOLLOW_ANY (EINVAL).
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW_ANY | libc::O_CLOEXEC)
        .open(path)
        .map_err(|_| ConfigIoError::UnsafePath)?;
    let info = file.metadata().map_err(|_| ConfigIoError::Unavailable)?;
    // Root-owned system ancestors and this user's directories form the allowed chain.
    let uid = unsafe { libc::geteuid() };
    let sticky_system_directory = info.uid() == 0 && info.mode() & libc::S_ISVTX as u32 != 0;
    if !info.is_dir()
        || (info.uid() != 0 && info.uid() != uid)
        || (info.mode() & 0o022 != 0 && !sticky_system_directory)
    {
        return Err(ConfigIoError::UnsafePath);
    }
    Ok(DirectoryPin {
        path: path.to_owned(),
        device: info.dev(),
        inode: info.ino(),
        file,
    })
}

pub(super) fn pin_parent_directories(path: &Path) -> Result<DirectoryPins, ConfigIoError> {
    validate_lexical_path(path)?;
    let parent = path.parent().ok_or(ConfigIoError::UnsafePath)?;
    let mut pins = Vec::new();
    let mut current = PathBuf::from("/");
    pins.push(open_directory(&current)?);
    for part in parent.components() {
        if let Component::Normal(name) = part {
            current.push(name);
            pins.push(open_directory(&current)?);
        }
    }
    validate_pinned_directories(&pins)?;
    Ok(pins)
}

pub(super) fn validate_pinned_directories(pins: &DirectoryPins) -> Result<(), ConfigIoError> {
    for pin in pins {
        let held = pin
            .file
            .metadata()
            .map_err(|_| ConfigIoError::Unavailable)?;
        let named = open_directory(&pin.path)?;
        if pin.device != held.dev()
            || pin.inode != held.ino()
            || pin.device != named.device
            || pin.inode != named.inode
        {
            return Err(ConfigIoError::UnsafePath);
        }
    }
    Ok(())
}

pub(crate) fn ensure_fixed_volume(path: &Path) -> Result<(), ConfigIoError> {
    validate_lexical_path(path)?;
    let mut ancestor = path.parent().ok_or(ConfigIoError::InvalidRoot)?;
    loop {
        match fs::symlink_metadata(ancestor) {
            Ok(_) => break,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                ancestor = ancestor.parent().ok_or(ConfigIoError::InvalidRoot)?;
            }
            Err(_) => return Err(ConfigIoError::Unavailable),
        }
    }
    let directory = open_directory(ancestor)?;
    let mut volume = std::mem::MaybeUninit::<libc::statfs>::uninit();
    // SAFETY: directory owns the live fd and volume points to a writable statfs.
    if unsafe { libc::fstatfs(directory.file.as_raw_fd(), volume.as_mut_ptr()) } != 0 {
        return Err(ConfigIoError::Unavailable);
    }
    let volume = unsafe { volume.assume_init() };
    if volume.f_flags & libc::MNT_LOCAL as u32 == 0
        || !volume
            .f_fstypename
            .iter()
            .take(5)
            .map(|byte| *byte as u8)
            .eq(b"apfs\0".iter().copied())
    {
        return Err(ConfigIoError::InvalidRoot);
    }
    Ok(())
}

pub(in crate::integrations) fn open_read_shared(path: &Path) -> Result<File, ConfigIoError> {
    let pins = pin_parent_directories(path)?;
    let parent = pins.last().ok_or(ConfigIoError::InvalidRoot)?;
    let name = CString::new(
        path.file_name()
            .ok_or(ConfigIoError::UnsafePath)?
            .as_bytes(),
    )
    .map_err(|_| ConfigIoError::UnsafePath)?;
    // SAFETY: a single filename resolves beneath a held, verified parent directory.
    let fd = unsafe {
        libc::openat(
            parent.file.as_raw_fd(),
            name.as_ptr(),
            libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC,
        )
    };
    if fd < 0 {
        return Err(ConfigIoError::Unavailable);
    }
    let file = unsafe { File::from_raw_fd(fd) };
    validate_pinned_directories(&pins)?;
    file_identity(&file)?;
    Ok(file)
}

pub(super) fn file_identity(file: &File) -> Result<FileIdentity, ConfigIoError> {
    let info = file.metadata().map_err(|_| ConfigIoError::Unavailable)?;
    if !info.is_file()
        || info.nlink() != 1
        || info.uid() != unsafe { libc::geteuid() }
        || info.mode() & 0o022 != 0
        || info.mode() & 0o7000 != 0
    {
        return Err(ConfigIoError::UnsafePath);
    }
    Ok(FileIdentity {
        volume: u32::try_from(info.dev()).map_err(|_| ConfigIoError::Unavailable)?,
        high: (info.ino() >> 32) as u32,
        low: info.ino() as u32,
        attributes: info.mode(),
        stamp: FileStamp::from(&info),
    })
}

pub(super) fn validate_file_pin(
    path: &Path,
    file: &File,
    expected: FileIdentity,
) -> Result<(), ConfigIoError> {
    let held = file_identity(file)?;
    let named = file_identity(&open_read_shared(path)?)?;
    if held != expected || named != expected {
        return Err(ConfigIoError::UnsafePath);
    }
    Ok(())
}

#[cfg(test)]
#[path = "macos_tests.rs"]
mod tests;
