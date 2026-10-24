use super::discovery::{DiscoveryManager, InstallationSelectionError, InstallationSummary};
use super::plan::{
    preview_config_with_presence, ClientKind, ConfigPresence, ConfigScope, ConfigSource,
    ConnectionPlan, PlanContext, PlanError, PlanStatus,
};
use crate::runtime::manager::{ReadyStableEndpoint, RuntimeManager, StableEndpointAccessError};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Take};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use uuid::Uuid;

#[path = "config_io/claude_context.rs"]
mod claude_context;
#[path = "config_io/claude_layers.rs"]
mod claude_layers;
#[path = "config_io/codex.rs"]
mod codex;
#[path = "config_io/codex_verify.rs"]
mod codex_verify;
#[path = "connection_actions.rs"]
mod connection_actions;

pub(crate) use claude_context::{
    ClaudeContextErrorDetail, ClaudeContextErrorReply, ClaudeContextReply,
    ClaudeContextSnapshotRequest, ClaudeScopeSelectionRequest,
};

#[cfg(test)]
#[path = "config_io/codex_tests.rs"]
mod codex_tests;

pub(crate) use codex::{
    connection_status as codex_connection_status, failure_reply as codex_status_failure_reply,
    CodexStatusReply, CodexStatusRequest,
};
pub(crate) use codex_verify::{
    failure_reply as codex_verify_failure_reply, CodexVerifyManager, CodexVerifyReply,
    CodexVerifyRequest,
};
#[cfg(test)]
pub(crate) use connection_actions::{
    ActionOwnershipSummary, ConnectionActionSuccess, OwnershipSummary,
};
pub(crate) use connection_actions::{
    ApplyConnectionRequest, ConnectionActionError, ConnectionActionErrorDetail,
    ConnectionActionReply, OwnershipSnapshotRequest, UndoConnectionRequest,
};

pub(crate) fn ownership_store_for_app_local_data(
    app_local_data_dir: Option<PathBuf>,
) -> Result<crate::state::ownership::OwnershipStore, ConfigIoError> {
    if cfg!(debug_assertions) {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let project_root = manifest_dir.parent().ok_or(ConfigIoError::InvalidRoot)?;
        return crate::state::ownership::OwnershipStore::new(
            project_root,
            Path::new("work/app-state/ownership"),
        )
        .map_err(|_| ConfigIoError::Unavailable);
    }

    let target = app_local_data_dir.ok_or(ConfigIoError::Unavailable)?;
    validate_lexical_path(&target)?;
    ensure_fixed_volume(&target)?;
    let mut ancestor = target.clone();
    let mut suffix = Vec::new();
    let boundary = loop {
        match fs::symlink_metadata(&ancestor) {
            Ok(metadata) => {
                if !metadata.is_dir() || is_reparse(&metadata) {
                    return Err(ConfigIoError::UnsafePath);
                }
                let pins = pin_existing_directory_chain(&ancestor)?;
                pins.validate()?;
                let canonical =
                    fs::canonicalize(&ancestor).map_err(|_| ConfigIoError::UnsafePath)?;
                if !same_path(&ancestor, &canonical) {
                    return Err(ConfigIoError::UnsafePath);
                }
                break canonical;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let name = ancestor.file_name().ok_or(ConfigIoError::InvalidRoot)?;
                suffix.push(name.to_os_string());
                ancestor = ancestor
                    .parent()
                    .ok_or(ConfigIoError::InvalidRoot)?
                    .to_path_buf();
            }
            Err(_) => return Err(ConfigIoError::Unavailable),
        }
    };
    let mut relative_root = PathBuf::new();
    for component in suffix.into_iter().rev() {
        relative_root.push(component);
    }
    relative_root.push("ownership");
    crate::state::ownership::OwnershipStore::new(boundary, relative_root)
        .map_err(|_| ConfigIoError::Unavailable)
}

pub(crate) fn claude_context_failure_reply(raw_request_id: String) -> ClaudeContextReply {
    let request_id = if valid_uuid(&raw_request_id) {
        raw_request_id
    } else {
        ZERO_UUID.to_owned()
    };
    ClaudeContextReply::Error(ClaudeContextErrorReply {
        schema_version: 1,
        request_id,
        status: "error",
        error: ClaudeContextErrorDetail {
            code: "unavailable",
            retryable: true,
            message_key: "connection.config_unavailable",
            diagnostic_id: None,
        },
    })
}

pub(crate) fn claude_settings_snapshot_failure_reply(
    raw_request_id: String,
) -> ClaudeSettingsSnapshotReply {
    let request_id = if valid_uuid(&raw_request_id) {
        raw_request_id
    } else {
        ZERO_UUID.to_owned()
    };
    ClaudeSettingsSnapshotReply::Error(ConnectionActionError {
        schema_version: 1,
        request_id,
        status: "error",
        error: ConnectionActionErrorDetail {
            code: "config_unavailable",
            retryable: true,
            message_key: "connection.config_unavailable",
            diagnostic_id: None,
        },
    })
}

const MAX_CONFIG_BYTES: usize = 256 * 1024;
#[cfg(windows)]
const MAX_CONFIG_PATH_UNITS: usize = 32_767;
const MAX_PLANS: usize = 64;
const PLAN_TTL_SECONDS: u64 = 300;
const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;
const ZERO_UUID: &str = "00000000-0000-0000-0000-000000000000";

#[cfg(windows)]
use std::os::windows::ffi::OsStrExt;
#[cfg(windows)]
use std::os::windows::fs::OpenOptionsExt;
#[cfg(windows)]
use std::os::windows::io::{AsRawHandle, RawHandle};
#[cfg(windows)]
use std::os::windows::io::{FromRawHandle, OwnedHandle};
#[cfg(windows)]
use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
#[cfg(windows)]
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, GetDriveTypeW, GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION,
    FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_REPARSE_POINT, FILE_FLAG_BACKUP_SEMANTICS,
    FILE_FLAG_OPEN_REPARSE_POINT, FILE_FLAG_SEQUENTIAL_SCAN, FILE_LIST_DIRECTORY,
    FILE_READ_ATTRIBUTES, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING, SYNCHRONIZE,
};
#[cfg(windows)]
use windows_sys::Win32::System::WindowsProgramming::DRIVE_FIXED;

