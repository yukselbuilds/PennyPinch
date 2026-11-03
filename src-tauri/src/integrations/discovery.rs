use crate::integrations::transaction_fs::FileIdentity;
use crate::state::choices::{
    ChoiceAvailability, ChoiceClient, ChoiceOrigin, ChoiceStore, ChoiceSurface, InstallationChoice,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::env;
use std::fs::{self, File, OpenOptions};
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use uuid::Uuid;

#[cfg(windows)]
use std::ffi::c_void;
#[cfg(windows)]
use std::os::windows::ffi::OsStrExt;
#[cfg(windows)]
use std::os::windows::fs::{MetadataExt, OpenOptionsExt};
#[cfg(windows)]
use std::os::windows::io::AsRawHandle;
#[cfg(windows)]
use windows_sys::Win32::Storage::FileSystem::{
    GetDriveTypeW, GetFileInformationByHandle, GetFileVersionInfoSizeW, GetFileVersionInfoW,
    VerQueryValueW, BY_HANDLE_FILE_INFORMATION, FILE_ATTRIBUTE_DIRECTORY,
    FILE_ATTRIBUTE_REPARSE_POINT, FILE_SHARE_READ,
};
#[cfg(windows)]
use windows_sys::Win32::System::WindowsProgramming::DRIVE_FIXED;

const SCHEMA_VERSION: u8 = 1;
const MAX_PATH_ENTRIES: usize = 64;
const MAX_CANDIDATES: usize = 512;
const MAX_PACKAGE_JSON_BYTES: u64 = 16 * 1024;
const MAX_VERSION_RESOURCE_BYTES: usize = 128 * 1024;
const MAX_WINDOWS_APP_ENTRIES: usize = 256;
const MAX_SCOOP_ENTRIES: usize = 64;
const MAX_SCOOP_VERSIONS_PER_APP: usize = 16;
const MAX_CODEX_LOCAL_ENTRIES: usize = 64;
const MAX_CODEX_LOCAL_VERSION_DIRS: usize = 16;
const CODEX_CLI_0111_SIZE: u64 = 336_011_568;
const CODEX_CLI_0111_SHA256: &str =
    "e757246b4de87359e660647fa9514955a0971ff6a30affdd8c145d1d5ece7b84";
const CODEX_CLI_0112_SIZE: u64 = 336_134_960;
const CODEX_CLI_0112_SHA256: &str =
    "c81ed464e5e7cafd09a5fa26af390c480bed8e9fab2a87a54b285fbd56e7122a";
const CLAUDE_CODE_CLI_2158_SIZE: u64 = 236_306_080;
const CLAUDE_CODE_CLI_2158_SHA256: &str =
    "10fa305545b20baf6e074a086762bd252b89dfe7035848a5d41385503b1a6c74";
const OPENCODE_V1_CLI_1193_SIZE: u64 = 180_982_664;
const OPENCODE_V1_CLI_1193_SHA256: &str =
    "cab0c9a8480f71fbb7cbc88ad91057712ecb500ab41fb9c01dd586cb9d8ea375";
const MAX_PATH_CHARS: usize = 32_767;
const MAX_MANUAL_FILES: usize = 32;
const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;

#[cfg(debug_assertions)]
macro_rules! discovery_trace {
    ($($argument:tt)*) => {
        eprintln!("headroom_discovery {}", format_args!($($argument)*));
    };
}

#[cfg(not(debug_assertions))]
macro_rules! discovery_trace {
    ($($argument:tt)*) => {};
}

#[derive(Clone)]
pub struct DiscoveryManager {
    inner: Arc<DiscoveryInner>,
}

struct DiscoveryInner {
    state: RwLock<DiscoveryState>,
    scanning: AtomicBool,
    choice_store: Option<ChoiceStore>,
}

#[derive(Clone, Default)]
struct DiscoveryState {
    scanned: bool,
    generation: u64,
    installations: Vec<InstallationSummary>,
    paths: HashMap<String, PathBuf>,
    observations: HashMap<String, InstallationObservation>,
    manual_candidates: Vec<ManualCandidate>,
    warnings: Vec<&'static str>,
}

#[derive(Clone, Copy, Eq, PartialEq)]
struct InstallationObservation {
    identity: FileIdentity,
    size_bytes: u64,
}

#[derive(Clone)]
struct ManualCandidate {
    candidate: Candidate,
    observation: Option<InstallationObservation>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(untagged)]
pub enum DiscoveryReply {
    Snapshot(DiscoverySnapshot),
    Error(DiscoveryError),
    Cancelled(DiscoveryCancelled),
}

#[derive(Clone, Debug, Serialize)]
pub struct DiscoveryCancelled {
    pub schema_version: u8,
    pub request_id: String,
    pub status: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct DiscoverySnapshot {
    pub schema_version: u8,
    pub request_id: String,
    pub status: &'static str,
    pub scan_generation: u64,
    pub installations: Vec<InstallationSummary>,
    pub warnings: Vec<&'static str>,
}

#[derive(Clone, Debug, Serialize)]
pub struct InstallationSummary {
    pub installation_id: String,
    pub client_kind: &'static str,
    pub surface: &'static str,
    pub version: Option<String>,
    pub source: &'static str,
    pub location_label: &'static str,
    pub status: &'static str,
    pub connection_status: &'static str,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SavedInstallationChoiceRequest {
    pub schema_version: u8,
    pub request_id: String,
    pub client_kind: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RememberInstallationChoiceRequest {
    pub schema_version: u8,
    pub request_id: String,
    pub scan_generation: u64,
    pub installation_id: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(untagged)]
pub enum SavedInstallationChoiceReply {
    Snapshot(SavedInstallationChoiceSnapshot),
    Error(DiscoveryError),
}

#[derive(Clone, Debug, Serialize)]
pub struct SavedInstallationChoiceSnapshot {
    pub schema_version: u8,
    pub request_id: String,
    pub status: &'static str,
    pub client_kind: &'static str,
    pub scan_generation: Option<u64>,
    pub installation_id: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum InstallationSelectionError {
    InvalidInstallationId,
    InvalidScanGeneration,
    StaleScanGeneration,
    NotScanned,
    InstallationNotFound,
}

#[derive(Clone, Debug, Serialize)]
pub struct DiscoveryError {
    pub schema_version: u8,
    pub request_id: String,
    pub status: &'static str,
    pub error: DiscoveryErrorDetail,
}

#[derive(Clone, Debug, Serialize)]
pub struct DiscoveryErrorDetail {
    pub code: &'static str,
    pub retryable: bool,
    pub message_key: &'static str,
    pub diagnostic_id: Option<String>,
}

#[derive(Clone, Copy)]
enum ErrorCode {
    ValidationFailed,
    Busy,
    EnvironmentBlocked,
}

impl ErrorCode {
    fn parts(self) -> (&'static str, bool, &'static str) {
        match self {
            Self::ValidationFailed => ("validation_failed", false, "discovery.validation_failed"),
            Self::Busy => ("busy", true, "discovery.busy"),
            Self::EnvironmentBlocked => (
                "environment_blocked",
                false,
                "discovery.environment_blocked",
            ),
        }
    }
}

impl DiscoveryManager {
    pub fn new() -> Self {
        Self::with_choice_store(None)
    }

    pub(crate) fn with_choice_store(choice_store: Option<ChoiceStore>) -> Self {
        Self {
            inner: Arc::new(DiscoveryInner {
                state: RwLock::new(DiscoveryState::default()),
                scanning: AtomicBool::new(false),
                choice_store,
            }),
        }
    }

    pub fn list_snapshot(&self, request_id: String) -> DiscoveryReply {
        if !valid_request_id(&request_id) {
            return error_reply(&request_id, ErrorCode::ValidationFailed);
        }
        match self.inner.state.read() {
            Ok(state) => snapshot_reply(request_id, &state),
            Err(poisoned) => snapshot_reply(request_id, &poisoned.into_inner()),
        }
    }

    pub(crate) fn select_installation(
        &self,
        installation_id: &str,
        expected_scan_generation: u64,
    ) -> Result<InstallationSummary, InstallationSelectionError> {
        self.select_installation_with_path(installation_id, expected_scan_generation)
            .map(|(summary, _)| summary)
    }

    pub(crate) fn select_installation_with_path(
        &self,
        installation_id: &str,
        expected_scan_generation: u64,
    ) -> Result<(InstallationSummary, PathBuf), InstallationSelectionError> {
        if !valid_request_id(installation_id) {
            return Err(InstallationSelectionError::InvalidInstallationId);
        }
        let state = self
            .inner
            .state
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if !state.scanned {
            return Err(InstallationSelectionError::NotScanned);
        }
        if expected_scan_generation == 0
            || expected_scan_generation > MAX_SAFE_INTEGER
            || state.generation > MAX_SAFE_INTEGER
        {
            return Err(InstallationSelectionError::InvalidScanGeneration);
        }
        if expected_scan_generation != state.generation {
            return Err(InstallationSelectionError::StaleScanGeneration);
        }
        let installation = state
            .installations
            .iter()
            .find(|installation| installation.installation_id == installation_id)
            .cloned()
            .ok_or(InstallationSelectionError::InstallationNotFound)?;
        let path = state
            .paths
            .get(installation_id)
            .cloned()
            .ok_or(InstallationSelectionError::InstallationNotFound)?;
        Ok((installation, path))
    }

    pub(crate) fn saved_installation_choice(
        &self,
        request: SavedInstallationChoiceRequest,
    ) -> SavedInstallationChoiceReply {
        let Some(client) = choice_client_kind(&request.client_kind) else {
            return saved_choice_error(&request.request_id, ErrorCode::ValidationFailed);
        };
        if request.schema_version != SCHEMA_VERSION || !valid_request_id(&request.request_id) {
            return saved_choice_error(&request.request_id, ErrorCode::ValidationFailed);
        }
        if self
            .inner
            .scanning
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return saved_choice_error(&request.request_id, ErrorCode::Busy);
        }
        let _guard = ScanGuard(&self.inner.scanning);
        let Some(store) = self.inner.choice_store.as_ref() else {
            return saved_choice_error(&request.request_id, ErrorCode::EnvironmentBlocked);
        };
        let choices = match store.read() {
            Ok(choices) => choices,
            Err(_) => {
                return saved_choice_error(&request.request_id, ErrorCode::EnvironmentBlocked)
            }
        };
        let state = self
            .inner
            .state
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let saved = choices
            .installations
            .iter()
            .find(|choice| choice.client == client);
        let Some(saved) = saved else {
            return SavedInstallationChoiceReply::Snapshot(SavedInstallationChoiceSnapshot {
                schema_version: SCHEMA_VERSION,
                request_id: request.request_id,
                status: "not_selected",
                client_kind: client_kind_name(client),
                scan_generation: state.scanned.then_some(state.generation),
                installation_id: None,
            });
        };
        if saved.availability != ChoiceAvailability::Available {
            return SavedInstallationChoiceReply::Snapshot(SavedInstallationChoiceSnapshot {
                schema_version: SCHEMA_VERSION,
                request_id: request.request_id,
                status: "unavailable",
                client_kind: client_kind_name(client),
                scan_generation: state.scanned.then_some(state.generation),
                installation_id: None,
            });
        }
        if !state.scanned {
            return SavedInstallationChoiceReply::Snapshot(SavedInstallationChoiceSnapshot {
                schema_version: SCHEMA_VERSION,
                request_id: request.request_id,
                status: "not_scanned",
                client_kind: client_kind_name(client),
                scan_generation: None,
                installation_id: None,
            });
        }
        let installation_id = state
            .installations
            .iter()
            .find(|installation| {
                installation.client_kind == client_kind_name(client)
                    && installation.surface == surface_name(saved.surface)
                    && state
                        .paths
                        .get(&installation.installation_id)
                        .is_some_and(|path| path_key(path) == path_key(&saved.executable_path))
                    && state
                        .observations
                        .get(&installation.installation_id)
                        .is_some_and(|observed| {
                            observed.identity == saved.identity
                                && observed.size_bytes == saved.size_bytes
                        })
            })
            .map(|installation| installation.installation_id.clone());
        SavedInstallationChoiceReply::Snapshot(SavedInstallationChoiceSnapshot {
            schema_version: SCHEMA_VERSION,
            request_id: request.request_id,
            status: if installation_id.is_some() {
                "available"
            } else {
                "unavailable"
            },
            client_kind: client_kind_name(client),
            scan_generation: Some(state.generation),
            installation_id,
        })
    }

    pub(crate) fn remember_installation_choice(
        &self,
        request: RememberInstallationChoiceRequest,
    ) -> SavedInstallationChoiceReply {
        if request.schema_version != SCHEMA_VERSION
            || !valid_request_id(&request.request_id)
            || !valid_request_id(&request.installation_id)
            || request.scan_generation == 0
            || request.scan_generation > MAX_SAFE_INTEGER
        {
            return saved_choice_error(&request.request_id, ErrorCode::ValidationFailed);
        }
        if self
            .inner
            .scanning
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return saved_choice_error(&request.request_id, ErrorCode::Busy);
        }
        let _guard = ScanGuard(&self.inner.scanning);
        let Some(store) = self.inner.choice_store.as_ref() else {
            return saved_choice_error(&request.request_id, ErrorCode::EnvironmentBlocked);
        };
        let (summary, path, observed) = {
            let state = self
                .inner
                .state
                .read()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if !state.scanned || state.generation != request.scan_generation {
                return saved_choice_error(&request.request_id, ErrorCode::ValidationFailed);
            }
            let Some(summary) = state
                .installations
                .iter()
                .find(|item| item.installation_id == request.installation_id)
                .cloned()
            else {
                return saved_choice_error(&request.request_id, ErrorCode::ValidationFailed);
            };
            let Some(path) = state.paths.get(&request.installation_id).cloned() else {
                return saved_choice_error(&request.request_id, ErrorCode::ValidationFailed);
            };
            let Some(observed) = state.observations.get(&request.installation_id).copied() else {
                return saved_choice_error(&request.request_id, ErrorCode::EnvironmentBlocked);
            };
            (summary, path, observed)
        };
        let Some(client) = choice_client_kind(summary.client_kind) else {
            return saved_choice_error(&request.request_id, ErrorCode::ValidationFailed);
        };
        let Some(surface) = choice_surface(summary.surface) else {
            return saved_choice_error(&request.request_id, ErrorCode::ValidationFailed);
        };
        let Some(origin) = choice_origin(summary.source) else {
            return saved_choice_error(&request.request_id, ErrorCode::ValidationFailed);
        };
        let choice = match ChoiceStore::capture_installation(
            client,
            surface,
            origin,
            path.clone(),
            summary.version.clone(),
        ) {
            Ok(choice) => choice,
            Err(_) => {
                return saved_choice_error(&request.request_id, ErrorCode::EnvironmentBlocked)
            }
        };
        if choice.identity != observed.identity
            || choice.size_bytes != observed.size_bytes
            || path_key(&choice.executable_path) != path_key(&path)
        {
            return saved_choice_error(&request.request_id, ErrorCode::ValidationFailed);
        }
        if store.save_installation(choice).is_err() {
            return saved_choice_error(&request.request_id, ErrorCode::EnvironmentBlocked);
        }
        SavedInstallationChoiceReply::Snapshot(SavedInstallationChoiceSnapshot {
            schema_version: SCHEMA_VERSION,
            request_id: request.request_id,
            status: "saved",
            client_kind: client_kind_name(client),
            scan_generation: Some(request.scan_generation),
            installation_id: Some(request.installation_id),
        })
    }

    pub fn scan(&self, request_id: String) -> DiscoveryReply {
        self.scan_from_inputs(request_id, DiscoveryInputs::from_environment())
    }

    #[cfg(test)]
    pub(super) fn scan_with_inputs(
        &self,
        request_id: String,
        inputs: DiscoveryInputs,
    ) -> DiscoveryReply {
        self.scan_from_inputs(request_id, inputs)
    }

    fn scan_from_inputs(&self, request_id: String, inputs: DiscoveryInputs) -> DiscoveryReply {
        if !valid_request_id(&request_id) {
            return error_reply(&request_id, ErrorCode::ValidationFailed);
        }
        if self
            .inner
            .scanning
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return error_reply(&request_id, ErrorCode::Busy);
        }
        let _guard = ScanGuard(&self.inner.scanning);
        let (previous_manual, previous_generation) = match self.inner.state.read() {
            Ok(state) => (state.manual_candidates.clone(), state.generation),
            Err(poisoned) => {
                let state = poisoned.into_inner();
                (state.manual_candidates.clone(), state.generation)
            }
        };
        discovery_trace!(
            "event=scan_begin manual_before={} generation={}",
            previous_manual.len(),
            previous_generation
        );
        let mut report = discover(inputs);
        let mut retained_manual = Vec::with_capacity(previous_manual.len());
        let mut manual_rejected = 0usize;
        for previous in previous_manual {
            let candidate = previous.candidate;
            let revalidated = match manual_candidate(candidate.kind, &candidate.executable_path) {
                Ok(candidate) => candidate,
                Err(reason) => {
                    report.warnings.insert("candidate_location_unavailable");
                    manual_rejected += 1;
                    discovery_trace!("event=manual_rejected reason={}", reason.label());
                    continue;
                }
            };
            let current_observation = observe_candidate(&revalidated);
            if previous.observation.is_some() && previous.observation != current_observation {
                report.warnings.insert("candidate_location_unavailable");
                manual_rejected += 1;
                continue;
            }
            if report_contains_executable(&report, revalidated.kind, &revalidated.executable_path) {
                if revalidated.version.is_some() {
                    let existing_index = report.installations.iter().position(|installation| {
                        installation.client_kind == revalidated.kind
                            && report.paths.get(&installation.installation_id).is_some_and(
                                |candidate| {
                                    path_key(candidate) == path_key(&revalidated.executable_path)
                                },
                            )
                    });
                    if let Some(index) = existing_index {
                        if report.installations[index].version.is_none() {
                            report.installations[index].version = revalidated.version.clone();
                        }
                    }
                }
            } else if report.installations.len() >= MAX_CANDIDATES {
                report.warnings.insert("candidate_limit_reached");
            } else {
                let id = Uuid::new_v4().to_string();
                report
                    .paths
                    .insert(id.clone(), revalidated.executable_path.clone());
                report.installations.push(InstallationSummary {
                    installation_id: id,
                    client_kind: revalidated.kind,
                    surface: revalidated.surface,
                    version: revalidated.version.clone(),
                    source: "manual_file",
                    location_label: "Selected file",
                    status: "detected",
                    connection_status: "not_verified",
                });
            }
            retained_manual.push(ManualCandidate {
                candidate: revalidated,
                observation: current_observation,
            });
        }
        if let Some(store) = self.inner.choice_store.as_ref() {
            if let Ok(choices) = store.read() {
                append_saved_manual_candidates(
                    choices.installations,
                    &mut report,
                    &mut retained_manual,
                );
            }
        }
        let observations = collect_observations(&report.installations, &report.paths);
        let manual_retained = retained_manual.len();
        let installation_count = report.installations.len();
        {
            let mut state = match self.inner.state.write() {
                Ok(state) => state,
                Err(poisoned) => poisoned.into_inner(),
            };
            state.scanned = true;
            state.generation = state.generation.saturating_add(1);
            state.installations = report.installations;
            state.paths = report.paths;
            state.observations = observations;
            state.manual_candidates = retained_manual;
            state.warnings = report.warnings.into_iter().collect();
            discovery_trace!(
                "event=scan_end manual_retained={} manual_rejected={} installation_count={} generation={}",
                manual_retained,
                manual_rejected,
                installation_count,
                state.generation
            );
            return snapshot_reply(request_id, &state);
        }
    }

    pub fn select_client_file_with<F>(
        &self,
        request_id: String,
        requested_client_kind: String,
        selector: F,
    ) -> DiscoveryReply
    where
        F: FnOnce(&'static str) -> Result<Option<PathBuf>, ()>,
    {
        if !valid_request_id(&request_id) {
            return error_reply(&request_id, ErrorCode::ValidationFailed);
        }
        let Some(client_kind) = client_kind(&requested_client_kind) else {
            return error_reply(&request_id, ErrorCode::ValidationFailed);
        };
        if self
            .inner
            .scanning
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return error_reply(&request_id, ErrorCode::Busy);
        }
        let _guard = ScanGuard(&self.inner.scanning);
        let selected = match selector(client_kind) {
            Ok(Some(path)) => path,
            Ok(None) => return cancelled_reply(request_id),
            Err(()) => return error_reply(&request_id, ErrorCode::EnvironmentBlocked),
        };
        let candidate = match manual_candidate(client_kind, &selected) {
            Ok(candidate) => candidate,
            Err(reason) => {
                discovery_trace!("event=select_rejected reason={}", reason.label());
                return error_reply(&request_id, ErrorCode::ValidationFailed);
            }
        };
        let observation = observe_candidate(&candidate);

        let mut state = match self.inner.state.write() {
            Ok(state) => state,
            Err(poisoned) => poisoned.into_inner(),
        };
        let key = path_key(&candidate.executable_path);
        let already_manual = state.manual_candidates.iter().any(|existing| {
            existing.candidate.kind == client_kind
                && path_key(&existing.candidate.executable_path) == key
        });
        if already_manual {
            discovery_trace!(
                "event=select_duplicate manual_count={} generation={}",
                state.manual_candidates.len(),
                state.generation
            );
            return snapshot_reply(request_id, &state);
        }
        let discovered_index = state.installations.iter().position(|installation| {
            installation.client_kind == client_kind
                && state
                    .paths
                    .get(&installation.installation_id)
                    .is_some_and(|path| path_key(path) == key)
        });
        if state.manual_candidates.len() >= MAX_MANUAL_FILES {
            return error_reply(&request_id, ErrorCode::ValidationFailed);
        }
        if let Some(index) = discovered_index {
            if candidate.version.is_some() {
                if state.installations[index].version.is_none() {
                    state.installations[index].version = candidate.version.clone();
                }
            }
            state.manual_candidates.push(ManualCandidate {
                candidate,
                observation,
            });
            discovery_trace!(
                "event=select_duplicate manual_count={} generation={}",
                state.manual_candidates.len(),
                state.generation
            );
            return snapshot_reply(request_id, &state);
        }
        if state.installations.len() >= MAX_CANDIDATES {
            return error_reply(&request_id, ErrorCode::ValidationFailed);
        }

        let id = Uuid::new_v4().to_string();
        state.installations.push(InstallationSummary {
            installation_id: id.clone(),
            client_kind,
            surface: "cli",
            version: candidate.version.clone(),
            source: "manual_file",
            location_label: "Selected file",
            status: "detected",
            connection_status: "not_verified",
        });
        state
            .paths
            .insert(id.clone(), candidate.executable_path.clone());
        if let Some(observation) = observation {
            state.observations.insert(id, observation);
        }
        state.manual_candidates.push(ManualCandidate {
            candidate,
            observation,
        });
        if !state.scanned {
            state.scanned = true;
            state.warnings = vec!["absence_is_not_unsupported", "scope_partial"];
        }
        state.generation = state.generation.saturating_add(1);
        discovery_trace!(
            "event=select_accepted manual_count={} generation={}",
            state.manual_candidates.len(),
            state.generation
        );
        snapshot_reply(request_id, &state)
    }

    pub fn failure_reply(request_id: String) -> DiscoveryReply {
        error_reply(&request_id, ErrorCode::EnvironmentBlocked)
    }

    pub(crate) fn saved_choice_failure_reply(request_id: String) -> SavedInstallationChoiceReply {
        saved_choice_error(&request_id, ErrorCode::EnvironmentBlocked)
    }

    #[cfg(test)]
    pub(super) fn installation_path(&self, installation_id: &str) -> Option<PathBuf> {
        self.inner
            .state
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .paths
            .get(installation_id)
            .cloned()
    }
}

impl Default for DiscoveryManager {
    fn default() -> Self {
        Self::new()
    }
}

struct ScanGuard<'a>(&'a AtomicBool);

