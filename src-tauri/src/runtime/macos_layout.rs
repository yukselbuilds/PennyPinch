//! Validate Unix resources against the manifest digest embedded at build time.

use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, OpenOptions};
use std::io::Read;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Component, Path, PathBuf};

#[cfg(target_os = "macos")]
pub(super) const COMPILED_MANIFEST_SHA256: Option<&str> =
    option_env!("HEADROOM_MACOS_RUNTIME_MANIFEST_SHA256");
#[cfg(target_os = "linux")]
pub(super) const COMPILED_MANIFEST_SHA256: Option<&str> =
    option_env!("HEADROOM_LINUX_RUNTIME_MANIFEST_SHA256");
#[cfg(target_os = "macos")]
const RUNTIME_PLATFORM: &str = "darwin-arm64";
#[cfg(target_os = "linux")]
const RUNTIME_PLATFORM: &str = "linux-x86_64";
const MAX_PAYLOAD_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const MAX_ENTRIES: usize = 100_000;
const OVERLAYS: [&str; 4] = [
    "cache_control_shim.py",
    "macos_engine.py",
    "optimization-sources.json",
    "optimization_control.py",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MacRuntimeError {
    Unavailable,
    InvalidManifest,
    InvalidProfile,
    Integrity,
    UnsafeState,
    Busy,
    Spawn,
    NotReady,
    OwnershipLost,
    Control,
    Cleanup,
}

#[derive(Deserialize, Eq, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum Entry {
    File {
        mode: u32,
        size: u64,
        sha256: String,
    },
    Directory {
        mode: u32,
    },
    Symlink {
        target: String,
    },
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    schema_version: u8,
    platform: String,
    engine_version: String,
    python_version: String,
    dependency_lock_sha256: String,
    python: String,
    site_packages: String,
    root_mode: u32,
    entries: BTreeMap<String, Entry>,
    overlays: BTreeMap<String, String>,
}

pub(crate) struct MacRuntime {
    pub root: PathBuf,
    pub manifest_sha256: String,
}

impl MacRuntime {
    pub(crate) fn open(root: &Path) -> Result<Self, MacRuntimeError> {
        let expected = COMPILED_MANIFEST_SHA256.ok_or(MacRuntimeError::Unavailable)?;
        verify(root, expected)
    }

    pub(crate) fn python(&self) -> PathBuf {
        self.root.join("payload/python/bin/python3.13")
    }
}