#[derive(Clone, Debug, Default)]
pub(super) struct ConfigRootInputs {
    pub codex_home: Option<PathBuf>,
    pub claude_config_dir: Option<PathBuf>,
    pub claude_project_directory: Option<PathBuf>,
    pub xdg_config_home: Option<PathBuf>,
    pub user_profile: Option<PathBuf>,
    #[cfg(test)]
    pub claude_test_managed_settings: Option<PathBuf>,
    #[cfg(test)]
    pub claude_test_route_environment: Option<Vec<(String, String)>>,
}

impl ConfigRootInputs {
    fn from_environment() -> Self {
        Self {
            codex_home: env::var_os("CODEX_HOME").map(PathBuf::from),
            claude_config_dir: env::var_os("CLAUDE_CONFIG_DIR").map(PathBuf::from),
            claude_project_directory: None,
            xdg_config_home: env::var_os("XDG_CONFIG_HOME").map(PathBuf::from),
            user_profile: env::var_os("USERPROFILE").map(PathBuf::from),
            #[cfg(test)]
            claude_test_managed_settings: None,
            #[cfg(test)]
            claude_test_route_environment: None,
        }
    }

    fn target(&self, kind: ClientKind) -> Result<ConfigTarget, ConfigIoError> {
        let profile = || self.user_profile.clone().ok_or(ConfigIoError::InvalidRoot);
        match kind {
            ClientKind::Codex => {
                let root = match &self.codex_home {
                    Some(value) => value.clone(),
                    None => profile()?.join(".codex"),
                };
                Ok(ConfigTarget::Single {
                    path: root.join("config.toml"),
                    source: ConfigSource::CodexToml,
                })
            }
            ClientKind::ClaudeCode => {
                if let Some(project_directory) = &self.claude_project_directory {
                    let project_config_directory = project_directory.join(".claude");
                    match fs::symlink_metadata(&project_config_directory) {
                        Ok(metadata) if metadata.is_dir() && !is_reparse(&metadata) => {}
                        Ok(_) => return Err(ConfigIoError::UnsafePath),
                        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                            return Err(ConfigIoError::Unavailable)
                        }
                        Err(_) => return Err(ConfigIoError::Unavailable),
                    }
                    return Ok(ConfigTarget::Single {
                        path: project_config_directory.join("settings.local.json"),
                        source: ConfigSource::ClaudeJson,
                    });
                }
                let root = match &self.claude_config_dir {
                    Some(value) => value.clone(),
                    None => profile()?.join(".claude"),
                };
                Ok(ConfigTarget::Single {
                    path: root.join("settings.json"),
                    source: ConfigSource::ClaudeJson,
                })
            }
            ClientKind::OpenCodeV1 => {
                let root = match &self.xdg_config_home {
                    Some(value) => value.clone(),
                    None => profile()?.join(".config"),
                }
                .join("opencode");
                Ok(ConfigTarget::OpenCode {
                    json: root.join("opencode.json"),
                    jsonc: root.join("opencode.jsonc"),
                })
            }
        }
    }
}

#[derive(Clone, Debug)]
enum ConfigTarget {
    Single { path: PathBuf, source: ConfigSource },
    OpenCode { json: PathBuf, jsonc: PathBuf },
}

#[derive(Clone, Debug)]
pub(super) struct ConfigRead {
    pub bytes: Vec<u8>,
    pub presence: ConfigPresence,
    pub source: ConfigSource,
    pub fingerprint: String,
}

/// A bounded Codex config observation whose file and parent-directory handles
/// remain open while the caller reads the corresponding ownership record.
/// The held handles prevent ordinary writes, deletes, and path replacement
/// during that shared observation window.
pub(super) struct PinnedCodexConfigRead {
    pub presence: ConfigPresence,
    pub bytes: Vec<u8>,
    pin: CandidatePin,
}