impl Drop for ScanGuard<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

fn snapshot_reply(request_id: String, state: &DiscoveryState) -> DiscoveryReply {
    DiscoveryReply::Snapshot(DiscoverySnapshot {
        schema_version: SCHEMA_VERSION,
        request_id,
        status: if state.scanned {
            "partial"
        } else {
            "not_scanned"
        },
        scan_generation: state.generation,
        installations: state.installations.clone(),
        warnings: if state.scanned {
            state.warnings.clone()
        } else {
            vec!["scan_not_run"]
        },
    })
}

fn error_reply(request_id: &str, code: ErrorCode) -> DiscoveryReply {
    let (code, retryable, message_key) = code.parts();
    DiscoveryReply::Error(DiscoveryError {
        schema_version: SCHEMA_VERSION,
        request_id: if valid_request_id(request_id) {
            request_id.to_owned()
        } else {
            String::new()
        },
        status: "error",
        error: DiscoveryErrorDetail {
            code,
            retryable,
            message_key,
            diagnostic_id: None,
        },
    })
}

fn saved_choice_error(request_id: &str, code: ErrorCode) -> SavedInstallationChoiceReply {
    match error_reply(request_id, code) {
        DiscoveryReply::Error(error) => SavedInstallationChoiceReply::Error(error),
        _ => unreachable!("error_reply always returns an error"),
    }
}

