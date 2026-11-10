use super::update::{UpdateChannel, UpdateError, UpdatePolicy, VerifiedUpdate};
use serde::{Deserialize, Serialize};
use std::fs::File;
#[cfg(not(windows))]
use std::path::Path;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, TryLockError};
use uuid::Uuid;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Request {
    pub(crate) request_id: String,
    action: Action,
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum Action {
    Snapshot,
    Check { channel: UpdateChannel },
    Download { candidate_id: String },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Failure {
    ValidationFailed,
    Busy,
    Unavailable,
    SourceFailed,
    InvalidRelease,
    CandidateChanged,
    StorageFailed,
    DownloadFailed,
    IntegrityFailed,
    PreparationFailed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum Status {
    Unavailable,
    Idle,
    Current,
    Available,
    WaitingForEngine,
    Ready,
    Error,
}

#[derive(Debug, Serialize)]
pub(crate) struct Reply {
    schema_version: u8,
    request_id: String,
    installed_version: String,
    status: Status,
    channel: Option<UpdateChannel>,
    candidate: Option<CandidateView>,
    error: Option<Failure>,
}

#[derive(Debug, Serialize)]
struct CandidateView {
    id: String,
    version: String,
    notes: String,
    size_bytes: u64,
}

#[derive(Default)]
enum Phase {
    #[default]
    Idle,
    Current(UpdateChannel),
    Candidate(Candidate),
}

struct Candidate {
    id: Uuid,
    update: VerifiedUpdate,
    staged: Option<File>,
}

#[derive(Clone)]
pub(crate) struct UpdateFlow(Arc<Inner>);

struct Inner {
    source: Option<ReleaseSource>,
    app_identifier: String,
    installed_version: semver::Version,
    app_data: Option<PathBuf>,
    phase: Mutex<Phase>,
}

fn valid_id(id: &str) -> bool {
    Uuid::parse_str(id).is_ok_and(|value| !value.is_nil() && value.to_string() == id)
}

impl UpdateFlow {
    pub(crate) fn new(
        app_identifier: String,
        installed_version: semver::Version,
        app_data: Option<PathBuf>,
    ) -> Self {
        #[cfg(debug_assertions)]
        let app_data = {
            let _ = app_data;
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .map(|root| root.join("work/app-state"))
        };
        Self::with_source(
            app_identifier,
            installed_version,
            app_data,
            ReleaseSource::configured(),
        )
    }

    fn with_source(
        app_identifier: String,
        installed_version: semver::Version,
        app_data: Option<PathBuf>,
        source: Option<ReleaseSource>,
    ) -> Self {
        Self(Arc::new(Inner {
            source,
            app_identifier,
            installed_version,
            app_data,
            phase: Mutex::default(),
        }))
    }

    pub(crate) fn error(&self, request_id: &str, error: Failure) -> Reply {
        Reply {
            schema_version: 1,
            request_id: if valid_id(request_id) {
                request_id.to_owned()
            } else {
                Uuid::nil().to_string()
            },
            installed_version: self.0.installed_version.to_string(),
            status: Status::Error,
            channel: None,
            candidate: None,
            error: Some(error),
        }
    }

    pub(crate) fn handle(&self, request: Request, engine_stopped: impl FnOnce() -> bool) -> Reply {
        let id = request.request_id;
        if !valid_id(&id) {
            return self.error(&id, Failure::ValidationFailed);
        }
        let mut phase = match self.0.phase.try_lock() {
            Ok(value) => value,
            Err(TryLockError::WouldBlock) => return self.error(&id, Failure::Busy),
            Err(TryLockError::Poisoned(_)) => return self.error(&id, Failure::Unavailable),
        };
        let Some(source) = &self.0.source else {
            return self.snapshot(id, Status::Unavailable, None, None);
        };
        match request.action {
            Action::Snapshot => (),
            Action::Check { channel } => {
                *phase = Phase::Idle;
                let policy = UpdatePolicy {
                    app_identifier: &self.0.app_identifier,
                    installed_version: &self.0.installed_version,
                    channel,
                    trusted_tauri_public_key: source.public_key(),
                };
                match source.check(&policy) {
                    Ok(Some(update)) => {
                        *phase = Phase::Candidate(Candidate {
                            id: Uuid::new_v4(),
                            update,
                            staged: None,
                        })
                    }
                    Ok(None) => *phase = Phase::Current(channel),
                    Err(error) => return self.error(&id, error),
                }
            }
            Action::Download { candidate_id } => {
                if !valid_id(&candidate_id) {
                    return self.error(&id, Failure::ValidationFailed);
                }
                let Phase::Candidate(candidate) = &mut *phase else {
                    return self.error(&id, Failure::CandidateChanged);
                };
                if candidate.id.to_string() != candidate_id {
                    return self.error(&id, Failure::CandidateChanged);
                }
                if candidate.staged.is_none() {
                    let Some(app_data) = self.0.app_data.as_deref() else {
                        return self.error(&id, Failure::StorageFailed);
                    };
                    match source.download(app_data, &candidate.update) {
                        Ok(file) => candidate.staged = Some(file),
                        Err(error) => return self.error(&id, error),
                    }
                }
            }
        }
        match &*phase {
            Phase::Idle => self.snapshot(id, Status::Idle, None, None),
            Phase::Current(channel) => self.snapshot(id, Status::Current, Some(*channel), None),
            Phase::Candidate(candidate) => {
                let status = if candidate.staged.is_none() {
                    Status::Available
                } else if engine_stopped() {
                    Status::Ready
                } else {
                    Status::WaitingForEngine
                };
                self.snapshot(
                    id,
                    status,
                    Some(candidate.update.channel()),
                    Some(CandidateView {
                        id: candidate.id.to_string(),
                        version: candidate.update.version().to_string(),
                        notes: candidate.update.notes().to_owned(),
                        size_bytes: candidate.update.installer_size(),
                    }),
                )
            }
        }
    }

    fn snapshot(
        &self,
        id: String,
        status: Status,
        channel: Option<UpdateChannel>,
        candidate: Option<CandidateView>,
    ) -> Reply {
        Reply {
            schema_version: 1,
            request_id: id,
            installed_version: self.0.installed_version.to_string(),
            status,
            channel,
            candidate,
            error: None,
        }
    }
}

#[cfg(windows)]
#[path = "update_source.rs"]
mod source;
#[cfg(windows)]
use source::ReleaseSource;

#[cfg(not(windows))]
struct ReleaseSource;

#[cfg(not(windows))]
impl ReleaseSource {
    fn configured() -> Option<Self> {
        None
    }
    fn public_key(&self) -> &str {
        ""
    }
    fn check(&self, _: &UpdatePolicy<'_>) -> Result<Option<VerifiedUpdate>, Failure> {
        Err(Failure::Unavailable)
    }
    fn download(&self, _: &Path, _: &VerifiedUpdate) -> Result<File, Failure> {
        Err(Failure::Unavailable)
    }
}

#[cfg(all(test, windows))]
#[path = "update_flow_tests.rs"]
mod tests;