impl PinnedCodexConfigRead {
    pub(super) fn validate(&self) -> Result<(), ConfigIoError> {
        self.pin.validate()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ConfigIoError {
    InvalidRoot,
    UnsafePath,
    Unavailable,
    TooLarge,
    Ambiguous,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ClaudeSettingsSnapshotRequest {
    pub schema_version: u8,
    pub request_id: String,
    pub scope: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(untagged)]
pub(crate) enum ClaudeSettingsSnapshotReply {
    Snapshot(claude_layers::ClaudeSettingsSnapshot),
    Error(connection_actions::ConnectionActionError),
}

/// A crate-internal pin for an existing directory chain. It keeps the same
/// no-delete directory handles used by config reads/writes alive while an
/// application-owned state child is created.
pub(crate) struct PinnedDirectoryChain {
    pins: DirectoryPins,
}

pub(crate) fn pin_existing_directory_chain(
    directory: &Path,
) -> Result<PinnedDirectoryChain, ConfigIoError> {
    let placeholder = directory.join(".headroom-directory-pin");
    validate_lexical_path(&placeholder)?;
    let pins = pin_parent_directories(&placeholder)?;
    validate_pinned_directories(&pins)?;
    Ok(PinnedDirectoryChain { pins })
}

impl PinnedDirectoryChain {
    pub(crate) fn validate(&self) -> Result<(), ConfigIoError> {
        validate_pinned_directories(&self.pins)
    }
}

pub(super) fn read_user_config(
    kind: ClientKind,
    inputs: &ConfigRootInputs,
) -> Result<ConfigRead, ConfigIoError> {
    let target = inputs.target(kind)?;
    let (source, bytes, presence, identity, stable_path) = match target {
        ConfigTarget::Single { path, source } => match read_candidate(&path)? {
            CandidateRead::Missing { target_path, .. } => (
                source,
                empty_document(kind),
                ConfigPresence::Missing,
                None,
                target_path,
            ),
            CandidateRead::Present {
                bytes,
                identity,
                target_path,
                ..
            } => (
                source,
                bytes,
                ConfigPresence::Present,
                Some(identity),
                target_path,
            ),
        },
        ConfigTarget::OpenCode { json, jsonc } => {
            let first = read_candidate(&json)?;
            let second = read_candidate(&jsonc)?;
            match (first, second) {
                (CandidateRead::Present { .. }, CandidateRead::Present { .. }) => {
                    return Err(ConfigIoError::Ambiguous)
                }
                (
                    CandidateRead::Present {
                        bytes,
                        identity,
                        target_path,
                        ..
                    },
                    CandidateRead::Missing { .. },
                ) => (
                    ConfigSource::OpenCodeJson,
                    bytes,
                    ConfigPresence::Present,
                    Some(identity),
                    target_path,
                ),
                (
                    CandidateRead::Missing { .. },
                    CandidateRead::Present {
                        bytes,
                        identity,
                        target_path,
                        ..
                    },
                ) => (
                    ConfigSource::OpenCodeJsonc,
                    bytes,
                    ConfigPresence::Present,
                    Some(identity),
                    target_path,
                ),
                (CandidateRead::Missing { .. }, CandidateRead::Missing { .. }) => (
                    ConfigSource::OpenCodeMissing,
                    empty_document(kind),
                    ConfigPresence::Missing,
                    None,
                    json,
                ),
            }
        }
    };
    let identity = identity.map(|value| (value.volume, value.high, value.low));
    let fingerprint = super::plan::fingerprint_config(
        kind,
        source,
        presence,
        &bytes,
        identity,
        Some(&stable_path),
    );
    Ok(ConfigRead {
        bytes,
        presence,
        source,
        fingerprint,
    })
}

pub(super) fn read_codex_config_pinned(
    inputs: &ConfigRootInputs,
) -> Result<PinnedCodexConfigRead, ConfigIoError> {
    let ConfigTarget::Single { path, source } = inputs.target(ClientKind::Codex)? else {
        return Err(ConfigIoError::InvalidRoot);
    };
    if source != ConfigSource::CodexToml {
        return Err(ConfigIoError::InvalidRoot);
    }
    match read_candidate(&path)? {
        CandidateRead::Present { bytes, pin, .. } => Ok(PinnedCodexConfigRead {
            presence: ConfigPresence::Present,
            bytes,
            pin,
        }),
        CandidateRead::Missing { pin: Some(pin), .. } => Ok(PinnedCodexConfigRead {
            presence: ConfigPresence::Missing,
            bytes: Vec::new(),
            pin,
        }),
        CandidateRead::Missing { pin: None, .. } => Err(ConfigIoError::Unavailable),
    }
}

fn empty_document(kind: ClientKind) -> Vec<u8> {
    match kind {
        ClientKind::Codex => Vec::new(),
        ClientKind::ClaudeCode | ClientKind::OpenCodeV1 => b"{}".to_vec(),
    }
}

enum CandidateRead {
    Missing {
        target_path: PathBuf,
        pin: Option<CandidatePin>,
    },
    Present {
        bytes: Vec<u8>,
        identity: FileIdentity,
        target_path: PathBuf,
        pin: CandidatePin,
    },
}

struct CandidatePin {
    path: PathBuf,
    expected_path: Option<PathBuf>,
    parent_pins: Option<DirectoryPins>,
    file: Option<File>,
    identity: Option<FileIdentity>,
}

impl CandidatePin {
    fn missing(path: &Path, parent_pins: DirectoryPins) -> Self {
        Self {
            path: path.to_path_buf(),
            expected_path: None,
            parent_pins: Some(parent_pins),
            file: None,
            identity: None,
        }
    }

    fn present(
        path: &Path,
        expected_path: PathBuf,
        parent_pins: DirectoryPins,
        file: File,
        identity: FileIdentity,
    ) -> Self {
        Self {
            path: path.to_path_buf(),
            expected_path: Some(expected_path),
            parent_pins: Some(parent_pins),
            file: Some(file),
            identity: Some(identity),
        }
    }

    fn validate(&self) -> Result<(), ConfigIoError> {
        let Some(parent_pins) = self.parent_pins.as_ref() else {
            return Err(ConfigIoError::Unavailable);
        };
        validate_pinned_directories(parent_pins)?;
        let parent = self.path.parent().ok_or(ConfigIoError::UnsafePath)?;
        let canonical_parent = fs::canonicalize(parent).map_err(|_| ConfigIoError::UnsafePath)?;
        if !same_path(parent, &canonical_parent) {
            return Err(ConfigIoError::UnsafePath);
        }

        match (&self.file, self.identity, &self.expected_path) {
            (Some(file), Some(expected_identity), Some(expected_path)) => {
                let metadata =
                    fs::symlink_metadata(&self.path).map_err(|_| ConfigIoError::Unavailable)?;
                if !metadata.is_file() || is_reparse(&metadata) {
                    return Err(ConfigIoError::UnsafePath);
                }
                let current_identity = file_identity(file)?;
                if !same_file_object(expected_identity, current_identity)
                    || !handle_is_regular_file(current_identity.attributes)
                    || !same_path(
                        &fs::canonicalize(&self.path).map_err(|_| ConfigIoError::UnsafePath)?,
                        expected_path,
                    )
                {
                    return Err(ConfigIoError::UnsafePath);
                }
            }
            (None, None, None) => match fs::symlink_metadata(&self.path) {
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Ok(_) | Err(_) => return Err(ConfigIoError::Unavailable),
            },
            _ => return Err(ConfigIoError::Unavailable),
        }
        Ok(())
    }
}

fn read_candidate(path: &Path) -> Result<CandidateRead, ConfigIoError> {
    validate_lexical_path(path)?;
    ensure_fixed_volume(path)?;
    let Some(_) = validate_path_chain(path)? else {
        return read_missing_candidate(path);
    };
    let parent_pins = pin_parent_directories(path)?;
    let Some(canonical_parent) = validate_path_chain(path)? else {
        return read_missing_candidate(path);
    };
    let parent = path.parent().ok_or(ConfigIoError::UnsafePath)?;
    let name = path.file_name().ok_or(ConfigIoError::UnsafePath)?;
    let expected_path = canonical_parent.join(name);
    if !same_path(path.parent().unwrap_or(path), &canonical_parent) {
        return Err(ConfigIoError::UnsafePath);
    }

    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return read_missing_candidate(path)
        }
        Err(_) => return Err(ConfigIoError::Unavailable),
    };
    if !metadata.is_file() || is_reparse(&metadata) {
        return Err(ConfigIoError::UnsafePath);
    }
    if metadata.len() > MAX_CONFIG_BYTES as u64 {
        return Err(ConfigIoError::TooLarge);
    }

    let mut file = open_read_shared(path)?;
    let first_identity = file_identity(&file)?;
    validate_pinned_directories(&parent_pins)?;
    if !handle_is_regular_file(first_identity.attributes) {
        return Err(ConfigIoError::UnsafePath);
    }
    let canonical_file = fs::canonicalize(path).map_err(|_| ConfigIoError::UnsafePath)?;
    if !same_path(&canonical_file, &expected_path) || !same_path(parent, &canonical_parent) {
        return Err(ConfigIoError::UnsafePath);
    }
    let handle_len = file
        .metadata()
        .map_err(|_| ConfigIoError::Unavailable)?
        .len();
    if handle_len > MAX_CONFIG_BYTES as u64 {
        return Err(ConfigIoError::TooLarge);
    }
    let mut limited: Take<&mut File> = file.by_ref().take((MAX_CONFIG_BYTES + 1) as u64);
    let mut bytes = Vec::with_capacity(handle_len as usize);
    limited
        .read_to_end(&mut bytes)
        .map_err(|_| ConfigIoError::Unavailable)?;
    if bytes.len() > MAX_CONFIG_BYTES {
        return Err(ConfigIoError::TooLarge);
    }

    let final_metadata = fs::symlink_metadata(path).map_err(|_| ConfigIoError::Unavailable)?;
    if !final_metadata.is_file() || is_reparse(&final_metadata) {
        return Err(ConfigIoError::UnsafePath);
    }
    let verification_file = open_read_shared(path)?;
    let final_identity = file_identity(&verification_file)?;
    if !same_file_object(first_identity, final_identity)
        || validate_pinned_directories(&parent_pins).is_err()
        || !handle_is_regular_file(final_identity.attributes)
        || !same_path(
            &fs::canonicalize(path).map_err(|_| ConfigIoError::UnsafePath)?,
            &expected_path,
        )
    {
        return Err(ConfigIoError::UnsafePath);
    }
    Ok(CandidateRead::Present {
        bytes,
        identity: first_identity,
        target_path: canonical_file,
        pin: CandidatePin::present(path, expected_path, parent_pins, file, first_identity),
    })
}

fn read_missing_candidate(path: &Path) -> Result<CandidateRead, ConfigIoError> {
    let parent = path.parent().ok_or(ConfigIoError::UnsafePath)?;
    let parent_metadata = match fs::symlink_metadata(parent) {
        Ok(metadata) if metadata.is_dir() && !is_reparse(&metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(CandidateRead::Missing {
                target_path: path.to_path_buf(),
                pin: None,
            })
        }
        Ok(_) => return Err(ConfigIoError::UnsafePath),
        Err(_) => return Err(ConfigIoError::Unavailable),
    };
    let _ = parent_metadata;
    let parent_pins = pin_parent_directories(path)?;
    validate_pinned_directories(&parent_pins)?;
    let canonical_parent = fs::canonicalize(parent).map_err(|_| ConfigIoError::UnsafePath)?;
    if !same_path(parent, &canonical_parent) {
        return Err(ConfigIoError::UnsafePath);
    }
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(CandidateRead::Missing {
            target_path: path.to_path_buf(),
            pin: Some(CandidatePin::missing(path, parent_pins)),
        }),
        Ok(_) => Err(ConfigIoError::Unavailable),
        Err(_) => Err(ConfigIoError::Unavailable),
    }
}