fn choice_client_kind(value: &str) -> Option<ChoiceClient> {
    match value {
        "codex" => Some(ChoiceClient::Codex),
        "claude_code" => Some(ChoiceClient::ClaudeCode),
        "opencode" => Some(ChoiceClient::OpenCode),
        _ => None,
    }
}

fn client_kind_name(value: ChoiceClient) -> &'static str {
    match value {
        ChoiceClient::Codex => "codex",
        ChoiceClient::ClaudeCode => "claude_code",
        ChoiceClient::OpenCode => "opencode",
    }
}

fn choice_surface(value: &str) -> Option<ChoiceSurface> {
    match value {
        "cli" => Some(ChoiceSurface::Cli),
        "desktop" => Some(ChoiceSurface::Desktop),
        _ => None,
    }
}

fn surface_name(value: ChoiceSurface) -> &'static str {
    match value {
        ChoiceSurface::Cli => "cli",
        ChoiceSurface::Desktop => "desktop",
    }
}

fn choice_origin(value: &str) -> Option<ChoiceOrigin> {
    match value {
        "manual_file" => Some(ChoiceOrigin::ManualFile),
        "path" => Some(ChoiceOrigin::Path),
        "scoop" => Some(ChoiceOrigin::Scoop),
        "user_local_native" => Some(ChoiceOrigin::UserLocalNative),
        "user_local_npm" => Some(ChoiceOrigin::UserLocalNpm),
        "windows_app_package" => Some(ChoiceOrigin::WindowsAppPackage),
        _ => None,
    }
}

