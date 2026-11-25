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
    Apply { candidate_id: String },
    Rollback { recovery_id: String },
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct Recovery {
    pub(crate) id: String,
    pub(crate) version: String,
    pub(crate) can_restore: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Failure {
    ValidationFailed,
    Busy,
    Unavailable,
    SourceFailed,
    InvalidRelease,
    #[cfg(windows)]
    FullInstallerRequired,
    CandidateChanged,
    StorageFailed,
    DownloadFailed,
    IntegrityFailed,
    PreparationFailed,
    EngineRunning,
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
    #[cfg(all(windows, not(debug_assertions)))]
    Exiting,
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
    can_apply: bool,
    recovery: Option<Recovery>,
}

#[cfg(all(windows, not(debug_assertions)))]
impl Reply {
    pub(crate) fn installed_actions(
        &mut self,
        installation: &super::update_install::Installation,
        stopped: bool,
    ) {
        if self.status == Status::Error {
            return;
        }
        self.can_apply = self.can_apply && installation.can_apply();
        self.recovery = installation.recovery().map(|mut item| {
            item.can_restore = stopped;
            item
        });
    }
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
    #[cfg(all(windows, not(debug_assertions)))]
    pending: Mutex<Option<PendingInstall>>,
}

#[cfg(all(windows, not(debug_assertions)))]
struct PendingInstall {
    launch: super::update_install::Launch,
    armed: bool,
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
            #[cfg(all(windows, not(debug_assertions)))]
            pending: Mutex::default(),
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
            can_apply: false,
            recovery: None,
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
            Action::Apply { candidate_id }
            | Action::Rollback {
                recovery_id: candidate_id,
            } => {
                return self.error(
                    &id,
                    if valid_id(&candidate_id) {
                        Failure::Unavailable
                    } else {
                        Failure::ValidationFailed
                    },
                );
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
                let reply = self.snapshot(
                    id,
                    status,
                    Some(candidate.update.channel()),
                    Some(CandidateView {
                        id: candidate.id.to_string(),
                        version: candidate.update.version().to_string(),
                        notes: candidate.update.notes().to_owned(),
                        size_bytes: candidate.update.installer_size(),
                    }),
                );
                #[cfg(all(windows, not(debug_assertions)))]
                let reply = Reply {
                    can_apply: status == Status::Ready
                        && candidate.update.application_size().is_some(),
                    ..reply
                };
                reply
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
            can_apply: false,
            recovery: None,
        }
    }
}

#[cfg(all(windows, not(debug_assertions)))]
impl UpdateFlow {
    pub(crate) fn queue_install(
        &self,
        launch: super::update_install::Launch,
    ) -> Result<(), Failure> {
        let mut pending = self.0.pending.try_lock().map_err(|_| Failure::Busy)?;
        if pending.is_some() {
            return Err(Failure::Busy);
        }
        *pending = Some(PendingInstall {
            launch,
            armed: false,
        });
        Ok(())
    }

    pub(crate) fn arm_install(&self) -> bool {
        let mut pending = self
            .0
            .pending
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let Some(pending) = pending.as_mut() else {
            return false;
        };
        pending.armed = true;
        true
    }

    pub(crate) fn finish_install(&self, clean: bool) -> bool {
        let launch = self
            .0
            .pending
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .take();
        match launch {
            Some(pending) if clean && pending.armed => pending.launch.spawn().is_ok(),
            _ => true,
        }
    }

    pub(crate) fn cancel_install(&self) {
        self.0
            .pending
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .take();
    }

    pub(crate) fn exiting(&self, id: String) -> Reply {
        self.snapshot(id, Status::Exiting, None, None)
    }

    pub(crate) fn prepare_install(
        &self,
        request: &Request,
        installation: &super::update_install::Installation,
    ) -> Result<super::update_install::Launch, Failure> {
        if !valid_id(&request.request_id) {
            return Err(Failure::ValidationFailed);
        }
        match &request.action {
            Action::Apply { candidate_id } => {
                if !valid_id(candidate_id) {
                    return Err(Failure::ValidationFailed);
                }
                let mut phase = self.0.phase.try_lock().map_err(|_| Failure::Busy)?;
                let Phase::Candidate(candidate) = &mut *phase else {
                    return Err(Failure::CandidateChanged);
                };
                if candidate.id.to_string() != *candidate_id
                    || candidate.update.application_size().is_none()
                {
                    return Err(Failure::CandidateChanged);
                }
                let staged = candidate.staged.as_mut().ok_or(Failure::CandidateChanged)?;
                installation
                    .prepare(&candidate.update, staged)
                    .map_err(|_| Failure::PreparationFailed)
            }
            Action::Rollback { recovery_id } if valid_id(recovery_id) => installation
                .prepare_restore(
                    Uuid::parse_str(recovery_id).map_err(|_| Failure::ValidationFailed)?,
                )
                .map_err(|_| Failure::PreparationFailed),
            _ => Err(Failure::ValidationFailed),
        }
    }
}

#[cfg(all(windows, not(debug_assertions)))]
impl Request {
    pub(crate) fn changes_installation(&self) -> bool {
        matches!(self.action, Action::Apply { .. } | Action::Rollback { .. })
    }
}

#[cfg(windows)]
#[path = "update_source.rs"]
mod source;
#[cfg(windows)]
use source::ReleaseSource;

#[cfg(all(windows, any(test, not(debug_assertions))))]
pub(super) fn matches_configured_release(base: &str, public_key: &str) -> bool {
    ReleaseSource::configured().is_some_and(|source| source.matches_release(base, public_key))
}

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