pub(super) fn validate_lexical_path(path: &Path) -> Result<(), ConfigIoError> {
    if !path.is_absolute() {
        return Err(ConfigIoError::InvalidRoot);
    }
    let mut saw_disk = false;
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => {
                if !matches!(prefix.kind(), std::path::Prefix::Disk(_)) {
                    return Err(ConfigIoError::InvalidRoot);
                }
                saw_disk = true;
            }
            Component::RootDir => {}
            Component::Normal(value) => {
                if value.is_empty() || value.to_string_lossy().contains(':') {
                    return Err(ConfigIoError::UnsafePath);
                }
            }
            Component::CurDir | Component::ParentDir => return Err(ConfigIoError::UnsafePath),
        }
    }
    if !saw_disk {
        return Err(ConfigIoError::InvalidRoot);
    }
    #[cfg(windows)]
    {
        let wide: Vec<u16> = path.as_os_str().encode_wide().collect();
        if wide.len() > MAX_CONFIG_PATH_UNITS || wide.contains(&0) {
            return Err(ConfigIoError::UnsafePath);
        }
    }
    Ok(())
}

fn validate_path_chain(path: &Path) -> Result<Option<PathBuf>, ConfigIoError> {
    let mut current = volume_root(path)?;
    let mut components = path
        .components()
        .filter_map(|component| match component {
            Component::Normal(name) => Some(name.to_os_string()),
            _ => None,
        })
        .peekable();
    while let Some(name) = components.next() {
        current.push(name);
        match fs::symlink_metadata(&current) {
            Ok(metadata) => {
                if is_reparse(&metadata) {
                    return Err(ConfigIoError::UnsafePath);
                }
                let is_final = components.peek().is_none();
                if !is_final && !metadata.is_dir() {
                    return Err(ConfigIoError::UnsafePath);
                }
                if is_final && !metadata.is_file() {
                    return Err(ConfigIoError::UnsafePath);
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err(ConfigIoError::Unavailable),
        }
    }
    let parent = path.parent().ok_or(ConfigIoError::UnsafePath)?;
    let canonical_parent = fs::canonicalize(parent).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            ConfigIoError::Unavailable
        } else {
            ConfigIoError::UnsafePath
        }
    })?;
    if !same_path(parent, &canonical_parent) {
        return Err(ConfigIoError::UnsafePath);
    }
    Ok(Some(canonical_parent))
}

#[cfg(windows)]
pub(super) type DirectoryPins = Vec<DirectoryPin>;
#[cfg(not(windows))]
pub(super) type DirectoryPins = ();

#[cfg(windows)]
pub(super) struct DirectoryPin {
    path: PathBuf,
    volume: u32,
    high: u32,
    low: u32,
    attributes: u32,
    _handle: OwnedHandle,
}

#[cfg(windows)]
impl DirectoryPin {
    pub(super) fn raw_handle(&self) -> RawHandle {
        self._handle.as_raw_handle()
    }

    pub(super) fn attributes(&self) -> u32 {
        self.attributes
    }
}

pub(super) fn pin_parent_directories(path: &Path) -> Result<DirectoryPins, ConfigIoError> {
    #[cfg(windows)]
    {
        let mut current = volume_root(path)?;
        let mut pins = Vec::new();
        pins.push(open_pinned_directory(&current)?);
        let parent = path.parent().ok_or(ConfigIoError::UnsafePath)?;
        for component in parent.components() {
            if let Component::Normal(part) = component {
                current.push(part);
                pins.push(open_pinned_directory(&current)?);
            }
        }
        Ok(pins)
    }
    #[cfg(not(windows))]
    {
        let _ = path;
        Ok(())
    }
}

