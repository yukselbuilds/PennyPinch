use super::control::{self, ControlError, ControlOperation, ControlTarget, OptimizationState};
use serde::de::{self, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use std::env;
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(test)]
use std::sync::OnceLock;
use std::sync::{mpsc, Arc, Mutex, RwLock, Weak};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use uuid::Uuid;

#[cfg(target_os = "macos")]
#[path = "manager_macos.rs"]
mod macos;
#[cfg(target_os = "macos")]
use macos::ControllerSession;

const DTO_SCHEMA_VERSION: u8 = 1;
const HELPER_PROTOCOL_VERSION: u8 = 1;
const MAX_HELPER_FRAME_BYTES: usize = 8192;
const STARTUP_TIMEOUT: Duration = Duration::from_secs(90);
const STOP_EVENT_TIMEOUT: Duration = Duration::from_secs(30);
const PROCESS_EXIT_TIMEOUT: Duration = Duration::from_secs(30);
const FORCE_EXIT_TIMEOUT: Duration = Duration::from_secs(5);
const READER_JOIN_TIMEOUT: Duration = Duration::from_millis(750);
// Commands wake the actor immediately; passive probes stay inside the 5s health window.
const ACTOR_POLL_INTERVAL: Duration = Duration::from_secs(2);
const HEALTH_EXPIRY: Duration = Duration::from_secs(5);
const HEALTH_IO_TIMEOUT: Duration = Duration::from_millis(300);
const PORT_CLOSE_TIMEOUT: Duration = Duration::from_secs(3);
const START_REPLY_TIMEOUT: Duration = Duration::from_secs(100);
const STOP_REPLY_TIMEOUT: Duration = Duration::from_secs(90);
const SHUTDOWN_REPLY_TIMEOUT: Duration = Duration::from_secs(90);
const INGRESS_DRAIN_TIMEOUT: Duration = Duration::from_secs(3);
const INGRESS_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(1);
const ACTOR_QUEUE_CAPACITY: usize = 8;

#[cfg(test)]
#[derive(Clone, Debug)]
struct StartupDiagnostic {
    stage: &'static str,
    generation_valid: Option<bool>,
    process_chain_valid: Option<bool>,
    listener_owner_match: Option<bool>,
    job_active_processes: Option<u32>,
    job_member_kinds: Option<Vec<&'static str>>,
    controller_in_job: Option<bool>,
    helper_in_job: Option<bool>,
    engine_in_job: Option<bool>,
    controller_handle_live: Option<bool>,
    helper_handle_live: Option<bool>,
    engine_handle_live: Option<bool>,
    lease_active: Option<bool>,
    lease_generation_match: Option<bool>,
    lease_alive: Option<bool>,
    relay_accepting: Option<bool>,
    route_generation_match: Option<bool>,
}

#[cfg(test)]
impl Default for StartupDiagnostic {
    fn default() -> Self {
        Self {
            stage: "not_started",
            generation_valid: None,
            process_chain_valid: None,
            listener_owner_match: None,
            job_active_processes: None,
            job_member_kinds: None,
            controller_in_job: None,
            helper_in_job: None,
            engine_in_job: None,
            controller_handle_live: None,
            helper_handle_live: None,
            engine_handle_live: None,
            lease_active: None,
            lease_generation_match: None,
            lease_alive: None,
            relay_accepting: None,
            route_generation_match: None,
        }
    }
}

#[cfg(test)]
fn startup_diagnostic_cell() -> &'static Mutex<StartupDiagnostic> {
    static DIAGNOSTIC: OnceLock<Mutex<StartupDiagnostic>> = OnceLock::new();
    DIAGNOSTIC.get_or_init(|| Mutex::new(StartupDiagnostic::default()))
}

#[cfg(test)]
fn set_startup_diagnostic_stage(stage: &'static str) {
    if let Ok(mut diagnostic) = startup_diagnostic_cell().lock() {
        diagnostic.stage = stage;
    }
}

#[cfg(test)]
fn update_startup_diagnostic(update: impl FnOnce(&mut StartupDiagnostic)) {
    if let Ok(mut diagnostic) = startup_diagnostic_cell().lock() {
        update(&mut diagnostic);
    }
}

#[cfg(test)]
fn reset_startup_diagnostic() {
    if let Ok(mut diagnostic) = startup_diagnostic_cell().lock() {
        *diagnostic = StartupDiagnostic::default();
    }
}

#[cfg(test)]
fn startup_diagnostic_for_test() -> StartupDiagnostic {
    startup_diagnostic_cell()
        .lock()
        .map(|diagnostic| diagnostic.clone())
        .unwrap_or_default()
}

pub(crate) const EXPECTED_ENGINE_VERSION: &str = "0.44.0";
const EXPECTED_MANIFEST_SHA256: &str =
    "146177205032b3948cab344d86ed2fd69d171bff8d6a9f77f77fd9459ad13a4b";
const EXPECTED_WINDOWS_MANIFEST_SHA256: &str =
    "c1254c77956d49c702a46fccab85dca6b41695c126cc637efaedf5e816bfac04";
pub(crate) const ACTIVE_MANIFEST: &str = if cfg!(debug_assertions) {
    include_str!("../../../runtime/manifest.json")
} else {
    include_str!("../../../runtime/manifest.windows.json")
};
const ACTIVE_MANIFEST_SHA256: &str = if cfg!(debug_assertions) {
    EXPECTED_MANIFEST_SHA256
} else {
    EXPECTED_WINDOWS_MANIFEST_SHA256
};
const EXPECTED_CONTROLLER_SHA256: &str =
    "b7bf3642b41b592955de3537220ae0dacc60d681e673d9092946a7a8eb442936";
const EXPECTED_HELPER_SHA256: &str =
    "0fa23e60d7b4191d44dc672d216da80d80338423a2c1a0360bae2bee0b4b2249";
#[cfg(windows)]
const EXPECTED_RETRIEVAL_BRIDGE_SHA256: &str =
    "163a28b73967782668f557ae509c45b55c63087eb4523ba7a294aaf9839a3b5a";
const EXPECTED_SHIM_SHA256: &str =
    "9e47ad68053289d7d02ec942927d5f0c2d14e9d09a867333211f24646f371a8b";
const EXPECTED_OPTIMIZATION_SHA256: &str =
    "03b46c263d839ba29ffdf3a399367b75261ab73d376fc3bf90220b074e0df27f";
const EXPECTED_OPTIMIZATION_SOURCES_SHA256: &str =
    "d46b875b93c16b2ad27dc44b0537652d99fd6b6981283e05ad3b7ddb00164a3f";
pub(crate) const EXPECTED_RUNTIME_MANIFEST_SHA256: &str =
    "e7274bed1e89c18b74c690c5bd414eabbd1579f7352d8a0094477804559d3dbe";
const EXPECTED_RUNNER_SHA256: &str =
    "86de03a4eca728c108a12293b498b0012890812aa490e9f2fd516550455ff5ee";
const EXPECTED_PYTHON_SHA256: &str =
    "282c2635fbd49b4aeefda4a965d84a745508c87e6e5eb95947efd0b5a1c1fb07";
const EXPECTED_PREFIX_SHA256: &str =
    "29c7d63d8b63ed3dc4906bd3762ff672f5941947f8f8803180c5d1f13a8bbec0";
const EXPECTED_PYTHON_VERSION: &str = "3.13.13";
const EXPECTED_SITE_PACKAGES: &str = "Lib/site-packages";
const EXPECTED_RUNNER: &str = "headroom_runtime.py";
const EXPECTED_PYTHON: &str = "python.exe";
const EXPECTED_PREFIX: &str = "Lib/site-packages/headroom/cache/prefix_tracker.py";

#[derive(Serialize)]
#[serde(untagged)]
pub enum RuntimeReply {
    Snapshot(RuntimeSnapshot),
    Error(SnapshotError),
}

#[derive(Serialize)]
#[serde(untagged)]
pub enum IngressReply {
    Snapshot(IngressSnapshot),
    Error(IngressSnapshotError),
}

#[derive(Serialize)]
pub struct IngressSnapshot {
    schema_version: u8,
    request_id: String,
    snapshot_id: String,
    observed_at_utc: String,
    endpoint: IngressEndpointState,
    backend: IngressBackendState,
    active_connection_count: Option<u64>,
}

#[derive(Serialize)]
pub struct IngressSnapshotError {
    schema_version: u8,
    request_id: String,
    status: &'static str,
    error: ErrorDetail,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ReadyStableEndpoint {
    pub(crate) endpoint_id: String,
    pub(crate) host: &'static str,
    pub(crate) port: u16,
}

pub(crate) struct CodexRuntimeObservation {
    pub(crate) endpoint_status: &'static str,
    pub(crate) endpoint_id: Option<String>,
    pub(crate) endpoint_port: Option<u16>,
    pub(crate) endpoint_ready: bool,
    pub(crate) engine_status: &'static str,
    pub(crate) engine_ready: bool,
    pub(crate) runtime_generation: Option<String>,
    pub(crate) active_connection_count: Option<u64>,
    pub(crate) error_code: Option<&'static str>,
}

#[derive(Serialize)]
pub(crate) struct CatalogRuntime {
    pub(crate) pinned_version: &'static str,
    pub(crate) manifest_sha256: &'static str,
    pub(crate) active_version: Option<String>,
    pub(crate) engine_status: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum StableEndpointAccessError {
    Unavailable,
    Stale,
    Invalid,
    BackendNotReady,
}

#[derive(Serialize)]
struct IngressEndpointState {
    status: &'static str,
    endpoint_id: Option<String>,
    address: Option<IngressAddress>,
    error_code: Option<&'static str>,
}

#[derive(Serialize)]
struct IngressAddress {
    host: &'static str,
    port: u16,
}

#[derive(Serialize)]
struct IngressBackendState {
    status: &'static str,
    generation: Option<String>,
    error_code: Option<&'static str>,
}

#[derive(Serialize)]
pub struct RuntimeSnapshot {
    schema_version: u8,
    request_id: String,
    snapshot_id: String,
    observed_at_utc: String,
    runtime_generation: Option<String>,
    engine_version: Option<String>,
    engine: EngineState,
    compression: CompressionState,
    discovery: DiscoveryState,
    metrics: MetricsState,
    capabilities: Capabilities,
}

#[derive(Serialize)]
pub struct SnapshotError {
    schema_version: u8,
    request_id: String,
    status: &'static str,
    error: ErrorDetail,
}

#[derive(Serialize)]
struct ErrorDetail {
    code: &'static str,
    retryable: bool,
    message_key: &'static str,
    diagnostic_id: Option<String>,
}

#[derive(Serialize)]
struct EngineState {
    status: &'static str,
    error_code: Option<&'static str>,
}

#[derive(Serialize)]
struct CompressionState {
    status: &'static str,
    active_stream_count: Option<u64>,
    revision: Option<u64>,
}

#[derive(Serialize)]
struct DiscoveryState {
    status: &'static str,
    clients: Vec<String>,
}

#[derive(Serialize)]
struct MetricsState {
    status: &'static str,
    before_total: Option<u64>,
    after_total: Option<u64>,
    saved_total: Option<i64>,
    saved_percent: Option<f64>,
    request_count: Option<u64>,
    covered_count: Option<u64>,
}

#[derive(Serialize)]
struct Capabilities {
    start: bool,
    stop: bool,
    pause: bool,
    connect: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum PublicError {
    ValidationFailed,
    UnsupportedProtocol,
    EnvironmentBlocked,
    Busy,
    PortConflict,
    EngineNotReady,
    OwnershipLost,
    Timeout,
    CleanupFailed,
    ConfigConflict,
}

#[derive(Clone, Copy, Debug)]
struct StopFailure {
    error: PublicError,
    #[allow(dead_code)]
    stage: &'static str,
}

#[cfg(not(target_os = "macos"))]
#[derive(Clone, Copy, Debug)]
struct ForceStopReport {
    controller_exited: bool,
    process_tree_absent: bool,
    port_closed: bool,
    job_empty: bool,
    reader_stopped: bool,
}

#[cfg(target_os = "macos")]
#[derive(Clone, Copy, Debug)]
struct ForceStopReport {
    owned_child_and_socket_released: bool,
}

impl ForceStopReport {
    fn clean(self) -> bool {
        #[cfg(not(target_os = "macos"))]
        {
            self.controller_exited
                && self.process_tree_absent
                && self.port_closed
                && self.job_empty
                && self.reader_stopped
        }
        #[cfg(target_os = "macos")]
        self.owned_child_and_socket_released
    }
}

impl PublicError {
    fn code(self) -> &'static str {
        match self {
            Self::ValidationFailed => "validation_failed",
            Self::UnsupportedProtocol => "unsupported_protocol",
            Self::EnvironmentBlocked => "environment_blocked",
            Self::Busy => "busy",
            Self::PortConflict => "port_conflict",
            Self::EngineNotReady => "engine_not_ready",
            Self::OwnershipLost => "ownership_lost",
            Self::Timeout => "timeout",
            Self::CleanupFailed => "cleanup_failed",
            Self::ConfigConflict => "config_conflict",
        }
    }

    fn retryable(self) -> bool {
        matches!(
            self,
            Self::Busy
                | Self::PortConflict
                | Self::EngineNotReady
                | Self::Timeout
                | Self::CleanupFailed
        )
    }