fn cancelled_reply(request_id: String) -> DiscoveryReply {
    DiscoveryReply::Cancelled(DiscoveryCancelled {
        schema_version: SCHEMA_VERSION,
        request_id,
        status: "cancelled",
    })
}

fn client_kind(value: &str) -> Option<&'static str> {
    match value {
        "codex" => Some("codex"),
        "claude_code" => Some("claude_code"),
        "opencode" => Some("opencode"),
        _ => None,
    }
}

fn manual_candidate(
    client_kind: &'static str,
    path: &Path,
) -> Result<Candidate, ManualRejectReason> {
    // Reject network/device prefixes before any filesystem metadata call.
    if is_network_or_device_path(path) {
        return Err(ManualRejectReason::Unsafe);
    }
    if !manual_filename_matches(client_kind, path) {
        return Err(ManualRejectReason::Filename);
    }
    let executable = (if client_kind == "codex"
        && path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("exe"))
    {
        resolve_codex_executable(path)
    } else {
        check_existing_path(path, PathKind::File)
    })
    .map_err(|error| match error {
        PathCheckError::Missing => ManualRejectReason::Missing,
        PathCheckError::Unsafe => ManualRejectReason::Unsafe,
        PathCheckError::Unavailable => ManualRejectReason::Unavailable,
    })?;
    let version = if client_kind == "codex"
        && path
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("exe"))
    {
        verified_codex_cli_version(&executable)
    } else if client_kind == "claude_code"
        && executable
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("exe"))
    {
        verified_claude_code_cli_version(&executable)
    } else if client_kind == "opencode"
        && executable
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("exe"))
    {
        verified_opencode_v1_cli_version(&executable)
    } else {
        None
    };
    Ok(Candidate {
        kind: client_kind,
        surface: "cli",
        version,
        source: "manual_file",
        location_label: "Selected file",
        identity_path: executable.clone(),
        executable_path: executable,
    })
}

#[derive(Clone, Copy)]
enum ManualRejectReason {
    Missing,
    Unsafe,
    Unavailable,
    Filename,
}

impl ManualRejectReason {
    fn label(self) -> &'static str {
        match self {
            Self::Missing => "missing",
            Self::Unsafe => "unsafe",
            Self::Unavailable => "unavailable",
            Self::Filename => "filename",
        }
    }
}

fn manual_filename_matches(client_kind: &str, path: &Path) -> bool {
    let Some(filename) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    let expected = match client_kind {
        "codex" => ["codex.exe", "codex.cmd", "codex.bat"],
        "claude_code" => ["claude.exe", "claude.cmd", "claude.bat"],
        "opencode" => ["opencode.exe", "opencode.cmd", "opencode.bat"],
        _ => return false,
    };
    expected
        .iter()
        .any(|name| filename.eq_ignore_ascii_case(name))
}

fn report_contains_executable(report: &DiscoveryReport, client_kind: &str, path: &Path) -> bool {
    let key = path_key(path);
    report.installations.iter().any(|installation| {
        installation.client_kind == client_kind
            && report
                .paths
                .get(&installation.installation_id)
                .is_some_and(|candidate| path_key(candidate) == key)
    })
}

fn observe_candidate(candidate: &Candidate) -> Option<InstallationObservation> {
    let choice = ChoiceStore::capture_installation(
        choice_client_kind(candidate.kind)?,
        choice_surface(candidate.surface)?,
        choice_origin(candidate.source)?,
        candidate.executable_path.clone(),
        candidate.version.clone(),
    )
    .ok()?;
    Some(InstallationObservation {
        identity: choice.identity,
        size_bytes: choice.size_bytes,
    })
}

fn collect_observations(
    installations: &[InstallationSummary],
    paths: &HashMap<String, PathBuf>,
) -> HashMap<String, InstallationObservation> {
    let mut observations = HashMap::new();
    for installation in installations {
        let Some(path) = paths.get(&installation.installation_id) else {
            continue;
        };
        let candidate = Candidate {
            kind: installation.client_kind,
            surface: installation.surface,
            version: installation.version.clone(),
            source: installation.source,
            location_label: installation.location_label,
            identity_path: path.clone(),
            executable_path: path.clone(),
        };
        if let Some(observation) = observe_candidate(&candidate) {
            observations.insert(installation.installation_id.clone(), observation);
        }
    }
    observations
}

fn append_saved_manual_candidates(
    saved_choices: Vec<InstallationChoice>,
    report: &mut DiscoveryReport,
    retained_manual: &mut Vec<ManualCandidate>,
) {
    for choice in saved_choices {
        if choice.origin != ChoiceOrigin::ManualFile
            || choice.surface != ChoiceSurface::Cli
            || choice.availability != ChoiceAvailability::Available
        {
            continue;
        }
        let kind = client_kind_name(choice.client);
        let Ok(candidate) = manual_candidate(kind, &choice.executable_path) else {
            continue;
        };
        let Some(observation) = observe_candidate(&candidate) else {
            continue;
        };
        if observation.identity != choice.identity
            || observation.size_bytes != choice.size_bytes
            || path_key(&candidate.executable_path) != path_key(&choice.executable_path)
            || report_contains_executable(report, kind, &candidate.executable_path)
        {
            continue;
        }
        if report.installations.len() >= MAX_CANDIDATES {
            report.warnings.insert("candidate_limit_reached");
            continue;
        }
        let id = Uuid::new_v4().to_string();
        report
            .paths
            .insert(id.clone(), candidate.executable_path.clone());
        report.installations.push(InstallationSummary {
            installation_id: id,
            client_kind: candidate.kind,
            surface: candidate.surface,
            version: candidate.version.clone(),
            source: "manual_file",
            location_label: "Selected file",
            status: "detected",
            connection_status: "not_verified",
        });
        retained_manual.push(ManualCandidate {
            candidate,
            observation: Some(observation),
        });
    }
}

fn valid_request_id(request_id: &str) -> bool {
    if request_id.len() != 36 || !request_id.is_ascii() {
        return false;
    }
    let bytes = request_id.as_bytes();
    if bytes[8] != b'-' || bytes[13] != b'-' || bytes[18] != b'-' || bytes[23] != b'-' {
        return false;
    }
    for (index, byte) in bytes.iter().enumerate() {
        if [8, 13, 18, 23].contains(&index) {
            continue;
        }
        if !byte.is_ascii_hexdigit() {
            return false;
        }
    }
    matches!(bytes[14].to_ascii_lowercase(), b'1'..=b'8')
        && matches!(bytes[19].to_ascii_lowercase(), b'8' | b'9' | b'a' | b'b')
}

#[derive(Default)]
pub(super) struct DiscoveryInputs {
    pub(super) path_entries: Vec<PathBuf>,
    pub(super) path_was_truncated: bool,
    pub(super) npm_roots: Vec<PathBuf>,
    pub(super) local_app_data: Option<PathBuf>,
    pub(super) user_profile: Option<PathBuf>,
    pub(super) program_files: Option<PathBuf>,
    pub(super) registered_codex_packages: Vec<PathBuf>,
}

impl DiscoveryInputs {
    fn from_environment() -> Self {
        let mut inputs = Self::default();
        inputs.registered_codex_packages = super::codex_binary::registered_desktop_roots();
        discovery_trace!(
            "event=registered_packages count={}",
            inputs.registered_codex_packages.len()
        );
        if let Some(path) = env::var_os("PATH") {
            for entry in env::split_paths(&path) {
                if inputs.path_entries.len() == MAX_PATH_ENTRIES {
                    inputs.path_was_truncated = true;
                    break;
                }
                inputs.path_entries.push(entry);
            }
        }
        if let Some(app_data) = env::var_os("APPDATA") {
            inputs.npm_roots.push(PathBuf::from(app_data).join("npm"));
        }
        if let Some(local_app_data) = env::var_os("LOCALAPPDATA") {
            inputs.local_app_data = Some(PathBuf::from(&local_app_data));
            inputs
                .npm_roots
                .push(PathBuf::from(local_app_data).join("npm"));
        }
        inputs.user_profile = env::var_os("USERPROFILE").map(PathBuf::from);
        inputs.program_files = env::var_os("ProgramFiles").map(PathBuf::from);
        inputs
    }
}

#[derive(Clone, Copy)]
struct Product {
    kind: &'static str,
    command: &'static str,
    package: &'static str,
    bin_name: &'static str,
}

const PRODUCTS: [Product; 3] = [
    Product {
        kind: "codex",
        command: "codex",
        package: "@openai/codex",
        bin_name: "codex",
    },
    Product {
        kind: "claude_code",
        command: "claude",
        package: "@anthropic-ai/claude-code",
        bin_name: "claude",
    },
    Product {
        kind: "opencode",
        command: "opencode",
        package: "opencode-ai",
        bin_name: "opencode",
    },
];