#[cfg(windows)]
fn open_pinned_directory(path: &Path) -> Result<DirectoryPin, ConfigIoError> {
    use std::ptr::null_mut;

    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    let raw = unsafe {
        CreateFileW(
            wide.as_ptr(),
            FILE_LIST_DIRECTORY | FILE_READ_ATTRIBUTES | SYNCHRONIZE,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            null_mut(),
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT,
            null_mut(),
        )
    };
    if raw == INVALID_HANDLE_VALUE || raw.is_null() {
        return Err(ConfigIoError::UnsafePath);
    }
    let handle = unsafe { OwnedHandle::from_raw_handle(raw as RawHandle) };
    let mut info = BY_HANDLE_FILE_INFORMATION::default();
    let ok = unsafe { GetFileInformationByHandle(handle.as_raw_handle(), &mut info) };
    if ok == 0
        || info.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY == 0
        || info.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT != 0
    {
        return Err(ConfigIoError::UnsafePath);
    }
    Ok(DirectoryPin {
        path: path.to_path_buf(),
        volume: info.dwVolumeSerialNumber,
        high: info.nFileIndexHigh,
        low: info.nFileIndexLow,
        attributes: info.dwFileAttributes,
        _handle: handle,
    })
}

#[cfg(windows)]
pub(super) fn validate_pinned_directories(pins: &[DirectoryPin]) -> Result<(), ConfigIoError> {
    for pin in pins {
        let current = open_pinned_directory(&pin.path)?;
        if pin.volume != current.volume || pin.high != current.high || pin.low != current.low {
            return Err(ConfigIoError::UnsafePath);
        }
    }
    Ok(())
}

#[cfg(not(windows))]
fn validate_pinned_directories(_: &DirectoryPins) -> Result<(), ConfigIoError> {
    Ok(())
}

fn volume_root(path: &Path) -> Result<PathBuf, ConfigIoError> {
    let mut components = path.components();
    let Some(Component::Prefix(prefix)) = components.next() else {
        return Err(ConfigIoError::InvalidRoot);
    };
    if !matches!(prefix.kind(), std::path::Prefix::Disk(_))
        || !matches!(components.next(), Some(Component::RootDir))
    {
        return Err(ConfigIoError::InvalidRoot);
    }
    let mut root = PathBuf::from(prefix.as_os_str());
    root.push("\\");
    Ok(root)
}

pub(super) fn ensure_fixed_volume(path: &Path) -> Result<(), ConfigIoError> {
    #[cfg(windows)]
    {
        let root = volume_root(path)?;
        let wide: Vec<u16> = root.as_os_str().encode_wide().chain(Some(0)).collect();
        let drive_type = unsafe { GetDriveTypeW(wide.as_ptr()) };
        if drive_type != DRIVE_FIXED {
            return Err(ConfigIoError::InvalidRoot);
        }
    }
    #[cfg(not(windows))]
    let _ = path;
    Ok(())
}

fn is_reparse(metadata: &fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    }
    #[cfg(not(windows))]
    {
        false
    }
}

pub(super) fn same_path(left: &Path, right: &Path) -> bool {
    #[cfg(windows)]
    {
        path_key(left).eq_ignore_ascii_case(&path_key(right))
    }
    #[cfg(not(windows))]
    {
        left == right
    }
}

#[cfg(windows)]
fn path_key(path: &Path) -> String {
    let text = path.to_string_lossy();
    text.strip_prefix(r"\\?\").unwrap_or(&text).to_owned()
}

pub(super) fn open_read_shared(path: &Path) -> Result<File, ConfigIoError> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(windows)]
    options
        .share_mode(FILE_SHARE_READ)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_SEQUENTIAL_SCAN);
    options.open(path).map_err(|_| ConfigIoError::Unavailable)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FileIdentity {
    volume: u32,
    high: u32,
    low: u32,
    attributes: u32,
}

fn file_identity(file: &File) -> Result<FileIdentity, ConfigIoError> {
    #[cfg(windows)]
    {
        let mut info = BY_HANDLE_FILE_INFORMATION::default();
        let ok = unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut info) };
        if ok == 0 {
            return Err(ConfigIoError::Unavailable);
        }
        Ok(FileIdentity {
            volume: info.dwVolumeSerialNumber,
            high: info.nFileIndexHigh,
            low: info.nFileIndexLow,
            attributes: info.dwFileAttributes,
        })
    }
    #[cfg(not(windows))]
    {
        let _ = file;
        Ok(FileIdentity {
            volume: 0,
            high: 0,
            low: 0,
            attributes: 0,
        })
    }
}

fn same_file_object(left: FileIdentity, right: FileIdentity) -> bool {
    left.volume == right.volume && left.high == right.high && left.low == right.low
}