    fn message_key(self) -> &'static str {
        match self {
            Self::ValidationFailed => "runtime.command.validation_failed",
            Self::UnsupportedProtocol => "runtime.command.unsupported_protocol",
            Self::EnvironmentBlocked => "runtime.command.environment_blocked",
            Self::Busy => "runtime.command.busy",
            Self::PortConflict => "runtime.command.port_conflict",
            Self::EngineNotReady => "runtime.command.engine_not_ready",
            Self::OwnershipLost => "runtime.command.ownership_lost",
            Self::Timeout => "runtime.command.timeout",
            Self::CleanupFailed => "runtime.command.cleanup_failed",
            Self::ConfigConflict => "runtime.command.config_conflict",
        }
    }
}

fn snapshot_error(request_id: &str, error: PublicError) -> RuntimeReply {
    RuntimeReply::Error(SnapshotError {
        schema_version: DTO_SCHEMA_VERSION,
        request_id: if valid_request_id(request_id) {
            request_id.to_owned()
        } else {
            String::new()
        },
        status: "error",
        error: ErrorDetail {
            code: error.code(),
            retryable: error.retryable(),
            message_key: error.message_key(),
            diagnostic_id: None,
        },
    })
}

fn map_control_error(error: ControlError) -> PublicError {
    match error {
        ControlError::InvalidInput => PublicError::ValidationFailed,
        ControlError::InvalidResponse => PublicError::UnsupportedProtocol,
        ControlError::Unavailable => PublicError::EnvironmentBlocked,
        ControlError::OwnershipLost => PublicError::OwnershipLost,
        ControlError::Transport => PublicError::Timeout,
        ControlError::ConfigConflict => PublicError::ConfigConflict,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum EngineStatus {
    Unavailable,
    Stopped,
    Starting,
    Ready,
    Stopping,
    Failed,
    Unknown,
}

impl EngineStatus {
    fn label(self) -> &'static str {
        match self {
            Self::Unavailable => "unavailable",
            Self::Stopped => "stopped",
            Self::Starting => "starting",
            Self::Ready => "ready",
            Self::Stopping => "stopping",
            Self::Failed => "failed",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Clone)]
struct RuntimeView {
    protected_tools: Option<Vec<String>>,
    optimization: Option<OptimizationState>,
    status: EngineStatus,
    error_code: Option<&'static str>,
    cleanup_uncertain: bool,
    runtime_generation: Option<String>,
    engine_version: Option<String>,
    listener_port: Option<u16>,
    listener_owner_pid: Option<u32>,
    last_health: Option<Instant>,
    #[cfg(test)]
    stop_diagnostics: Option<(StopFailure, ForceStopReport)>,
}

impl RuntimeView {
    fn new(status: EngineStatus) -> Self {
        Self {
            optimization: None,
            protected_tools: None,
            status,
            error_code: None,
            cleanup_uncertain: false,
            runtime_generation: None,
            engine_version: None,
            listener_port: None,
            listener_owner_pid: None,
            last_health: None,
            #[cfg(test)]
            stop_diagnostics: None,
        }
    }

    fn snapshot(&self, request_id: String) -> RuntimeReply {
        let now = match OffsetDateTime::now_utc().format(&Rfc3339) {
            Ok(value) => value,
            Err(_) => return snapshot_error(&request_id, PublicError::EnvironmentBlocked),
        };
        let active_generation = self.runtime_generation.is_some();
        let start = !self.cleanup_uncertain
            && matches!(self.status, EngineStatus::Stopped | EngineStatus::Failed);
        RuntimeReply::Snapshot(RuntimeSnapshot {
            schema_version: DTO_SCHEMA_VERSION,
            request_id,
            snapshot_id: Uuid::new_v4().to_string(),
            observed_at_utc: now,
            runtime_generation: self.runtime_generation.clone(),
            engine_version: self.engine_version.clone(),
            engine: EngineState {
                status: self.status.label(),
                error_code: self.error_code,
            },
            compression: CompressionState {
                status: if self.status == EngineStatus::Ready {
                    self.optimization.map_or("unknown", |state| {
                        if state.enabled {
                            "enabled"
                        } else {
                            "paused"
                        }
                    })
                } else {
                    "unavailable"
                },
                active_stream_count: None,
                revision: self
                    .optimization
                    .filter(|_| self.status == EngineStatus::Ready)
                    .map(|state| state.revision),
            },
            discovery: DiscoveryState {
                status: "not_scanned",
                clients: Vec::new(),
            },
            metrics: MetricsState {
                status: "waiting",
                before_total: None,
                after_total: None,
                saved_total: None,
                saved_percent: None,
                request_count: None,
                covered_count: None,
            },
            capabilities: Capabilities {
                start,
                stop: active_generation,
                pause: self.status == EngineStatus::Ready
                    && self
                        .optimization
                        .is_some_and(|state| !state.enabled || state.can_pause),
                connect: false,
            },
        })
    }
}

#[derive(Clone)]
struct IngressView {
    endpoint_status: &'static str,
    endpoint_id: Option<String>,
    address: Option<IngressAddressValue>,
    endpoint_error_code: Option<&'static str>,
    last_endpoint_observation: Option<Instant>,
    route_status: &'static str,
    route_generation: Option<String>,
    route_error_code: Option<&'static str>,
    active_connection_count: Option<u64>,
}

#[derive(Clone)]
struct IngressAddressValue {
    host: &'static str,
    port: u16,
}

impl IngressView {
    fn starting() -> Self {
        Self {
            endpoint_status: "starting",
            endpoint_id: None,
            address: None,
            endpoint_error_code: None,
            last_endpoint_observation: None,
            route_status: "none",
            route_generation: None,
            route_error_code: None,
            active_connection_count: Some(0),
        }
    }

    fn unavailable(error: PublicError, endpoint_status: &'static str) -> Self {
        Self {
            endpoint_status,
            endpoint_id: None,
            address: None,
            endpoint_error_code: Some(error.code()),
            last_endpoint_observation: None,
            route_status: "unavailable",
            route_generation: None,
            route_error_code: Some(error.code()),
            active_connection_count: None,
        }
    }

    fn ready_stable_endpoint(
        &self,
        runtime: &RuntimeView,
        now: Instant,
    ) -> Result<ReadyStableEndpoint, StableEndpointAccessError> {
        if self.endpoint_error_code == Some(PublicError::Timeout.code()) {
            return Err(StableEndpointAccessError::Stale);
        }
        if self.endpoint_status != "ready" || self.endpoint_error_code.is_some() {
            return Err(StableEndpointAccessError::Unavailable);
        }
        if !self
            .last_endpoint_observation
            .is_some_and(|last| health_is_fresh(last, now))
        {
            return Err(StableEndpointAccessError::Stale);
        }

        let endpoint_id = self
            .endpoint_id
            .as_deref()
            .filter(|value| valid_request_id(value))
            .ok_or(StableEndpointAccessError::Invalid)?;
        let address = self
            .address
            .as_ref()
            .ok_or(StableEndpointAccessError::Invalid)?;
        if address.host != "127.0.0.1" || address.port == 0 {
            return Err(StableEndpointAccessError::Invalid);
        }

        match runtime.status {
            EngineStatus::Stopped
                if runtime.error_code.is_none()
                    && self.route_status == "none"
                    && self.route_generation.is_none()
                    && self.route_error_code.is_none() => {}
            EngineStatus::Ready => {
                let generation = runtime
                    .runtime_generation
                    .as_deref()
                    .filter(|value| valid_generation(value))
                    .ok_or(StableEndpointAccessError::BackendNotReady)?;
                if runtime.error_code.is_some()
                    || !runtime
                        .last_health
                        .is_some_and(|last| health_is_fresh(last, now))
                    || self.route_status != "ready"
                    || self.route_error_code.is_some()
                    || self.route_generation.as_deref() != Some(generation)
                {
                    return Err(StableEndpointAccessError::BackendNotReady);
                }
            }
            _ => return Err(StableEndpointAccessError::BackendNotReady),
        }

        Ok(ReadyStableEndpoint {
            endpoint_id: endpoint_id.to_owned(),
            host: address.host,
            port: address.port,
        })
    }

    fn snapshot(&self, request_id: String, runtime: &RuntimeView) -> IngressReply {
        let health_expired = runtime.status == EngineStatus::Unknown
            && runtime.error_code == Some(PublicError::Timeout.code());
        let endpoint_unavailable = self.endpoint_status != "ready";
        let observed_at_utc = match OffsetDateTime::now_utc().format(&Rfc3339) {
            Ok(value) => value,
            Err(_) => return ingress_snapshot_error(&request_id, PublicError::EnvironmentBlocked),
        };
        let (backend_status, generation, error_code) = match runtime.status {
            EngineStatus::Unavailable => (
                "unavailable",
                None,
                runtime.error_code.or(self.route_error_code),
            ),
            EngineStatus::Stopped => (
                if self.route_status == "none" {
                    "stopped"
                } else {
                    "unknown"
                },
                None,
                if self.route_status == "none" {
                    None
                } else {
                    Some(PublicError::OwnershipLost.code())
                },
            ),
            EngineStatus::Starting => ("starting", None, None),
            EngineStatus::Ready => {
                let generation_matches =
                    runtime.runtime_generation.as_deref() == self.route_generation.as_deref();
                if self.endpoint_status == "ready"
                    && self.route_status == "ready"
                    && generation_matches
                {
                    ("ready", runtime.runtime_generation.clone(), None)
                } else {
                    (
                        "unknown",
                        None,
                        Some(
                            self.endpoint_error_code
                                .unwrap_or(PublicError::OwnershipLost.code()),
                        ),
                    )
                }
            }
            EngineStatus::Stopping => ("draining", runtime.runtime_generation.clone(), None),
            EngineStatus::Failed => (
                "failed",
                runtime.runtime_generation.clone(),
                runtime.error_code.or(self.route_error_code),
            ),
            EngineStatus::Unknown => (
                "unknown",
                runtime.runtime_generation.clone(),
                runtime.error_code.or(self.route_error_code),
            ),
        };
        let snapshot = IngressSnapshot {
            schema_version: DTO_SCHEMA_VERSION,
            request_id,
            snapshot_id: Uuid::new_v4().to_string(),
            observed_at_utc,
            endpoint: IngressEndpointState {
                status: self.endpoint_status,
                endpoint_id: self.endpoint_id.clone(),
                address: self.address.as_ref().map(|address| IngressAddress {
                    host: address.host,
                    port: address.port,
                }),
                error_code: self.endpoint_error_code,
            },
            backend: IngressBackendState {
                status: backend_status,
                generation,
                error_code,
            },
            active_connection_count: if health_expired || endpoint_unavailable {
                None
            } else {
                self.active_connection_count
            },
        };
        IngressReply::Snapshot(snapshot)
    }
}

fn ingress_snapshot_error(request_id: &str, error: PublicError) -> IngressReply {
    IngressReply::Error(IngressSnapshotError {
        schema_version: DTO_SCHEMA_VERSION,
        request_id: if valid_request_id(request_id) {
            request_id.to_owned()
        } else {
            String::new()
        },
        status: "error",
        error: ErrorDetail {
            code: error.code(),
            retryable: error.retryable(),
            message_key: error.message_key(),
            diagnostic_id: None,
        },
    })
}

fn expire_stale_ready_view(view: &mut RuntimeView, now: Instant) {
    if view.status == EngineStatus::Ready
        && !view
            .last_health
            .is_some_and(|last| health_is_fresh(last, now))
    {
        view.status = EngineStatus::Unknown;
        view.error_code = Some(PublicError::Timeout.code());
    }
}

fn expire_stale_ingress_endpoint(view: &mut IngressView, now: Instant) {
    if view.endpoint_status == "ready"
        && !view
            .last_endpoint_observation
            .is_some_and(|last| health_is_fresh(last, now))
    {
        view.endpoint_status = "unknown";
        view.endpoint_error_code = Some(PublicError::Timeout.code());
        view.last_endpoint_observation = None;
        view.active_connection_count = None;
    }
}

fn record_endpoint_observation(view: &mut IngressView, active_connection_count: u64) {
    view.endpoint_status = "ready";
    view.endpoint_error_code = None;
    view.last_endpoint_observation = Some(Instant::now());
    view.active_connection_count = Some(active_connection_count);
}

fn cleanup_result_error(original: PublicError, cleanup_verified: bool) -> PublicError {
    if cleanup_verified {
        original
    } else {
        PublicError::CleanupFailed
    }
}

#[cfg(any(windows, target_os = "macos"))]
fn map_ingress_error(error: super::ingress::IngressError) -> PublicError {
    use super::ingress::IngressError;
    match error {
        IngressError::EndpointConflict => PublicError::PortConflict,
        IngressError::InvalidEndpointState => PublicError::ConfigConflict,
        IngressError::InvalidRoot
        | IngressError::StateUnavailable
        | IngressError::SocketUnavailable => PublicError::EnvironmentBlocked,
        #[cfg(windows)]
        IngressError::PeerOwnerUnknown
        | IngressError::PeerOwnerTableUnavailable
        | IngressError::PeerOwnerMissing
        | IngressError::PeerOwnerMismatch
        | IngressError::PeerOwnerAmbiguous => PublicError::OwnershipLost,
    }
}

#[derive(Clone)]
struct RuntimeLayout {
    preferences: Option<crate::state::store::PreferenceStore>,
    boundary_root: PathBuf,
    state_boundary_root: PathBuf,
    runtime_root: PathBuf,
    manifest_path: PathBuf,
    manifest_sha256: &'static str,
    state_root: PathBuf,
}

impl RuntimeLayout {
    fn resolve(
        resource_dir: Option<PathBuf>,
        app_data_dir: Option<PathBuf>,
    ) -> Result<Self, PublicError> {
        if cfg!(debug_assertions) && !cfg!(target_os = "macos") {
            let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
            let project_root = manifest_dir
                .parent()
                .ok_or(PublicError::EnvironmentBlocked)?;
            let boundary_root =
                fs::canonicalize(project_root).map_err(|_| PublicError::EnvironmentBlocked)?;
            let runtime_root = boundary_root.join("runtime");
            let state_root = boundary_root.join("work").join("app-state").join("runtime");
            return Self::from_roots_with_state_boundary(
                boundary_root.clone(),
                runtime_root,
                boundary_root,
                state_root,
            );
        }

        let resource_dir = resource_dir.ok_or(PublicError::EnvironmentBlocked)?;
        let app_data_dir = app_data_dir.ok_or(PublicError::EnvironmentBlocked)?;
        #[cfg(not(target_os = "macos"))]
        return Self::from_bundle(resource_dir, app_data_dir);
        #[cfg(target_os = "macos")]
        Self::from_macos_bundle(resource_dir, app_data_dir)
    }

    fn from_bundle(resource_dir: PathBuf, app_data_dir: PathBuf) -> Result<Self, PublicError> {
        let boundary_root =
            fs::canonicalize(resource_dir).map_err(|_| PublicError::EnvironmentBlocked)?;
        let runtime_root = boundary_root.join("runtime");
        let state_boundary_root = canonical_platform_directory(app_data_dir)?;
        let state_root = state_boundary_root.join("runtime");
        let mut layout = Self::from_roots_with_state_boundary(
            boundary_root,
            runtime_root,
            state_boundary_root,
            state_root,
        )?;
        layout.manifest_sha256 = EXPECTED_WINDOWS_MANIFEST_SHA256;
        Ok(layout)
    }

    #[cfg(test)]
    fn from_roots(
        boundary_root: PathBuf,
        runtime_root: PathBuf,
        state_root: PathBuf,
    ) -> Result<Self, PublicError> {
        Self::from_roots_with_state_boundary(
            boundary_root.clone(),
            runtime_root,
            boundary_root,
            state_root,
        )
    }

    fn from_roots_with_state_boundary(
        boundary_root: PathBuf,
        runtime_root: PathBuf,
        state_boundary_root: PathBuf,
        state_root: PathBuf,
    ) -> Result<Self, PublicError> {
        let boundary_root =
            fs::canonicalize(boundary_root).map_err(|_| PublicError::EnvironmentBlocked)?;
        let state_boundary_root =
            fs::canonicalize(state_boundary_root).map_err(|_| PublicError::EnvironmentBlocked)?;
        let runtime_root =
            fs::canonicalize(runtime_root).map_err(|_| PublicError::EnvironmentBlocked)?;
        if !runtime_root.starts_with(&boundary_root) {
            return Err(PublicError::EnvironmentBlocked);
        }
        if !state_root.starts_with(&state_boundary_root) {
            return Err(PublicError::EnvironmentBlocked);
        }
        let manifest_path = runtime_root.join("manifest.json");
        if !manifest_path.is_file() {
            return Err(PublicError::EnvironmentBlocked);
        }
        Ok(Self {
            boundary_root,
            preferences: None,
            state_boundary_root,
            runtime_root,
            manifest_path,
            manifest_sha256: ACTIVE_MANIFEST_SHA256,
            state_root,
        })
    }

    fn state_paths(&self) -> Result<(PathBuf, PathBuf, PathBuf), PublicError> {
        let root = self.state_runtime_root()?;
        let workspace = root.join("workspace");
        let config = root.join("config");
        ensure_existing_ancestor_inside(&self.state_boundary_root, &workspace)?;
        ensure_existing_ancestor_inside(&self.state_boundary_root, &config)?;
        fs::create_dir_all(&workspace).map_err(|_| PublicError::EnvironmentBlocked)?;
        fs::create_dir_all(&config).map_err(|_| PublicError::EnvironmentBlocked)?;
        let workspace = fs::canonicalize(workspace).map_err(|_| PublicError::EnvironmentBlocked)?;
        let config = fs::canonicalize(config).map_err(|_| PublicError::EnvironmentBlocked)?;
        if !workspace.starts_with(&root) || !config.starts_with(&root) {
            return Err(PublicError::EnvironmentBlocked);
        }
        Ok((root, workspace, config))
    }

    #[cfg(not(target_os = "macos"))]
    fn state_runtime_root(&self) -> Result<PathBuf, PublicError> {
        ensure_existing_ancestor_inside(&self.state_boundary_root, &self.state_root)?;
        fs::create_dir_all(&self.state_root).map_err(|_| PublicError::EnvironmentBlocked)?;
        let root =
            fs::canonicalize(&self.state_root).map_err(|_| PublicError::EnvironmentBlocked)?;
        if !root.starts_with(&self.state_boundary_root) {
            return Err(PublicError::EnvironmentBlocked);
        }
        Ok(root)
    }

    fn verify(&self) -> Result<VerifiedRuntime, PublicError> {
        let (payload_root, overlay_controller, python) = self.verify_payload()?;
        let (state_root, workspace, config) = self.state_paths()?;
        Ok(VerifiedRuntime {
            payload_root,
            manifest_path: self.manifest_path.clone(),
            controller_path: overlay_controller,
            state_root,
            workspace,
            config,
            python_path: python,
            protected_tools: self.protected_tools()?,
        })
    }

    fn verify_payload(&self) -> Result<(PathBuf, PathBuf, PathBuf), PublicError> {
        if sha256_file(&self.manifest_path)? != self.manifest_sha256 {
            return Err(PublicError::ConfigConflict);
        }
        let raw_manifest =
            fs::read(&self.manifest_path).map_err(|_| PublicError::EnvironmentBlocked)?;
        let manifest: RuntimeManifest =
            serde_json::from_slice(&raw_manifest).map_err(|_| PublicError::ConfigConflict)?;
        validate_manifest(&manifest)?;

        let payload_root = resolve_payload_root(
            &self.boundary_root,
            &self
                .manifest_path
                .parent()
                .ok_or(PublicError::EnvironmentBlocked)?,
            &manifest.engine.payload_root,
        )?;
        let overlay_controller = resolve_relative(&self.runtime_root, "desktop_controller.py")?;
        let overlay_helper = resolve_relative(&self.runtime_root, "desktop_helper.py")?;
        let overlay_shim = resolve_relative(&self.runtime_root, "cache_control_shim.py")?;
        let overlay_control = resolve_relative(&self.runtime_root, "optimization_control.py")?;
        let overlay_control_sources =
            resolve_relative(&self.runtime_root, "optimization-sources.json")?;
        let packaged_manifest = resolve_relative(&payload_root, &manifest.engine.runtime_manifest)?;
        let runner = resolve_relative(&payload_root, &manifest.engine.runner)?;
        let python = resolve_relative(&payload_root, &manifest.engine.python)?;
        let prefix_tracker = resolve_relative(&payload_root, &manifest.engine.prefix_tracker)?;
        for (path, expected) in [
            (&overlay_controller, EXPECTED_CONTROLLER_SHA256),
            (&overlay_helper, EXPECTED_HELPER_SHA256),
            (&overlay_shim, EXPECTED_SHIM_SHA256),
            (&overlay_control, EXPECTED_OPTIMIZATION_SHA256),
            (
                &overlay_control_sources,
                EXPECTED_OPTIMIZATION_SOURCES_SHA256,
            ),
            (&packaged_manifest, EXPECTED_RUNTIME_MANIFEST_SHA256),
            (&runner, EXPECTED_RUNNER_SHA256),
            (&python, EXPECTED_PYTHON_SHA256),
            (&prefix_tracker, EXPECTED_PREFIX_SHA256),
        ] {
            if sha256_file(path)? != expected {
                return Err(PublicError::ConfigConflict);
            }
        }
        Ok((payload_root, overlay_controller, python))
    }

    fn protected_tools(&self) -> Result<Vec<String>, PublicError> {
        self.preferences.as_ref().map_or(Ok(Vec::new()), |store| {
            store
                .operating()
                .map(|p| p.protected_tools)
                .map_err(|_| PublicError::ConfigConflict)
        })
    }
}

fn canonical_platform_directory(path: PathBuf) -> Result<PathBuf, PublicError> {
    if !path.exists() {
        fs::create_dir_all(&path).map_err(|_| PublicError::EnvironmentBlocked)?;
    }
    fs::canonicalize(path).map_err(|_| PublicError::EnvironmentBlocked)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RuntimeManifest {
    schema_version: u8,
    protocol_version: u8,
    engine: EngineManifest,
    shim: ShimManifest,
    optimization_control: OptimizationManifest,
    profile: ProfileManifest,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EngineManifest {
    name: String,
    version: String,
    payload_root: String,
    runtime_manifest: String,
    runtime_manifest_sha256: String,
    runner: String,
    runner_sha256: String,
    python: String,
    python_version: String,
    python_sha256: String,
    site_packages: String,
    prefix_tracker: String,
    prefix_tracker_sha256: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ShimManifest {
    file: String,
    sha256: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OptimizationManifest {
    file: String,
    sha256: String,
    sources: String,
    sources_sha256: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ProfileManifest {
    mode: String,
    savings_profile: String,
    ccr_ttl_seconds: u64,
    telemetry: String,
    beacon: String,
    output_shaper: String,
    model_router_enabled: String,
    memory: bool,
    learn: bool,
}

fn validate_manifest(manifest: &RuntimeManifest) -> Result<(), PublicError> {
    let engine = &manifest.engine;
    let profile = &manifest.profile;
    if manifest.schema_version != 1
        || manifest.protocol_version != HELPER_PROTOCOL_VERSION
        || engine.name != "headroom-ai"
        || engine.version != EXPECTED_ENGINE_VERSION
        || engine.runtime_manifest_sha256 != EXPECTED_RUNTIME_MANIFEST_SHA256
        || engine.runner != EXPECTED_RUNNER
        || engine.runner_sha256 != EXPECTED_RUNNER_SHA256
        || engine.python != EXPECTED_PYTHON
        || engine.python_version != EXPECTED_PYTHON_VERSION
        || engine.python_sha256 != EXPECTED_PYTHON_SHA256
        || engine.site_packages != EXPECTED_SITE_PACKAGES
        || engine.prefix_tracker != EXPECTED_PREFIX
        || engine.prefix_tracker_sha256 != EXPECTED_PREFIX_SHA256
        || manifest.shim.file != "cache_control_shim.py"
        || manifest.shim.sha256 != EXPECTED_SHIM_SHA256
        || manifest.optimization_control.file != "optimization_control.py"
        || manifest.optimization_control.sha256 != EXPECTED_OPTIMIZATION_SHA256
        || manifest.optimization_control.sources != "optimization-sources.json"
        || manifest.optimization_control.sources_sha256 != EXPECTED_OPTIMIZATION_SOURCES_SHA256
        || profile.mode != "cache"
        || profile.savings_profile != "coding"
        || profile.ccr_ttl_seconds != 7200
        || profile.telemetry != "on"
        || profile.beacon != "off"
        || profile.output_shaper != "0"
        || profile.model_router_enabled != "0"
        || profile.memory
        || profile.learn
    {
        return Err(PublicError::ConfigConflict);
    }
    Ok(())
}

struct VerifiedRuntime {
    protected_tools: Vec<String>,
    payload_root: PathBuf,
    manifest_path: PathBuf,
    controller_path: PathBuf,
    state_root: PathBuf,
    workspace: PathBuf,
    config: PathBuf,
    python_path: PathBuf,
}

fn resolve_relative(root: &Path, relative: &str) -> Result<PathBuf, PublicError> {
    let relative = Path::new(relative);
    if relative.is_absolute() {
        return Err(PublicError::EnvironmentBlocked);
    }
    let root = fs::canonicalize(root).map_err(|_| PublicError::EnvironmentBlocked)?;
    let path =
        fs::canonicalize(root.join(relative)).map_err(|_| PublicError::EnvironmentBlocked)?;
    if !path.starts_with(&root) || !path.is_file() {
        return Err(PublicError::EnvironmentBlocked);
    }
    Ok(path)
}

fn resolve_payload_root(
    boundary: &Path,
    base: &Path,
    relative: &str,
) -> Result<PathBuf, PublicError> {
    let relative = Path::new(relative);
    if relative.is_absolute() {
        return Err(PublicError::EnvironmentBlocked);
    }
    let boundary = fs::canonicalize(boundary).map_err(|_| PublicError::EnvironmentBlocked)?;
    let base = fs::canonicalize(base).map_err(|_| PublicError::EnvironmentBlocked)?;
    let path =
        fs::canonicalize(base.join(relative)).map_err(|_| PublicError::EnvironmentBlocked)?;
    if !base.starts_with(&boundary) || !path.starts_with(&boundary) || !path.is_dir() {
        return Err(PublicError::EnvironmentBlocked);
    }
    Ok(path)
}

fn ensure_existing_ancestor_inside(boundary: &Path, target: &Path) -> Result<(), PublicError> {
    let boundary = fs::canonicalize(boundary).map_err(|_| PublicError::EnvironmentBlocked)?;
    if !target.starts_with(&boundary) {
        return Err(PublicError::EnvironmentBlocked);
    }
    let mut existing = target;
    while !existing.exists() {
        existing = existing.parent().ok_or(PublicError::EnvironmentBlocked)?;
    }
    let resolved = fs::canonicalize(existing).map_err(|_| PublicError::EnvironmentBlocked)?;
    if resolved.starts_with(&boundary) {
        Ok(())
    } else {
        Err(PublicError::EnvironmentBlocked)
    }
}

fn sha256_file(path: &Path) -> Result<String, PublicError> {
    let mut file = File::open(path).map_err(|_| PublicError::EnvironmentBlocked)?;
    let mut digest = Sha256::new();
    let mut buffer = [0u8; 1024 * 1024];
    loop {
        let count = file
            .read(&mut buffer)
            .map_err(|_| PublicError::EnvironmentBlocked)?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    Ok(format!("{:x}", digest.finalize()))
}

fn exact_keys(value: &Value, expected: &[&str]) -> bool {
    let Some(object) = value.as_object() else {
        return false;
    };
    object.len() == expected.len() && expected.iter().all(|key| object.contains_key(*key))
}

fn object_value<'a>(value: &'a Value, key: &str) -> Result<&'a Value, PublicError> {
    value
        .as_object()
        .and_then(|object| object.get(key))
        .ok_or(PublicError::UnsupportedProtocol)
}

fn string_value<'a>(value: &'a Value, key: &str) -> Result<&'a str, PublicError> {
    object_value(value, key)?
        .as_str()
        .ok_or(PublicError::UnsupportedProtocol)
}

fn u64_value(value: &Value, key: &str) -> Result<u64, PublicError> {
    object_value(value, key)?
        .as_u64()
        .ok_or(PublicError::UnsupportedProtocol)
}

fn bool_value(value: &Value, key: &str) -> Result<bool, PublicError> {
    object_value(value, key)?
        .as_bool()
        .ok_or(PublicError::UnsupportedProtocol)
}

fn parse_json_frame(raw: &[u8]) -> Result<Value, PublicError> {
    if raw.len() > MAX_HELPER_FRAME_BYTES
        || raw.last() != Some(&b'\n')
        || raw[..raw.len().saturating_sub(1)].contains(&b'\n')
    {
        return Err(PublicError::UnsupportedProtocol);
    }
    parse_unique_json(&raw[..raw.len() - 1])
}

fn parse_unique_json(raw: &[u8]) -> Result<Value, PublicError> {
    let mut deserializer = serde_json::Deserializer::from_slice(raw);
    let UniqueJson(value) =
        UniqueJson::deserialize(&mut deserializer).map_err(|_| PublicError::UnsupportedProtocol)?;
    deserializer
        .end()
        .map_err(|_| PublicError::UnsupportedProtocol)?;
    Ok(value)
}

struct UniqueJson(Value);

impl<'de> Deserialize<'de> for UniqueJson {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_any(UniqueJsonVisitor)
    }
}

struct UniqueJsonVisitor;

impl<'de> Visitor<'de> for UniqueJsonVisitor {
    type Value = UniqueJson;

    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("a JSON value without duplicate object keys")
    }

    fn visit_bool<E>(self, value: bool) -> Result<Self::Value, E> {
        Ok(UniqueJson(Value::Bool(value)))
    }

    fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E> {
        Ok(UniqueJson(Value::Number(value.into())))
    }

    fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E> {
        Ok(UniqueJson(Value::Number(value.into())))
    }

    fn visit_f64<E: de::Error>(self, value: f64) -> Result<Self::Value, E> {
        serde_json::Number::from_f64(value)
            .map(Value::Number)
            .map(UniqueJson)
            .ok_or_else(|| E::custom("non-finite JSON number"))
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(UniqueJson(Value::String(value.to_owned())))
    }

    fn visit_string<E>(self, value: String) -> Result<Self::Value, E> {
        Ok(UniqueJson(Value::String(value)))
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E> {
        Ok(UniqueJson(Value::Null))
    }

    fn visit_none<E>(self) -> Result<Self::Value, E> {
        Ok(UniqueJson(Value::Null))
    }

    fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        UniqueJson::deserialize(deserializer)
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut values = Vec::new();
        while let Some(UniqueJson(value)) = sequence.next_element()? {
            values.push(value);
        }
        Ok(UniqueJson(Value::Array(values)))
    }

    fn visit_map<A>(self, mut object: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut values = Map::new();
        while let Some(key) = object.next_key::<String>()? {
            if values.contains_key(&key) {
                return Err(de::Error::custom("duplicate object key"));
            }
            let UniqueJson(value) = object.next_value()?;
            values.insert(key, value);
        }
        Ok(UniqueJson(Value::Object(values)))
    }
}

fn parse_common(
    value: &Value,
    expected_request_id: &str,
    expected_generation: Option<&str>,
    event: &str,
    expected_keys: &[&str],
) -> Result<(), PublicError> {
    if !exact_keys(value, expected_keys)
        || u64_value(value, "protocol_version")? != u64::from(HELPER_PROTOCOL_VERSION)
        || string_value(value, "event")? != event
        || string_value(value, "request_id")? != expected_request_id
    {
        return Err(PublicError::UnsupportedProtocol);
    }
    let generation = string_value(value, "runtime_generation")?;
    if !valid_generation(generation)
        || expected_generation.is_some_and(|expected| expected != generation)
    {
        return Err(PublicError::OwnershipLost);
    }
    Ok(())
}

pub(crate) fn valid_generation(value: &str) -> bool {
    value.len() == 32 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

#[derive(Clone, Eq, PartialEq)]
struct ControlSecret(String);
impl std::fmt::Debug for ControlSecret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("[redacted]")
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
struct ReadyInfo {
    protected_tools: Vec<String>,
    control_secret: ControlSecret,
    generation: String,
    helper_pid: u32,
    engine_pid: u32,
    listener_port: u16,
}

fn parse_controller_ready(
    raw: &[u8],
    expected_request_id: &str,
    controller_pid: u32,
) -> Result<ReadyInfo, PublicError> {
    let value = parse_json_frame(raw)?;
    if string_value(&value, "event").ok() == Some("error") {
        return Err(parse_controller_error(&value, expected_request_id, None)?);
    }
    const KEYS: &[&str] = &[
        "protocol_version",
        "request_id",
        "runtime_generation",
        "event",
        "helper_pid",
        "engine_pid",
        "listener_port",
        "listener_pid",
        "engine_version",
        "controller_consoleless",
        "manager_hidden_console",
        "engine_console_allocated",
        "config",
        "shim",
        "optimization_control",
    ];
    parse_common(&value, expected_request_id, None, "ready", KEYS)?;
    let helper_pid = u64_value(&value, "helper_pid")?;
    let engine_pid = u64_value(&value, "engine_pid")?;
    let listener_port = u64_value(&value, "listener_port")?;
    let listener_pid = u64_value(&value, "listener_pid")?;
    if helper_pid == 0
        || engine_pid == 0
        || helper_pid == u64::from(controller_pid)
        || engine_pid == u64::from(controller_pid)
        || helper_pid == engine_pid
        || listener_pid != engine_pid
        || listener_port == 0
        || listener_port > u64::from(u16::MAX)
        || string_value(&value, "engine_version")? != EXPECTED_ENGINE_VERSION
        || !bool_value(&value, "controller_consoleless")?
        || !bool_value(&value, "manager_hidden_console")?
        || !bool_value(&value, "engine_console_allocated")?
    {
        return Err(PublicError::EngineNotReady);
    }
    let config = object_value(&value, "config")?;
    const CONFIG_KEYS: &[&str] = &[
        "mode",
        "savings_profile",
        "ttl_seconds",
        "telemetry_enabled",
        "beacon_enabled",
        "memory_enabled",
        "learning_enabled",
        "output_shaper_enabled",
        "model_router_enabled",
        "protected_tools",
    ];
    if !exact_keys(config, CONFIG_KEYS)
        || string_value(config, "mode")? != "cache"
        || string_value(config, "savings_profile")? != "coding"
        || u64_value(config, "ttl_seconds")? != 7200
        || !bool_value(config, "telemetry_enabled")?
        || bool_value(config, "beacon_enabled")?
        || bool_value(config, "memory_enabled")?
        || bool_value(config, "learning_enabled")?
        || bool_value(config, "output_shaper_enabled")?
        || bool_value(config, "model_router_enabled")?
    {
        return Err(PublicError::ConfigConflict);
    }
    let protected_tools: Vec<String> = serde_json::from_value(config["protected_tools"].clone())
        .map_err(|_| PublicError::UnsupportedProtocol)?;
    if !(crate::state::store::OperatingPreferences {
        protected_tools: protected_tools.clone(),
        ..Default::default()
    })
    .is_valid()
    {
        return Err(PublicError::ConfigConflict);
    }
    let shim = object_value(&value, "shim")?;
    const SHIM_KEYS: &[&str] = &["active", "version", "prefix_tracker_sha256", "shim_sha256"];
    if !exact_keys(shim, SHIM_KEYS)
        || !bool_value(shim, "active")?
        || string_value(shim, "version")? != EXPECTED_ENGINE_VERSION
        || string_value(shim, "prefix_tracker_sha256")? != EXPECTED_PREFIX_SHA256
        || string_value(shim, "shim_sha256")? != EXPECTED_SHIM_SHA256
    {
        return Err(PublicError::ConfigConflict);
    }
    let control = object_value(&value, "optimization_control")?;
    if !exact_keys(
        control,
        &[
            "protocol_version",
            "control_token",
            "module_sha256",
            "sources_sha256",
        ],
    ) || u64_value(control, "protocol_version")? != 1
        || string_value(control, "module_sha256")? != EXPECTED_OPTIMIZATION_SHA256
        || string_value(control, "sources_sha256")? != EXPECTED_OPTIMIZATION_SOURCES_SHA256
        || !super::control::valid_secret(string_value(control, "control_token")?)
    {
        return Err(PublicError::ConfigConflict);
    }
    Ok(ReadyInfo {
        protected_tools,
        control_secret: ControlSecret(string_value(control, "control_token")?.to_owned()),
        generation: string_value(&value, "runtime_generation")?.to_owned(),
        helper_pid: u32::try_from(helper_pid).map_err(|_| PublicError::UnsupportedProtocol)?,
        engine_pid: u32::try_from(engine_pid).map_err(|_| PublicError::UnsupportedProtocol)?,
        listener_port: u16::try_from(listener_port)
            .map_err(|_| PublicError::UnsupportedProtocol)?,
    })
}

fn parse_controller_error(
    value: &Value,
    expected_request_id: &str,
    expected_generation: Option<&str>,
) -> Result<PublicError, PublicError> {
    const KEYS: &[&str] = &[
        "protocol_version",
        "request_id",
        "runtime_generation",
        "event",
        "error_code",
    ];
    if !exact_keys(value, KEYS)
        || u64_value(value, "protocol_version")? != u64::from(HELPER_PROTOCOL_VERSION)
        || string_value(value, "event")? != "error"
    {
        return Err(PublicError::UnsupportedProtocol);
    }
    let request_id = object_value(value, "request_id")?;
    if !request_id.is_null() && request_id.as_str() != Some(expected_request_id) {
        return Err(PublicError::UnsupportedProtocol);
    }
    let generation = object_value(value, "runtime_generation")?;
    if !generation.is_null() {
        let actual = generation
            .as_str()
            .ok_or(PublicError::UnsupportedProtocol)?;
        if !valid_generation(actual)
            || expected_generation.is_some_and(|expected| expected != actual)
        {
            return Err(PublicError::OwnershipLost);
        }
    }
    let code = string_value(value, "error_code")?;
    Ok(match code {
        "unsupported_version" => PublicError::UnsupportedProtocol,
        "generation_mismatch" => PublicError::OwnershipLost,
        "helper_timeout" => PublicError::Timeout,
        "cleanup_failed" => PublicError::CleanupFailed,
        "runtime_pin_mismatch" => PublicError::ConfigConflict,
        "manifest_invalid"
        | "windows_only"
        | "controller_console_required"
        | "helper_start_failed" => PublicError::EnvironmentBlocked,
        "helper_protocol_invalid" => PublicError::UnsupportedProtocol,
        "engine_readiness_failed" | "engine_start_failed" => PublicError::EngineNotReady,
        "frame_too_large"
        | "invalid_json"
        | "invalid_shape"
        | "invalid_request_id"
        | "unknown_event"
        | "invalid_state"
        | "effective_config_failed"
        | "shutdown_incomplete" => PublicError::EngineNotReady,
        "port_conflict" => PublicError::PortConflict,
        _ => return Err(PublicError::UnsupportedProtocol),
    })
}

fn parse_controller_stopped(
    raw: &[u8],
    expected_request_id: &str,
    expected_generation: &str,
) -> Result<(), PublicError> {
    let value = parse_json_frame(raw)?;
    if string_value(&value, "event").ok() == Some("error") {
        return Err(parse_controller_error(
            &value,
            expected_request_id,
            Some(expected_generation),
        )?);
    }
    const KEYS: &[&str] = &[
        "protocol_version",
        "request_id",
        "runtime_generation",
        "event",
        "shutdown",
        "engine_exit_code",
        "owned_port_closed",
        "hidden_console_verified",
        "lifespan_complete",
        "proxy_summary",
        "force_fallback",
    ];
    parse_common(
        &value,
        expected_request_id,
        Some(expected_generation),
        "stopped",
        KEYS,
    )?;
    if string_value(&value, "shutdown")? != "graceful"
        || object_value(&value, "engine_exit_code")?.as_i64() != Some(0)
        || !bool_value(&value, "owned_port_closed")?
        || !bool_value(&value, "hidden_console_verified")?
        || !bool_value(&value, "lifespan_complete")?
        || !bool_value(&value, "proxy_summary")?
        || bool_value(&value, "force_fallback")?
    {
        return Err(PublicError::CleanupFailed);
    }
    Ok(())
}

fn request_frame(
    request_id: &str,
    generation: Option<&str>,
    event: &str,
) -> Result<Vec<u8>, PublicError> {
    let mut value = json!({
        "protocol_version": HELPER_PROTOCOL_VERSION,
        "request_id": request_id,
        "event": event,
    });
    if let Some(generation) = generation {
        value["runtime_generation"] = Value::String(generation.to_owned());
    }
    let mut frame = serde_json::to_vec(&value).map_err(|_| PublicError::UnsupportedProtocol)?;
    frame.push(b'\n');
    if frame.len() > 4096 {
        return Err(PublicError::ValidationFailed);
    }
    Ok(frame)
}

#[derive(Debug, Eq, PartialEq)]
enum ReaderMessage {
    Frame(Vec<u8>),
    Closed,
    Invalid,
}

fn read_bounded_frame(reader: &mut impl Read) -> Result<Vec<u8>, ReaderMessage> {
    let mut frame = Vec::with_capacity(256);
    loop {
        let mut byte = [0u8; 1];
        match reader.read(&mut byte) {
            Ok(0) if frame.is_empty() => return Err(ReaderMessage::Closed),
            Ok(0) => return Err(ReaderMessage::Invalid),
            Ok(_) => {
                frame.push(byte[0]);
                if frame.len() > MAX_HELPER_FRAME_BYTES {
                    return Err(ReaderMessage::Invalid);
                }
                if byte[0] == b'\n' {
                    return Ok(frame);
                }
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(_) => return Err(ReaderMessage::Invalid),
        }
    }
}

fn spawn_frame_reader(
    mut stdout: ChildStdout,
) -> Result<(mpsc::Receiver<ReaderMessage>, JoinHandle<()>), PublicError> {
    let (sender, receiver) = mpsc::sync_channel(1);
    let thread = thread::Builder::new()
        .name("headroom-controller-stdout".to_owned())
        .spawn(move || loop {
            match read_bounded_frame(&mut stdout) {
                Ok(frame) => {
                    if sender.try_send(ReaderMessage::Frame(frame)).is_err() {
                        break;
                    }
                }
                Err(message) => {
                    let _ = sender.try_send(message);
                    break;
                }
            }
        })
        .map_err(|_| PublicError::EnvironmentBlocked)?;
    Ok((receiver, thread))
}

fn join_reader_bounded(thread: JoinHandle<()>) -> bool {
    let deadline = Instant::now() + READER_JOIN_TIMEOUT;
    while !thread.is_finished() && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(10));
    }
    if thread.is_finished() {
        thread.join().is_ok()
    } else {
        false
    }
}

fn join_session_reader(thread: &mut Option<JoinHandle<()>>) -> bool {
    let Some(reader) = thread.as_ref() else {
        return true;
    };
    let deadline = Instant::now() + READER_JOIN_TIMEOUT;
    while !reader.is_finished() && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(10));
    }
    if reader.is_finished() {
        thread.take().is_some_and(|reader| reader.join().is_ok())
    } else {
        false
    }
}

fn receive_frame(
    receiver: &mpsc::Receiver<ReaderMessage>,
    deadline: Instant,
    cancel: Option<&AtomicBool>,
) -> Result<Vec<u8>, PublicError> {
    loop {
        if cancel.is_some_and(|flag| flag.load(Ordering::SeqCst)) {
            return Err(PublicError::Timeout);
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(PublicError::Timeout);
        }
        let wait = remaining.min(Duration::from_millis(100));
        match receiver.recv_timeout(wait) {
            Ok(ReaderMessage::Frame(frame)) => return Ok(frame),
            Ok(ReaderMessage::Closed) => return Err(PublicError::EngineNotReady),
            Ok(ReaderMessage::Invalid) => return Err(PublicError::UnsupportedProtocol),
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(mpsc::RecvTimeoutError::Disconnected) => return Err(PublicError::EngineNotReady),
        }
    }
}

fn safe_environment(state_root: &Path) -> Result<Vec<(String, String)>, PublicError> {
    let system_root = env::var("SystemRoot")
        .or_else(|_| env::var("WINDIR"))
        .map_err(|_| PublicError::EnvironmentBlocked)?;
    let system_drive = env::var("SystemDrive").unwrap_or_else(|_| "C:".to_owned());
    let home = state_root.join("controller-home");
    let temp = state_root.join("controller-temp");
    let appdata = home.join("AppData").join("Roaming");
    let localappdata = home.join("AppData").join("Local");
    let state_boundary =
        fs::canonicalize(state_root).map_err(|_| PublicError::EnvironmentBlocked)?;
    for path in [&home, &temp, &appdata, &localappdata] {
        ensure_existing_ancestor_inside(&state_boundary, path)?;
        fs::create_dir_all(path).map_err(|_| PublicError::EnvironmentBlocked)?;
        if !fs::canonicalize(path)
            .map_err(|_| PublicError::EnvironmentBlocked)?
            .starts_with(&state_boundary)
        {
            return Err(PublicError::EnvironmentBlocked);
        }
    }
    let home_text = home.to_string_lossy().into_owned();
    let home_path = home_text
        .strip_prefix(&system_drive)
        .unwrap_or(&home_text)
        .to_owned();
    Ok(vec![
        ("SystemRoot".to_owned(), system_root.clone()),
        ("SystemDrive".to_owned(), system_drive),
        ("WINDIR".to_owned(), system_root.clone()),
        (
            "PATH".to_owned(),
            PathBuf::from(&system_root)
                .join("System32")
                .to_string_lossy()
                .into_owned(),
        ),
        ("TEMP".to_owned(), temp.to_string_lossy().into_owned()),
        ("TMP".to_owned(), temp.to_string_lossy().into_owned()),
        ("USERPROFILE".to_owned(), home_text.clone()),
        ("HOMEDRIVE".to_owned(), home_text.chars().take(2).collect()),
        ("HOMEPATH".to_owned(), home_path),
        ("HOME".to_owned(), home_text),
        ("APPDATA".to_owned(), appdata.to_string_lossy().into_owned()),
        (
            "LOCALAPPDATA".to_owned(),
            localappdata.to_string_lossy().into_owned(),
        ),
    ])
}

#[cfg(not(target_os = "macos"))]
struct ControllerSession {
    protected_tools: Vec<String>,
    control_secret: ControlSecret,
    optimization: Option<OptimizationState>,
    child: Child,
    stdin: ChildStdin,
    receiver: mpsc::Receiver<ReaderMessage>,
    reader_thread: Option<JoinHandle<()>>,
    #[cfg(windows)]
    job: Option<Arc<Mutex<OwnedJob>>>,
    #[cfg(windows)]
    backend_lease: Arc<BackendLivenessLease>,
    controller_pid: u32,
    helper_pid: u32,
    generation: String,
    engine_pid: u32,
    listener_port: u16,
    last_health: Instant,
}

#[cfg(not(target_os = "macos"))]
impl ControllerSession {
    fn tcp_listener(&self) -> Option<(u16, u32)> {
        Some((self.listener_port, self.engine_pid))
    }