#[derive(Clone)]
struct Candidate {
    kind: &'static str,
    surface: &'static str,
    version: Option<String>,
    source: &'static str,
    location_label: &'static str,
    identity_path: PathBuf,
    executable_path: PathBuf,
}

struct DiscoveryReport {
    installations: Vec<InstallationSummary>,
    paths: HashMap<String, PathBuf>,
    warnings: BTreeSet<&'static str>,
}

fn discover(inputs: DiscoveryInputs) -> DiscoveryReport {
    let mut warnings = BTreeSet::from(["scope_partial", "absence_is_not_unsupported"]);
    if inputs.path_was_truncated {
        warnings.insert("path_entry_limit_reached");
    }
    if inputs.path_entries.len() > MAX_PATH_ENTRIES {
        warnings.insert("path_entry_limit_reached");
    }
    let mut candidates = HashMap::<String, Candidate>::new();
    let mut candidate_count = 0usize;

    for npm_root in &inputs.npm_roots {
        for product in PRODUCTS {
            if candidate_count >= MAX_CANDIDATES {
                warnings.insert("candidate_limit_reached");
                break;
            }
            candidate_count += 1;
            add_npm_candidate(npm_root, product, &mut candidates, &mut warnings);
        }
    }

    if let Some(local_app_data) = inputs.local_app_data.as_deref() {
        add_codex_local_candidates(
            local_app_data,
            &mut candidates,
            &mut warnings,
            &mut candidate_count,
        );
    }

    if let Some(user_profile) = inputs.user_profile.as_deref() {
        add_user_native_candidates(
            user_profile,
            &mut candidates,
            &mut warnings,
            &mut candidate_count,
        );
        add_scoop_candidates(
            user_profile,
            &mut candidates,
            &mut warnings,
            &mut candidate_count,
        );
    }

    for package in inputs.registered_codex_packages.iter().take(16) {
        add_codex_desktop_package(
            package,
            &mut candidates,
            &mut warnings,
            &mut candidate_count,
        );
    }
    if inputs.registered_codex_packages.is_empty() {
        if let Some(program_files) = inputs.program_files.as_deref() {
            add_codex_desktop_candidates(
                program_files,
                &mut candidates,
                &mut warnings,
                &mut candidate_count,
            );
        }
    }

    let mut path_entries = HashSet::new();
    for entry in inputs.path_entries.into_iter().take(MAX_PATH_ENTRIES) {
        if is_network_or_device_path(&entry) {
            warnings.insert("network_location_skipped");
            continue;
        }
        // Official standalone installs use local junctions for their current bin directory.
        if candidate_count < MAX_CANDIDATES {
            if let Ok(executable) = resolve_codex_executable(&entry.join("codex.exe")) {
                if let Some(version) = verified_codex_cli_version(&executable) {
                    candidate_count += 1;
                    insert_candidate(
                        &mut candidates,
                        Candidate {
                            kind: "codex",
                            surface: "cli",
                            version: Some(version),
                            source: "path",
                            location_label: "PATH",
                            identity_path: executable.clone(),
                            executable_path: executable,
                        },
                    );
                }
            }
        }
        let Some(canonical_dir) = checked_existing_path(&entry, PathKind::Directory, &mut warnings)
        else {
            continue;
        };
        let normalized = path_key(&canonical_dir);
        if !path_entries.insert(normalized) {
            continue;
        }
        for product in PRODUCTS {
            for extension in ["exe", "cmd", "bat"] {
                if candidate_count >= MAX_CANDIDATES {
                    warnings.insert("candidate_limit_reached");
                    break;
                }
                candidate_count += 1;
                let executable = canonical_dir.join(format!("{}.{}", product.command, extension));
                if let Some(canonical_exe) =
                    checked_existing_path(&executable, PathKind::File, &mut warnings)
                {
                    insert_candidate(
                        &mut candidates,
                        Candidate {
                            kind: product.kind,
                            surface: "cli",
                            version: if product.kind == "claude_code" && extension == "exe" {
                                verified_claude_code_cli_version(&canonical_exe)
                            } else if product.kind == "opencode" && extension == "exe" {
                                verified_opencode_v1_cli_version(&canonical_exe)
                            } else {
                                None
                            },
                            source: "path",
                            location_label: "PATH",
                            identity_path: canonical_exe.clone(),
                            executable_path: canonical_exe,
                        },
                    );
                }
            }
        }
    }

    let mut candidate_list: Vec<_> = candidates.into_values().collect();
    candidate_list.sort_by(|left, right| {
        (left.kind, left.surface, path_key(&left.identity_path)).cmp(&(
            right.kind,
            right.surface,
            path_key(&right.identity_path),
        ))
    });
    let mut paths = HashMap::new();
    let installations = candidate_list
        .into_iter()
        .map(|candidate| {
            let id = Uuid::new_v4().to_string();
            paths.insert(id.clone(), candidate.executable_path);
            InstallationSummary {
                installation_id: id,
                client_kind: candidate.kind,
                surface: candidate.surface,
                version: candidate.version,
                source: candidate.source,
                location_label: candidate.location_label,
                status: "detected",
                connection_status: "not_verified",
            }
        })
        .collect();

    DiscoveryReport {
        installations,
        paths,
        warnings,
    }
}

fn add_npm_candidate(
    npm_root: &Path,
    product: Product,
    candidates: &mut HashMap<String, Candidate>,
    warnings: &mut BTreeSet<&'static str>,
) {
    if is_network_or_device_path(npm_root) {
        warnings.insert("network_location_skipped");
        return;
    }
    let package_root = npm_root.join("node_modules").join(product.package);
    let Some(package_root) = checked_existing_path(&package_root, PathKind::Directory, warnings)
    else {
        return;
    };
    let manifest_path = package_root.join("package.json");
    let Some(manifest_path) = checked_existing_path(&manifest_path, PathKind::File, warnings)
    else {
        return;
    };
    let manifest = match read_limited_json(&manifest_path) {
        Ok(value) => value,
        Err(ReadMetadataError::Missing) => return,
        Err(ReadMetadataError::Invalid) => {
            warnings.insert("package_metadata_untrusted");
            return;
        }
    };
    if manifest.get("name").and_then(Value::as_str) != Some(product.package) {
        warnings.insert("package_metadata_untrusted");
        return;
    }
    let Some(bin) = package_bin(&manifest, product.bin_name) else {
        warnings.insert("package_metadata_untrusted");
        return;
    };
    let Some(relative_bin) = safe_relative_path(bin) else {
        warnings.insert("package_metadata_untrusted");
        return;
    };
    let executable = package_root.join(relative_bin);
    let Some(executable) = checked_existing_path(&executable, PathKind::File, warnings) else {
        return;
    };
    let version = manifest
        .get("version")
        .and_then(Value::as_str)
        .and_then(valid_display_version);
    if version.is_none() {
        warnings.insert("package_version_unknown");
    }
    insert_candidate(
        candidates,
        Candidate {
            kind: product.kind,
            surface: "cli",
            version,
            source: "user_local_npm",
            location_label: "User-local npm",
            identity_path: package_root,
            executable_path: executable,
        },
    );
}

fn package_bin<'a>(manifest: &'a Value, expected_name: &str) -> Option<&'a str> {
    match manifest.get("bin")? {
        Value::String(value) => Some(value),
        Value::Object(values) => values.get(expected_name).and_then(Value::as_str),
        _ => None,
    }
}

fn safe_relative_path(value: &str) -> Option<PathBuf> {
    if value.is_empty() || value.len() > 512 || !value.is_ascii() || value.contains(':') {
        return None;
    }
    let path = Path::new(value);
    if path.is_absolute() {
        return None;
    }
    let mut parts = 0usize;
    for component in path.components() {
        match component {
            Component::Normal(name) if !name.is_empty() => parts += 1,
            _ => return None,
        }
    }
    if parts == 0 || parts > 16 {
        None
    } else {
        Some(path.to_path_buf())
    }
}

fn read_limited_json(path: &Path) -> Result<Value, ReadMetadataError> {
    let file = File::open(path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            ReadMetadataError::Missing
        } else {
            ReadMetadataError::Invalid
        }
    })?;
    let mut bytes = Vec::new();
    file.take(MAX_PACKAGE_JSON_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| ReadMetadataError::Invalid)?;
    if bytes.len() as u64 > MAX_PACKAGE_JSON_BYTES {
        return Err(ReadMetadataError::Invalid);
    }
    serde_json::from_slice(&bytes).map_err(|_| ReadMetadataError::Invalid)
}

enum ReadMetadataError {
    Missing,
    Invalid,
}

