use super::super::report::{Error, MAX_REPORT_BYTES};
use crate::integrations::config_io::{ensure_fixed_volume, pin_existing_directory_chain};
use std::ffi::{CStr, CString};
use std::fs::File;
use std::io::{self, Write};
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::MetadataExt;
use std::path::Path;

pub(in crate::export) fn save_new(path: &Path, extension: &str, bytes: &[u8]) -> Result<(), Error> {
    if bytes.is_empty() || bytes.len() > MAX_REPORT_BYTES {
        return Err(Error::InvalidInput);
    }
    let name = path.file_name().ok_or(Error::UnsafePath)?;
    if !matches!(extension, "json" | "csv")
        || name.as_bytes().iter().any(|byte| byte.is_ascii_control())
        || !path
            .extension()
            .and_then(|value| value.to_str())
            .is_some_and(|value| value.eq_ignore_ascii_case(extension))
    {
        return Err(Error::UnsafePath);
    }
    ensure_fixed_volume(path).map_err(|_| Error::UnsafePath)?;
    let pins = pin_existing_directory_chain(path.parent().ok_or(Error::UnsafePath)?)
        .map_err(|_| Error::UnsafePath)?;
    let directory = pins.directory_file().map_err(|_| Error::UnsafePath)?;
    let name = CString::new(name.as_bytes()).map_err(|_| Error::UnsafePath)?;
    if named_identity(directory, &name)?.is_some() {
        return Err(Error::FileExists);
    }
    let temporary = CString::new(format!(".headroom-export-{}.tmp", uuid::Uuid::new_v4()))
        .map_err(|_| Error::WriteFailed)?;
    // SAFETY: an exclusive new file resolves beneath the pinned directory, never through a symlink.
    let fd = unsafe {
        libc::openat(
            directory.as_raw_fd(),
            temporary.as_ptr(),
            libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            0o600,
        )
    };
    if fd < 0 {
        return Err(Error::WriteFailed);
    }
    let mut staging = Staging {
        directory,
        name: temporary,
        file: unsafe { File::from_raw_fd(fd) },
    };
    staging
        .file
        .write_all(bytes)
        .and_then(|_| staging.file.sync_all())
        .map_err(|_| Error::WriteFailed)?;
    pins.validate().map_err(|_| Error::UnsafePath)?;
    staging.publish(&name)?;
    pins.validate().map_err(|_| Error::UnsafePath)
}

fn named_identity(directory: &File, name: &CStr) -> Result<Option<(u64, u64)>, Error> {
    let mut info = std::mem::MaybeUninit::<libc::stat>::uninit();
    // SAFETY: the live directory and single-component name outlive the stat call.
    if unsafe {
        libc::fstatat(
            directory.as_raw_fd(),
            name.as_ptr(),
            info.as_mut_ptr(),
            libc::AT_SYMLINK_NOFOLLOW,
        )
    } != 0
    {
        return if io::Error::last_os_error().kind() == io::ErrorKind::NotFound {
            Ok(None)
        } else {
            Err(Error::UnsafePath)
        };
    }
    let info = unsafe { info.assume_init() };
    Ok(Some((info.st_dev as u64, info.st_ino)))
}

struct Staging<'a> {
    directory: &'a File,
    name: CString,
    file: File,
}

impl Staging<'_> {
    fn identity(&self) -> Result<(u64, u64), Error> {
        let info = self.file.metadata().map_err(|_| Error::WriteFailed)?;
        if !info.is_file()
            || info.uid() != unsafe { libc::geteuid() }
            || info.mode() & 0o7777 != 0o600
            || info.nlink() != 1
        {
            return Err(Error::UnsafePath);
        }
        Ok((info.dev(), info.ino()))
    }

    fn publish(&self, target: &CStr) -> Result<(), Error> {
        let identity = self.identity()?;
        if named_identity(self.directory, &self.name)? != Some(identity) {
            return Err(Error::UnsafePath);
        }
        let fd = self.directory.as_raw_fd();
        // RENAME_EXCL is atomic on APFS and preserves a racing destination, including symlinks.
        if unsafe {
            libc::renameatx_np(
                fd,
                self.name.as_ptr(),
                fd,
                target.as_ptr(),
                libc::RENAME_EXCL,
            )
        } != 0
        {
            return Err(
                if io::Error::last_os_error().kind() == io::ErrorKind::AlreadyExists {
                    Error::FileExists
                } else {
                    Error::WriteFailed
                },
            );
        }
        if named_identity(self.directory, target)? != Some(identity) {
            return Err(Error::UnsafePath);
        }
        self.directory.sync_all().map_err(|_| Error::WriteFailed)
    }
}

impl Drop for Staging<'_> {
    fn drop(&mut self) {
        if self
            .file
            .metadata()
            .ok()
            .map(|info| (info.dev(), info.ino()))
            .is_some_and(|identity| {
                named_identity(self.directory, &self.name).ok().flatten() == Some(identity)
            })
        {
            // Check the held inode before cleaning the random staging name.
            unsafe {
                libc::unlinkat(self.directory.as_raw_fd(), self.name.as_ptr(), 0);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, OpenOptions};
    use std::os::unix::fs::OpenOptionsExt;

    #[test]
    fn changed_staging_name_is_rejected_and_foreign_replacement_survives_cleanup() {
        let directory = super::super::macos_tests::Directory::new();
        let pins = pin_existing_directory_chain(&directory.0).unwrap();
        let parent = pins.directory_file().unwrap();
        let name = CString::new("staging.tmp").unwrap();
        let temporary = directory.0.join("staging.tmp");
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&temporary)
            .unwrap();
        let staging = Staging {
            directory: parent,
            name,
            file,
        };
        fs::rename(&temporary, directory.0.join("held.tmp")).unwrap();
        fs::write(&temporary, b"foreign replacement").unwrap();
        assert_eq!(
            staging.publish(&CString::new("report.json").unwrap()),
            Err(Error::UnsafePath)
        );
        drop(staging);
        assert_eq!(fs::read(&temporary).unwrap(), b"foreign replacement");
        assert!(!directory.0.join("report.json").exists());
    }
}
