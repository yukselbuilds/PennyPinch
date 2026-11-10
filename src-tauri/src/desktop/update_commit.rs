use crate::integrations::config_io::{pin_existing_directory_chain, PinnedDirectoryChain};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::os::windows::{
    ffi::OsStrExt,
    fs::{MetadataExt, OpenOptionsExt},
};
use std::path::{Path, PathBuf};
use uuid::Uuid;
use windows_sys::Win32::Storage::FileSystem::{
    MoveFileExW, FILE_ATTRIBUTE_REPARSE_POINT, FILE_FLAG_OPEN_REPARSE_POINT, FILE_SHARE_READ,
    MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
};

const EXE_LIMIT: u64 = 128 * 1024 * 1024;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct FileDigest {
    pub(super) size: u64,
    pub(super) sha256: String,
}

impl FileDigest {
    pub(super) fn from_file(file: &mut File) -> io::Result<Self> {
        file.seek(SeekFrom::Start(0))?;
        let mut hasher = Sha256::new();
        let size = io::copy(&mut (&mut *file).take(EXE_LIMIT + 1), &mut hasher)?;
        if !(1..=EXE_LIMIT).contains(&size) {
            return Err(io::Error::other("update_file_size"));
        }
        file.seek(SeekFrom::Start(0))?;
        Ok(Self {
            size,
            sha256: format!("{:x}", hasher.finalize()),
        })
    }

    pub(super) fn check(&self, file: &mut File) -> io::Result<()> {
        if self != &Self::from_file(file)? {
            return Err(io::Error::other("update_file_changed"));
        }
        Ok(())
    }
}

pub(super) fn read_owned(path: &Path) -> io::Result<File> {
    let file = OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
        return Err(io::Error::other("update_file_type"));
    }
    Ok(file)
}

pub(super) fn read_small(path: &Path, limit: u64) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    read_owned(path)?.take(limit + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Err(io::Error::other("update_record_size"));
    }
    Ok(bytes)
}

pub(super) fn create_bytes(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut file = new_file(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

fn new_file(path: &Path) -> io::Result<File> {
    OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .share_mode(0)
        .open(path)
}

pub(super) fn copy_verified(path: &Path, source: &mut File, digest: &FileDigest) -> io::Result<()> {
    source.seek(SeekFrom::Start(0))?;
    let mut file = new_file(path)?;
    let result = (|| {
        #[cfg(test)]
        if fault::take_disk_full() {
            io::copy(&mut (&mut *source).take(3), &mut file)?;
            return Err(io::Error::from_raw_os_error(
                windows_sys::Win32::Foundation::ERROR_DISK_FULL as i32,
            ));
        }
        io::copy(&mut source.take(digest.size.saturating_add(1)), &mut file)?;
        digest.check(&mut file)?;
        file.sync_all()
    })();
    drop(file);
    if result.is_err() {
        let _ = fs::remove_file(path);
    }
    result
}

pub(super) struct Swap {
    live: PathBuf,
    staged: PathBuf,
    digest: FileDigest,
    pins: PinnedDirectoryChain,
}

impl Swap {
    pub(super) fn stage(live: &Path, source: &mut File, digest: &FileDigest) -> io::Result<Self> {
        let parent = live
            .parent()
            .ok_or_else(|| io::Error::other("update_install_path"))?;
        let pins = pin_existing_directory_chain(parent)
            .map_err(|_| io::Error::other("update_install_path"))?;
        let staged = parent.join(format!(".headroom-next-{}.exe", Uuid::new_v4()));
        copy_verified(&staged, source, digest)?;
        Ok(Self {
            live: live.to_owned(),
            staged,
            digest: digest.clone(),
            pins,
        })
    }

    pub(super) fn path(&self) -> &Path {
        &self.staged
    }

    pub(super) fn seal(&self) -> io::Result<File> {
        let mut file = read_owned(&self.staged)?;
        self.digest.check(&mut file)?;
        Ok(file)
    }

    pub(super) fn publish(self, expected: &FileDigest) -> io::Result<()> {
        self.pins
            .validate()
            .map_err(|_| io::Error::other("update_install_path"))?;
        self.digest.check(&mut read_owned(&self.staged)?)?;
        expected.check(&mut read_owned(&self.live)?)?;
        let wide = |path: &Path| {
            path.as_os_str()
                .encode_wide()
                .chain(Some(0))
                .collect::<Vec<_>>()
        };
        let source = wide(&self.staged);
        let target = wide(&self.live);
        #[cfg(test)]
        fault::checkpoint("before_publish");
        if unsafe {
            MoveFileExW(
                source.as_ptr(),
                target.as_ptr(),
                MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        #[cfg(test)]
        fault::checkpoint("after_publish");
        Ok(())
    }
}

impl Drop for Swap {
    fn drop(&mut self) {
        if self.pins.validate().is_ok() {
            let _ = fs::remove_file(&self.staged);
        }
    }
}

#[cfg(test)]
#[path = "update_commit_tests.rs"]
mod tests;

#[cfg(test)]
pub(super) mod fault {
    use super::*;
    use std::cell::Cell;
    use std::time::Duration;

    thread_local! { static DISK_FULL: Cell<bool> = const { Cell::new(false) }; }

    pub(super) fn disk_full_next_copy() {
        DISK_FULL.set(true);
    }

    pub(super) fn take_disk_full() -> bool {
        DISK_FULL.replace(false)
    }

    pub(super) fn checkpoint(point: &str) {
        if std::env::var("HEADROOM_TEST_SWAP_POINT").as_deref() != Ok(point) {
            return;
        }
        let directory = PathBuf::from(std::env::var_os("HEADROOM_TEST_SWAP_ROOT").unwrap());
        let boundary = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("work/proofs/update/swap");
        assert!(fs::canonicalize(&directory)
            .unwrap()
            .starts_with(fs::canonicalize(&boundary).unwrap()));
        create_bytes(&directory.join("checkpoint"), point.as_bytes()).unwrap();
        loop {
            std::thread::sleep(Duration::from_millis(50));
        }
    }
}
