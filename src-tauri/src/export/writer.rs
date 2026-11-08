use super::report::Error;
use std::path::Path;

#[cfg(windows)]
pub(super) fn save_new(path: &Path, extension: &str, bytes: &[u8]) -> Result<(), Error> {
    use crate::integrations::config_io::pin_existing_directory_chain;
    use std::fs::{self, OpenOptions};
    use std::io::Write;
    use std::os::windows::fs::OpenOptionsExt;
    use windows_sys::Win32::Foundation::GENERIC_WRITE;
    use windows_sys::Win32::Storage::FileSystem::{
        DELETE, FILE_ATTRIBUTE_TEMPORARY, FILE_FLAG_OPEN_REPARSE_POINT, FILE_SHARE_READ,
        SYNCHRONIZE,
    };
    if bytes.is_empty() || bytes.len() > super::report::MAX_REPORT_BYTES {
        return Err(Error::InvalidInput);
    }
    validate_target(path, extension)?;
    let parent = path.parent().ok_or(Error::UnsafePath)?;
    let pins = pin_existing_directory_chain(parent).map_err(|_| Error::UnsafePath)?;
    match fs::symlink_metadata(path) {
        Ok(_) => return Err(Error::FileExists),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
        Err(_) => return Err(Error::UnsafePath),
    }
    let temporary = parent.join(format!(".headroom-export-{}.tmp", uuid::Uuid::new_v4()));
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .access_mode(GENERIC_WRITE | DELETE | SYNCHRONIZE)
        .share_mode(FILE_SHARE_READ)
        .custom_flags(FILE_ATTRIBUTE_TEMPORARY | FILE_FLAG_OPEN_REPARSE_POINT)
        .open(&temporary)
        .map_err(|_| Error::WriteFailed)?;
    let mut staging = Staging {
        file,
        published: false,
    };
    staging
        .file
        .write_all(bytes)
        .map_err(|_| Error::WriteFailed)?;
    staging.file.sync_all().map_err(|_| Error::WriteFailed)?;
    pins.validate().map_err(|_| Error::UnsafePath)?;
    staging.publish(path)
}

#[cfg(windows)]
fn validate_target(path: &Path, extension: &str) -> Result<(), Error> {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or(Error::UnsafePath)?;
    let stem = name.split('.').next().unwrap_or("").to_ascii_uppercase();
    let reserved = ["CON", "PRN", "AUX", "NUL"].contains(&stem.as_str())
        || (stem.starts_with("COM") || stem.starts_with("LPT"))
            && stem.len() == 4
            && matches!(stem.as_bytes()[3], b'1'..=b'9');
    if !path.is_absolute()
        || name.is_empty()
        || name.ends_with([' ', '.'])
        || name
            .chars()
            .any(|c| c.is_control() || "<>:\"|?*".contains(c))
        || reserved
        || !path
            .extension()
            .and_then(|s| s.to_str())
            .is_some_and(|s| s.eq_ignore_ascii_case(extension))
    {
        return Err(Error::UnsafePath);
    }
    Ok(())
}

#[cfg(windows)]
struct Staging {
    file: std::fs::File,
    published: bool,
}

#[cfg(windows)]
impl Staging {
    fn publish(&mut self, target: &Path) -> Result<(), Error> {
        use std::os::windows::{ffi::OsStrExt, io::AsRawHandle};
        use windows_sys::Win32::Storage::FileSystem::{
            FileRenameInfo, SetFileInformationByHandle, FILE_RENAME_INFO,
        };
        let name: Vec<u16> = target.as_os_str().encode_wide().collect();
        if name.len() > 32_767 {
            return Err(Error::UnsafePath);
        }
        let bytes = (std::mem::offset_of!(FILE_RENAME_INFO, FileName) + (name.len() + 1) * 2)
            .max(std::mem::size_of::<FILE_RENAME_INFO>());
        let mut buffer = vec![0u64; bytes.div_ceil(8)];
        let info = buffer.as_mut_ptr().cast::<FILE_RENAME_INFO>();
        // Rename the held file, not a reopenable staging path. FALSE preserves any racing destination.
        let success = unsafe {
            (*info).Anonymous.ReplaceIfExists = false;
            (*info).FileNameLength = (name.len() * 2) as u32;
            std::ptr::copy_nonoverlapping(
                name.as_ptr(),
                std::ptr::addr_of_mut!((*info).FileName).cast::<u16>(),
                name.len(),
            );
            SetFileInformationByHandle(
                self.file.as_raw_handle(),
                FileRenameInfo,
                info.cast(),
                bytes as u32,
            )
        };
        if success == 0 {
            return Err(match std::io::Error::last_os_error().raw_os_error() {
                Some(80 | 183) => Error::FileExists,
                _ => Error::WriteFailed,
            });
        }
        self.published = true;
        Ok(())
    }
}

#[cfg(windows)]
impl Drop for Staging {
    fn drop(&mut self) {
        if !self.published {
            use std::os::windows::io::AsRawHandle;
            use windows_sys::Win32::Storage::FileSystem::{
                FileDispositionInfo, SetFileInformationByHandle, FILE_DISPOSITION_INFO,
            };
            let disposition = FILE_DISPOSITION_INFO { DeleteFile: true };
            // This handle still denies foreign write/delete; cleanup cannot target a replacement path.
            unsafe {
                SetFileInformationByHandle(
                    self.file.as_raw_handle(),
                    FileDispositionInfo,
                    std::ptr::addr_of!(disposition).cast(),
                    std::mem::size_of_val(&disposition) as u32,
                );
            }
        }
    }
}

#[cfg(not(windows))]
pub(super) fn save_new(_path: &Path, _extension: &str, _bytes: &[u8]) -> Result<(), Error> {
    Err(Error::Unavailable)
}

#[cfg(all(test, windows))]
#[path = "writer_tests.rs"]
mod tests;