fn valid_display_version(value: &str) -> Option<String> {
    if value.is_empty()
        || value.len() > 64
        || !value.is_ascii()
        || value.bytes().any(|byte| byte.is_ascii_whitespace())
    {
        return None;
    }
    let (without_build, build) = value
        .split_once('+')
        .map_or((value, None), |(head, tail)| (head, Some(tail)));
    if build.is_some_and(|part| !valid_semver_identifiers(part)) {
        return None;
    }
    let (core, prerelease) = without_build
        .split_once('-')
        .map_or((without_build, None), |(head, tail)| (head, Some(tail)));
    let components: Vec<_> = core.split('.').collect();
    if !(2..=4).contains(&components.len())
        || components.iter().any(|part| {
            part.is_empty()
                || part.len() > 10
                || !part.bytes().all(|byte| byte.is_ascii_digit())
                || (part.len() > 1 && part.starts_with('0'))
        })
        || prerelease.is_some_and(|part| !valid_semver_identifiers(part))
    {
        return None;
    }
    Some(value.to_owned())
}

fn valid_semver_identifiers(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 32
        && value.split('.').all(|part| {
            !part.is_empty()
                && part
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        })
}

fn add_user_native_candidates(
    user_profile: &Path,
    candidates: &mut HashMap<String, Candidate>,
    warnings: &mut BTreeSet<&'static str>,
    candidate_count: &mut usize,
) {
    let known = [
        ("claude_code", user_profile.join(".local/bin/claude.exe")),
        ("opencode", user_profile.join(".local/bin/opencode.exe")),
    ];
    for (kind, path) in known {
        if *candidate_count >= MAX_CANDIDATES {
            warnings.insert("candidate_limit_reached");
            return;
        }
        *candidate_count += 1;
        if let Some(executable) = checked_existing_path(&path, PathKind::File, warnings) {
            insert_candidate(
                candidates,
                Candidate {
                    kind,
                    surface: "cli",
                    version: if kind == "claude_code" {
                        verified_claude_code_cli_version(&executable)
                    } else if kind == "opencode" {
                        verified_opencode_v1_cli_version(&executable)
                    } else {
                        None
                    },
                    source: "user_local_native",
                    location_label: "User-local install",
                    identity_path: executable.clone(),
                    executable_path: executable,
                },
            );
        }
    }
}

fn add_codex_local_candidates(
    local_app_data: &Path,
    candidates: &mut HashMap<String, Candidate>,
    warnings: &mut BTreeSet<&'static str>,
    candidate_count: &mut usize,
) {
    let bin_root = local_app_data.join("OpenAI/Codex/bin");
    if is_network_or_device_path(&bin_root) {
        warnings.insert("network_location_skipped");
        return;
    }
    let Some(bin_root) = checked_existing_path(&bin_root, PathKind::Directory, warnings) else {
        return;
    };
    let entries = match fs::read_dir(&bin_root) {
        Ok(entries) => entries,
        Err(_) => {
            warnings.insert("candidate_location_unavailable");
            return;
        }
    };
    let mut entry_count = 0usize;
    let mut version_dir_count = 0usize;
    for entry in entries {
        if entry_count >= MAX_CODEX_LOCAL_ENTRIES {
            warnings.insert("candidate_limit_reached");
            break;
        }
        entry_count += 1;
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => {
                warnings.insert("candidate_location_unavailable");
                continue;
            }
        };
        let file_type = match entry.file_type() {
            Ok(file_type) => file_type,
            Err(_) => {
                warnings.insert("candidate_location_unavailable");
                continue;
            }
        };
        if file_type.is_symlink() || !file_type.is_dir() {
            continue;
        }
        if version_dir_count >= MAX_CODEX_LOCAL_VERSION_DIRS {
            warnings.insert("candidate_limit_reached");
            break;
        }
        let Some(version_dir) = checked_existing_path(&entry.path(), PathKind::Directory, warnings)
        else {
            continue;
        };
        version_dir_count += 1;
        if *candidate_count >= MAX_CANDIDATES {
            warnings.insert("candidate_limit_reached");
            break;
        }
        *candidate_count += 1;
        let Some(executable) =
            checked_existing_path(&version_dir.join("codex.exe"), PathKind::File, warnings)
        else {
            continue;
        };
        insert_candidate(
            candidates,
            Candidate {
                kind: "codex",
                surface: "cli",
                version: verified_codex_cli_version(&executable),
                source: "user_local_native",
                location_label: "User-local install",
                identity_path: executable.clone(),
                executable_path: executable,
            },
        );
    }
}

fn add_scoop_candidates(
    user_profile: &Path,
    candidates: &mut HashMap<String, Candidate>,
    warnings: &mut BTreeSet<&'static str>,
    candidate_count: &mut usize,
) {
    for (kind, package_name, executable_name) in [
        ("codex", "codex", "codex.exe"),
        ("claude_code", "claude-code", "claude.exe"),
        ("opencode", "opencode", "opencode.exe"),
    ] {
        let app_root = user_profile.join("scoop/apps").join(package_name);
        if is_network_or_device_path(&app_root) {
            warnings.insert("network_location_skipped");
            continue;
        }
        let Some(app_root) = checked_existing_path(&app_root, PathKind::Directory, warnings) else {
            continue;
        };
        let Ok(entries) = fs::read_dir(&app_root) else {
            warnings.insert("candidate_location_unavailable");
            continue;
        };
        let mut version_count = 0usize;
        for (index, entry) in entries.enumerate() {
            if index >= MAX_SCOOP_ENTRIES {
                warnings.insert("scoop_entry_limit_reached");
                break;
            }
            if version_count >= MAX_SCOOP_VERSIONS_PER_APP {
                warnings.insert("scoop_version_limit_reached");
                break;
            }
            let Ok(entry) = entry else {
                warnings.insert("candidate_location_unavailable");
                continue;
            };
            if entry
                .file_name()
                .to_string_lossy()
                .eq_ignore_ascii_case("current")
            {
                continue;
            }
            let Some(version_dir) =
                checked_existing_path(&entry.path(), PathKind::Directory, warnings)
            else {
                continue;
            };
            version_count += 1;
            if *candidate_count >= MAX_CANDIDATES {
                warnings.insert("candidate_limit_reached");
                return;
            }
            *candidate_count += 1;
            let executable_path = version_dir.join(executable_name);
            if let Some(executable) =
                checked_existing_path(&executable_path, PathKind::File, warnings)
            {
                let version = entry.file_name().to_string_lossy().to_string();
                let version = valid_display_version(&version);
                insert_candidate(
                    candidates,
                    Candidate {
                        kind,
                        surface: "cli",
                        version,
                        source: "scoop",
                        location_label: "Scoop",
                        identity_path: version_dir,
                        executable_path: executable,
                    },
                );
            }
        }
    }
}

fn add_codex_desktop_candidates(
    program_files: &Path,
    candidates: &mut HashMap<String, Candidate>,
    warnings: &mut BTreeSet<&'static str>,
    candidate_count: &mut usize,
) {
    let windows_apps = program_files.join("WindowsApps");
    if is_network_or_device_path(&windows_apps) {
        warnings.insert("network_location_skipped");
        return;
    }
    let Some(windows_apps) = checked_existing_path(&windows_apps, PathKind::Directory, warnings)
    else {
        return;
    };
    let Ok(entries) = fs::read_dir(windows_apps) else {
        warnings.insert("candidate_location_unavailable");
        return;
    };
    for (index, entry) in entries.enumerate() {
        if index >= MAX_WINDOWS_APP_ENTRIES {
            warnings.insert("windows_app_entry_limit_reached");
            break;
        }
        let Ok(entry) = entry else {
            warnings.insert("candidate_location_unavailable");
            continue;
        };
        let folder = entry.file_name().to_string_lossy().to_string();
        if !folder.to_ascii_lowercase().starts_with("openai.codex_") {
            continue;
        }
        add_codex_desktop_package(&entry.path(), candidates, warnings, candidate_count);
    }
}

fn add_codex_desktop_package(
    path: &Path,
    candidates: &mut HashMap<String, Candidate>,
    warnings: &mut BTreeSet<&'static str>,
    candidate_count: &mut usize,
) {
    let Some(folder) = path.file_name().and_then(|v| v.to_str()) else {
        return;
    };
    if !folder.to_ascii_lowercase().starts_with("openai.codex_") {
        return;
    }
    if *candidate_count >= MAX_CANDIDATES {
        warnings.insert("candidate_limit_reached");
        return;
    }
    *candidate_count += 1;
    let Some(package_root) = checked_existing_path(path, PathKind::Directory, warnings) else {
        return;
    };
    let found = ["app/Codex.exe", "app/ChatGPT.exe"]
        .iter()
        .filter_map(|name| {
            checked_existing_path(&package_root.join(name), PathKind::File, warnings)
        })
        .find_map(|executable| {
            verified_codex_desktop_version(&executable).map(|version| (executable, version))
        });
    let Some((executable, binary_version)) = found else {
        warnings.insert("desktop_metadata_untrusted");
        return;
    };
    let version = folder
        .split('_')
        .nth(1)
        .filter(|v| super::codex_binary::desktop_version_supported(v))
        .map(str::to_owned)
        .or(binary_version);
    let identity_path = package_root;
    insert_candidate(
        candidates,
        Candidate {
            kind: "codex",
            surface: "desktop",
            version,
            source: "windows_app_package",
            location_label: "Windows app package",
            identity_path,
            executable_path: executable,
        },
    );
}