fn verify(root: &Path, expected: &str) -> Result<MacRuntime, MacRuntimeError> {
    if !valid_hash(expected)
        || !root.is_absolute()
        || fs::canonicalize(root).ok().as_deref() != Some(root)
    {
        return Err(MacRuntimeError::InvalidManifest);
    }
    let root_info = trusted_metadata(root)?;
    if !root_info.is_dir() {
        return Err(MacRuntimeError::InvalidManifest);
    }
    let allowed: BTreeSet<_> = OVERLAYS
        .iter()
        .copied()
        .chain(["manifest.json", "payload"])
        .map(str::to_owned)
        .collect();
    let actual: BTreeSet<_> = fs::read_dir(root)
        .map_err(|_| MacRuntimeError::Integrity)?
        .map(|item| {
            item.map_err(|_| MacRuntimeError::Integrity)
                .and_then(|item| {
                    item.file_name()
                        .into_string()
                        .map_err(|_| MacRuntimeError::Integrity)
                })
        })
        .collect::<Result<_, _>>()?;
    if actual != allowed {
        return Err(MacRuntimeError::Integrity);
    }
    let manifest_path = root.join("manifest.json");
    let info = trusted_metadata(&manifest_path)?;
    if !info.is_file() || info.len() > 16 * 1024 * 1024 || info.nlink() != 1 {
        return Err(MacRuntimeError::InvalidManifest);
    }
    let bytes = read_regular(&manifest_path, &info, 16 * 1024 * 1024)?;
    if format!("{:x}", Sha256::digest(&bytes)) != expected {
        return Err(MacRuntimeError::Integrity);
    }
    let manifest: Manifest =
        serde_json::from_slice(&bytes).map_err(|_| MacRuntimeError::InvalidManifest)?;
    if manifest.schema_version != 1
        || manifest.platform != RUNTIME_PLATFORM
        || manifest.engine_version != "0.44.0"
        || manifest.python_version != "3.13.13"
        || manifest.dependency_lock_sha256
            != "e1142cbbeaa139a6a23fcc3b1298af5a38f1f33b03d4833e744631c3015de3e7"
        || manifest.python != "python/bin/python3.13"
        || manifest.site_packages != "site-packages"
        || manifest.entries.is_empty()
        || manifest.entries.len() > MAX_ENTRIES
        || manifest.overlays.len() != OVERLAYS.len()
        || !OVERLAYS
            .iter()
            .all(|name| manifest.overlays.contains_key(*name))
    {
        return Err(MacRuntimeError::InvalidManifest);
    }
    let payload = root.join("payload");
    let info = trusted_metadata(&payload)?;
    if !info.is_dir() || info.mode() & 0o7777 != manifest.root_mode {
        return Err(MacRuntimeError::Integrity);
    }
    let mut observed = BTreeMap::new();
    let mut pending = vec![payload.clone()];
    let mut directories = vec![(root.to_owned(), identity(&root_info))];
    let mut total_bytes = 0u64;
    while let Some(directory) = pending.pop() {
        let before = trusted_metadata(&directory)?;
        if !before.is_dir() {
            return Err(MacRuntimeError::Integrity);
        }
        directories.push((directory.clone(), identity(&before)));
        for item in fs::read_dir(&directory).map_err(|_| MacRuntimeError::Integrity)? {
            let path = item.map_err(|_| MacRuntimeError::Integrity)?.path();
            let name = path
                .strip_prefix(&payload)
                .ok()
                .and_then(|v| v.to_str())
                .ok_or(MacRuntimeError::Integrity)?;
            if !Path::new(name)
                .components()
                .all(|v| matches!(v, Component::Normal(_)))
            {
                return Err(MacRuntimeError::Integrity);
            }
            let info = trusted_metadata(&path)?;
            let entry = if info.is_dir() {
                pending.push(path.clone());
                Entry::Directory {
                    mode: info.mode() & 0o7777,
                }
            } else if info.is_file() {
                total_bytes = total_bytes
                    .checked_add(info.len())
                    .ok_or(MacRuntimeError::Integrity)?;
                if total_bytes > MAX_PAYLOAD_BYTES || info.nlink() != 1 {
                    return Err(MacRuntimeError::Integrity);
                }
                Entry::File {
                    mode: info.mode() & 0o7777,
                    size: info.len(),
                    sha256: hash_regular(&path, &info)?,
                }
            } else if info.file_type().is_symlink() {
                let target = fs::read_link(&path).map_err(|_| MacRuntimeError::Integrity)?;
                let resolved = fs::canonicalize(&path).map_err(|_| MacRuntimeError::Integrity)?;
                if target.is_absolute() || !resolved.starts_with(&payload) || !resolved.is_file() {
                    return Err(MacRuntimeError::Integrity);
                }
                Entry::Symlink {
                    target: target
                        .to_str()
                        .ok_or(MacRuntimeError::Integrity)?
                        .to_owned(),
                }
            } else {
                return Err(MacRuntimeError::Integrity);
            };
            if observed.insert(name.to_owned(), entry).is_some() || observed.len() > MAX_ENTRIES {
                return Err(MacRuntimeError::Integrity);
            }
        }
        if identity(&trusted_metadata(&directory)?) != identity(&before) {
            return Err(MacRuntimeError::Integrity);
        }
    }
    if observed != manifest.entries {
        return Err(MacRuntimeError::Integrity);
    }
    for name in OVERLAYS {
        let path = root.join(name);
        let info = trusted_metadata(&path)?;
        if !info.is_file()
            || info.nlink() != 1
            || hash_regular(&path, &info)? != manifest.overlays[name]
        {
            return Err(MacRuntimeError::Integrity);
        }
    }
    let python = payload.join(&manifest.python);
    #[cfg(target_os = "macos")]
    let mut header = [0u8; 12];
    #[cfg(target_os = "linux")]
    let mut header = [0u8; 20];
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(python)
        .map_err(|_| MacRuntimeError::Integrity)?;
    file.read_exact(&mut header)
        .map_err(|_| MacRuntimeError::Integrity)?;
    #[cfg(target_os = "macos")]
    let valid_header = header[..4] == [0xcf, 0xfa, 0xed, 0xfe]
        && u32::from_le_bytes(header[4..8].try_into().unwrap()) == 0x0100_000c;
    #[cfg(target_os = "linux")]
    let valid_header = &header[..7] == b"\x7fELF\x02\x01\x01"
        && matches!(u16::from_le_bytes([header[16], header[17]]), 2 | 3)
        && u16::from_le_bytes([header[18], header[19]]) == 62;
    if !valid_header {
        return Err(MacRuntimeError::Integrity);
    }
    for (path, before) in directories {
        if identity(&trusted_metadata(&path)?) != before {
            return Err(MacRuntimeError::Integrity);
        }
    }
    Ok(MacRuntime {
        root: root.to_owned(),
        manifest_sha256: expected.to_owned(),
    })
}

