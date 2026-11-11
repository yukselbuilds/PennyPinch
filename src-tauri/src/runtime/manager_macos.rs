use super::{
    health_is_fresh, map_control_error, ControlOperation, ForceStopReport, OptimizationState,
    PublicError, RuntimeLayout, StopFailure,
};
use crate::runtime::ingress_relay::BackendTarget;
use crate::runtime::macos_layout::{MacRuntimeError, COMPILED_MANIFEST_SHA256};
use crate::runtime::macos_session::{private_directory, MacEngineSession};
use crate::runtime::metrics_read::{OwnedMetricsSnapshot, RuntimeMetricsError};
use crate::state::store::canonical_existing_ancestor;
use std::fs::{self, DirBuilder};
use std::io;
use std::os::unix::fs::{DirBuilderExt, MetadataExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;
use uuid::Uuid;

impl RuntimeLayout {
    pub(super) fn from_macos_bundle(
        resource_dir: PathBuf,
        app_data_dir: PathBuf,
    ) -> Result<Self, PublicError> {
        let manifest_sha256 = COMPILED_MANIFEST_SHA256.ok_or(PublicError::EnvironmentBlocked)?;
        if !resource_dir.is_absolute() || !app_data_dir.is_absolute() {
            return Err(PublicError::EnvironmentBlocked);
        }
        let boundary_root =
            fs::canonicalize(resource_dir).map_err(|_| PublicError::EnvironmentBlocked)?;
        let runtime_root = boundary_root.join("runtime");
        if fs::canonicalize(&runtime_root).ok().as_ref() != Some(&runtime_root)
            || !runtime_root.is_dir()
        {
            return Err(PublicError::EnvironmentBlocked);
        }
        let (ancestor, suffix) = canonical_existing_ancestor(&app_data_dir)
            .map_err(|_| PublicError::EnvironmentBlocked)?;
        let state_boundary_root = ancestor.join(suffix);
        if state_boundary_root.starts_with(&runtime_root)
            || runtime_root.starts_with(&state_boundary_root)
        {
            return Err(PublicError::EnvironmentBlocked);
        }
        DirBuilder::new()
            .mode(0o700)
            .recursive(true)
            .create(&state_boundary_root)
            .map_err(|_| PublicError::EnvironmentBlocked)?;
        validate_state_boundary(&state_boundary_root)?;
        let state_root = state_boundary_root.join("runtime");
        let mut layout = Self::from_roots_with_state_boundary(
            boundary_root,
            runtime_root,
            state_boundary_root,
            state_root,
        )?;
        layout.manifest_sha256 = manifest_sha256;
        Ok(layout)
    }

    pub(super) fn state_runtime_root(&self) -> Result<PathBuf, PublicError> {
        validate_state_boundary(&self.state_boundary_root)?;
        match DirBuilder::new().mode(0o700).create(&self.state_root) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(_) => return Err(PublicError::EnvironmentBlocked),
        }
        private_directory(&self.state_root).map_err(map_macos_error)?;
        Ok(self.state_root.clone())
    }
}

fn validate_state_boundary(path: &Path) -> Result<(), PublicError> {
    let info = fs::symlink_metadata(path).map_err(|_| PublicError::EnvironmentBlocked)?;
    // SAFETY: geteuid has no memory preconditions.
    if !info.is_dir()
        || info.uid() != unsafe { libc::geteuid() }
        || info.mode() & 0o022 != 0
        || fs::canonicalize(path).ok().as_deref() != Some(path)
    {
        return Err(PublicError::EnvironmentBlocked);
    }
    Ok(())
}

fn map_macos_error(error: MacRuntimeError) -> PublicError {
    match error {
        MacRuntimeError::Unavailable | MacRuntimeError::UnsafeState | MacRuntimeError::Spawn => {
            PublicError::EnvironmentBlocked
        }
        MacRuntimeError::InvalidManifest
        | MacRuntimeError::InvalidProfile
        | MacRuntimeError::Integrity => PublicError::ConfigConflict,
        MacRuntimeError::Busy => PublicError::Busy,
        MacRuntimeError::NotReady => PublicError::EngineNotReady,
        MacRuntimeError::OwnershipLost => PublicError::OwnershipLost,
        MacRuntimeError::Control => PublicError::UnsupportedProtocol,
        MacRuntimeError::Cleanup => PublicError::CleanupFailed,
    }
}

pub(super) struct ControllerSession {
    engine: MacEngineSession,
    pub(super) generation: String,
    pub(super) last_health: Instant,
    pub(super) optimization: Option<OptimizationState>,
    pub(super) protected_tools: Vec<String>,
}

impl ControllerSession {
    pub(super) fn launch(
        layout: &RuntimeLayout,
        _request_id: &str,
        shutting_down: &AtomicBool,
    ) -> Result<Self, PublicError> {
        let protected_tools = layout.protected_tools()?;
        let (mut engine, initial) = MacEngineSession::start(
            &layout.runtime_root,
            &layout.state_root,
            &protected_tools,
            shutting_down,
        )
        .map_err(map_macos_error)?;
        if shutting_down.load(Ordering::Acquire) {
            engine.force_stop().map_err(map_macos_error)?;
            return Err(PublicError::EnvironmentBlocked);
        }
        Ok(Self {
            generation: engine.generation().to_owned(),
            last_health: Instant::now(),
            optimization: Some(initial),
            protected_tools,
            engine,
        })
    }

    pub(super) fn healthy(&mut self) -> bool {
        if !self.backend_is_alive() {
            return false;
        }
        match self.engine.ready() {
            Ok(()) => {
                self.last_health = Instant::now();
                self.optimization = self
                    .optimization_control(&Uuid::new_v4().to_string(), ControlOperation::Read)
                    .ok();
                self.backend_is_alive()
            }
            Err(MacRuntimeError::NotReady) => {
                self.backend_is_alive() && health_is_fresh(self.last_health, Instant::now())
            }
            Err(_) => false,
        }
    }

    pub(super) fn optimization_control(
        &self,
        id: &str,
        operation: ControlOperation,
    ) -> Result<OptimizationState, PublicError> {
        self.engine
            .optimization(id, operation)
            .map_err(map_control_error)
    }

    pub(super) fn capture_metrics(&self) -> Result<OwnedMetricsSnapshot, RuntimeMetricsError> {
        self.engine.capture_metrics()
    }

    pub(super) fn backend_target(&self) -> Result<BackendTarget, PublicError> {
        self.engine.backend_target().map_err(map_macos_error)
    }

    pub(super) fn backend_is_alive(&self) -> bool {
        self.engine.can_route()
    }

    pub(super) fn tcp_listener(&self) -> Option<(u16, u32)> {
        None
    }

    pub(super) fn revoke_backend(&self) {
        self.engine.revoke_route();
    }

    pub(super) fn graceful_stop(&mut self, _request_id: &str) -> Result<(), StopFailure> {
        match self.engine.stop() {
            Ok(true) => Ok(()),
            Ok(false) => Err(StopFailure {
                error: PublicError::CleanupFailed,
                stage: "engine_exit_status",
            }),
            Err(error) => Err(StopFailure {
                error: map_macos_error(error),
                stage: "owned_engine_cleanup",
            }),
        }
    }

    pub(super) fn force_stop(&mut self) -> ForceStopReport {
        ForceStopReport {
            owned_child_and_socket_released: self.engine.force_stop().is_ok(),
        }
    }
}