    #[cfg(windows)]
    fn backend_is_alive(&self) -> bool {
        self.backend_lease.is_alive_for(&self.generation)
    }

    fn optimization_control(
        &self,
        id: &str,
        operation: ControlOperation,
    ) -> Result<OptimizationState, PublicError> {
        #[cfg(windows)]
        {
            control::request(
                ControlTarget {
                    port: self.listener_port,
                    owner_pid: self.engine_pid,
                    generation: &self.generation,
                    secret: &self.control_secret.0,
                },
                id,
                operation,
                || {
                    self.backend_lease.is_alive_for(&self.generation)
                        && owned_process_chain(
                            self.controller_pid,
                            self.helper_pid,
                            self.engine_pid,
                        )
                        && listener_owner(self.listener_port) == Some(self.engine_pid)
                },
            )
            .map_err(map_control_error)
        }
        #[cfg(not(windows))]
        {
            let _ = (id, operation);
            Err(PublicError::EnvironmentBlocked)
        }
    }
    fn capture_metrics(
        &self,
    ) -> Result<super::metrics_read::OwnedMetricsSnapshot, super::metrics_read::RuntimeMetricsError>
    {
        #[cfg(windows)]
        {
            super::metrics_read::read_owned_projection(
                self.listener_port,
                self.engine_pid,
                &self.generation,
                Some(&self.control_secret.0),
                || {
                    self.backend_lease.is_alive_for(&self.generation)
                        && owned_process_chain(
                            self.controller_pid,
                            self.helper_pid,
                            self.engine_pid,
                        )
                        && listener_owner(self.listener_port) == Some(self.engine_pid)
                },
            )
        }
        #[cfg(not(windows))]
        {
            Err(super::metrics_read::RuntimeMetricsError::Unavailable)
        }
    }
    fn launch(
        layout: &RuntimeLayout,
        request_id: &str,
        shutting_down: &AtomicBool,
    ) -> Result<Self, PublicError> {
        #[cfg(test)]
        set_startup_diagnostic_stage("runtime_validation");
        let start = request_frame(request_id, None, "start")?;
        let verified = layout.verify()?;
        let environment = safe_environment(&verified.state_root)?;
        let mut command = Command::new(&verified.python_path);
        #[cfg(windows)]
        use std::os::windows::process::CommandExt;
        command
            .arg("-I")
            .arg("-B")
            .arg("-u")
            .arg(&verified.controller_path)
            .arg("--manifest")
            .arg(&verified.manifest_path)
            .arg("--state-root")
            .arg(&verified.state_root)
            .arg("--workspace")
            .arg(&verified.workspace)
            .arg("--config")
            .arg(&verified.config)
            .current_dir(&verified.payload_root)
            .env_clear()
            .envs(environment)
            .env(
                "HEADROOM_DESKTOP_PROTECTED_TOOLS",
                serde_json::to_string(&verified.protected_tools)
                    .map_err(|_| PublicError::ValidationFailed)?,
            )
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        #[cfg(windows)]
        command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
        #[cfg(windows)]
        #[cfg(test)]
        set_startup_diagnostic_stage("job_create");
        #[cfg(windows)]
        let job = Arc::new(Mutex::new(OwnedJob::create()?));
        #[cfg(not(windows))]
        let job = OwnedJob::create()?;
        #[cfg(test)]
        set_startup_diagnostic_stage("controller_spawn");
        let mut child = command
            .spawn()
            .map_err(|_| PublicError::EnvironmentBlocked)?;
        #[cfg(all(test, windows))]
        windows_crash_tests::observe_before_job_assignment(&child, &job);
        #[cfg(windows)]
        #[cfg(test)]
        set_startup_diagnostic_stage("job_assignment");
        #[cfg(windows)]
        if let Err(error) = job
            .lock()
            .map_err(|_| PublicError::OwnershipLost)
            .and_then(|job| job.assign(&child))
        {
            return Err(cleanup_failed_launch(&mut child, &job, None, error));
        }
        let stdout = match child.stdout.take() {
            Some(stdout) => stdout,
            None => {
                return Err(cleanup_failed_launch(
                    &mut child,
                    &job,
                    None,
                    PublicError::EnvironmentBlocked,
                ));
            }
        };
        let (receiver, reader_thread) = match spawn_frame_reader(stdout) {
            Ok(value) => value,
            Err(error) => {
                return Err(cleanup_failed_launch(&mut child, &job, None, error));
            }
        };
        let mut stdin = match child.stdin.take() {
            Some(stdin) => stdin,
            None => {
                return Err(cleanup_failed_launch(
                    &mut child,
                    &job,
                    Some(reader_thread),
                    PublicError::EnvironmentBlocked,
                ));
            }
        };
        if stdin.write_all(&start).and_then(|_| stdin.flush()).is_err() {
            return Err(cleanup_failed_launch(
                &mut child,
                &job,
                Some(reader_thread),
                PublicError::EngineNotReady,
            ));
        }
        #[cfg(test)]
        set_startup_diagnostic_stage("controller_ready_frame");
        let frame = receive_frame(
            &receiver,
            Instant::now() + STARTUP_TIMEOUT,
            Some(shutting_down),
        );
        let ready = match frame.and_then(|raw| parse_controller_ready(&raw, request_id, child.id()))
        {
            Ok(value) => value,
            Err(error) => {
                return Err(cleanup_failed_launch(
                    &mut child,
                    &job,
                    Some(reader_thread),
                    error,
                ));
            }
        };
        #[cfg(test)]
        update_startup_diagnostic(|diagnostic| {
            diagnostic.stage = "controller_ready_parsed";
            diagnostic.generation_valid = Some(valid_generation(&ready.generation));
        });
        #[cfg(not(windows))]
        {
            return Err(cleanup_failed_launch(
                &mut child,
                &job,
                Some(reader_thread),
                PublicError::EnvironmentBlocked,
            ));
        }
        #[cfg(windows)]
        {
            let controller_pid = child.id();
            #[cfg(test)]
            set_startup_diagnostic_stage("ready_process_ownership");
            let process_chain_valid =
                owned_process_chain(controller_pid, ready.helper_pid, ready.engine_pid);
            let listener_owner_match =
                listener_owner(ready.listener_port) == Some(ready.engine_pid);
            #[cfg(test)]
            update_startup_diagnostic(|diagnostic| {
                diagnostic.stage = "ready_process_ownership";
                diagnostic.process_chain_valid = Some(process_chain_valid);
                diagnostic.listener_owner_match = Some(listener_owner_match);
            });
            let ownership_verified = process_chain_valid && listener_owner_match;
            if !ownership_verified
                || ready.protected_tools != verified.protected_tools
                || !probe_ready(ready.listener_port, EXPECTED_ENGINE_VERSION)
            {
                let error = if ownership_verified {
                    PublicError::EngineNotReady
                } else {
                    PublicError::OwnershipLost
                };
                let original_error =
                    cleanup_failed_launch(&mut child, &job, Some(reader_thread), error);
                let port_closed = !ownership_verified
                    || owned_listener_closed(
                        ready.listener_port,
                        ready.engine_pid,
                        PORT_CLOSE_TIMEOUT,
                    );
                return Err(
                    if original_error != PublicError::CleanupFailed && port_closed {
                        original_error
                    } else {
                        PublicError::CleanupFailed
                    },
                );
            }
            #[cfg(test)]
            set_startup_diagnostic_stage("backend_lease_create");
            let backend_lease = match BackendLivenessLease::new(
                controller_pid,
                ready.helper_pid,
                ready.engine_pid,
                ready.listener_port,
                ready.generation.clone(),
                Arc::downgrade(&job),
            ) {
                Ok(lease) => Arc::new(lease),
                Err(error) => {
                    let original_error =
                        cleanup_failed_launch(&mut child, &job, Some(reader_thread), error);
                    let port_closed = owned_listener_closed(
                        ready.listener_port,
                        ready.engine_pid,
                        PORT_CLOSE_TIMEOUT,
                    );
                    return Err(
                        if original_error != PublicError::CleanupFailed && port_closed {
                            original_error
                        } else {
                            PublicError::CleanupFailed
                        },
                    );
                }
            };
            #[cfg(test)]
            set_startup_diagnostic_stage("controller_session_ready");
            let mut session = Self {
                protected_tools: ready.protected_tools,
                control_secret: ready.control_secret,
                optimization: None,
                child,
                stdin,
                receiver,
                reader_thread: Some(reader_thread),
                job: Some(job),
                controller_pid,
                helper_pid: ready.helper_pid,
                generation: ready.generation,
                engine_pid: ready.engine_pid,
                listener_port: ready.listener_port,
                last_health: Instant::now(),
                backend_lease,
            };
            match session.optimization_control(request_id, ControlOperation::Read) {
                Ok(state) if state.enabled && state.revision == 0 && state.can_pause => {
                    session.optimization = Some(state)
                }
                _ => {
                    let report = session.force_stop();
                    return Err(cleanup_result_error(
                        PublicError::ConfigConflict,
                        report.clean(),
                    ));
                }
            }
            Ok(session)
        }
    }