fn insert_candidate(candidates: &mut HashMap<String, Candidate>, candidate: Candidate) {
    let key = format!(
        "{}|{}|{}",
        candidate.kind,
        candidate.surface,
        path_key(&candidate.identity_path)
    );
    match candidates.get_mut(&key) {
        Some(existing) if existing.version.is_none() && candidate.version.is_some() => {
            *existing = candidate;
        }
        Some(_) => {}
        None => {
            candidates.insert(key, candidate);
        }
    }
}

enum PathKind {
    Directory,
    File,
}

fn checked_existing_path(
    path: &Path,
    expected: PathKind,
    warnings: &mut BTreeSet<&'static str>,
) -> Option<PathBuf> {
    match check_existing_path(path, expected) {
        Ok(path) => Some(path),
        Err(PathCheckError::Missing | PathCheckError::Unsafe) => None,
        Err(PathCheckError::Unavailable) => {
            warnings.insert("candidate_location_unavailable");
            None
        }
    }
}

enum PathCheckError {
    Missing,
    Unsafe,
    Unavailable,
}

fn resolve_codex_executable(path: &Path) -> Result<PathBuf, PathCheckError> {
    let mut pending = path.to_owned();
    for _ in 0..8 {
        if is_network_or_device_path(&pending)
            || !pending.is_absolute()
            || !is_local_fixed_volume(&pending)
            || pending.as_os_str().len() > MAX_PATH_CHARS
        {
            return Err(PathCheckError::Unsafe);
        }
        let mut current = PathBuf::new();
        let components: Vec<_> = pending.components().collect();
        let mut redirected = false;
        for (index, component) in components.iter().enumerate() {
            if matches!(component, Component::CurDir | Component::ParentDir) {
                return Err(PathCheckError::Unsafe);
            }
            current.push(component.as_os_str());
            if matches!(component, Component::Prefix(_)) {
                continue;
            }
            let metadata = fs::symlink_metadata(&current).map_err(|error| {
                if error.kind() == std::io::ErrorKind::NotFound {
                    PathCheckError::Missing
                } else {
                    PathCheckError::Unavailable
                }
            })?;
            if is_reparse_point(&metadata) || metadata.file_type().is_symlink() {
                let target = fs::read_link(&current).map_err(|_| PathCheckError::Unsafe)?;
                let mut target =
                    normalize_local_canonical_path(target).ok_or(PathCheckError::Unsafe)?;
                if !target.is_absolute() || !is_local_fixed_volume(&target) {
                    return Err(PathCheckError::Unsafe);
                }
                for rest in &components[index + 1..] {
                    target.push(rest.as_os_str());
                }
                pending = target;
                redirected = true;
                break;
            }
        }
        if !redirected {
            return check_existing_path(&pending, PathKind::File);
        }
    }
    Err(PathCheckError::Unsafe)
}

fn check_existing_path(path: &Path, expected: PathKind) -> Result<PathBuf, PathCheckError> {
    if is_network_or_device_path(path) || path.as_os_str().to_string_lossy().len() > MAX_PATH_CHARS
    {
        return Err(PathCheckError::Unsafe);
    }
    if !path.is_absolute() || !is_local_fixed_volume(path) {
        return Err(PathCheckError::Unsafe);
    }
    let mut current = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => current.push(prefix.as_os_str()),
            Component::RootDir => current.push(component.as_os_str()),
            Component::CurDir => return Err(PathCheckError::Unsafe),
            Component::ParentDir => return Err(PathCheckError::Unsafe),
            Component::Normal(part) => current.push(part),
        }
        if matches!(component, Component::Prefix(_)) {
            continue;
        }
        let metadata = match fs::symlink_metadata(&current) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Err(PathCheckError::Missing)
            }
            Err(_) => return Err(PathCheckError::Unavailable),
        };
        if is_reparse_point(&metadata) || metadata.file_type().is_symlink() {
            return Err(PathCheckError::Unsafe);
        }
        if !matches!(component, Component::Normal(_)) && !metadata.is_dir() {
            return Err(PathCheckError::Unsafe);
        }
    }
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            PathCheckError::Missing
        } else {
            PathCheckError::Unavailable
        }
    })?;
    let matches = match expected {
        PathKind::Directory => metadata.is_dir(),
        PathKind::File => metadata.is_file(),
    };
    if !matches || is_reparse_point(&metadata) || metadata.file_type().is_symlink() {
        return Err(PathCheckError::Unsafe);
    }
    let canonical = fs::canonicalize(path).map_err(|_| PathCheckError::Unavailable)?;
    let canonical = normalize_local_canonical_path(canonical).ok_or(PathCheckError::Unsafe)?;
    if is_network_or_device_path(&canonical) || !is_local_fixed_volume(&canonical) {
        return Err(PathCheckError::Unsafe);
    }
    Ok(canonical)
}

#[cfg(windows)]
fn normalize_local_canonical_path(path: PathBuf) -> Option<PathBuf> {
    let raw = path.as_os_str().to_string_lossy();
    if let Some(local) = raw.strip_prefix(r"\\?\") {
        let bytes = local.as_bytes();
        if bytes.len() < 3
            || !bytes[0].is_ascii_alphabetic()
            || bytes[1] != b':'
            || !matches!(bytes[2], b'\\' | b'/')
        {
            return None;
        }
        return Some(PathBuf::from(local));
    }
    if is_network_or_device_path(&path) {
        return None;
    }
    Some(path)
}

#[cfg(not(windows))]
fn normalize_local_canonical_path(path: PathBuf) -> Option<PathBuf> {
    (!is_network_or_device_path(&path)).then_some(path)
}

fn is_network_or_device_path(path: &Path) -> bool {
    let raw = path.as_os_str().to_string_lossy();
    raw.starts_with(r"\") || raw.starts_with(r"\?") || raw.starts_with(r"\.")
}

#[cfg(windows)]
fn is_local_fixed_volume(path: &Path) -> bool {
    let Some(Component::Prefix(prefix)) = path.components().next() else {
        return false;
    };
    let root = format!("{}\\", prefix.as_os_str().to_string_lossy());
    let wide: Vec<u16> = std::ffi::OsStr::new(&root)
        .encode_wide()
        .chain(Some(0))
        .collect();
    unsafe { GetDriveTypeW(wide.as_ptr()) == DRIVE_FIXED }
}

#[cfg(not(windows))]
fn is_local_fixed_volume(_path: &Path) -> bool {
    false
}

#[cfg(windows)]
fn is_reparse_point(metadata: &fs::Metadata) -> bool {
    metadata.file_attributes() & 0x400 != 0
}

#[cfg(not(windows))]
fn is_reparse_point(_metadata: &fs::Metadata) -> bool {
    false
}

fn path_key(path: &Path) -> String {
    path.to_string_lossy().to_lowercase()
}

pub(super) fn codex_cli_version_from_digest(file_size: u64, digest_hex: &str) -> Option<String> {
    match (file_size, digest_hex) {
        (CODEX_CLI_0111_SIZE, CODEX_CLI_0111_SHA256) => Some("0.163.0-alpha.11.1".to_owned()),
        (CODEX_CLI_0112_SIZE, CODEX_CLI_0112_SHA256) => Some("0.163.0-alpha.11.2".to_owned()),
        _ => None,
    }
}

pub(super) fn claude_code_cli_version_from_digest(
    file_size: u64,
    digest_hex: &str,
) -> Option<String> {
    (file_size == CLAUDE_CODE_CLI_2158_SIZE && digest_hex == CLAUDE_CODE_CLI_2158_SHA256)
        .then(|| "2.1.158".to_owned())
}

pub(super) fn opencode_v1_cli_version_from_digest(
    file_size: u64,
    digest_hex: &str,
) -> Option<String> {
    (file_size == OPENCODE_V1_CLI_1193_SIZE && digest_hex == OPENCODE_V1_CLI_1193_SHA256)
        .then(|| "1.19.3".to_owned())
}

fn verified_claude_code_cli_version(path: &Path) -> Option<String> {
    verified_bounded_cli_version(
        path,
        CLAUDE_CODE_CLI_2158_SIZE,
        claude_code_cli_version_from_digest,
    )
}

fn verified_opencode_v1_cli_version(path: &Path) -> Option<String> {
    verified_bounded_cli_version(
        path,
        OPENCODE_V1_CLI_1193_SIZE,
        opencode_v1_cli_version_from_digest,
    )
}

fn verified_bounded_cli_version(
    path: &Path,
    expected_size: u64,
    version_from_digest: fn(u64, &str) -> Option<String>,
) -> Option<String> {
    let canonical_path = check_existing_path(path, PathKind::File).ok()?;
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(windows)]
    options.share_mode(FILE_SHARE_READ);
    let mut file = options.open(&canonical_path).ok()?;
    let initial_metadata = file.metadata().ok()?;
    if !initial_metadata.is_file() || initial_metadata.len() != expected_size {
        return None;
    }
    let initial_identity = file_identity(&file)?;

    let mut hasher = Sha256::new();
    let mut bytes_read = 0u64;
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let remaining_with_growth_check =
            expected_size.saturating_sub(bytes_read).saturating_add(1);
        let read_size = remaining_with_growth_check.min(buffer.len() as u64) as usize;
        let count = file.read(&mut buffer[..read_size]).ok()?;
        if count == 0 {
            break;
        }
        bytes_read = bytes_read.checked_add(count as u64)?;
        if bytes_read > expected_size {
            return None;
        }
        hasher.update(&buffer[..count]);
    }
    if bytes_read != expected_size {
        return None;
    }
    if file.metadata().ok()?.len() != expected_size || file_identity(&file)? != initial_identity {
        return None;
    }
    let final_path = check_existing_path(&canonical_path, PathKind::File).ok()?;
    if path_key(&final_path) != path_key(&canonical_path) {
        return None;
    }
    let mut final_path_options = OpenOptions::new();
    final_path_options.read(true);
    #[cfg(windows)]
    final_path_options.share_mode(FILE_SHARE_READ);
    let final_path_file = final_path_options.open(&final_path).ok()?;
    if file_identity(&final_path_file)? != initial_identity {
        return None;
    }

    version_from_digest(bytes_read, &format!("{:x}", hasher.finalize()))
}