fn handle_is_regular_file(attributes: u32) -> bool {
    #[cfg(windows)]
    {
        attributes & FILE_ATTRIBUTE_REPARSE_POINT == 0 && attributes & FILE_ATTRIBUTE_DIRECTORY == 0
    }
    #[cfg(not(windows))]
    {
        let _ = attributes;
        true
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PreviewRequest {
    pub schema_version: u8,
    pub request_id: String,
    pub installation_id: String,
    pub scan_generation: u64,
    pub scope: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(untagged)]
pub(crate) enum PlanReply {
    Preview(PlanPreviewReply),
    Error(PlanErrorReply),
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct PlanPreviewReply {
    pub schema_version: u8,
    pub request_id: String,
    pub status: &'static str,
    pub plan: ConnectionPlan,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct PlanErrorReply {
    pub schema_version: u8,
    pub request_id: String,
    pub status: &'static str,
    pub error: PlanReplyError,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct PlanReplyError {
    pub code: &'static str,
    pub retryable: bool,
    pub message_key: &'static str,
    pub diagnostic_id: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ReplyError {
    Validation,
    UnsupportedScope,
    StaleSelection,
    NotFound,
    EndpointUnavailable,
    ConfigUnavailable,
    ConfigUnsafe,
    ConfigTooLarge,
    ConfigConflict,
    ConfigInvalid,
    PlanCapacity,
}

impl ReplyError {
    fn detail(self) -> PlanReplyError {
        let (code, retryable, message_key) = match self {
            Self::Validation => ("validation_failed", false, "connection.validation_failed"),
            Self::UnsupportedScope => ("unsupported_scope", false, "connection.unsupported_scope"),
            Self::StaleSelection => ("stale_selection", true, "connection.stale_selection"),
            Self::NotFound => ("not_found", false, "connection.not_found"),
            Self::EndpointUnavailable => (
                "endpoint_unavailable",
                true,
                "connection.endpoint_unavailable",
            ),
            Self::ConfigUnavailable => {
                ("config_unavailable", true, "connection.config_unavailable")
            }
            Self::ConfigUnsafe => ("config_unsafe", false, "connection.config_unsafe"),
            Self::ConfigTooLarge => ("config_too_large", false, "connection.config_too_large"),
            Self::ConfigConflict => ("config_conflict", true, "connection.config_conflict"),
            Self::ConfigInvalid => ("config_invalid", false, "connection.config_invalid"),
            Self::PlanCapacity => ("busy", true, "connection.busy"),
        };
        PlanReplyError {
            code,
            retryable,
            message_key,
            diagnostic_id: None,
        }
    }
}

#[derive(Clone)]
struct PlanLease {
    plan: ConnectionPlan,
    expires: Instant,
    installation: InstallationSummary,
    endpoint: ReadyStableEndpoint,
    roots: ConfigRootInputs,
    claude_layers_fingerprint: Option<String>,
}

pub(crate) struct PlanManager {
    discovery: DiscoveryManager,
    claude_context: claude_context::ClaudeContextManager,
    roots_provider: Arc<dyn Fn() -> ConfigRootInputs + Send + Sync>,
    endpoint_provider:
        Arc<dyn Fn() -> Result<ReadyStableEndpoint, StableEndpointAccessError> + Send + Sync>,
    leases: Mutex<HashMap<String, PlanLease>>,
    preview_in_flight: AtomicBool,
    ownership_store: Option<crate::state::ownership::OwnershipStore>,
    #[cfg(test)]
    after_first_read: Option<Arc<dyn Fn() + Send + Sync>>,
    #[cfg(test)]
    after_codex_config_rollback: Option<Arc<dyn Fn() + Send + Sync>>,
    #[cfg(test)]
    fail_action_before_commit: bool,
    #[cfg(test)]
    fail_undo_before_commit: bool,
}

impl PlanManager {
    pub(crate) fn new(discovery: DiscoveryManager, runtime: RuntimeManager) -> Self {
        Self {
            discovery,
            claude_context: claude_context::ClaudeContextManager::new(),
            roots_provider: Arc::new(ConfigRootInputs::from_environment),
            endpoint_provider: Arc::new(move || runtime.ready_stable_endpoint()),
            leases: Mutex::new(HashMap::new()),
            preview_in_flight: AtomicBool::new(false),
            ownership_store: None,
            #[cfg(test)]
            after_first_read: None,
            #[cfg(test)]
            after_codex_config_rollback: None,
            #[cfg(test)]
            fail_action_before_commit: false,
            #[cfg(test)]
            fail_undo_before_commit: false,
        }
    }

    #[cfg(test)]
    pub(super) fn with_test_sources(
        discovery: DiscoveryManager,
        roots: ConfigRootInputs,
        endpoint: Result<ReadyStableEndpoint, StableEndpointAccessError>,
    ) -> Self {
        Self::with_test_sources_fn(discovery, roots, move || endpoint.clone())
    }

    #[cfg(test)]
    pub(super) fn with_test_sources_fn<F>(
        discovery: DiscoveryManager,
        roots: ConfigRootInputs,
        endpoint_provider: F,
    ) -> Self
    where
        F: Fn() -> Result<ReadyStableEndpoint, StableEndpointAccessError> + Send + Sync + 'static,
    {
        Self {
            discovery,
            claude_context: claude_context::ClaudeContextManager::new(),
            roots_provider: Arc::new(move || roots.clone()),
            endpoint_provider: Arc::new(endpoint_provider),
            leases: Mutex::new(HashMap::new()),
            preview_in_flight: AtomicBool::new(false),
            ownership_store: None,
            after_first_read: None,
            after_codex_config_rollback: None,
            fail_action_before_commit: false,
            fail_undo_before_commit: false,
        }
    }

    #[cfg(test)]
    pub(super) fn with_test_providers<R, F>(
        discovery: DiscoveryManager,
        roots_provider: R,
        endpoint_provider: F,
    ) -> Self
    where
        R: Fn() -> ConfigRootInputs + Send + Sync + 'static,
        F: Fn() -> Result<ReadyStableEndpoint, StableEndpointAccessError> + Send + Sync + 'static,
    {
        Self {
            discovery,
            claude_context: claude_context::ClaudeContextManager::new(),
            roots_provider: Arc::new(roots_provider),
            endpoint_provider: Arc::new(endpoint_provider),
            leases: Mutex::new(HashMap::new()),
            preview_in_flight: AtomicBool::new(false),
            ownership_store: None,
            after_first_read: None,
            after_codex_config_rollback: None,
            fail_action_before_commit: false,
            fail_undo_before_commit: false,
        }
    }

    #[cfg(test)]
    pub(super) fn with_test_sources_and_hook<F, H>(
        discovery: DiscoveryManager,
        roots: ConfigRootInputs,
        endpoint_provider: F,
        after_first_read: H,
    ) -> Self
    where
        F: Fn() -> Result<ReadyStableEndpoint, StableEndpointAccessError> + Send + Sync + 'static,
        H: Fn() + Send + Sync + 'static,
    {
        Self {
            discovery,
            claude_context: claude_context::ClaudeContextManager::new(),
            roots_provider: Arc::new(move || roots.clone()),
            endpoint_provider: Arc::new(endpoint_provider),
            leases: Mutex::new(HashMap::new()),
            preview_in_flight: AtomicBool::new(false),
            ownership_store: None,
            after_first_read: Some(Arc::new(after_first_read)),
            after_codex_config_rollback: None,
            fail_action_before_commit: false,
            fail_undo_before_commit: false,
        }
    }

    pub(crate) fn preview(&self, request: PreviewRequest) -> PlanReply {
        let request_id = if valid_uuid(&request.request_id) {
            request.request_id.clone()
        } else {
            ZERO_UUID.to_owned()
        };
        if request.schema_version != 1
            || !valid_uuid(&request.request_id)
            || request.installation_id.len() != 36
            || request.scan_generation == 0
            || request.scan_generation > MAX_SAFE_INTEGER
        {
            return error_reply(request_id, ReplyError::Validation);
        }
        let Some(_preview_guard) = PreviewGuard::try_acquire(&self.preview_in_flight) else {
            return error_reply(request_id, ReplyError::PlanCapacity);
        };

        let selected = match self
            .discovery
            .select_installation(&request.installation_id, request.scan_generation)
        {
            Ok(value) => value,
            Err(error) => return error_reply(request_id, map_selection_error(error)),
        };
        let Some(kind) = parse_client_kind(selected.client_kind) else {
            return error_reply(request_id, ReplyError::NotFound);
        };
        let requested_scope = match request.scope.as_str() {
            "user_config" => ConfigScope::UserConfig,
            "project_local_config" => ConfigScope::ProjectLocalConfig,
            _ => return error_reply(request_id, ReplyError::UnsupportedScope),
        };
        let (roots, selected_scope) = match self.config_roots_for_context(kind) {
            Ok(value) => value,
            Err(error) => return error_reply(request_id, map_config_error(error)),
        };
        if requested_scope != selected_scope || !scope_matches_client(kind, requested_scope) {
            return error_reply(request_id, ReplyError::UnsupportedScope);
        }
        let endpoint = match (self.endpoint_provider)() {
            Ok(value) => value,
            Err(_) => return error_reply(request_id, ReplyError::EndpointUnavailable),
        };
        let first_read = match read_user_config(kind, &roots) {
            Ok(value) => value,
            Err(error) => return error_reply(request_id, map_config_error(error)),
        };
        let now_unix_seconds = match SystemTime::now().duration_since(UNIX_EPOCH) {
            Ok(duration) if duration.as_secs() <= MAX_SAFE_INTEGER - PLAN_TTL_SECONDS => {
                duration.as_secs()
            }
            _ => return error_reply(request_id, ReplyError::Validation),
        };
        let context = PlanContext {
            request_id: request_id.clone(),
            installation_id: selected.installation_id.clone(),
            client_kind: kind,
            config_source: first_read.source,
            surface: selected.surface.to_owned(),
            version: selected.version.clone(),
            scope: selected_scope,
            scan_generation: request.scan_generation,
            endpoint: super::plan::StableEndpoint {
                endpoint_id: endpoint.endpoint_id.clone(),
                host: endpoint.host.to_owned(),
                port: endpoint.port,
            },
            now_unix_seconds,
        };
        let first_claude_layers = if kind == ClientKind::ClaudeCode {
            match claude_layers::observe(&roots, selected_scope, Some(&first_read), &context) {
                Ok(value) => Some(value),
                Err(error) => return error_reply(request_id, map_claude_layers_error(error)),
            }
        } else {
            None
        };
        let mut plan =
            match preview_config_with_presence(&context, &first_read.bytes, first_read.presence) {
                Ok(value) => value,
                Err(error) => return error_reply(request_id, map_plan_error(error)),
            };
        plan.config_fingerprint = first_read.fingerprint.clone();
        if let Some(layers) = &first_claude_layers {
            for blocker in &layers.route_blockers {
                if !plan.blockers.contains(blocker) {
                    plan.blockers.push(*blocker);
                }
            }
            if !plan.blockers.is_empty() {
                plan.status = PlanStatus::Blocked;
                plan.endpoint_field.after_headroom = false;
            }
        }

        #[cfg(test)]
        if let Some(hook) = &self.after_first_read {
            hook();
        }
        let second_read = match read_user_config(kind, &roots) {
            Ok(value) => value,
            Err(error) => return error_reply(request_id, map_config_error(error)),
        };
        let second_claude_layers = if kind == ClientKind::ClaudeCode {
            match claude_layers::observe(&roots, selected_scope, Some(&second_read), &context) {
                Ok(value) => Some(value),
                Err(error) => return error_reply(request_id, map_claude_layers_error(error)),
            }
        } else {
            None
        };
        let current_context = self.config_roots_for_context(kind);
        if second_read.presence != first_read.presence
            || second_read.source != first_read.source
            || second_read.fingerprint != first_read.fingerprint
            || first_claude_layers.as_ref().map(|value| &value.fingerprint)
                != second_claude_layers
                    .as_ref()
                    .map(|value| &value.fingerprint)
            || !current_context
                .as_ref()
                .is_ok_and(|(current_roots, current_scope)| {
                    *current_scope == selected_scope
                        && same_config_target(&roots, current_roots, kind)
                })
            || !same_installation(
                &selected,
                &self
                    .discovery
                    .select_installation(&request.installation_id, request.scan_generation),
            )
            || !same_endpoint(&endpoint, &(self.endpoint_provider)())
        {
            return error_reply(request_id, ReplyError::ConfigConflict);
        }

        if !self.store_lease(
            &plan,
            &selected,
            &endpoint,
            &roots,
            first_claude_layers.map(|layers| layers.fingerprint),
        ) {
            return error_reply(request_id, ReplyError::PlanCapacity);
        }
        PlanReply::Preview(
            // The full plan is already redacted and contains only bounded typed metadata.
            PlanPreviewReply {
                schema_version: 1,
                request_id,
                status: "preview",
                plan,
            },
        )
    }

    #[cfg(test)]
    pub(crate) fn with_ownership_store(
        mut self,
        store: crate::state::ownership::OwnershipStore,
    ) -> Self {
        self.ownership_store = Some(store);
        self
    }

    pub(crate) fn with_optional_ownership_store(
        mut self,
        store: Option<crate::state::ownership::OwnershipStore>,
    ) -> Self {
        self.ownership_store = store;
        self
    }

    pub(crate) fn with_claude_project_picker<F>(mut self, picker: F) -> Self
    where
        F: Fn() -> Result<Option<PathBuf>, ()> + Send + Sync + 'static,
    {
        self.claude_context = claude_context::ClaudeContextManager::with_picker(picker);
        self
    }

    #[cfg(test)]
    pub(super) fn with_action_commit_fault(mut self) -> Self {
        self.fail_action_before_commit = true;
        self
    }

    #[cfg(test)]
    pub(super) fn with_codex_status_rollback_hook<H>(mut self, hook: H) -> Self
    where
        H: Fn() + Send + Sync + 'static,
    {
        self.after_codex_config_rollback = Some(Arc::new(hook));
        self
    }

    #[cfg(test)]
    pub(super) fn with_undo_commit_fault(mut self) -> Self {
        self.fail_undo_before_commit = true;
        self
    }

    #[cfg(test)]
    pub(super) fn expire_plan_for_test(&self, plan_id: &str) -> bool {
        let Ok(mut leases) = self.leases.lock() else {
            return false;
        };
        let Some(lease) = leases.get_mut(plan_id) else {
            return false;
        };
        lease.expires = Instant::now()
            .checked_sub(Duration::from_secs(1))
            .unwrap_or_else(Instant::now);
        lease.plan.expires_at_unix_seconds = 0;
        true
    }

    #[cfg(test)]
    pub(super) fn rescan_with_test_inputs(
        &self,
        request_id: String,
        inputs: super::discovery::DiscoveryInputs,
    ) -> super::discovery::DiscoveryReply {
        self.discovery.scan_with_inputs(request_id, inputs)
    }

    pub(crate) fn failure_reply(request_id: String) -> PlanReply {
        let safe_request_id = if valid_uuid(&request_id) {
            request_id
        } else {
            ZERO_UUID.to_owned()
        };
        error_reply(safe_request_id, ReplyError::PlanCapacity)
    }

    fn store_lease(
        &self,
        plan: &ConnectionPlan,
        installation: &InstallationSummary,
        endpoint: &ReadyStableEndpoint,
        roots: &ConfigRootInputs,
        claude_layers_fingerprint: Option<String>,
    ) -> bool {
        let Ok(mut leases) = self.leases.lock() else {
            return false;
        };
        let now = Instant::now();
        leases.retain(|plan_id, lease| lease.expires > now && lease.plan.plan_id == *plan_id);
        if leases.len() >= MAX_PLANS {
            return false;
        }
        leases.insert(
            plan.plan_id.clone(),
            PlanLease {
                plan: plan.clone(),
                expires: now + Duration::from_secs(PLAN_TTL_SECONDS),
                installation: installation.clone(),
                endpoint: endpoint.clone(),
                roots: roots.clone(),
                claude_layers_fingerprint,
            },
        );
        true
    }

    fn config_roots_for_context(
        &self,
        kind: ClientKind,
    ) -> Result<(ConfigRootInputs, ConfigScope), ConfigIoError> {
        let mut roots = (self.roots_provider)();
        if kind != ClientKind::ClaudeCode {
            roots.claude_project_directory = None;
            return Ok((roots, ConfigScope::UserConfig));
        }
        let selection = self
            .claude_context
            .current()
            .map_err(|_| ConfigIoError::Unavailable)?;
        roots.claude_project_directory = selection.project_directory.clone();
        Ok((roots, selection.scope))
    }
}

struct PreviewGuard<'a>(&'a AtomicBool);

impl<'a> PreviewGuard<'a> {
    fn try_acquire(flag: &'a AtomicBool) -> Option<Self> {
        flag.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .ok()
            .map(|_| Self(flag))
    }
}

impl Drop for PreviewGuard<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

fn valid_uuid(value: &str) -> bool {
    value.len() == 36 && Uuid::parse_str(value).is_ok_and(|parsed| parsed.to_string() == value)
}

fn parse_client_kind(value: &str) -> Option<ClientKind> {
    match value {
        "codex" => Some(ClientKind::Codex),
        "claude_code" => Some(ClientKind::ClaudeCode),
        "opencode" => Some(ClientKind::OpenCodeV1),
        _ => None,
    }
}

fn scope_matches_client(kind: ClientKind, scope: ConfigScope) -> bool {
    matches!(
        (kind, scope),
        (ClientKind::Codex, ConfigScope::UserConfig)
            | (ClientKind::ClaudeCode, ConfigScope::UserConfig)
            | (ClientKind::ClaudeCode, ConfigScope::ProjectLocalConfig)
            | (ClientKind::OpenCodeV1, ConfigScope::UserConfig)
    )
}

fn same_config_target(
    expected: &ConfigRootInputs,
    actual: &ConfigRootInputs,
    kind: ClientKind,
) -> bool {
    match (expected.target(kind), actual.target(kind)) {
        (
            Ok(ConfigTarget::Single {
                path: left,
                source: left_source,
            }),
            Ok(ConfigTarget::Single {
                path: right,
                source: right_source,
            }),
        ) => left_source == right_source && same_path(&left, &right),
        (
            Ok(ConfigTarget::OpenCode {
                json: left_json,
                jsonc: left_jsonc,
            }),
            Ok(ConfigTarget::OpenCode {
                json: right_json,
                jsonc: right_jsonc,
            }),
        ) => same_path(&left_json, &right_json) && same_path(&left_jsonc, &right_jsonc),
        _ => false,
    }
}

fn map_selection_error(error: InstallationSelectionError) -> ReplyError {
    match error {
        InstallationSelectionError::InvalidInstallationId
        | InstallationSelectionError::InvalidScanGeneration => ReplyError::Validation,
        InstallationSelectionError::StaleScanGeneration
        | InstallationSelectionError::NotScanned => ReplyError::StaleSelection,
        InstallationSelectionError::InstallationNotFound => ReplyError::NotFound,
    }
}

fn map_config_error(error: ConfigIoError) -> ReplyError {
    match error {
        ConfigIoError::InvalidRoot | ConfigIoError::UnsafePath => ReplyError::ConfigUnsafe,
        ConfigIoError::Unavailable => ReplyError::ConfigUnavailable,
        ConfigIoError::TooLarge => ReplyError::ConfigTooLarge,
        ConfigIoError::Ambiguous => ReplyError::ConfigConflict,
    }
}

fn map_claude_layers_error(error: claude_layers::ClaudeLayersError) -> ReplyError {
    match error {
        claude_layers::ClaudeLayersError::Io(error) => map_config_error(error),
        claude_layers::ClaudeLayersError::TooLarge => ReplyError::ConfigTooLarge,
        claude_layers::ClaudeLayersError::InvalidDocument
        | claude_layers::ClaudeLayersError::DuplicateKey => ReplyError::ConfigInvalid,
    }
}

fn map_plan_error(error: PlanError) -> ReplyError {
    match error {
        PlanError::InvalidContext => ReplyError::Validation,
        PlanError::ConfigTooLarge => ReplyError::ConfigTooLarge,
        PlanError::DuplicateKey | PlanError::InvalidConfig => ReplyError::ConfigInvalid,
    }
}

fn same_installation(
    expected: &InstallationSummary,
    actual: &Result<InstallationSummary, InstallationSelectionError>,
) -> bool {
    actual.as_ref().is_ok_and(|actual| {
        expected.installation_id == actual.installation_id
            && expected.client_kind == actual.client_kind
            && expected.surface == actual.surface
            && expected.version == actual.version
            && expected.source == actual.source
            && expected.status == actual.status
            && expected.connection_status == actual.connection_status
    })
}

fn same_endpoint(
    expected: &ReadyStableEndpoint,
    actual: &Result<ReadyStableEndpoint, StableEndpointAccessError>,
) -> bool {
    actual.as_ref().is_ok_and(|actual| {
        expected.endpoint_id == actual.endpoint_id
            && expected.host == actual.host
            && expected.port == actual.port
    })
}

fn error_reply(request_id: String, error: ReplyError) -> PlanReply {
    PlanReply::Error(PlanErrorReply {
        schema_version: 1,
        request_id,
        status: "error",
        error: error.detail(),
    })
}