    #[cfg(windows)]
    fn healthy(&mut self) -> bool {
        match self.child.try_wait() {
            Ok(Some(_)) | Err(_) => false,
            Ok(None) => {
                if !owned_process_chain(self.controller_pid, self.helper_pid, self.engine_pid)
                    || listener_owner(self.listener_port) != Some(self.engine_pid)
                    || !self.backend_lease.is_alive_for(&self.generation)
                {
                    return false;
                }
                if probe_ready(self.listener_port, EXPECTED_ENGINE_VERSION) {
                    self.last_health = Instant::now();
                    self.optimization = self
                        .optimization_control(&Uuid::new_v4().to_string(), ControlOperation::Read)
                        .ok();
                    true
                } else {
                    health_is_fresh(self.last_health, Instant::now())
                }
            }
        }
    }

    #[cfg(not(windows))]
    fn healthy(&mut self) -> bool {
        false
    }

    fn graceful_stop(&mut self, request_id: &str) -> Result<(), StopFailure> {
        self.revoke_backend();
        let frame = request_frame(request_id, Some(&self.generation), "stop").map_err(|error| {
            StopFailure {
                error,
                stage: "request_frame",
            }
        })?;
        self.stdin
            .write_all(&frame)
            .and_then(|_| self.stdin.flush())
            .map_err(|_| StopFailure {
                error: PublicError::EngineNotReady,
                stage: "request_write",
            })?;
        let response = receive_frame(&self.receiver, Instant::now() + STOP_EVENT_TIMEOUT, None)
            .map_err(|error| StopFailure {
                error,
                stage: "stopped_frame",
            })?;
        parse_controller_stopped(&response, request_id, &self.generation).map_err(|error| {
            StopFailure {
                error,
                stage: "stopped_evidence",
            }
        })?;
        let status = wait_for_exit(&mut self.child, PROCESS_EXIT_TIMEOUT).ok_or(StopFailure {
            error: PublicError::Timeout,
            stage: "controller_exit_timeout",
        })?;
        if !status.success() {
            return Err(StopFailure {
                error: PublicError::CleanupFailed,
                stage: "controller_exit_status",
            });
        }
        if !owned_listener_closed(self.listener_port, self.engine_pid, PORT_CLOSE_TIMEOUT) {
            return Err(StopFailure {
                error: PublicError::CleanupFailed,
                stage: "owned_port_close",
            });
        }
        if !processes_absent(self.helper_pid, self.engine_pid) {
            return Err(StopFailure {
                error: PublicError::CleanupFailed,
                stage: "owned_process_tree_exit",
            });
        }
        #[cfg(windows)]
        if !job_active_processes(&self.job).is_some_and(|active| active == 0) {
            return Err(StopFailure {
                error: PublicError::CleanupFailed,
                stage: "owned_job_empty",
            });
        }
        if !self.join_reader_after_exit() {
            return Err(StopFailure {
                error: PublicError::CleanupFailed,
                stage: "stdout_reader_exit",
            });
        }
        self.close_job();
        Ok(())
    }

    fn force_stop(&mut self) -> ForceStopReport {
        self.revoke_backend();
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.kill();
        }
        #[cfg(windows)]
        if let Some(job) = self.job.as_ref() {
            if let Ok(job) = job.lock() {
                let _ = job.terminate();
            }
        }
        let controller_exited = wait_for_exit(&mut self.child, FORCE_EXIT_TIMEOUT).is_some();
        let reader_stopped = if controller_exited {
            self.join_reader_after_exit()
        } else {
            false
        };
        let process_tree_absent = processes_absent(self.helper_pid, self.engine_pid);
        let port_closed =
            owned_listener_closed(self.listener_port, self.engine_pid, PORT_CLOSE_TIMEOUT);
        #[cfg(windows)]
        let job_empty = self.job.as_ref().is_some_and(|job| {
            job.lock()
                .is_ok_and(|job| job.terminate_and_wait_empty(FORCE_EXIT_TIMEOUT))
        });
        #[cfg(not(windows))]
        let job_empty = false;
        let report = ForceStopReport {
            controller_exited,
            process_tree_absent,
            port_closed,
            job_empty,
            reader_stopped,
        };
        if report.clean() {
            self.close_job();
        }
        report
    }

    fn close_job(&mut self) {
        #[cfg(windows)]
        {
            self.job.take();
        }
    }

    #[cfg(windows)]
    fn backend_target(&self) -> Result<super::ingress_relay::BackendTarget, PublicError> {
        let generation =
            Uuid::parse_str(&self.generation).map_err(|_| PublicError::OwnershipLost)?;
        let lease = Arc::downgrade(&self.backend_lease);
        let expected_generation = self.generation.clone();
        super::ingress_relay::BackendTarget::new(
            self.listener_port,
            generation,
            self.engine_pid,
            Arc::new(move || {
                lease
                    .upgrade()
                    .is_some_and(|lease| lease.is_alive_for(&expected_generation))
            }),
        )
        .map_err(|_| PublicError::OwnershipLost)
    }

    #[cfg(windows)]
    fn revoke_backend(&self) {
        self.backend_lease.revoke();
    }

    #[cfg(not(windows))]
    fn revoke_backend(&self) {}

    fn join_reader_after_exit(&mut self) -> bool {
        join_session_reader(&mut self.reader_thread)
    }
}

#[cfg(not(target_os = "macos"))]
impl Drop for ControllerSession {
    fn drop(&mut self) {
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.force_stop();
        } else {
            self.close_job();
            let _ = join_session_reader(&mut self.reader_thread);
        }
    }
}

fn terminate_child(child: &mut Child) -> bool {
    if child.try_wait().ok().flatten().is_some() {
        return true;
    }
    let _ = child.kill();
    wait_for_exit(child, FORCE_EXIT_TIMEOUT).is_some()
}

#[cfg(windows)]
fn cleanup_failed_launch(
    child: &mut Child,
    job: &Arc<Mutex<OwnedJob>>,
    reader: Option<JoinHandle<()>>,
    original_error: PublicError,
) -> PublicError {
    let child_exited = terminate_child(child);
    let job_empty = job
        .lock()
        .is_ok_and(|job| job.terminate_and_wait_empty(FORCE_EXIT_TIMEOUT));
    let reader_stopped = reader.is_none_or(join_reader_bounded);
    if child_exited && job_empty && reader_stopped {
        original_error
    } else {
        PublicError::CleanupFailed
    }
}

#[cfg(windows)]
fn job_active_processes(job: &Option<Arc<Mutex<OwnedJob>>>) -> Option<u32> {
    job.as_ref()
        .and_then(|job| job.lock().ok().and_then(|job| job.active_processes()))
}

#[cfg(not(windows))]
fn cleanup_failed_launch(
    child: &mut Child,
    _job: &OwnedJob,
    reader: Option<JoinHandle<()>>,
    original_error: PublicError,
) -> PublicError {
    let child_exited = terminate_child(child);
    let reader_stopped = reader.is_none_or(join_reader_bounded);
    if child_exited && reader_stopped {
        original_error
    } else {
        PublicError::CleanupFailed
    }
}

fn wait_for_exit(child: &mut Child, timeout: Duration) -> Option<ExitStatus> {
    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Some(status),
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(25)),
            _ => return None,
        }
    }
}

fn health_is_fresh(last_success: Instant, now: Instant) -> bool {
    now.saturating_duration_since(last_success) <= HEALTH_EXPIRY
}

fn probe_ready(port: u16, expected_version: &str) -> bool {
    let deadline = Instant::now() + HEALTH_IO_TIMEOUT;
    let address = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port);
    let Ok(mut stream) = TcpStream::connect_timeout(&address, HEALTH_IO_TIMEOUT) else {
        return false;
    };
    let remaining = deadline.saturating_duration_since(Instant::now());
    if remaining.is_zero() || stream.set_write_timeout(Some(remaining)).is_err() {
        return false;
    }
    if stream
        .write_all(b"GET /readyz HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")
        .is_err()
    {
        return false;
    }
    let Some((status, body)) = read_http_body(&mut stream, deadline) else {
        return false;
    };
    status == 200 && health_payload_matches(&body, expected_version)
}

fn health_payload_matches(body: &[u8], expected_version: &str) -> bool {
    let Ok(value) = parse_unique_json(body) else {
        return false;
    };
    value.get("service").and_then(Value::as_str) == Some("headroom-proxy")
        && value.get("status").and_then(Value::as_str) == Some("healthy")
        && value.get("ready").and_then(Value::as_bool) == Some(true)
        && value.get("version").and_then(Value::as_str) == Some(expected_version)
}

fn read_http_body(stream: &mut TcpStream, deadline: Instant) -> Option<(u16, Vec<u8>)> {
    read_http_body_with_limit(stream, deadline, 64 * 1024)
}

pub(super) trait TimedRead: Read {
    fn set_read_timeout(&self, timeout: Option<Duration>) -> io::Result<()>;
}