pub(super) fn verified_codex_cli_version(path: &Path) -> Option<String> {
    verified_codex_cli_file(path).map(|(version, _)| version)
}

/// Pins a recognized Codex CLI file while its path is used to start a
/// verification process. The shared read-only handle prevents ordinary
/// replacement or writes between hashing and CreateProcess.
pub(crate) fn pin_verified_codex_cli(path: &Path, expected_version: &str) -> Option<File> {
    let (version, file) = verified_codex_cli_file(path)?;
    (version == expected_version).then_some(file)
}

fn verified_codex_cli_file(path: &Path) -> Option<(String, File)> {
    let canonical_path = resolve_codex_executable(path).ok()?;
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(windows)]
    options.share_mode(FILE_SHARE_READ);
    let mut file = options.open(&canonical_path).ok()?;
    let initial_metadata = file.metadata().ok()?;
    let expected_size = initial_metadata.len();
    if !initial_metadata.is_file() || !(1024 * 1024..=1024 * 1024 * 1024).contains(&expected_size) {
        return None;
    }
    let initial_identity = file_identity(&file)?;

    let mut hasher = Sha256::new();
    let mut bytes_read = 0u64;
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let remaining_with_growth_check =
            expected_size.saturating_sub(bytes_read).saturating_add(1);
        let read_size = remaining_with_growth_check.min(buffer.len() as u64) as usize;
        let count = file.read(&mut buffer[..read_size]).ok()?;
        if count == 0 {
            break;
        }
        bytes_read = bytes_read.checked_add(count as u64)?;
        if bytes_read > expected_size {
            return None;
        }
        hasher.update(&buffer[..count]);
    }
    if bytes_read != expected_size {
        return None;
    }
    let final_handle_metadata = file.metadata().ok()?;
    if final_handle_metadata.len() != expected_size || file_identity(&file)? != initial_identity {
        return None;
    }
    let final_path = check_existing_path(&canonical_path, PathKind::File).ok()?;
    if path_key(&final_path) != path_key(&canonical_path) {
        return None;
    }
    let mut final_path_options = OpenOptions::new();
    final_path_options.read(true);
    #[cfg(windows)]
    final_path_options.share_mode(FILE_SHARE_READ);
    let final_path_file = final_path_options.open(&final_path).ok()?;
    if file_identity(&final_path_file)? != initial_identity {
        return None;
    }

    let digest = format!("{:x}", hasher.finalize());
    let version = codex_cli_version_from_digest(bytes_read, &digest)
        .or_else(|| super::codex_binary::signed_version(&canonical_path, &file, &digest))?;
    if file_identity(&file)? != initial_identity || file.metadata().ok()?.len() != expected_size {
        return None;
    }
    Some((version, file))
}

#[cfg(windows)]
fn file_identity(file: &File) -> Option<(u32, u64)> {
    let mut information = BY_HANDLE_FILE_INFORMATION::default();
    if unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut information) } == 0
        || information.dwFileAttributes & (FILE_ATTRIBUTE_DIRECTORY | FILE_ATTRIBUTE_REPARSE_POINT)
            != 0
    {
        return None;
    }
    Some((
        information.dwVolumeSerialNumber,
        ((information.nFileIndexHigh as u64) << 32) | information.nFileIndexLow as u64,
    ))
}

#[cfg(not(windows))]
fn file_identity(_file: &File) -> Option<(u32, u64)> {
    None
}

#[cfg(windows)]
fn verified_codex_desktop_version(path: &Path) -> Option<Option<String>> {
    let mut options = OpenOptions::new();
    options.read(true).share_mode(FILE_SHARE_READ);
    let file = options.open(path).ok()?;
    if !super::codex_binary::openai_signature(path, &file) {
        discovery_trace!("event=desktop_signature trusted=0");
        return None;
    }
    let wide_path: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    let mut ignored_handle = 0u32;
    let size = unsafe { GetFileVersionInfoSizeW(wide_path.as_ptr(), &mut ignored_handle) } as usize;
    discovery_trace!("event=desktop_resource bytes={size}");
    if size == 0 || size > MAX_VERSION_RESOURCE_BYTES {
        return None;
    }
    let mut bytes = vec![0u8; size];
    if unsafe {
        GetFileVersionInfoW(
            wide_path.as_ptr(),
            0,
            size as u32,
            bytes.as_mut_ptr().cast::<c_void>(),
        )
    } == 0
    {
        return None;
    }
    let translations = version_translations(&bytes);
    let mut company = None;
    let mut product = None;
    let mut file_version = None;
    for (language, code_page) in translations.into_iter().take(16) {
        company = company.or_else(|| version_string(&bytes, language, code_page, "CompanyName"));
        product = product.or_else(|| version_string(&bytes, language, code_page, "ProductName"));
        file_version =
            file_version.or_else(|| version_string(&bytes, language, code_page, "FileVersion"));
    }
    discovery_trace!(
        "event=desktop_resource_keys company={} product={} version={}",
        company.is_some() as u8,
        product.is_some() as u8,
        file_version.is_some() as u8
    );
    let company = company?;
    let product = product?;
    if !company.to_ascii_lowercase().contains("openai")
        || !(product.to_ascii_lowercase().contains("codex")
            || product.to_ascii_lowercase().contains("chatgpt"))
    {
        return None;
    }
    Some(file_version.as_deref().and_then(valid_display_version))
}

#[cfg(not(windows))]
fn verified_codex_desktop_version(_path: &Path) -> Option<Option<String>> {
    None
}

#[cfg(windows)]
fn version_translations(bytes: &[u8]) -> Vec<(u16, u16)> {
    let key: Vec<u16> = "\\VarFileInfo\\Translation"
        .encode_utf16()
        .chain(Some(0))
        .collect();
    let mut value = std::ptr::null_mut();
    let mut length = 0u32;
    let ok = unsafe {
        VerQueryValueW(
            bytes.as_ptr().cast::<c_void>(),
            key.as_ptr(),
            &mut value,
            &mut length,
        )
    };
    if ok == 0 || value.is_null() || length < 4 {
        return vec![(0x0409, 0x04b0)];
    }
    let count = (length as usize / 4).min(16);
    let words = unsafe { std::slice::from_raw_parts(value.cast::<u16>(), count * 2) };
    words
        .chunks_exact(2)
        .map(|pair| (pair[0], pair[1]))
        .collect()
}

#[cfg(windows)]
fn version_string(bytes: &[u8], language: u16, code_page: u16, field: &str) -> Option<String> {
    let key = format!("\\StringFileInfo\\{language:04x}{code_page:04x}\\{field}");
    let wide: Vec<u16> = key.encode_utf16().chain(Some(0)).collect();
    let mut value = std::ptr::null_mut();
    let mut length = 0u32;
    let ok = unsafe {
        VerQueryValueW(
            bytes.as_ptr().cast::<c_void>(),
            wide.as_ptr(),
            &mut value,
            &mut length,
        )
    };
    if ok == 0 || value.is_null() || length == 0 || length as usize > 256 {
        return None;
    }
    let words = unsafe { std::slice::from_raw_parts(value.cast::<u16>(), length as usize) };
    let end = words
        .iter()
        .position(|unit| *unit == 0)
        .unwrap_or(words.len());
    String::from_utf16(&words[..end]).ok()
}