pub(super) fn trusted_metadata(path: &Path) -> Result<fs::Metadata, MacRuntimeError> {
    let info = fs::symlink_metadata(path).map_err(|_| MacRuntimeError::Integrity)?;
    // SAFETY: geteuid has no memory preconditions.
    let uid = unsafe { libc::geteuid() };
    if !info.file_type().is_symlink()
        && ((info.uid() != uid && info.uid() != 0) || info.mode() & 0o6022 != 0)
    {
        return Err(MacRuntimeError::Integrity);
    }
    Ok(info)
}

fn identity(info: &fs::Metadata) -> (u64, u64, u64, i64, i64, i64, i64) {
    (
        info.dev(),
        info.ino(),
        info.len(),
        info.mtime(),
        info.mtime_nsec(),
        info.ctime(),
        info.ctime_nsec(),
    )
}

fn hash_regular(path: &Path, expected: &fs::Metadata) -> Result<String, MacRuntimeError> {
    if !expected.is_file() || expected.len() > MAX_PAYLOAD_BYTES {
        return Err(MacRuntimeError::Integrity);
    }
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
        .map_err(|_| MacRuntimeError::Integrity)?;
    if identity(&file.metadata().map_err(|_| MacRuntimeError::Integrity)?) != identity(expected) {
        return Err(MacRuntimeError::Integrity);
    }
    let mut digest = Sha256::new();
    let mut buffer = vec![0u8; 1024 * 1024];
    loop {
        let count = file
            .read(&mut buffer)
            .map_err(|_| MacRuntimeError::Integrity)?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    if identity(&file.metadata().map_err(|_| MacRuntimeError::Integrity)?) != identity(expected)
        || identity(&trusted_metadata(path)?) != identity(expected)
    {
        return Err(MacRuntimeError::Integrity);
    }
    Ok(format!("{:x}", digest.finalize()))
}

fn read_regular(
    path: &Path,
    expected: &fs::Metadata,
    limit: u64,
) -> Result<Vec<u8>, MacRuntimeError> {
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
        .map_err(|_| MacRuntimeError::Integrity)?;
    if identity(&file.metadata().map_err(|_| MacRuntimeError::Integrity)?) != identity(expected) {
        return Err(MacRuntimeError::Integrity);
    }
    let mut bytes = Vec::new();
    file.take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| MacRuntimeError::Integrity)?;
    if bytes.len() as u64 > limit || identity(&trusted_metadata(path)?) != identity(expected) {
        return Err(MacRuntimeError::Integrity);
    }
    Ok(bytes)
}

fn valid_hash(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|v| v.is_ascii_digit() || (b'a'..=b'f').contains(&v))
}
