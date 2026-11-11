#[cfg(target_os = "macos")]
use super::{
    ensure_fixed_volume, is_reparse, pin_existing_directory_chain, same_path, validate_lexical_path,
};
#[cfg(windows)]
use super::{
    ensure_fixed_volume, is_reparse, pin_existing_directory_chain, same_path,
    validate_lexical_path, MAX_CONFIG_PATH_UNITS,
};
use super::{ConfigIoError, PlanManager, PreviewGuard};
use crate::integrations::plan::ConfigScope;
use crate::state::choices::{ChoiceAvailability, ChoiceStore, ClaudeChoice, ClaudeChoiceScope};
use serde::{Deserialize, Serialize};
#[cfg(any(windows, target_os = "macos"))]
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use uuid::Uuid;

const ZERO_UUID: &str = "00000000-0000-0000-0000-000000000000";

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ClaudeContextSnapshotRequest {
    pub schema_version: u8,
    pub request_id: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ClaudeScopeSelectionRequest {
    pub schema_version: u8,
    pub request_id: String,
    pub scope: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(untagged)]
pub(crate) enum ClaudeContextReply {
    State(ClaudeContextStateReply),
    Error(ClaudeContextErrorReply),
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct ClaudeContextStateReply {
    pub schema_version: u8,
    pub request_id: String,
    pub status: &'static str,
    pub scope: &'static str,
    pub project_directory: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct ClaudeContextErrorReply {
    pub schema_version: u8,
    pub request_id: String,
    pub status: &'static str,
    pub error: ClaudeContextErrorDetail,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct ClaudeContextErrorDetail {
    pub code: &'static str,
    pub retryable: bool,
    pub message_key: &'static str,
    pub diagnostic_id: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ClaudeContextSelection {
    pub scope: ConfigScope,
    pub project_directory: Option<PathBuf>,
    project_identity: Option<ProjectDirectoryIdentity>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ProjectDirectoryIdentity {
    volume: u32,
    file_index_high: u32,
    file_index_low: u32,
}

struct ValidatedProjectDirectory {
    canonical_path: PathBuf,
    identity: ProjectDirectoryIdentity,
}

impl ClaudeContextSelection {
    fn user_config() -> Self {
        Self {
            scope: ConfigScope::UserConfig,
            project_directory: None,
            project_identity: None,
        }
    }

    fn project_local(project_directory: PathBuf, identity: ProjectDirectoryIdentity) -> Self {
        Self {
            scope: ConfigScope::ProjectLocalConfig,
            project_directory: Some(project_directory),
            project_identity: Some(identity),
        }
    }

    fn is_valid(&self) -> bool {
        match self.scope {
            ConfigScope::UserConfig => {
                self.project_directory.is_none() && self.project_identity.is_none()
            }
            ConfigScope::ProjectLocalConfig => {
                self.project_directory.is_some() && self.project_identity.is_some()
            }
        }
    }
}

type ProjectDirectoryPicker = dyn Fn(Option<PathBuf>) -> Result<Option<PathBuf>, ()> + Send + Sync;

pub(super) struct ClaudeContextManager {
    selected: std::sync::Mutex<Option<ClaudeContextSelection>>,
    picker: Arc<ProjectDirectoryPicker>,
    choice_store: Option<ChoiceStore>,
    choice_store_required: bool,
}

#[derive(Clone, Copy)]
pub(super) enum ClaudeContextSelectionError {
    InvalidRequest,
    UnsupportedScope,
    UnsafeProjectDirectory,
    PickerUnavailable,
    Busy,
    StateUnavailable,
}

impl ClaudeContextManager {
    pub(super) fn new() -> Self {
        Self {
            selected: std::sync::Mutex::new(Some(ClaudeContextSelection::user_config())),
            picker: Arc::new(|_| Ok(None)),
            choice_store: None,
            choice_store_required: false,
        }
    }

    pub(super) fn set_picker<F>(&mut self, picker: F)
    where
        F: Fn(Option<PathBuf>) -> Result<Option<PathBuf>, ()> + Send + Sync + 'static,
    {
        self.picker = Arc::new(picker);
    }

    pub(super) fn set_choice_store(&mut self, store: Option<ChoiceStore>) {
        self.choice_store_required = true;
        self.selected = std::sync::Mutex::new(store.as_ref().and_then(load_saved_selection));
        self.choice_store = store;
    }

    pub(super) fn current(&self) -> Result<ClaudeContextSelection, ClaudeContextSelectionError> {
        let selected = self
            .selected
            .lock()
            .map_err(|_| ClaudeContextSelectionError::StateUnavailable)?
            .clone()
            .ok_or(ClaudeContextSelectionError::StateUnavailable)?;
        if !selected.is_valid() {
            return Err(ClaudeContextSelectionError::StateUnavailable);
        }
        if let Some(project_directory) = &selected.project_directory {
            let current = validate_project_directory(project_directory)
                .map_err(|_| ClaudeContextSelectionError::StateUnavailable)?;
            if current.canonical_path != *project_directory
                || Some(current.identity) != selected.project_identity
            {
                return Err(ClaudeContextSelectionError::StateUnavailable);
            }
        }
        Ok(selected)
    }

    pub(super) fn prepare_selection(
        &self,
        scope: &str,
    ) -> Result<Option<ClaudeContextSelection>, ClaudeContextSelectionError> {
        match scope {
            "user_config" => Ok(Some(ClaudeContextSelection::user_config())),
            "project_local_config" => match (self.picker)(self.preferred_picker_directory()) {
                Ok(Some(path)) => validate_project_directory(&path)
                    .map(|project| {
                        ClaudeContextSelection::project_local(
                            project.canonical_path,
                            project.identity,
                        )
                    })
                    .map(Some)
                    .map_err(|_| ClaudeContextSelectionError::UnsafeProjectDirectory),
                Ok(None) => Ok(None),
                Err(()) => Err(ClaudeContextSelectionError::PickerUnavailable),
            },
            _ => Err(ClaudeContextSelectionError::UnsupportedScope),
        }
    }

    pub(super) fn replace(
        &self,
        selected: ClaudeContextSelection,
    ) -> Result<bool, ClaudeContextSelectionError> {
        if !selected.is_valid() {
            return Err(ClaudeContextSelectionError::StateUnavailable);
        }
        let mut current = self
            .selected
            .lock()
            .map_err(|_| ClaudeContextSelectionError::StateUnavailable)?;
        let changed = current.as_ref() != Some(&selected);
        if self.choice_store_required {
            let store = self
                .choice_store
                .as_ref()
                .ok_or(ClaudeContextSelectionError::StateUnavailable)?;
            let choice = ClaudeChoice {
                scope: match selected.scope {
                    ConfigScope::UserConfig => ClaudeChoiceScope::UserConfig,
                    ConfigScope::ProjectLocalConfig => ClaudeChoiceScope::ProjectLocalConfig,
                },
                project_root: selected.project_directory.clone(),
                project_identity: selected.project_identity.map(to_file_identity),
                availability: ChoiceAvailability::Available,
            };
            store
                .save_claude(choice)
                .map_err(|_| ClaudeContextSelectionError::StateUnavailable)?;
        }
        *current = Some(selected);
        Ok(changed)
    }

    fn preferred_picker_directory(&self) -> Option<PathBuf> {
        if let Ok(selected) = self.current() {
            if selected.scope == ConfigScope::ProjectLocalConfig {
                if let Some(path) = selected.project_directory {
                    return Some(path);
                }
            }
        }

        let candidates = [
            std::env::var_os("USERPROFILE").map(PathBuf::from),
            std::env::current_dir().ok(),
        ];
        candidates.into_iter().flatten().find_map(|candidate| {
            validate_project_directory(&candidate)
                .ok()
                .map(|validated| validated.canonical_path)
        })
    }
}

fn load_saved_selection(store: &ChoiceStore) -> Option<ClaudeContextSelection> {
    let snapshot = store.read().ok()?;
    let Some(choice) = snapshot.claude else {
        return Some(ClaudeContextSelection::user_config());
    };
    match choice.scope {
        ClaudeChoiceScope::UserConfig
            if choice.project_root.is_none()
                && choice.project_identity.is_none()
                && choice.availability == ChoiceAvailability::Available =>
        {
            Some(ClaudeContextSelection::user_config())
        }
        ClaudeChoiceScope::ProjectLocalConfig
            if choice.availability == ChoiceAvailability::Available =>
        {
            let path = choice.project_root?;
            let identity = choice.project_identity?;
            let current = validate_project_directory(&path).ok()?;
            let expected = ProjectDirectoryIdentity {
                volume: identity.volume_serial,
                file_index_high: (identity.file_index >> 32) as u32,
                file_index_low: identity.file_index as u32,
            };
            (current.canonical_path == path && current.identity == expected).then(|| {
                ClaudeContextSelection::project_local(current.canonical_path, current.identity)
            })
        }
        _ => None,
    }
}

fn to_file_identity(
    identity: ProjectDirectoryIdentity,
) -> crate::integrations::transaction_fs::FileIdentity {
    crate::integrations::transaction_fs::FileIdentity {
        volume_serial: identity.volume,
        file_index: (u64::from(identity.file_index_high) << 32)
            | u64::from(identity.file_index_low),
    }
}

impl PlanManager {
    pub(crate) fn claude_context_snapshot(
        &self,
        request: ClaudeContextSnapshotRequest,
    ) -> ClaudeContextReply {
        let request_id = safe_request_id(&request.request_id);
        if request.schema_version != 1 || !valid_uuid(&request.request_id) {
            return error_reply(request_id, ClaudeContextSelectionError::InvalidRequest);
        }
        match self.claude_context.current() {
            Ok(selected) => state_reply(request_id, "snapshot", selected),
            Err(error) => error_reply(request_id, error),
        }
    }

    pub(crate) fn select_claude_scope(
        &self,
        request: ClaudeScopeSelectionRequest,
    ) -> ClaudeContextReply {
        let request_id = safe_request_id(&request.request_id);
        if request.schema_version != 1 || !valid_uuid(&request.request_id) {
            return error_reply(request_id, ClaudeContextSelectionError::InvalidRequest);
        }
        if !matches!(
            request.scope.as_str(),
            "user_config" | "project_local_config"
        ) {
            return error_reply(request_id, ClaudeContextSelectionError::UnsupportedScope);
        }
        let Some(_guard) = PreviewGuard::try_acquire(&self.preview_in_flight) else {
            return error_reply(request_id, ClaudeContextSelectionError::Busy);
        };
        let candidate = match self.claude_context.prepare_selection(&request.scope) {
            Ok(Some(value)) => value,
            Ok(None) => {
                return match self.claude_context.current() {
                    Ok(current) => state_reply(request_id, "cancelled", current),
                    Err(error) => error_reply(request_id, error),
                }
            }
            Err(error) => return error_reply(request_id, error),
        };
        let mut leases = match self.leases.lock() {
            Ok(value) => value,
            Err(_) => {
                return error_reply(request_id, ClaudeContextSelectionError::StateUnavailable)
            }
        };
        let changed = match self.claude_context.replace(candidate.clone()) {
            Ok(value) => value,
            Err(error) => return error_reply(request_id, error),
        };
        if changed {
            leases.clear();
        }
        state_reply(request_id, "selected", candidate)
    }
}

fn state_reply(
    request_id: String,
    status: &'static str,
    selected: ClaudeContextSelection,
) -> ClaudeContextReply {
    let project_directory = match selected.project_directory {
        Some(path) => match path.to_str() {
            Some(path) => Some(path.to_owned()),
            None => return error_reply(request_id, ClaudeContextSelectionError::StateUnavailable),
        },
        None => None,
    };
    ClaudeContextReply::State(ClaudeContextStateReply {
        schema_version: 1,
        request_id,
        status,
        scope: scope_code(selected.scope),
        project_directory,
    })
}

fn error_reply(request_id: String, error: ClaudeContextSelectionError) -> ClaudeContextReply {
    let (code, retryable, message_key) = match error {
        ClaudeContextSelectionError::InvalidRequest => {
            ("validation_failed", false, "connection.validation_failed")
        }
        ClaudeContextSelectionError::UnsupportedScope => {
            ("unsupported_scope", false, "connection.unsupported_scope")
        }
        ClaudeContextSelectionError::UnsafeProjectDirectory => (
            "unsafe_project_directory",
            false,
            "connection.config_unsafe",
        ),
        ClaudeContextSelectionError::PickerUnavailable => {
            ("picker_unavailable", true, "connection.config_unavailable")
        }
        ClaudeContextSelectionError::Busy => ("busy", true, "connection.busy"),
        ClaudeContextSelectionError::StateUnavailable => {
            ("unavailable", true, "connection.config_unavailable")
        }
    };
    ClaudeContextReply::Error(ClaudeContextErrorReply {
        schema_version: 1,
        request_id,
        status: "error",
        error: ClaudeContextErrorDetail {
            code,
            retryable,
            message_key,
            diagnostic_id: None,
        },
    })
}

fn scope_code(scope: ConfigScope) -> &'static str {
    match scope {
        ConfigScope::UserConfig => "user_config",
        ConfigScope::ProjectLocalConfig => "project_local_config",
    }
}

fn safe_request_id(value: &str) -> String {
    if valid_uuid(value) {
        value.to_owned()
    } else {
        ZERO_UUID.to_owned()
    }
}

fn valid_uuid(value: &str) -> bool {
    value.len() == 36 && Uuid::parse_str(value).is_ok_and(|parsed| parsed.to_string() == value)
}

fn validate_project_directory(path: &Path) -> Result<ValidatedProjectDirectory, ConfigIoError> {
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        let _ = path;
        return Err(ConfigIoError::InvalidRoot);
    }
    #[cfg(any(windows, target_os = "macos"))]
    {
        #[cfg(windows)]
        use std::os::windows::ffi::OsStrExt;

        validate_lexical_path(path)?;
        ensure_fixed_volume(path)?;
        let metadata = fs::symlink_metadata(path).map_err(|_| ConfigIoError::UnsafePath)?;
        if !metadata.is_dir() || is_reparse(&metadata) {
            return Err(ConfigIoError::UnsafePath);
        }
        let pins = pin_existing_directory_chain(path)?;
        pins.validate()?;
        let canonical = fs::canonicalize(path).map_err(|_| ConfigIoError::UnsafePath)?;
        let canonical = normalize_canonical_directory(&canonical)?;
        if !same_path(path, &canonical) {
            return Err(ConfigIoError::UnsafePath);
        }
        let final_metadata = fs::symlink_metadata(path).map_err(|_| ConfigIoError::UnsafePath)?;
        let final_canonical = normalize_canonical_directory(
            &fs::canonicalize(path).map_err(|_| ConfigIoError::UnsafePath)?,
        )?;
        if !final_metadata.is_dir()
            || is_reparse(&final_metadata)
            || !same_path(&canonical, &final_canonical)
            || pins.validate().is_err()
        {
            return Err(ConfigIoError::UnsafePath);
        }
        #[cfg(windows)]
        {
            let canonical_text = canonical.as_os_str().encode_wide().collect::<Vec<_>>();
            if canonical_text.contains(&0) || canonical_text.len() > MAX_CONFIG_PATH_UNITS {
                return Err(ConfigIoError::UnsafePath);
            }
        }
        if canonical.to_str().is_none() {
            return Err(ConfigIoError::UnsafePath);
        }
        let identity = project_directory_identity(&canonical)?;
        pins.validate()?;
        if project_directory_identity(&canonical)? != identity {
            return Err(ConfigIoError::UnsafePath);
        }
        Ok(ValidatedProjectDirectory {
            canonical_path: canonical,
            identity,
        })
    }
}

#[cfg(windows)]
fn project_directory_identity(path: &Path) -> Result<ProjectDirectoryIdentity, ConfigIoError> {
    let pin = super::open_pinned_directory(path)?;
    Ok(ProjectDirectoryIdentity {
        volume: pin.volume,
        file_index_high: pin.high,
        file_index_low: pin.low,
    })
}

#[cfg(target_os = "macos")]
fn project_directory_identity(path: &Path) -> Result<ProjectDirectoryIdentity, ConfigIoError> {
    use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
    let directory = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW_ANY | libc::O_CLOEXEC)
        .open(path)
        .map_err(|_| ConfigIoError::UnsafePath)?;
    let info = directory
        .metadata()
        .map_err(|_| ConfigIoError::Unavailable)?;
    if !info.is_dir() || info.uid() != unsafe { libc::geteuid() } || info.mode() & 0o022 != 0 {
        return Err(ConfigIoError::UnsafePath);
    }
    Ok(ProjectDirectoryIdentity {
        volume: info
            .dev()
            .try_into()
            .map_err(|_| ConfigIoError::UnsafePath)?,
        file_index_high: (info.ino() >> 32) as u32,
        file_index_low: info.ino() as u32,
    })
}

#[cfg(target_os = "macos")]
fn normalize_canonical_directory(path: &Path) -> Result<PathBuf, ConfigIoError> {
    validate_lexical_path(path)?;
    ensure_fixed_volume(&path.join(".headroom-project-volume"))?;
    Ok(path.to_owned())
}

#[cfg(windows)]
fn normalize_canonical_directory(path: &Path) -> Result<PathBuf, ConfigIoError> {
    use std::ffi::OsString;
    use std::os::windows::ffi::{OsStrExt, OsStringExt};

    let wide: Vec<u16> = path.as_os_str().encode_wide().collect();
    let local = if wide.starts_with(&[92, 92, 63, 92]) {
        &wide[4..]
    } else {
        wide.as_slice()
    };
    if local.len() < 3
        || !((b'A' as u16..=b'Z' as u16).contains(&local[0])
            || (b'a' as u16..=b'z' as u16).contains(&local[0]))
        || local[1] != b':' as u16
        || !matches!(local[2], 92 | 47)
    {
        return Err(ConfigIoError::UnsafePath);
    }
    let normalized = PathBuf::from(OsString::from_wide(local));
    validate_lexical_path(&normalized)?;
    ensure_fixed_volume(&normalized)?;
    Ok(normalized)
}