impl TimedRead for TcpStream {
    fn set_read_timeout(&self, timeout: Option<Duration>) -> io::Result<()> {
        TcpStream::set_read_timeout(self, timeout)
    }
}

#[cfg(target_os = "macos")]
impl TimedRead for std::os::unix::net::UnixStream {
    fn set_read_timeout(&self, timeout: Option<Duration>) -> io::Result<()> {
        std::os::unix::net::UnixStream::set_read_timeout(self, timeout)
    }
}

pub(super) fn read_http_body_with_limit(
    stream: &mut impl TimedRead,
    deadline: Instant,
    max_body: usize,
) -> Option<(u16, Vec<u8>)> {
    const MAX_HEADER: usize = 2048;
    if max_body == 0 || max_body > 2 * 1024 * 1024 {
        return None;
    }
    let mut received = Vec::with_capacity(4096);
    let mut chunk = [0u8; 4096];
    let header_end = loop {
        if let Some(index) = received.windows(4).position(|part| part == b"\r\n\r\n") {
            break index + 4;
        }
        if received.len() >= MAX_HEADER || deadline <= Instant::now() {
            return None;
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        if stream
            .set_read_timeout(Some(remaining.min(HEALTH_IO_TIMEOUT)))
            .is_err()
        {
            return None;
        }
        match stream.read(&mut chunk) {
            Ok(0) => return None,
            Ok(count) => received.extend_from_slice(&chunk[..count]),
            Err(error)
                if error.kind() == io::ErrorKind::Interrupted
                    || (matches!(
                        error.kind(),
                        io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
                    ) && deadline > Instant::now()) =>
            {
                continue;
            }
            Err(_) => return None,
        }
        if received.len() > MAX_HEADER + max_body {
            return None;
        }
    };
    if header_end > MAX_HEADER {
        return None;
    }
    let header_text = std::str::from_utf8(&received[..header_end]).ok()?;
    let status = header_text
        .lines()
        .next()?
        .split_whitespace()
        .nth(1)?
        .parse()
        .ok()?;
    let mut content_length = None;
    for line in header_text.lines().skip(1) {
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        if name.eq_ignore_ascii_case("transfer-encoding") {
            return None;
        }
        if name.eq_ignore_ascii_case("content-length") {
            if content_length.is_some() {
                return None;
            }
            content_length = Some(value.trim().parse::<usize>().ok()?);
        }
    }
    let content_length = content_length?;
    if content_length > max_body {
        return None;
    }
    let body_start = header_end;
    while received.len().saturating_sub(body_start) < content_length {
        if deadline <= Instant::now() || received.len() > MAX_HEADER + max_body {
            return None;
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        if stream
            .set_read_timeout(Some(remaining.min(HEALTH_IO_TIMEOUT)))
            .is_err()
        {
            return None;
        }
        match stream.read(&mut chunk) {
            Ok(0) => return None,
            Ok(count) => received.extend_from_slice(&chunk[..count]),
            Err(error)
                if error.kind() == io::ErrorKind::Interrupted
                    || (matches!(
                        error.kind(),
                        io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
                    ) && deadline > Instant::now()) =>
            {
                continue;
            }
            Err(_) => return None,
        }
    }
    Some((
        status,
        received[body_start..body_start + content_length].to_vec(),
    ))
}

fn process_chain_matches(
    parents: &std::collections::HashMap<u32, u32>,
    controller_pid: u32,
    helper_pid: u32,
    engine_pid: u32,
) -> bool {
    controller_pid != helper_pid
        && helper_pid != engine_pid
        && controller_pid != engine_pid
        && parents.get(&helper_pid) == Some(&controller_pid)
        && parents.get(&engine_pid) == Some(&helper_pid)
}

#[cfg(windows)]
struct OwnedProcessHandle(windows_sys::Win32::Foundation::HANDLE);

#[cfg(windows)]
unsafe impl Send for OwnedProcessHandle {}

#[cfg(windows)]
unsafe impl Sync for OwnedProcessHandle {}

#[cfg(windows)]
impl OwnedProcessHandle {
    fn open(pid: u32) -> Result<Self, PublicError> {
        use windows_sys::Win32::System::Threading::{
            OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE,
        };
        let handle = unsafe {
            OpenProcess(
                PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE,
                0,
                pid,
            )
        };
        if handle.is_null() {
            Err(PublicError::OwnershipLost)
        } else {
            Ok(Self(handle))
        }
    }

    fn is_live(&self) -> bool {
        use windows_sys::Win32::Foundation::WAIT_TIMEOUT;
        use windows_sys::Win32::System::Threading::WaitForSingleObject;
        unsafe { WaitForSingleObject(self.0, 0) == WAIT_TIMEOUT }
    }
}

#[cfg(windows)]
impl Drop for OwnedProcessHandle {
    fn drop(&mut self) {
        unsafe { windows_sys::Win32::Foundation::CloseHandle(self.0) };
    }
}

fn backend_lease_handles_valid(
    active: bool,
    generation_matches: bool,
    handles_live: [bool; 3],
) -> bool {
    active && generation_matches && handles_live.into_iter().all(|live| live)
}

fn backend_job_membership_valid(active_processes: Option<u32>, members: [bool; 3]) -> bool {
    active_processes.is_some_and(|count| count >= 3) && members.into_iter().all(|member| member)
}

#[cfg(windows)]
struct BackendLivenessLease {
    generation: String,
    active: AtomicBool,
    controller_pid: u32,
    helper_pid: u32,
    engine_pid: u32,
    listener_port: u16,
    processes: [OwnedProcessHandle; 3],
    job: Weak<Mutex<OwnedJob>>,
}

#[cfg(windows)]
impl BackendLivenessLease {
    fn new(
        controller_pid: u32,
        helper_pid: u32,
        engine_pid: u32,
        listener_port: u16,
        generation: String,
        job: Weak<Mutex<OwnedJob>>,
    ) -> Result<Self, PublicError> {
        #[cfg(test)]
        set_startup_diagnostic_stage("backend_lease_precheck");
        let generation_valid = valid_generation(&generation);
        let identities_valid =
            controller_pid != 0 && helper_pid != 0 && engine_pid != 0 && listener_port != 0;
        if !generation_valid || !identities_valid {
            #[cfg(test)]
            update_startup_diagnostic(|diagnostic| {
                diagnostic.stage = "backend_lease_precheck";
                diagnostic.generation_valid = Some(generation_valid);
            });
            return Err(PublicError::OwnershipLost);
        }
        let process_chain_valid = owned_process_chain(controller_pid, helper_pid, engine_pid);
        if !process_chain_valid {
            #[cfg(test)]
            update_startup_diagnostic(|diagnostic| {
                diagnostic.stage = "backend_lease_process_chain";
                diagnostic.generation_valid = Some(generation_valid);
                diagnostic.process_chain_valid = Some(false);
            });
            return Err(PublicError::OwnershipLost);
        }
        let listener_owner_match = listener_owner(listener_port) == Some(engine_pid);
        if !listener_owner_match {
            #[cfg(test)]
            update_startup_diagnostic(|diagnostic| {
                diagnostic.stage = "backend_lease_listener_owner";
                diagnostic.generation_valid = Some(generation_valid);
                diagnostic.process_chain_valid = Some(process_chain_valid);
                diagnostic.listener_owner_match = Some(false);
            });
            return Err(PublicError::OwnershipLost);
        }
        let lease = Self {
            generation,
            active: AtomicBool::new(true),
            controller_pid,
            helper_pid,
            engine_pid,
            listener_port,
            processes: {
                #[cfg(test)]
                set_startup_diagnostic_stage("backend_lease_open_handles");
                let controller = match OwnedProcessHandle::open(controller_pid) {
                    Ok(handle) => handle,
                    Err(error) => {
                        #[cfg(test)]
                        update_startup_diagnostic(|diagnostic| {
                            diagnostic.stage = "backend_lease_open_controller_handle";
                            diagnostic.generation_valid = Some(generation_valid);
                            diagnostic.process_chain_valid = Some(process_chain_valid);
                            diagnostic.listener_owner_match = Some(listener_owner_match);
                            diagnostic.controller_handle_live = Some(false);
                        });
                        return Err(error);
                    }
                };
                let helper = match OwnedProcessHandle::open(helper_pid) {
                    Ok(handle) => handle,
                    Err(error) => {
                        #[cfg(test)]
                        update_startup_diagnostic(|diagnostic| {
                            diagnostic.stage = "backend_lease_open_helper_handle";
                            diagnostic.generation_valid = Some(generation_valid);
                            diagnostic.process_chain_valid = Some(process_chain_valid);
                            diagnostic.listener_owner_match = Some(listener_owner_match);
                            diagnostic.controller_handle_live = Some(controller.is_live());
                            diagnostic.helper_handle_live = Some(false);
                        });
                        return Err(error);
                    }
                };
                let engine = match OwnedProcessHandle::open(engine_pid) {
                    Ok(handle) => handle,
                    Err(error) => {
                        #[cfg(test)]
                        update_startup_diagnostic(|diagnostic| {
                            diagnostic.stage = "backend_lease_open_engine_handle";
                            diagnostic.generation_valid = Some(generation_valid);
                            diagnostic.process_chain_valid = Some(process_chain_valid);
                            diagnostic.listener_owner_match = Some(listener_owner_match);
                            diagnostic.controller_handle_live = Some(controller.is_live());
                            diagnostic.helper_handle_live = Some(helper.is_live());
                            diagnostic.engine_handle_live = Some(false);
                        });
                        return Err(error);
                    }
                };
                [controller, helper, engine]
            },
            job,
        };
        #[cfg(test)]
        set_startup_diagnostic_stage("backend_lease_liveness");
        if !lease.is_alive_for(&lease.generation) {
            return Err(PublicError::OwnershipLost);
        }
        Ok(lease)
    }

    fn is_alive_for(&self, expected_generation: &str) -> bool {
        #[cfg(test)]
        {
            let lease_active = self.active.load(Ordering::Acquire);
            let lease_generation_match = self.generation == expected_generation;
            let controller_handle_live = self.processes[0].is_live();
            let helper_handle_live = self.processes[1].is_live();
            let engine_handle_live = self.processes[2].is_live();
            let handles_live = [
                controller_handle_live,
                helper_handle_live,
                engine_handle_live,
            ];
            if !backend_lease_handles_valid(lease_active, lease_generation_match, handles_live) {
                update_startup_diagnostic(|diagnostic| {
                    diagnostic.stage = "backend_lease_process_handles";
                    diagnostic.lease_active = Some(lease_active);
                    diagnostic.lease_generation_match = Some(lease_generation_match);
                    diagnostic.controller_handle_live = Some(controller_handle_live);
                    diagnostic.helper_handle_live = Some(helper_handle_live);
                    diagnostic.engine_handle_live = Some(engine_handle_live);
                    diagnostic.lease_alive = Some(false);
                });
                return false;
            }
            let Some(job) = self.job.upgrade() else {
                update_startup_diagnostic(|diagnostic| {
                    diagnostic.stage = "backend_lease_job_handle";
                    diagnostic.lease_active = Some(lease_active);
                    diagnostic.lease_generation_match = Some(lease_generation_match);
                    diagnostic.controller_handle_live = Some(controller_handle_live);
                    diagnostic.helper_handle_live = Some(helper_handle_live);
                    diagnostic.engine_handle_live = Some(engine_handle_live);
                    diagnostic.lease_alive = Some(false);
                });
                return false;
            };
            let Ok(job) = job.lock() else {
                update_startup_diagnostic(|diagnostic| {
                    diagnostic.stage = "backend_lease_job_lock";
                    diagnostic.lease_active = Some(lease_active);
                    diagnostic.lease_generation_match = Some(lease_generation_match);
                    diagnostic.controller_handle_live = Some(controller_handle_live);
                    diagnostic.helper_handle_live = Some(helper_handle_live);
                    diagnostic.engine_handle_live = Some(engine_handle_live);
                    diagnostic.lease_alive = Some(false);
                });
                return false;
            };
            let job_active_processes = job.active_processes();
            let memberships = [
                job.contains_process(self.processes[0].0),
                job.contains_process(self.processes[1].0),
                job.contains_process(self.processes[2].0),
            ];
            let process_chain_valid =
                owned_process_chain(self.controller_pid, self.helper_pid, self.engine_pid);
            let listener_owner_match = listener_owner(self.listener_port) == Some(self.engine_pid);
            let lease_alive =
                backend_lease_handles_valid(lease_active, lease_generation_match, handles_live)
                    && backend_job_membership_valid(job_active_processes, memberships)
                    && process_chain_valid
                    && listener_owner_match;
            let job_member_kinds = (!lease_alive)
                .then(|| {
                    job.member_basename_kinds(&[
                        (self.controller_pid, "controller"),
                        (self.helper_pid, "helper"),
                        (self.engine_pid, "engine"),
                    ])
                })
                .flatten();
            update_startup_diagnostic(|diagnostic| {
                diagnostic.stage = "backend_lease_ownership_checks";
                diagnostic.lease_active = Some(lease_active);
                diagnostic.lease_generation_match = Some(lease_generation_match);
                diagnostic.controller_handle_live = Some(controller_handle_live);
                diagnostic.helper_handle_live = Some(helper_handle_live);
                diagnostic.engine_handle_live = Some(engine_handle_live);
                diagnostic.job_active_processes = job_active_processes;
                diagnostic.job_member_kinds = job_member_kinds;
                diagnostic.controller_in_job = Some(memberships[0]);
                diagnostic.helper_in_job = Some(memberships[1]);
                diagnostic.engine_in_job = Some(memberships[2]);
                diagnostic.process_chain_valid = Some(process_chain_valid);
                diagnostic.listener_owner_match = Some(listener_owner_match);
                diagnostic.lease_alive = Some(lease_alive);
            });
            return lease_alive;
        }
        #[cfg(not(test))]
        {
            let lease_active = self.active.load(Ordering::Acquire);
            let lease_generation_match = self.generation == expected_generation;
            let handles_live = self.processes.each_ref().map(OwnedProcessHandle::is_live);
            if !backend_lease_handles_valid(lease_active, lease_generation_match, handles_live) {
                return false;
            }
            let Some(job) = self.job.upgrade() else {
                return false;
            };
            let Ok(job) = job.lock() else {
                return false;
            };
            let active_processes = job.active_processes();
            if !active_processes.is_some_and(|count| count >= 3) {
                return false;
            }
            let members = [
                job.contains_process(self.processes[0].0),
                job.contains_process(self.processes[1].0),
                job.contains_process(self.processes[2].0),
            ];
            if !backend_job_membership_valid(active_processes, members) {
                return false;
            }
            owned_process_chain(self.controller_pid, self.helper_pid, self.engine_pid)
                && listener_owner(self.listener_port) == Some(self.engine_pid)
        }
    }

    fn revoke(&self) {
        self.active.store(false, Ordering::Release);
    }
}

#[cfg(windows)]
pub(super) struct OwnedJob(windows_sys::Win32::Foundation::HANDLE);

#[cfg(windows)]
unsafe impl Send for OwnedJob {}

#[cfg(windows)]
impl OwnedJob {
    pub(super) fn create() -> Result<Self, PublicError> {
        use std::mem::size_of;
        use std::ptr::null;
        use windows_sys::Win32::System::JobObjects::{
            CreateJobObjectW, JobObjectExtendedLimitInformation, SetInformationJobObject,
            JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        };
        let handle = unsafe { CreateJobObjectW(null(), std::ptr::null()) };
        if handle.is_null() {
            return Err(PublicError::OwnershipLost);
        }
        let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { std::mem::zeroed() };
        info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        let result = unsafe {
            SetInformationJobObject(
                handle,
                JobObjectExtendedLimitInformation,
                &info as *const _ as *const _,
                size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
        };
        if result == 0 {
            unsafe { windows_sys::Win32::Foundation::CloseHandle(handle) };
            return Err(PublicError::OwnershipLost);
        }
        Ok(Self(handle))
    }

    pub(super) fn assign(&self, child: &Child) -> Result<(), PublicError> {
        use std::os::windows::io::AsRawHandle;
        let result = unsafe {
            windows_sys::Win32::System::JobObjects::AssignProcessToJobObject(
                self.0,
                child.as_raw_handle() as windows_sys::Win32::Foundation::HANDLE,
            )
        };
        if result == 0 {
            Err(PublicError::OwnershipLost)
        } else {
            Ok(())
        }
    }

    fn terminate(&self) -> Result<(), PublicError> {
        let result = unsafe {
            windows_sys::Win32::System::JobObjects::TerminateJobObject(self.0, 0xC000_013A)
        };
        if result == 0 {
            Err(PublicError::OwnershipLost)
        } else {
            Ok(())
        }
    }

    fn terminate_and_wait_empty(&self, timeout: Duration) -> bool {
        let _ = self.terminate();
        let deadline = Instant::now() + timeout;
        loop {
            match self.active_processes() {
                Some(0) => return true,
                Some(_) if Instant::now() < deadline => thread::sleep(Duration::from_millis(20)),
                _ => return false,
            }
        }
    }

    fn active_processes(&self) -> Option<u32> {
        use std::mem::size_of;
        use std::ptr::null_mut;
        use windows_sys::Win32::System::JobObjects::{
            JobObjectBasicAccountingInformation, QueryInformationJobObject,
            JOBOBJECT_BASIC_ACCOUNTING_INFORMATION,
        };
        let mut information = JOBOBJECT_BASIC_ACCOUNTING_INFORMATION::default();
        let result = unsafe {
            QueryInformationJobObject(
                self.0,
                JobObjectBasicAccountingInformation,
                &mut information as *mut _ as *mut _,
                size_of::<JOBOBJECT_BASIC_ACCOUNTING_INFORMATION>() as u32,
                null_mut(),
            )
        };
        (result != 0).then_some(information.ActiveProcesses)
    }

    fn contains_process(&self, process: windows_sys::Win32::Foundation::HANDLE) -> bool {
        use windows_sys::Win32::System::JobObjects::IsProcessInJob;
        let mut belongs = 0;
        unsafe { IsProcessInJob(process, self.0, &mut belongs) != 0 && belongs != 0 }
    }

    #[cfg(test)]
    fn member_basename_kinds(&self, expected: &[(u32, &'static str)]) -> Option<Vec<&'static str>> {
        use std::mem::size_of;
        use std::ptr::null_mut;
        use windows_sys::Win32::System::JobObjects::{
            JobObjectBasicProcessIdList, QueryInformationJobObject,
        };

        #[repr(C)]
        struct BoundedJobProcessIdList {
            assigned: u32,
            listed: u32,
            process_ids: [usize; 16],
        }

        let mut list: BoundedJobProcessIdList = unsafe { std::mem::zeroed() };
        let queried = unsafe {
            QueryInformationJobObject(
                self.0,
                JobObjectBasicProcessIdList,
                &mut list as *mut _ as *mut _,
                size_of::<BoundedJobProcessIdList>() as u32,
                null_mut(),
            )
        };
        if queried == 0 || list.listed > list.process_ids.len() as u32 {
            return None;
        }
        let mut kinds = Vec::with_capacity(list.listed as usize);
        for raw_pid in list.process_ids.iter().take(list.listed as usize) {
            let pid = u32::try_from(*raw_pid).ok()?;
            let kind = expected
                .iter()
                .find_map(|&(expected_pid, kind)| (pid == expected_pid).then_some(kind))
                .unwrap_or_else(|| process_basename_kind(pid));
            kinds.push(kind);
        }
        Some(kinds)
    }
}

#[cfg(all(windows, test))]
fn process_basename_kind(pid: u32) -> &'static str {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
        PROCESS_QUERY_LIMITED_INFORMATION,
    };

    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if process.is_null() {
        return "unavailable";
    }
    let mut image = [0u16; 1024];
    let mut length = image.len() as u32;
    let queried = unsafe {
        QueryFullProcessImageNameW(process, PROCESS_NAME_WIN32, image.as_mut_ptr(), &mut length)
    };
    let kind = if queried == 0 || length == 0 || length as usize > image.len() {
        "unavailable"
    } else {
        let image = String::from_utf16_lossy(&image[..length as usize]);
        let basename = image.rsplit(['\\', '/']).next().unwrap_or_default();
        match basename.to_ascii_lowercase().as_str() {
            "python.exe" => "python.exe",
            "pythonw.exe" => "pythonw.exe",
            "cmd.exe" => "cmd.exe",
            "powershell.exe" => "powershell.exe",
            "pwsh.exe" => "pwsh.exe",
            "conhost.exe" => "conhost.exe",
            _ => "other",
        }
    };
    unsafe { CloseHandle(process) };
    kind
}

#[cfg(windows)]
impl Drop for OwnedJob {
    fn drop(&mut self) {
        unsafe { windows_sys::Win32::Foundation::CloseHandle(self.0) };
    }
}

#[cfg(not(windows))]
struct OwnedJob;

#[cfg(not(windows))]
impl OwnedJob {
    fn create() -> Result<Self, PublicError> {
        Err(PublicError::EnvironmentBlocked)
    }
}

#[cfg(windows)]
fn process_parent_map() -> Option<std::collections::HashMap<u32, u32>> {
    use std::mem::size_of;
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };
    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    if snapshot == INVALID_HANDLE_VALUE {
        return None;
    }
    let mut entry = PROCESSENTRY32W::default();
    entry.dwSize = size_of::<PROCESSENTRY32W>() as u32;
    let mut parents = std::collections::HashMap::new();
    let mut ok = unsafe { Process32FirstW(snapshot, &mut entry) } != 0;
    while ok {
        parents.insert(entry.th32ProcessID, entry.th32ParentProcessID);
        ok = unsafe { Process32NextW(snapshot, &mut entry) } != 0;
    }
    unsafe { CloseHandle(snapshot) };
    (!parents.is_empty()).then_some(parents)
}

#[cfg(windows)]
fn owned_process_chain(controller_pid: u32, helper_pid: u32, engine_pid: u32) -> bool {
    if controller_pid == helper_pid || helper_pid == engine_pid || controller_pid == engine_pid {
        return false;
    }
    process_parent_map().is_some_and(|parents| {
        process_chain_matches(&parents, controller_pid, helper_pid, engine_pid)
    })
}

#[cfg(not(windows))]
fn owned_process_chain(_controller_pid: u32, _helper_pid: u32, _engine_pid: u32) -> bool {
    false
}

#[cfg(windows)]
fn processes_absent(helper_pid: u32, engine_pid: u32) -> bool {
    process_parent_map().is_some_and(|parents| {
        !parents.contains_key(&helper_pid) && !parents.contains_key(&engine_pid)
    })
}

#[cfg(not(windows))]
fn processes_absent(_helper_pid: u32, _engine_pid: u32) -> bool {
    false
}

#[cfg(windows)]
fn listener_owner(port: u16) -> Option<u32> {
    listener_owner_query(port, true).ok().flatten()
}

#[cfg(windows)]
fn listener_owner_query(port: u16, require_loopback: bool) -> Result<Option<u32>, ()> {
    use std::mem::size_of;
    use windows_sys::Win32::Foundation::{ERROR_INSUFFICIENT_BUFFER, NO_ERROR};
    use windows_sys::Win32::NetworkManagement::IpHelper::{
        GetExtendedTcpTable, MIB_TCPROW_OWNER_PID, TCP_TABLE_OWNER_PID_LISTENER,
    };

    const AF_INET: u32 = 2;
    const MAX_TABLE_BYTES: usize = 4 * 1024 * 1024;
    for _ in 0..3 {
        let mut size = 0u32;
        let mut buffer = Vec::<usize>::new();
        let mut result = unsafe {
            GetExtendedTcpTable(
                std::ptr::null_mut(),
                &mut size,
                0,
                AF_INET,
                TCP_TABLE_OWNER_PID_LISTENER,
                0,
            )
        };
        if result != ERROR_INSUFFICIENT_BUFFER || size as usize > MAX_TABLE_BYTES {
            return Err(());
        }
        buffer.resize((size as usize).div_ceil(size_of::<usize>()), 0);
        result = unsafe {
            GetExtendedTcpTable(
                buffer.as_mut_ptr().cast(),
                &mut size,
                0,
                AF_INET,
                TCP_TABLE_OWNER_PID_LISTENER,
                0,
            )
        };
        if result == ERROR_INSUFFICIENT_BUFFER {
            continue;
        }
        if result != NO_ERROR || size as usize > buffer.len() * size_of::<usize>() {
            return Err(());
        }
        let bytes = buffer.as_ptr().cast::<u8>();
        let count = unsafe { bytes.cast::<u32>().read_unaligned() } as usize;
        let row_bytes = count
            .checked_mul(size_of::<MIB_TCPROW_OWNER_PID>())
            .ok_or(())?;
        if size_of::<u32>() + row_bytes > size as usize {
            return Err(());
        }
        let rows = unsafe {
            std::slice::from_raw_parts(
                bytes.add(size_of::<u32>()).cast::<MIB_TCPROW_OWNER_PID>(),
                count,
            )
        };
        return tcp_rows_owner(rows, port, require_loopback);
    }
    Err(())
}

#[cfg(windows)]
fn tcp_rows_owner(
    rows: &[windows_sys::Win32::NetworkManagement::IpHelper::MIB_TCPROW_OWNER_PID],
    port: u16,
    require_loopback: bool,
) -> Result<Option<u32>, ()> {
    let loopback = u32::from_ne_bytes(Ipv4Addr::LOCALHOST.octets());
    let mut owner = None;
    let mut found_loopback = false;
    for row in rows.iter().filter(|row| {
        u16::from_be(row.dwLocalPort as u16) == port
            && (row.dwLocalAddr == loopback || row.dwLocalAddr == 0)
    }) {
        if owner.is_some_and(|current| current != row.dwOwningPid) {
            return Err(());
        }
        owner = Some(row.dwOwningPid);
        found_loopback |= row.dwLocalAddr == loopback;
    }
    if !require_loopback || found_loopback {
        Ok(owner)
    } else {
        Ok(None)
    }
}

#[cfg(not(windows))]
fn listener_owner(_port: u16) -> Option<u32> {
    None
}

#[cfg(not(windows))]
fn listener_owner_query(_port: u16, _require_loopback: bool) -> Result<Option<u32>, ()> {
    Err(())
}

fn owner_is_gone(result: Result<Option<u32>, ()>, owned_pid: u32) -> bool {
    match result {
        Ok(Some(owner)) if owner == owned_pid => false,
        Ok(_) => true,
        Err(()) => false,
    }
}

fn owned_listener_closed(port: u16, owned_pid: u32, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    loop {
        if owner_is_gone(listener_owner_query(port, false), owned_pid) {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        thread::sleep(Duration::from_millis(50));
    }
}

#[derive(Clone, Copy)]
struct IngressDrainResult {
    complete: bool,
    graceful: bool,
    active_connection_count: usize,
    listener_released: bool,
}

struct ManagedIngress {
    #[cfg(any(windows, target_os = "macos"))]
    relay: Option<super::ingress_relay::IngressRelay>,
    startup_error: Option<PublicError>,
    view: Arc<RwLock<IngressView>>,
}

impl ManagedIngress {
    #[cfg(any(windows, target_os = "macos"))]
    fn endpoint_is_current(relay: &super::ingress_relay::IngressRelay) -> bool {
        #[cfg(windows)]
        return relay.is_accepting();
        #[cfg(target_os = "macos")]
        relay.owns_endpoint()
    }

    fn open(layout: &Result<RuntimeLayout, PublicError>, view: Arc<RwLock<IngressView>>) -> Self {
        #[cfg(any(windows, target_os = "macos"))]
        {
            let opened = layout
                .as_ref()
                .map_err(|error| *error)
                .and_then(|layout| layout.state_runtime_root())
                .and_then(|root| {
                    let stable = super::ingress::StableIngress::open_or_create(&root)
                        .map_err(map_ingress_error)?;
                    let saved_port = stable.port();
                    let relay = super::ingress_relay::IngressRelay::new(
                        stable,
                        super::ingress_relay::RelayLimits::default(),
                    )
                    .map_err(|_| PublicError::EnvironmentBlocked)?;
                    Ok((relay, saved_port))
                });
            match opened {
                Ok((relay, saved_port)) => {
                    let address = match relay.local_addr() {
                        SocketAddr::V4(address)
                            if address.ip() == &Ipv4Addr::LOCALHOST
                                && address.port() == saved_port =>
                        {
                            IngressAddressValue {
                                host: "127.0.0.1",
                                port: address.port(),
                            }
                        }
                        _ => {
                            return Self::unavailable(view, PublicError::EnvironmentBlocked);
                        }
                    };
                    let endpoint_id = relay.endpoint_id().map(str::to_owned);
                    if endpoint_id.is_none() || !Self::endpoint_is_current(&relay) {
                        let mut relay = relay;
                        let _ = relay.shutdown(INGRESS_SHUTDOWN_TIMEOUT);
                        return Self::unavailable(view, PublicError::EnvironmentBlocked);
                    }
                    write_ingress_view(&view, |state| {
                        state.endpoint_status = "ready";
                        state.endpoint_id = endpoint_id;
                        state.address = Some(address);
                        state.endpoint_error_code = None;
                        state.last_endpoint_observation = Some(Instant::now());
                        state.route_status = "none";
                        state.route_generation = None;
                        state.route_error_code = None;
                        state.active_connection_count = Some(0);
                    });
                    Self {
                        relay: Some(relay),
                        startup_error: None,
                        view,
                    }
                }
                Err(error) => Self::unavailable(view, error),
            }
        }
        #[cfg(not(any(windows, target_os = "macos")))]
        {
            let error = layout
                .as_ref()
                .err()
                .copied()
                .unwrap_or(PublicError::EnvironmentBlocked);
            Self::unavailable(view, error)
        }
    }

    fn unavailable(view: Arc<RwLock<IngressView>>, error: PublicError) -> Self {
        let status = if error == PublicError::PortConflict {
            "conflict"
        } else {
            "unavailable"
        };
        write_ingress_view(&view, |state| {
            *state = IngressView::unavailable(error, status)
        });
        Self {
            #[cfg(any(windows, target_os = "macos"))]
            relay: None,
            startup_error: Some(error),
            view,
        }
    }

    fn startup_error(&self) -> Option<PublicError> {
        self.startup_error
    }

    fn observe_endpoint(&self) -> bool {
        #[cfg(any(windows, target_os = "macos"))]
        {
            let Some(relay) = self.relay.as_ref() else {
                self.mark_endpoint_unavailable();
                return false;
            };
            if !Self::endpoint_is_current(relay) {
                self.mark_endpoint_unavailable();
                return false;
            }
            let count = relay.active_connection_count() as u64;
            write_ingress_view(&self.view, |state| {
                record_endpoint_observation(state, count)
            });
            true
        }
        #[cfg(not(any(windows, target_os = "macos")))]
        {
            self.mark_endpoint_unavailable();
            false
        }
    }

    fn ensure_endpoint_for_start(&self) -> Result<(), PublicError> {
        #[cfg(test)]
        set_startup_diagnostic_stage("endpoint_acceptance_check");
        if self.observe_endpoint() {
            #[cfg(test)]
            set_startup_diagnostic_stage("endpoint_accepting");
            Ok(())
        } else {
            #[cfg(test)]
            set_startup_diagnostic_stage("endpoint_not_accepting");
            Err(PublicError::OwnershipLost)
        }
    }

    fn mark_endpoint_unavailable(&self) {
        write_ingress_view(&self.view, |state| {
            state.endpoint_status = "unknown";
            state.endpoint_error_code = Some(PublicError::OwnershipLost.code());
            state.last_endpoint_observation = None;
            state.active_connection_count = None;
            if state.route_status == "ready" {
                state.route_status = "failed";
                state.route_error_code = Some(PublicError::OwnershipLost.code());
            }
        });
    }

    #[cfg(any(windows, target_os = "macos"))]
    fn publish_route(&mut self, session: &ControllerSession) -> Result<(), PublicError> {
        #[cfg(test)]
        set_startup_diagnostic_stage("route_publish_precheck");
        let relay = self.relay.as_ref().ok_or_else(|| {
            self.startup_error
                .unwrap_or(PublicError::EnvironmentBlocked)
        })?;
        let relay_accepting = Self::endpoint_is_current(relay);
        #[cfg(test)]
        update_startup_diagnostic(|diagnostic| {
            diagnostic.stage = "route_publish_precheck";
            diagnostic.relay_accepting = Some(relay_accepting);
        });
        if !relay_accepting || !session.backend_is_alive() {
            return Err(PublicError::OwnershipLost);
        }
        #[cfg(test)]
        set_startup_diagnostic_stage("route_set_backend");
        let target = session.backend_target()?;
        relay
            .set_backend(target)
            .map_err(|_| PublicError::EngineNotReady)?;
        #[cfg(test)]
        set_startup_diagnostic_stage("route_verify_generation");
        let expected =
            Uuid::parse_str(&session.generation).map_err(|_| PublicError::OwnershipLost)?;
        let relay_accepting = Self::endpoint_is_current(relay);
        let route_generation_match = relay.route_generation() == Some(expected);
        #[cfg(test)]
        update_startup_diagnostic(|diagnostic| {
            diagnostic.stage = "route_verify_generation";
            diagnostic.relay_accepting = Some(relay_accepting);
            diagnostic.route_generation_match = Some(route_generation_match);
        });
        if !relay_accepting || !route_generation_match {
            let _ = relay.begin_drain();
            return Err(PublicError::OwnershipLost);
        }
        write_ingress_view(&self.view, |state| {
            record_endpoint_observation(state, relay.active_connection_count() as u64);
            state.route_status = "ready";
            state.route_generation = Some(session.generation.clone());
            state.route_error_code = None;
        });
        #[cfg(test)]
        set_startup_diagnostic_stage("route_published");
        Ok(())
    }

    #[cfg(not(any(windows, target_os = "macos")))]
    fn publish_route(&mut self, _session: &ControllerSession) -> Result<(), PublicError> {
        Err(PublicError::EnvironmentBlocked)
    }

    #[cfg(any(windows, target_os = "macos"))]
    fn route_healthy(&self, generation: &str) -> bool {
        let Some(relay) = self.relay.as_ref() else {
            return false;
        };
        Uuid::parse_str(generation).is_ok_and(|generation| {
            Self::endpoint_is_current(relay) && relay.route_generation() == Some(generation)
        })
    }

    #[cfg(not(any(windows, target_os = "macos")))]
    fn route_healthy(&self, _generation: &str) -> bool {
        false
    }

    #[cfg(any(windows, target_os = "macos"))]
    fn begin_drain(&mut self) -> Result<(), PublicError> {
        let Some(relay) = self.relay.as_ref() else {
            return Err(self
                .startup_error
                .unwrap_or(PublicError::EnvironmentBlocked));
        };
        relay
            .begin_drain()
            .map_err(|_| PublicError::CleanupFailed)?;
        write_ingress_view(&self.view, |state| {
            state.route_status = "draining";
            state.route_error_code = None;
            state.active_connection_count = Some(relay.active_connection_count() as u64);
        });
        Ok(())
    }

    #[cfg(not(any(windows, target_os = "macos")))]
    fn begin_drain(&mut self) -> Result<(), PublicError> {
        Err(PublicError::EnvironmentBlocked)
    }

    #[cfg(any(windows, target_os = "macos"))]
    fn finish_drain(&mut self, timeout: Duration) -> IngressDrainResult {
        let report = self
            .relay
            .as_ref()
            .map(|relay| {
                let report = relay.finish_drain(timeout);
                IngressDrainResult {
                    complete: report.complete,
                    graceful: report.graceful,
                    active_connection_count: report.active_connection_count,
                    listener_released: report.listener_released,
                }
            })
            .unwrap_or(IngressDrainResult {
                complete: false,
                graceful: false,
                active_connection_count: 0,
                listener_released: false,
            });
        write_ingress_view(&self.view, |state| {
            state.active_connection_count = Some(report.active_connection_count as u64);
            if report.complete {
                state.route_status = "none";
                state.route_generation = None;
                state.route_error_code = None;
            } else {
                state.route_status = "failed";
                state.route_error_code = Some(PublicError::CleanupFailed.code());
            }
        });
        IngressDrainResult {
            complete: report.complete,
            graceful: report.graceful,
            active_connection_count: report.active_connection_count,
            listener_released: false,
        }
    }

    #[cfg(not(any(windows, target_os = "macos")))]
    fn finish_drain(&mut self, _timeout: Duration) -> IngressDrainResult {
        IngressDrainResult {
            complete: false,
            graceful: false,
            active_connection_count: 0,
            listener_released: false,
        }
    }

    #[cfg(any(windows, target_os = "macos"))]
    fn shutdown(&mut self, timeout: Duration) -> IngressDrainResult {
        let report = self
            .relay
            .as_mut()
            .map(|relay| {
                let report = relay.shutdown(timeout);
                IngressDrainResult {
                    complete: report.complete,
                    graceful: report.graceful,
                    active_connection_count: report.active_connection_count,
                    listener_released: report.listener_released,
                }
            })
            .unwrap_or(IngressDrainResult {
                complete: self.startup_error.is_some(),
                graceful: self.startup_error.is_some(),
                active_connection_count: 0,
                listener_released: self.startup_error.is_some(),
            });
        write_ingress_view(&self.view, |state| {
            state.active_connection_count = Some(report.active_connection_count as u64);
            state.endpoint_status = if report.listener_released {
                "closed"
            } else {
                "unavailable"
            };
            if !report.listener_released || !report.complete {
                state.endpoint_error_code = Some(PublicError::CleanupFailed.code());
                state.route_error_code = Some(PublicError::CleanupFailed.code());
            }
        });
        IngressDrainResult {
            complete: report.complete,
            graceful: report.graceful,
            active_connection_count: report.active_connection_count,
            listener_released: report.listener_released,
        }
    }

    #[cfg(not(any(windows, target_os = "macos")))]
    fn shutdown(&mut self, _timeout: Duration) -> IngressDrainResult {
        IngressDrainResult {
            complete: self.startup_error.is_some(),
            graceful: self.startup_error.is_some(),
            active_connection_count: 0,
            listener_released: self.startup_error.is_some(),
        }
    }
}

fn write_ingress_view(view: &Arc<RwLock<IngressView>>, update: impl FnOnce(&mut IngressView)) {
    let mut state = view
        .write()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    update(&mut state);
}

enum ActorCommand {
    SetOptimization {
        request_id: String,
        expected_generation: String,
        expected_revision: u64,
        enabled: bool,
        reply: mpsc::SyncSender<RuntimeReply>,
    },
    CollectMetrics {
        reply: mpsc::SyncSender<
            Result<
                super::metrics_read::OwnedMetricsSnapshot,
                super::metrics_read::RuntimeMetricsError,
            >,
        >,
    },
    Start {
        request_id: String,
        reply: mpsc::SyncSender<RuntimeReply>,
    },
    Stop {
        request_id: String,
        expected_generation: String,
        reply: mpsc::SyncSender<RuntimeReply>,
    },
    Shutdown {
        reply: mpsc::SyncSender<bool>,
    },
}

#[cfg(all(test, windows))]
mod metrics_actor_tests {
    use super::*;
    use crate::runtime::metrics_read::{OwnedMetricsSnapshot, RuntimeMetricsError};
    fn manager(status: EngineStatus) -> (RuntimeManager, mpsc::Receiver<ActorCommand>) {
        let (sender, receiver) = mpsc::sync_channel(1);
        let mut view = RuntimeView::new(status);
        view.last_health = Some(Instant::now());
        (
            RuntimeManager {
                sender,
                view: Arc::new(RwLock::new(view)),
                ingress_view: Arc::new(RwLock::new(IngressView::starting())),
                shutting_down: Arc::new(AtomicBool::new(false)),
                update_start_gate: Arc::new(std::sync::Mutex::new(false)),
                actor_available: true,
                #[cfg(windows)]
                retrieval_layout: Err(PublicError::EnvironmentBlocked),
            },
            receiver,
        )
    }

    #[test]
    fn stopped_or_unavailable_runtime_never_queues_a_metadata_probe() {
        for (status, error) in [
            (EngineStatus::Stopped, RuntimeMetricsError::Stopped),
            (EngineStatus::Unavailable, RuntimeMetricsError::Unavailable),
            (EngineStatus::Starting, RuntimeMetricsError::NotReady),
        ] {
            let (manager, receiver) = manager(status);
            assert_eq!(manager.capture_metrics().unwrap_err(), error);
            assert!(receiver.try_recv().is_err());
        }
    }

    #[test]
    fn capture_uses_actor_generation_not_an_earlier_ui_view() {
        let (manager, receiver) = manager(EngineStatus::Ready);
        let actor = thread::spawn(move || {
            if let Ok(ActorCommand::CollectMetrics { reply }) =
                receiver.recv_timeout(Duration::from_millis(300))
            {
                let generation = "00000000-0000-4000-8000-000000000002";
                let projection = crate::metrics::normalize::project_headroom_041(
                    br#"{"recent_requests":[]}"#,
                    generation,
                    "0.44.0",
                    crate::metrics::normalize::Source::Proxy,
                )
                .unwrap();
                let _ = reply.send(Ok(OwnedMetricsSnapshot {
                    generation: generation.into(),
                    captured_at: chrono::Utc::now(),
                    projection,
                }));
            }
        });
        let captured = manager.capture_metrics();
        actor.join().unwrap();
        assert_eq!(
            captured.unwrap().generation,
            "00000000-0000-4000-8000-000000000002"
        );
    }

    #[test]
    fn metadata_backpressure_and_disconnected_actor_are_fixed_errors() {
        let (manager, receiver) = manager(EngineStatus::Ready);
        let (reply, _) = mpsc::sync_channel(1);
        assert!(manager
            .sender
            .try_send(ActorCommand::CollectMetrics { reply })
            .is_ok());
        assert_eq!(
            manager.capture_metrics().unwrap_err(),
            RuntimeMetricsError::Busy
        );
        drop(receiver);
        assert_eq!(
            manager.capture_metrics().unwrap_err(),
            RuntimeMetricsError::Unavailable
        );
    }
}

#[derive(Clone)]
pub struct RuntimeManager {
    sender: mpsc::SyncSender<ActorCommand>,
    view: Arc<RwLock<RuntimeView>>,
    ingress_view: Arc<RwLock<IngressView>>,
    shutting_down: Arc<AtomicBool>,
    update_start_gate: Arc<std::sync::Mutex<bool>>,
    actor_available: bool,
    #[cfg(windows)]
    retrieval_layout: Result<RuntimeLayout, PublicError>,
}

impl RuntimeManager {
    pub(crate) fn capture_metrics(
        &self,
    ) -> Result<super::metrics_read::OwnedMetricsSnapshot, super::metrics_read::RuntimeMetricsError>
    {
        use super::metrics_read::RuntimeMetricsError as Error;
        if self.shutting_down.load(Ordering::SeqCst) || !self.actor_available {
            return Err(Error::Unavailable);
        }
        {
            let view = self.view.read().unwrap_or_else(|p| p.into_inner());
            match view.status {
                EngineStatus::Stopped => return Err(Error::Stopped),
                EngineStatus::Unavailable => return Err(Error::Unavailable),
                EngineStatus::Ready
                    if view
                        .last_health
                        .is_some_and(|last| health_is_fresh(last, Instant::now())) => {}
                _ => return Err(Error::NotReady),
            }
        }
        let (reply, receiver) = mpsc::sync_channel(1);
        self.sender
            .try_send(ActorCommand::CollectMetrics { reply })
            .map_err(|e| match e {
                mpsc::TrySendError::Full(_) => Error::Busy,
                mpsc::TrySendError::Disconnected(_) => Error::Unavailable,
            })?;
        receiver
            .recv_timeout(Duration::from_secs(3))
            .map_err(|error| match error {
                mpsc::RecvTimeoutError::Timeout => Error::Busy,
                mpsc::RecvTimeoutError::Disconnected => Error::Unavailable,
            })?
    }
    pub fn new(resource_dir: Option<PathBuf>, app_data_dir: Option<PathBuf>) -> Self {
        Self::from_layout(RuntimeLayout::resolve(resource_dir, app_data_dir))
    }

    pub(crate) fn with_preferences(
        resource_dir: Option<PathBuf>,
        app_data_dir: Option<PathBuf>,
        preferences: crate::state::store::PreferenceStore,
    ) -> Self {
        Self::from_layout(
            RuntimeLayout::resolve(resource_dir, app_data_dir).map(|mut layout| {
                layout.preferences = Some(preferences);
                layout
            }),
        )
    }

    pub(crate) fn settings_observation(&self) -> Value {
        let mut view = self.view.write().unwrap_or_else(|e| e.into_inner());
        expire_stale_ready_view(&mut view, Instant::now());
        let verified = view.status == EngineStatus::Ready && view.protected_tools.is_some();
        serde_json::json!({
            "status": if verified { "verified" } else if view.status == EngineStatus::Stopped { "inactive" } else { "unknown" },
            "runtime_generation": if verified { view.runtime_generation.clone() } else { None },
            "protected_tools": if verified { view.protected_tools.clone() } else { None },
            "local_stats_enabled": if verified { Some(true) } else { None },
            "beacon_enabled": if verified { Some(false) } else { None },
            "mode": if verified { Some("cache") } else { None },
            "savings_profile": if verified { Some("coding") } else { None },
            "ccr_ttl_seconds": if verified { Some(7200_u32) } else { None }
        })
    }

    #[cfg(test)]
    pub(crate) fn isolated_for_test(state_root: PathBuf) -> Self {
        let project = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("project root")
            .to_path_buf();
        let layout = fs::canonicalize(state_root)
            .map_err(|_| PublicError::EnvironmentBlocked)
            .and_then(|state_root| {
                RuntimeLayout::from_roots(project.clone(), project.join("runtime"), state_root)
            });
        Self::from_layout(layout)
    }

    pub fn failure_reply(request_id: String) -> RuntimeReply {
        snapshot_error(&request_id, PublicError::EnvironmentBlocked)
    }

    fn from_layout(layout: Result<RuntimeLayout, PublicError>) -> Self {
        #[cfg(windows)]
        let retrieval_layout = layout.clone();
        let initial_status = if layout.is_ok() {
            EngineStatus::Stopped
        } else {
            EngineStatus::Unavailable
        };
        let mut initial_view = RuntimeView::new(initial_status);
        if initial_status == EngineStatus::Unavailable {
            initial_view.error_code = Some(PublicError::EnvironmentBlocked.code());
        }
        let view = Arc::new(RwLock::new(initial_view));
        let ingress_view = Arc::new(RwLock::new(match layout.as_ref() {
            Ok(_) => IngressView::starting(),
            Err(error) => IngressView::unavailable(
                *error,
                if *error == PublicError::PortConflict {
                    "conflict"
                } else {
                    "unavailable"
                },
            ),
        }));
        let shutting_down = Arc::new(AtomicBool::new(false));
        let update_start_gate = Arc::new(std::sync::Mutex::new(false));
        let (sender, receiver) = mpsc::sync_channel(ACTOR_QUEUE_CAPACITY);
        let actor_view = Arc::clone(&view);
        let actor_ingress_view = Arc::clone(&ingress_view);
        let actor_shutdown = Arc::clone(&shutting_down);
        let actor_update_gate = Arc::clone(&update_start_gate);
        let actor_available = thread::Builder::new()
            .name("headroom-runtime-manager".to_owned())
            .spawn(move || {
                run_actor(
                    receiver,
                    layout,
                    actor_view,
                    actor_ingress_view,
                    actor_shutdown,
                    actor_update_gate,
                )
            })
            .is_ok();
        if !actor_available {
            write_view(&view, |state| {
                state.status = EngineStatus::Unavailable;
                state.error_code = Some(PublicError::EnvironmentBlocked.code());
            });
            write_ingress_view(&ingress_view, |state| {
                *state = IngressView::unavailable(PublicError::EnvironmentBlocked, "unavailable");
            });
        }
        Self {
            sender,
            view,
            ingress_view,
            shutting_down,
            update_start_gate,
            actor_available,
            #[cfg(windows)]
            retrieval_layout,
        }
    }

    pub fn get_runtime_snapshot(&self, request_id: String) -> RuntimeReply {
        if !valid_request_id(&request_id) {
            return snapshot_error(&request_id, PublicError::ValidationFailed);
        }
        let mut view = self
            .view
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        expire_stale_ready_view(&mut view, Instant::now());
        view.snapshot(request_id)
    }

    pub(crate) fn update_engine_stopped(&self) -> bool {
        if !self.actor_available || self.shutting_down.load(Ordering::SeqCst) {
            return false;
        }
        let (Ok(runtime), Ok(ingress)) = (self.view.try_read(), self.ingress_view.try_read())
        else {
            return false;
        };
        runtime.status == EngineStatus::Stopped
            && !runtime.cleanup_uncertain
            && runtime.runtime_generation.is_none()
            && ingress.route_status == "none"
            && ingress.active_connection_count == Some(0)
    }

    #[cfg(all(windows, any(test, not(debug_assertions))))]
    pub(crate) fn reserve_for_update(&self) -> bool {
        let Ok(mut gate) = self.update_start_gate.try_lock() else {
            return false;
        };
        if *gate || !self.update_engine_stopped() {
            return false;
        }
        *gate = true;
        true
    }

    #[cfg(all(windows, any(test, not(debug_assertions))))]
    pub(crate) fn release_update_reservation(&self) {
        *self
            .update_start_gate
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = false;
    }

    pub(crate) fn catalog_runtime(&self) -> CatalogRuntime {
        let mut view = self.view.write().unwrap_or_else(|p| p.into_inner());
        expire_stale_ready_view(&mut view, Instant::now());
        CatalogRuntime {
            pinned_version: EXPECTED_ENGINE_VERSION,
            #[cfg(not(target_os = "macos"))]
            manifest_sha256: ACTIVE_MANIFEST_SHA256,
            #[cfg(target_os = "macos")]
            manifest_sha256: super::macos_layout::COMPILED_MANIFEST_SHA256.unwrap_or(""),
            active_version: view.engine_version.clone(),
            engine_status: view.status.label(),
        }
    }

    pub fn get_ingress_snapshot(&self, request_id: String) -> IngressReply {
        if !valid_request_id(&request_id) {
            return ingress_snapshot_error(&request_id, PublicError::ValidationFailed);
        }
        let ingress = {
            let mut ingress = self
                .ingress_view
                .write()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            expire_stale_ingress_endpoint(&mut ingress, Instant::now());
            ingress.clone()
        };
        let mut runtime = self
            .view
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        expire_stale_ready_view(&mut runtime, Instant::now());
        ingress.snapshot(request_id, &runtime)
    }

    pub(crate) fn ready_stable_endpoint(
        &self,
    ) -> Result<ReadyStableEndpoint, StableEndpointAccessError> {
        let now = Instant::now();
        let ingress = {
            let mut ingress = self
                .ingress_view
                .write()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            expire_stale_ingress_endpoint(&mut ingress, now);
            ingress.clone()
        };
        let mut runtime = self
            .view
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        expire_stale_ready_view(&mut runtime, now);
        ingress.ready_stable_endpoint(&runtime, now)
    }

    #[cfg(windows)]
    pub(super) fn learn_payload(&self) -> Option<(PathBuf, PathBuf)> {
        if self.shutting_down.load(Ordering::Acquire) {
            return None;
        }
        let (root, _, python) = self.retrieval_layout.as_ref().ok()?.verify_payload().ok()?;
        Some((root, python))
    }

    #[cfg(windows)]
    pub(crate) fn opencode_retrieval_command(
        &self,
        endpoint: &ReadyStableEndpoint,
    ) -> Result<crate::integrations::opencode_retrieval::RetrievalCommand, StableEndpointAccessError>
    {
        let blocked = || StableEndpointAccessError::Unavailable;
        let current = self.ready_stable_endpoint()?;
        if current.endpoint_id != endpoint.endpoint_id
            || current.host != endpoint.host
            || current.port != endpoint.port
        {
            return Err(blocked());
        }
        let layout = self.retrieval_layout.as_ref().map_err(|_| blocked())?;
        let (_, _, python) = layout.verify_payload().map_err(|_| blocked())?;
        let script =
            resolve_relative(&layout.runtime_root, "retrieval_bridge.py").map_err(|_| blocked())?;
        if sha256_file(&script).map_err(|_| blocked())? != EXPECTED_RETRIEVAL_BRIDGE_SHA256 {
            return Err(blocked());
        }
        let record = resolve_relative(&layout.state_root, "connection-endpoint.json")
            .map_err(|_| blocked())?;
        let executable = std::env::current_exe().map_err(|_| blocked())?;
        #[cfg(not(debug_assertions))]
        let desktop_use_gate = Some(
            crate::desktop::uninstall::gate_path(&layout.state_boundary_root)
                .map_err(|_| blocked())?,
        );
        #[cfg(debug_assertions)]
        let desktop_use_gate: Option<PathBuf> = None;
        crate::integrations::opencode_retrieval::RetrievalCommand::new(
            &python,
            &script,
            &record,
            &endpoint.endpoint_id,
            &executable,
            desktop_use_gate.as_deref(),
        )
        .map_err(|_| blocked())
    }

    pub(crate) fn codex_connection_observation(&self) -> CodexRuntimeObservation {
        let now = Instant::now();
        let ingress = {
            let mut ingress = self
                .ingress_view
                .write()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            expire_stale_ingress_endpoint(&mut ingress, now);
            ingress.clone()
        };
        let mut runtime = self
            .view
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        expire_stale_ready_view(&mut runtime, now);
        match ingress.snapshot(Uuid::new_v4().to_string(), &runtime) {
            IngressReply::Snapshot(snapshot) => {
                let endpoint_ready = snapshot.endpoint.status == "ready"
                    && snapshot.endpoint.error_code.is_none()
                    && snapshot
                        .endpoint
                        .endpoint_id
                        .as_deref()
                        .is_some_and(valid_request_id)
                    && snapshot.endpoint.address.as_ref().is_some_and(|address| {
                        address.host == "127.0.0.1" && (1..=u16::MAX).contains(&address.port)
                    });
                let engine_ready = endpoint_ready
                    && snapshot.backend.status == "ready"
                    && snapshot.backend.error_code.is_none()
                    && snapshot
                        .backend
                        .generation
                        .as_deref()
                        .is_some_and(valid_generation);
                CodexRuntimeObservation {
                    endpoint_status: snapshot.endpoint.status,
                    endpoint_id: snapshot.endpoint.endpoint_id,
                    endpoint_port: snapshot
                        .endpoint
                        .address
                        .filter(|address| address.host == "127.0.0.1" && address.port != 0)
                        .map(|address| address.port),
                    endpoint_ready,
                    engine_status: snapshot.backend.status,
                    engine_ready,
                    runtime_generation: snapshot.backend.generation,
                    active_connection_count: snapshot.active_connection_count,
                    error_code: snapshot.backend.error_code.or(snapshot.endpoint.error_code),
                }
            }
            IngressReply::Error(error) => CodexRuntimeObservation {
                endpoint_status: "unknown",
                endpoint_id: None,
                endpoint_port: None,
                endpoint_ready: false,
                engine_status: "unknown",
                engine_ready: false,
                runtime_generation: None,
                active_connection_count: None,
                error_code: Some(error.error.code),
            },
        }
    }

    pub fn start_engine(&self, request_id: String) -> RuntimeReply {
        if !valid_request_id(&request_id) {
            return snapshot_error(&request_id, PublicError::ValidationFailed);
        }
        if self.shutting_down.load(Ordering::SeqCst) || !self.actor_available {
            return snapshot_error(&request_id, PublicError::EnvironmentBlocked);
        }
        let (reply, receiver) = mpsc::sync_channel(1);
        if let Err(error) = self.sender.try_send(ActorCommand::Start {
            request_id: request_id.clone(),
            reply,
        }) {
            return snapshot_error(
                &request_id,
                match error {
                    mpsc::TrySendError::Full(_) => PublicError::Busy,
                    mpsc::TrySendError::Disconnected(_) => PublicError::EnvironmentBlocked,
                },
            );
        }
        receiver
            .recv_timeout(START_REPLY_TIMEOUT)
            .unwrap_or_else(|_| snapshot_error(&request_id, PublicError::Timeout))
    }

    pub fn stop_engine(&self, request_id: String, expected_generation: String) -> RuntimeReply {
        if !valid_request_id(&request_id) || !valid_generation(&expected_generation) {
            return snapshot_error(&request_id, PublicError::ValidationFailed);
        }
        if !self.actor_available {
            return snapshot_error(&request_id, PublicError::EnvironmentBlocked);
        }
        let (reply, receiver) = mpsc::sync_channel(1);
        if let Err(error) = self.sender.try_send(ActorCommand::Stop {
            request_id: request_id.clone(),
            expected_generation,
            reply,
        }) {
            return snapshot_error(
                &request_id,
                match error {
                    mpsc::TrySendError::Full(_) => PublicError::Busy,
                    mpsc::TrySendError::Disconnected(_) => PublicError::EnvironmentBlocked,
                },
            );
        }
        receiver
            .recv_timeout(STOP_REPLY_TIMEOUT)
            .unwrap_or_else(|_| snapshot_error(&request_id, PublicError::Timeout))
    }

    pub fn set_optimization(
        &self,
        request_id: String,
        expected_generation: String,
        expected_revision: u64,
        enabled: bool,
    ) -> RuntimeReply {
        if !valid_request_id(&request_id)
            || !valid_generation(&expected_generation)
            || expected_revision > control::MAX_CONTROL_COUNTER
        {
            return snapshot_error(&request_id, PublicError::ValidationFailed);
        }
        if !self.actor_available || self.shutting_down.load(Ordering::SeqCst) {
            return snapshot_error(&request_id, PublicError::EnvironmentBlocked);
        }
        let (reply, receiver) = mpsc::sync_channel(1);
        if let Err(error) = self.sender.try_send(ActorCommand::SetOptimization {
            request_id: request_id.clone(),
            expected_generation,
            expected_revision,
            enabled,
            reply,
        }) {
            return snapshot_error(
                &request_id,
                match error {
                    mpsc::TrySendError::Full(_) => PublicError::Busy,
                    mpsc::TrySendError::Disconnected(_) => PublicError::EnvironmentBlocked,
                },
            );
        }
        receiver
            .recv_timeout(Duration::from_secs(4))
            .unwrap_or_else(|_| snapshot_error(&request_id, PublicError::Timeout))
    }

    pub fn shutdown(&self) -> bool {
        if !self.actor_available {
            return true;
        }
        if self.shutting_down.swap(true, Ordering::SeqCst) {
            return true;
        }
        let (reply, receiver) = mpsc::sync_channel(1);
        let deadline = Instant::now() + SHUTDOWN_REPLY_TIMEOUT;
        let mut command = ActorCommand::Shutdown { reply };
        loop {
            match self.sender.try_send(command) {
                Ok(()) => break,
                Err(mpsc::TrySendError::Disconnected(_)) => return false,
                Err(mpsc::TrySendError::Full(returned)) => {
                    command = returned;
                    if Instant::now() >= deadline {
                        return false;
                    }
                    thread::sleep(Duration::from_millis(20));
                }
            }
        }
        receiver
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .unwrap_or(false)
    }

    #[cfg(test)]
    pub(crate) fn listener_port_for_test(&self) -> Option<u16> {
        self.view
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .listener_port
    }

    #[cfg(test)]
    pub(crate) fn ownership_diagnostic_for_test(&self) -> String {
        let diagnostic = startup_diagnostic_for_test();
        format!(
            "stage={};lease={:?};job={:?};chain={:?};listener={:?}",
            diagnostic.stage,
            diagnostic.lease_alive,
            diagnostic.job_active_processes,
            diagnostic.process_chain_valid,
            diagnostic.listener_owner_match
        )
    }

    #[cfg(test)]
    pub(crate) fn listener_owner_pid_for_test(&self) -> Option<u32> {
        self.view
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .listener_owner_pid
    }

    #[cfg(test)]
    fn stop_diagnostics_for_test(&self) -> Option<(StopFailure, ForceStopReport)> {
        self.view
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .stop_diagnostics
    }
}

fn write_view(view: &Arc<RwLock<RuntimeView>>, update: impl FnOnce(&mut RuntimeView)) {
    let mut state = view
        .write()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    update(&mut state);
}

fn set_status(view: &Arc<RwLock<RuntimeView>>, status: EngineStatus) {
    write_view(view, |state| {
        state.status = status;
        state.error_code = None;
    });
}

#[cfg(test)]
fn set_stop_diagnostics(
    view: &Arc<RwLock<RuntimeView>>,
    graceful: StopFailure,
    forced: ForceStopReport,
) {
    write_view(view, |state| {
        state.stop_diagnostics = Some((graceful, forced))
    });
}

fn set_ready_view(view: &Arc<RwLock<RuntimeView>>, session: &ControllerSession) {
    mark_ready_view(
        view,
        &session.generation,
        session.tcp_listener(),
        session.last_health,
        session.optimization,
        session.protected_tools.clone(),
    );
}

fn mark_ready_view(
    view: &Arc<RwLock<RuntimeView>>,
    generation: &str,
    listener: Option<(u16, u32)>,
    last_health: Instant,
    optimization: Option<OptimizationState>,
    protected_tools: Vec<String>,
) {
    write_view(view, |state| {
        state.status = EngineStatus::Ready;
        state.error_code = None;
        state.cleanup_uncertain = false;
        state.runtime_generation = Some(generation.to_owned());
        state.engine_version = Some(EXPECTED_ENGINE_VERSION.to_owned());
        state.listener_port = listener.map(|(port, _)| port);
        state.listener_owner_pid = listener.map(|(_, pid)| pid);
        state.last_health = Some(last_health);
        state.optimization = optimization;
        state.protected_tools = Some(protected_tools);
    });
}

fn clear_view(
    view: &Arc<RwLock<RuntimeView>>,
    status: EngineStatus,
    error_code: Option<&'static str>,
    cleanup_uncertain: bool,
) {
    write_view(view, |state| {
        state.status = status;
        state.error_code = error_code;
        state.cleanup_uncertain = cleanup_uncertain;
        state.runtime_generation = None;
        state.engine_version = None;
        state.listener_port = None;
        state.listener_owner_pid = None;
        state.last_health = None;
        state.optimization = None;
    });
}

fn mark_no_active_ingress_failure(view: &Arc<RwLock<RuntimeView>>, error: PublicError) {
    clear_view(view, EngineStatus::Unavailable, Some(error.code()), false);
}

fn mark_cleanup_uncertain(view: &Arc<RwLock<RuntimeView>>, session: &ControllerSession) {
    write_view(view, |state| {
        state.status = EngineStatus::Failed;
        state.error_code = Some(PublicError::CleanupFailed.code());
        state.cleanup_uncertain = true;
        state.runtime_generation = Some(session.generation.clone());
        state.engine_version = Some(EXPECTED_ENGINE_VERSION.to_owned());
        let listener = session.tcp_listener();
        state.listener_port = listener.map(|(port, _)| port);
        state.listener_owner_pid = listener.map(|(_, pid)| pid);
        state.last_health = Some(session.last_health);
    });
}

fn stop_lost_session(
    active: &mut Option<ControllerSession>,
    view: &Arc<RwLock<RuntimeView>>,
    ingress: &mut ManagedIngress,
    failure: PublicError,
) -> bool {
    let Some(mut session) = active.take() else {
        let status = if failure == PublicError::OwnershipLost {
            EngineStatus::Unavailable
        } else {
            EngineStatus::Failed
        };
        clear_view(view, status, Some(failure.code()), false);
        return true;
    };
    let _ = ingress.begin_drain();
    let forced = session.force_stop();
    let drained = ingress.finish_drain(INGRESS_DRAIN_TIMEOUT);
    if forced.clean() && drained.complete && drained.active_connection_count == 0 {
        drop(session);
        let status = if failure == PublicError::OwnershipLost {
            EngineStatus::Unavailable
        } else {
            EngineStatus::Failed
        };
        clear_view(view, status, Some(failure.code()), false);
        true
    } else {
        mark_cleanup_uncertain(view, &session);
        *active = Some(session);
        false
    }
}

fn snapshot_for(view: &Arc<RwLock<RuntimeView>>, request_id: &str) -> RuntimeReply {
    if !valid_request_id(request_id) {
        return snapshot_error(request_id, PublicError::ValidationFailed);
    }
    view.read()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .snapshot(request_id.to_owned())
}

fn run_actor(
    receiver: mpsc::Receiver<ActorCommand>,
    layout: Result<RuntimeLayout, PublicError>,
    view: Arc<RwLock<RuntimeView>>,
    ingress_view: Arc<RwLock<IngressView>>,
    shutting_down: Arc<AtomicBool>,
    update_start_gate: Arc<std::sync::Mutex<bool>>,
) {
    let mut ingress = ManagedIngress::open(&layout, Arc::clone(&ingress_view));
    if let Some(error) = ingress.startup_error() {
        let unavailable = matches!(
            error,
            PublicError::PortConflict
                | PublicError::EnvironmentBlocked
                | PublicError::ConfigConflict
        );
        clear_view(
            &view,
            if unavailable {
                EngineStatus::Unavailable
            } else {
                EngineStatus::Failed
            },
            Some(error.code()),
            false,
        );
    }
    let mut active: Option<ControllerSession> = None;
    loop {
        match receiver.recv_timeout(ACTOR_POLL_INTERVAL) {
            Ok(ActorCommand::SetOptimization {
                request_id,
                expected_generation,
                expected_revision,
                enabled,
                reply,
            }) => {
                if shutting_down.load(Ordering::SeqCst) {
                    let _ =
                        reply.send(snapshot_error(&request_id, PublicError::EnvironmentBlocked));
                    continue;
                }
                let Some(session) = active.as_mut() else {
                    let _ = reply.send(snapshot_error(&request_id, PublicError::EngineNotReady));
                    continue;
                };
                let ready = {
                    let state = view.read().unwrap_or_else(|p| p.into_inner());
                    state.status == EngineStatus::Ready && !state.cleanup_uncertain
                };
                if !ready {
                    let _ = reply.send(snapshot_error(&request_id, PublicError::EngineNotReady));
                    continue;
                }
                if session.generation != expected_generation {
                    let _ = reply.send(snapshot_error(&request_id, PublicError::OwnershipLost));
                    continue;
                }
                let known = session.optimization;
                if known.is_none_or(|state| {
                    state.revision != expected_revision || !enabled && !state.can_pause
                }) {
                    let _ = reply.send(snapshot_error(&request_id, PublicError::ConfigConflict));
                    continue;
                }
                match session.optimization_control(
                    &request_id,
                    ControlOperation::Set {
                        expected_revision,
                        enabled,
                    },
                ) {
                    Ok(state) => {
                        session.optimization = Some(state);
                        set_ready_view(&view, session);
                        let _ = reply.send(snapshot_for(&view, &request_id));
                    }
                    Err(error) => {
                        session.optimization = None;
                        write_view(&view, |state| state.optimization = None);
                        let _ = reply.send(snapshot_error(&request_id, error));
                    }
                }
            }
            Ok(ActorCommand::CollectMetrics { reply }) => {
                let result = if shutting_down.load(Ordering::SeqCst) {
                    Err(super::metrics_read::RuntimeMetricsError::Stopped)
                } else {
                    active
                        .as_ref()
                        .ok_or(super::metrics_read::RuntimeMetricsError::Stopped)
                        .and_then(ControllerSession::capture_metrics)
                };
                let _ = reply.send(result);
            }
            Ok(ActorCommand::Start { request_id, reply }) => {
                let Ok(start_gate) = update_start_gate.try_lock() else {
                    let _ = reply.send(snapshot_error(&request_id, PublicError::Busy));
                    continue;
                };
                if *start_gate {
                    let _ = reply.send(snapshot_error(&request_id, PublicError::Busy));
                    continue;
                }
                if shutting_down.load(Ordering::SeqCst) {
                    let _ =
                        reply.send(snapshot_error(&request_id, PublicError::EnvironmentBlocked));
                    continue;
                }
                if let Some(error) = ingress.startup_error() {
                    let _ = reply.send(snapshot_error(&request_id, error));
                    continue;
                }
                if let Err(error) = ingress.ensure_endpoint_for_start() {
                    if active.is_some() {
                        stop_lost_session(&mut active, &view, &mut ingress, error);
                    } else {
                        mark_no_active_ingress_failure(&view, error);
                    }
                    let _ = reply.send(snapshot_error(&request_id, error));
                    continue;
                }
                if let Some(session) = active.as_mut() {
                    if session.healthy() && ingress.route_healthy(&session.generation) {
                        set_ready_view(&view, session);
                        let _ = reply.send(snapshot_for(&view, &request_id));
                        continue;
                    }
                    stop_lost_session(
                        &mut active,
                        &view,
                        &mut ingress,
                        PublicError::EngineNotReady,
                    );
                    let _ = reply.send(snapshot_error(&request_id, PublicError::EngineNotReady));
                    continue;
                }
                if view
                    .read()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .cleanup_uncertain
                {
                    let _ = reply.send(snapshot_error(&request_id, PublicError::CleanupFailed));
                    continue;
                }
                set_status(&view, EngineStatus::Starting);
                let result = match &layout {
                    Ok(layout) => ControllerSession::launch(layout, &request_id, &shutting_down),
                    Err(error) => Err(*error),
                };
                match result {
                    Ok(mut session) => {
                        if let Err(error) = ingress.publish_route(&session) {
                            let _ = ingress.begin_drain();
                            let forced = session.force_stop();
                            let drained = ingress.finish_drain(INGRESS_DRAIN_TIMEOUT);
                            let cleanup_verified = forced.clean()
                                && drained.complete
                                && drained.active_connection_count == 0;
                            let reported = cleanup_result_error(error, cleanup_verified);
                            if cleanup_verified {
                                clear_view(
                                    &view,
                                    EngineStatus::Failed,
                                    Some(reported.code()),
                                    false,
                                );
                            } else {
                                mark_cleanup_uncertain(&view, &session);
                                active = Some(session);
                            }
                            let _ = reply.send(snapshot_error(&request_id, reported));
                            continue;
                        }
                        if !session.healthy() || !ingress.route_healthy(&session.generation) {
                            let _ = ingress.begin_drain();
                            let forced = session.force_stop();
                            let drained = ingress.finish_drain(INGRESS_DRAIN_TIMEOUT);
                            let cleanup_verified = forced.clean()
                                && drained.complete
                                && drained.active_connection_count == 0;
                            let reported =
                                cleanup_result_error(PublicError::EngineNotReady, cleanup_verified);
                            if cleanup_verified {
                                clear_view(
                                    &view,
                                    EngineStatus::Failed,
                                    Some(reported.code()),
                                    false,
                                );
                            } else {
                                mark_cleanup_uncertain(&view, &session);
                                active = Some(session);
                            }
                            let _ = reply.send(snapshot_error(&request_id, reported));
                            continue;
                        }
                        set_ready_view(&view, &session);
                        active = Some(session);
                        let _ = reply.send(snapshot_for(&view, &request_id));
                    }
                    Err(error) => {
                        let unavailable = matches!(
                            error,
                            PublicError::EnvironmentBlocked | PublicError::ConfigConflict
                        );
                        clear_view(
                            &view,
                            if unavailable {
                                EngineStatus::Unavailable
                            } else {
                                EngineStatus::Failed
                            },
                            Some(error.code()),
                            error == PublicError::CleanupFailed,
                        );
                        let _ = reply.send(snapshot_error(&request_id, error));
                    }
                }
            }
            Ok(ActorCommand::Stop {
                request_id,
                expected_generation,
                reply,
            }) => {
                if shutting_down.load(Ordering::SeqCst) {
                    let _ =
                        reply.send(snapshot_error(&request_id, PublicError::EnvironmentBlocked));
                    continue;
                }
                if active
                    .as_ref()
                    .is_none_or(|session| session.generation != expected_generation)
                {
                    let _ = reply.send(snapshot_error(&request_id, PublicError::OwnershipLost));
                    continue;
                }
                let Some(mut session) = active.take() else {
                    let _ = reply.send(snapshot_error(&request_id, PublicError::OwnershipLost));
                    continue;
                };
                set_status(&view, EngineStatus::Stopping);
                let drain_started = ingress.begin_drain().is_ok();
                if !drain_started {
                    session.revoke_backend();
                    let forced = session.force_stop();
                    let drained = ingress.finish_drain(INGRESS_DRAIN_TIMEOUT);
                    let clean =
                        forced.clean() && drained.complete && drained.active_connection_count == 0;
                    if clean {
                        drop(session);
                        clear_view(
                            &view,
                            EngineStatus::Failed,
                            Some(PublicError::CleanupFailed.code()),
                            false,
                        );
                    } else {
                        mark_cleanup_uncertain(&view, &session);
                        active = Some(session);
                    }
                    let _ = reply.send(snapshot_error(&request_id, PublicError::CleanupFailed));
                    continue;
                }
                let result = session.graceful_stop(&request_id);
                match result {
                    Ok(()) => {
                        let drained = ingress.finish_drain(INGRESS_DRAIN_TIMEOUT);
                        if drained.complete
                            && drained.graceful
                            && drained.active_connection_count == 0
                        {
                            drop(session);
                            clear_view(&view, EngineStatus::Stopped, None, false);
                            let _ = reply.send(snapshot_for(&view, &request_id));
                        } else {
                            let error = PublicError::CleanupFailed;
                            let cleanup_uncertain =
                                !drained.complete || drained.active_connection_count != 0;
                            drop(session);
                            clear_view(
                                &view,
                                EngineStatus::Failed,
                                Some(error.code()),
                                cleanup_uncertain,
                            );
                            let _ = reply.send(snapshot_error(&request_id, error));
                        }
                    }
                    Err(failure) => {
                        let forced = session.force_stop();
                        let drained = ingress.finish_drain(INGRESS_DRAIN_TIMEOUT);
                        #[cfg(test)]
                        set_stop_diagnostics(&view, failure, forced);
                        let clean = forced.clean()
                            && drained.complete
                            && drained.active_connection_count == 0;
                        let reported = if clean {
                            failure.error
                        } else {
                            PublicError::CleanupFailed
                        };
                        if clean {
                            drop(session);
                            clear_view(&view, EngineStatus::Failed, Some(reported.code()), false);
                        } else {
                            mark_cleanup_uncertain(&view, &session);
                            active = Some(session);
                        }
                        let _ = reply.send(snapshot_error(&request_id, reported));
                    }
                }
            }
            Ok(ActorCommand::Shutdown { reply }) => {
                let mut graceful = true;
                let mut lifecycle_clean = !view
                    .read()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .cleanup_uncertain;
                if let Some(mut session) = active.take() {
                    set_status(&view, EngineStatus::Stopping);
                    let request_id = Uuid::new_v4().to_string();
                    let drain_started = ingress.begin_drain().is_ok();
                    let stopped = drain_started && session.graceful_stop(&request_id).is_ok();
                    let forced_clean = if stopped {
                        true
                    } else {
                        session.force_stop().clean()
                    };
                    let drained = ingress.finish_drain(INGRESS_DRAIN_TIMEOUT);
                    graceful = stopped
                        && drained.graceful
                        && drained.complete
                        && drained.active_connection_count == 0;
                    lifecycle_clean =
                        forced_clean && drained.complete && drained.active_connection_count == 0;
                    drop(session);
                    clear_view(
                        &view,
                        if graceful {
                            EngineStatus::Stopped
                        } else {
                            EngineStatus::Failed
                        },
                        if graceful {
                            None
                        } else {
                            Some(PublicError::CleanupFailed.code())
                        },
                        !lifecycle_clean,
                    );
                }
                let ingress_stopped = ingress.shutdown(INGRESS_SHUTDOWN_TIMEOUT);
                let clean = lifecycle_clean
                    && graceful
                    && ingress_stopped.complete
                    && ingress_stopped.graceful
                    && ingress_stopped.listener_released;
                if !ingress_stopped.listener_released || !ingress_stopped.complete {
                    clear_view(
                        &view,
                        EngineStatus::Failed,
                        Some(PublicError::CleanupFailed.code()),
                        true,
                    );
                }
                let _ = reply.send(clean);
                break;
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if ingress.startup_error().is_some() {
                    continue;
                } else if ingress.observe_endpoint() {
                    if let Some(session) = active.as_mut() {
                        if session.healthy() && ingress.route_healthy(&session.generation) {
                            set_ready_view(&view, session);
                        } else {
                            stop_lost_session(
                                &mut active,
                                &view,
                                &mut ingress,
                                PublicError::EngineNotReady,
                            );
                        }
                    }
                } else if active.is_some() {
                    stop_lost_session(&mut active, &view, &mut ingress, PublicError::OwnershipLost);
                } else {
                    mark_no_active_ingress_failure(&view, PublicError::OwnershipLost);
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                let lifecycle_clean = if let Some(mut session) = active.take() {
                    let _ = ingress.begin_drain();
                    let forced = session.force_stop().clean();
                    let drained = ingress.finish_drain(INGRESS_DRAIN_TIMEOUT);
                    drop(session);
                    forced && drained.complete && drained.active_connection_count == 0
                } else {
                    true
                };
                let ingress_stopped = ingress.shutdown(INGRESS_SHUTDOWN_TIMEOUT);
                let clean = lifecycle_clean
                    && ingress_stopped.complete
                    && ingress_stopped.listener_released;
                clear_view(
                    &view,
                    if clean {
                        EngineStatus::Stopped
                    } else {
                        EngineStatus::Failed
                    },
                    if clean {
                        None
                    } else {
                        Some(PublicError::CleanupFailed.code())
                    },
                    !clean,
                );
                break;
            }
        }
    }
}

#[cfg(test)]
#[path = "manager_tests.rs"]
mod tests;

#[cfg(all(test, windows))]
#[path = "windows_crash_tests.rs"]
mod windows_crash_tests;

#[cfg(all(test, target_os = "macos"))]
#[path = "manager_macos_tests.rs"]
mod macos_tests;

#[cfg(all(test, windows))]
#[path = "settings_live.rs"]
mod operating_live;
