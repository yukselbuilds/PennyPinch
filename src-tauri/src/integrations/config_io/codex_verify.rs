use super::super::discovery::{InstallationSelectionError, InstallationSummary};
use super::super::plan::{
    preview_config_with_presence, ClientKind, ConfigScope, ConfigSource, PlanContext, PlanStatus,
    StableEndpoint,
};
use super::{read_codex_config_pinned, ConfigRootInputs, PinnedDirectoryChain, PlanManager};
use crate::runtime::manager::{ReadyStableEndpoint, RuntimeManager};
use serde::de::{self, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{Map, Value};
use std::collections::HashSet;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(windows)]
use std::sync::MutexGuard;
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use uuid::Uuid;

const SCHEMA_VERSION: u8 = 1;
const ZERO_UUID: &str = "00000000-0000-0000-0000-000000000000";
const CODEX_VERSION: &str = "0.163.0-alpha.11.2";
const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;
const MAX_OUTPUT_BYTES: usize = 4 * 1024 * 1024;
const MAX_LINE_BYTES: usize = 64 * 1024;
const MAX_EVENTS: usize = 2048;
const PROCESS_TIMEOUT: Duration = Duration::from_secs(90);
const PROCESS_POLL: Duration = Duration::from_millis(50);
const READER_RESULT_TIMEOUT: Duration = Duration::from_secs(2);
const OWNED_CLEANUP_TIMEOUT: Duration = Duration::from_secs(2);
const EXACT_VERIFY_PREFIX: &str = "HEADROOM_LOCAL_VERIFY_";

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CodexVerifyRequest {
    pub schema_version: u8,
    pub request_id: String,
    pub installation_id: String,
    pub scan_generation: u64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(untagged)]
pub(crate) enum CodexVerifyReply {
    Snapshot(CodexVerifySnapshot),
    Error(CodexVerifyError),
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct CodexVerifySnapshot {
    pub schema_version: u8,
    pub request_id: String,
    pub status: &'static str,
    pub installation_id: String,
    pub scan_generation: u64,
    pub probe_status: &'static str,
    pub failure_code: Option<&'static str>,
    pub output_diagnostic: Option<OutputDiagnostic>,
    pub cli_error_count: u32,
    pub scope: &'static str,
    pub endpoint_id: Option<String>,
    pub runtime_generation: Option<String>,
    pub route_eligible: bool,
    pub process_completed: bool,
    pub marker_received: bool,
    pub ingress_peer_observed: bool,
    pub selection_unchanged: bool,
    pub runtime_generation_unchanged: bool,
    pub saved_config_adoption: &'static str,
    pub auth_general: &'static str,
    pub subagent: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum OutputDiagnostic {
    ReaderTimeout,
    ReaderFailed,
    OutputOversize,
    LineOrEventLimit,
    InvalidJson,
    InvalidShape,
    UnexpectedError,
    UnexpectedTool,
    UnexpectedItem,
    UnexpectedEvent,
    MarkerMissing,
    MarkerMultiple,
    TerminalMissing,
    TerminalMultiple,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct CodexVerifyError {
    pub schema_version: u8,
    pub request_id: String,
    pub status: &'static str,
    pub error: CodexVerifyErrorDetail,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct CodexVerifyErrorDetail {
    pub code: &'static str,
    pub retryable: bool,
    pub message_key: &'static str,
    pub diagnostic_id: Option<String>,
}

#[derive(Clone)]
pub(crate) struct CodexVerifyManager {
    inner: Arc<VerifyInner>,
}

struct VerifyInner {
    workspace_root: Option<PathBuf>,
    busy: AtomicBool,
    shutting_down: AtomicBool,
    #[cfg(windows)]
    active_job: Mutex<Option<Arc<OwnedJob>>>,
}

impl CodexVerifyManager {
    pub(crate) fn new(app_local_data_dir: Option<PathBuf>) -> Self {
        let workspace_root = if cfg!(debug_assertions) {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .map(|root| {
                    root.join("work")
                        .join("app-state")
                        .join("codex-verification")
                })
        } else {
            app_local_data_dir.map(|root| root.join("codex-verification"))
        };
        Self {
            inner: Arc::new(VerifyInner {
                workspace_root,
                busy: AtomicBool::new(false),
                shutting_down: AtomicBool::new(false),
                #[cfg(windows)]
                active_job: Mutex::new(None),
            }),
        }
    }

    pub(crate) fn shutdown(&self) -> bool {
        self.inner.shutting_down.store(true, Ordering::SeqCst);
        #[cfg(windows)]
        {
            let job = match active_job_for_shutdown(&self.inner) {
                Ok(active) => active,
                Err(_) => return false,
            };
            return job.is_none_or(|job| job.terminate_and_wait_empty(OWNED_CLEANUP_TIMEOUT));
        }
        #[cfg(not(windows))]
        true
    }

    pub(crate) fn verify(
        &self,
        plans: &PlanManager,
        runtime: &RuntimeManager,
        request: CodexVerifyRequest,
    ) -> CodexVerifyReply {
        let request_id = if valid_uuid(&request.request_id) {
            request.request_id.clone()
        } else {
            ZERO_UUID.to_owned()
        };
        if request.schema_version != SCHEMA_VERSION
            || !valid_uuid(&request.request_id)
            || !valid_uuid(&request.installation_id)
            || request.scan_generation == 0
            || request.scan_generation > MAX_SAFE_INTEGER
        {
            return error_reply(&request_id, VerifyError::ValidationFailed);
        }
        if self.inner.shutting_down.load(Ordering::SeqCst) {
            return error_reply(&request_id, VerifyError::Unavailable);
        }
        let Some(_guard) = VerifyGuard::acquire(&self.inner.busy) else {
            return error_reply(&request_id, VerifyError::Busy);
        };
        #[cfg(windows)]
        {
            self.verify_windows(plans, runtime, request)
        }
        #[cfg(not(windows))]
        {
            let _ = (plans, runtime, request);
            error_reply(&request_id, VerifyError::UnsupportedPlatform)
        }
    }

    #[cfg(windows)]
    fn verify_windows(
        &self,
        plans: &PlanManager,
        runtime: &RuntimeManager,
        request: CodexVerifyRequest,
    ) -> CodexVerifyReply {
        use std::os::windows::process::CommandExt;

        let mut result = VerifyEvidence::initial();
        let (selected, executable) = match plans
            .discovery
            .select_installation_with_path(&request.installation_id, request.scan_generation)
        {
            Ok(value) => value,
            Err(InstallationSelectionError::StaleScanGeneration)
            | Err(InstallationSelectionError::NotScanned) => {
                return error_reply(&request.request_id, VerifyError::StaleSelection)
            }
            Err(InstallationSelectionError::InvalidInstallationId)
            | Err(InstallationSelectionError::InvalidScanGeneration) => {
                return error_reply(&request.request_id, VerifyError::ValidationFailed)
            }
            Err(InstallationSelectionError::InstallationNotFound) => {
                return error_reply(&request.request_id, VerifyError::NotFound)
            }
        };
        if selected.client_kind != "codex"
            || selected.surface != "cli"
            || selected.status != "detected"
            || selected.version.as_deref() != Some(CODEX_VERSION)
        {
            result.failure_code = Some("unsupported_installation");
            return snapshot_reply(&request, result, None, None, false, false);
        }
        let Some(executable_parent) = executable.parent() else {
            result.failure_code = Some("process_ownership_unavailable");
            return snapshot_reply(&request, result, None, None, false, false);
        };
        let Ok(executable_parent_pin) = super::pin_existing_directory_chain(executable_parent)
        else {
            result.failure_code = Some("process_ownership_unavailable");
            return snapshot_reply(&request, result, None, None, false, false);
        };
        let Some(_pinned_cli) =
            crate::integrations::discovery::pin_verified_codex_cli(&executable, CODEX_VERSION)
        else {
            result.failure_code = Some("unsupported_installation");
            return snapshot_reply(&request, result, None, None, false, false);
        };
        let Some((endpoint, runtime_generation)) = ready_runtime(runtime) else {
            result.failure_code = Some("runtime_unavailable");
            return snapshot_reply(&request, result, None, None, false, false);
        };
        let roots = (plans.roots_provider)();
        let config = match read_codex_config_pinned(&roots) {
            Ok(config) => config,
            Err(_) => {
                result.failure_code = Some("config_unavailable");
                return snapshot_reply(
                    &request,
                    result,
                    Some(&endpoint),
                    Some(&runtime_generation),
                    false,
                    false,
                );
            }
        };
        if !codex_route_eligible(
            &selected,
            request.scan_generation,
            &endpoint,
            &config.bytes,
            config.presence,
        ) {
            result.failure_code = Some("route_unsupported");
            return snapshot_reply(
                &request,
                result,
                Some(&endpoint),
                Some(&runtime_generation),
                false,
                false,
            );
        }
        result.route_eligible = true;
        let Some(workspace_root) = self.inner.workspace_root.as_deref() else {
            result.failure_code = Some("workspace_unavailable");
            return snapshot_reply(
                &request,
                result,
                Some(&endpoint),
                Some(&runtime_generation),
                false,
                false,
            );
        };
        let (workspace, _workspace_pin) = match create_run_workspace(workspace_root) {
            Ok(value) => value,
            Err(_) => {
                result.failure_code = Some("workspace_unavailable");
                return snapshot_reply(
                    &request,
                    result,
                    Some(&endpoint),
                    Some(&runtime_generation),
                    false,
                    false,
                );
            }
        };
        if !no_ancestor_codex_config(&workspace, &roots) {
            result.route_eligible = false;
            result.failure_code = Some("route_unsupported");
            return snapshot_reply(
                &request,
                result,
                Some(&endpoint),
                Some(&runtime_generation),
                false,
                false,
            );
        }
        if config.validate().is_err()
            || !same_selection(plans, &request, &selected, &executable)
            || !same_runtime(runtime, &endpoint, &runtime_generation)
        {
            result.route_eligible = false;
            result.failure_code = Some("state_changed");
            return snapshot_reply(
                &request,
                result,
                Some(&endpoint),
                Some(&runtime_generation),
                false,
                false,
            );
        }

        let nonce = format!("{EXACT_VERIFY_PREFIX}{}", Uuid::new_v4().simple());
        let prompt = format!("Reply with exactly this token and no other text: {nonce}\n");
        result.probe_status = "failed";
        result.failure_code = None;
        result.selection_unchanged = true;
        result.runtime_generation_unchanged = true;
        let mut command = Command::new(&executable);
        command
            .arg("--no-daemon")
            .arg("exec")
            .arg("--ephemeral")
            .arg("--json")
            .arg("--color")
            .arg("never")
            .arg("--skip-git-repo-check")
            .arg("--sandbox")
            .arg("read-only")
            .arg("-C")
            .arg(&workspace)
            .arg("-c")
            .arg(format!(
                "openai_base_url=http://127.0.0.1:{}/v1",
                endpoint.port
            ))
            .arg("-")
            .current_dir(&workspace)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .creation_flags(0x0800_0000);
        let job = match OwnedJob::create() {
            Ok(job) => Arc::new(job),
            Err(_) => {
                result.failure_code = Some("process_ownership_unavailable");
                return snapshot_reply(
                    &request,
                    result,
                    Some(&endpoint),
                    Some(&runtime_generation),
                    false,
                    false,
                );
            }
        };
        let mut active_job = match reserve_start_slot(&self.inner) {
            Ok(active) => active,
            Err(()) => {
                result.failure_code = Some(if self.inner.shutting_down.load(Ordering::SeqCst) {
                    "process_cancelled"
                } else {
                    "process_ownership_lost"
                });
                return snapshot_reply(
                    &request,
                    result,
                    Some(&endpoint),
                    Some(&runtime_generation),
                    false,
                    false,
                );
            }
        };
        if executable_parent_pin.validate().is_err() {
            result.failure_code = Some("process_ownership_lost");
            return snapshot_reply(
                &request,
                result,
                Some(&endpoint),
                Some(&runtime_generation),
                false,
                false,
            );
        }
        let mut child = match command.spawn() {
            Ok(child) => child,
            Err(_) => {
                drop(active_job);
                result.failure_code = Some("process_start_failed");
                return snapshot_reply(
                    &request,
                    result,
                    Some(&endpoint),
                    Some(&runtime_generation),
                    false,
                    false,
                );
            }
        };
        if job.assign(&child).is_err() {
            let _ = child.kill();
            let _ = child.wait();
            drop(active_job);
            result.failure_code = Some("process_ownership_lost");
            return snapshot_reply(
                &request,
                result,
                Some(&endpoint),
                Some(&runtime_generation),
                false,
                false,
            );
        }
        *active_job = Some(Arc::clone(&job));
        drop(active_job);
        if executable_parent_pin.validate().is_err() {
            let _ = job.terminate_and_wait_empty(OWNED_CLEANUP_TIMEOUT);
            self.clear_job(&job);
            result.failure_code = Some("process_ownership_lost");
            return snapshot_reply(
                &request,
                result,
                Some(&endpoint),
                Some(&runtime_generation),
                false,
                false,
            );
        }
        let Some(stdout) = child.stdout.take() else {
            let _ = job.terminate_and_wait_empty(OWNED_CLEANUP_TIMEOUT);
            self.clear_job(&job);
            result.failure_code = Some("process_output_invalid");
            result.output_diagnostic = Some(OutputDiagnostic::ReaderFailed);
            return snapshot_reply(
                &request,
                result,
                Some(&endpoint),
                Some(&runtime_generation),
                false,
                false,
            );
        };
        let (output_tx, output_rx) = mpsc::sync_channel(1);
        let reader = thread::Builder::new()
            .name("headroom-codex-verify-reader".to_owned())
            .spawn(move || {
                let output = read_bounded_output(stdout);
                let _ = output_tx.send(output);
            });
        if reader.is_err() {
            let _ = job.terminate_and_wait_empty(OWNED_CLEANUP_TIMEOUT);
            self.clear_job(&job);
            result.failure_code = Some("process_output_invalid");
            result.output_diagnostic = Some(OutputDiagnostic::ReaderFailed);
            return snapshot_reply(
                &request,
                result,
                Some(&endpoint),
                Some(&runtime_generation),
                false,
                false,
            );
        }
        let Some(mut stdin) = child.stdin.take() else {
            let _ = job.terminate_and_wait_empty(OWNED_CLEANUP_TIMEOUT);
            self.clear_job(&job);
            result.failure_code = Some("process_start_failed");
            return snapshot_reply(
                &request,
                result,
                Some(&endpoint),
                Some(&runtime_generation),
                false,
                false,
            );
        };
        if stdin.write_all(prompt.as_bytes()).is_err() || stdin.flush().is_err() {
            let _ = job.terminate_and_wait_empty(OWNED_CLEANUP_TIMEOUT);
            self.clear_job(&job);
            result.failure_code = Some("process_start_failed");
            return snapshot_reply(
                &request,
                result,
                Some(&endpoint),
                Some(&runtime_generation),
                false,
                false,
            );
        }
        drop(stdin);

        let deadline = Instant::now() + PROCESS_TIMEOUT;
        let mut output = None;
        let mut exit_code = None;
        let mut transport_seen = false;
        let mut output_invalid = false;
        while Instant::now() < deadline {
            if self.inner.shutting_down.load(Ordering::SeqCst) {
                result.failure_code = Some("process_cancelled");
                break;
            }
            if output.is_none() {
                match output_rx.try_recv() {
                    Ok(Ok(bytes)) => output = Some(bytes),
                    Ok(Err(diagnostic)) => {
                        output_invalid = true;
                        result.output_diagnostic = Some(diagnostic);
                        break;
                    }
                    Err(mpsc::TryRecvError::Disconnected) => {
                        output_invalid = true;
                        result.output_diagnostic = Some(OutputDiagnostic::ReaderFailed);
                        break;
                    }
                    Err(mpsc::TryRecvError::Empty) => {}
                }
            }
            match crate::runtime::ingress::verify_cli_connection_to_owned_ingress(
                endpoint.port,
                child.id(),
                std::process::id(),
            ) {
                Ok(()) => transport_seen = true,
                Err(crate::runtime::ingress::IngressError::PeerOwnerMissing) => {}
                Err(_) => {
                    result.failure_code = Some("transport_unverified");
                    break;
                }
            }
            match child.try_wait() {
                Ok(Some(status)) => {
                    exit_code = status.code();
                    result.process_completed = status.success();
                    break;
                }
                Ok(None) => {}
                Err(_) => {
                    result.failure_code = Some("process_wait_failed");
                    break;
                }
            }
            thread::sleep(PROCESS_POLL);
        }
        if result.failure_code.is_none() && exit_code.is_none() {
            result.failure_code = Some("process_timeout");
        }
        if result.failure_code.is_some() || exit_code != Some(0) {
            let _ = job.terminate_and_wait_empty(OWNED_CLEANUP_TIMEOUT);
            if exit_code.is_none() {
                exit_code = child.wait().ok().and_then(|status| status.code());
            }
        }
        self.clear_job(&job);
        if output.is_none() && !output_invalid {
            match output_rx.recv_timeout(READER_RESULT_TIMEOUT) {
                Ok(Ok(bytes)) => output = Some(bytes),
                Ok(Err(diagnostic)) => {
                    output_invalid = true;
                    result.output_diagnostic = Some(diagnostic);
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    output_invalid = true;
                    result.output_diagnostic =
                        Some(reader_error_diagnostic(mpsc::RecvTimeoutError::Timeout));
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    output_invalid = true;
                    result.output_diagnostic = Some(reader_error_diagnostic(
                        mpsc::RecvTimeoutError::Disconnected,
                    ));
                }
            }
        }
        result.ingress_peer_observed = transport_seen;
        result.selection_unchanged = same_selection(plans, &request, &selected, &executable);
        result.runtime_generation_unchanged = same_runtime(runtime, &endpoint, &runtime_generation);
        if !result.selection_unchanged {
            result.failure_code = Some("selection_changed");
        } else if !result.runtime_generation_unchanged {
            result.failure_code = Some("runtime_changed");
        }
        if output_invalid || output.is_none() {
            result.failure_code.get_or_insert("process_output_invalid");
        } else if let Some(output) = output {
            let completion = decode_cli_output(&output, &nonce);
            result.marker_received = completion.marker_received;
            result.cli_error_count = completion.cli_error_count;
            result.output_diagnostic = completion.diagnostic;
            if !completion.valid {
                result.failure_code.get_or_insert("process_output_invalid");
            }
        }
        if !transport_seen {
            result.failure_code.get_or_insert("transport_unverified");
        }
        if exit_code != Some(0) {
            result.failure_code.get_or_insert("process_failed");
        }
        if config.validate().is_err() {
            result.failure_code.get_or_insert("config_changed");
        }
        let proof_complete = result.failure_code.is_none()
            && result.route_eligible
            && result.process_completed
            && result.marker_received
            && result.ingress_peer_observed
            && result.selection_unchanged
            && result.runtime_generation_unchanged
            && exit_code == Some(0);
        if proof_complete && result.cli_error_count == 0 && result.output_diagnostic.is_none() {
            result.probe_status = "passed";
        } else if proof_complete
            && result.cli_error_count > 0
            && result.output_diagnostic == Some(OutputDiagnostic::UnexpectedError)
        {
            result.probe_status = "response_verified_cli_errors";
        } else if result.failure_code.is_none() {
            result.probe_status = "failed";
            result.failure_code = Some("process_failed");
        }
        let selection_unchanged = result.selection_unchanged;
        let runtime_generation_unchanged = result.runtime_generation_unchanged;
        snapshot_reply(
            &request,
            result,
            Some(&endpoint),
            Some(&runtime_generation),
            selection_unchanged,
            runtime_generation_unchanged,
        )
    }

    #[cfg(windows)]
    fn clear_job(&self, job: &Arc<OwnedJob>) {
        if let Ok(mut active) = self.inner.active_job.lock() {
            if active
                .as_ref()
                .is_some_and(|current| Arc::ptr_eq(current, job))
            {
                *active = None;
            }
        }
    }
}

#[cfg(windows)]
fn reserve_start_slot(inner: &VerifyInner) -> Result<MutexGuard<'_, Option<Arc<OwnedJob>>>, ()> {
    let active = inner.active_job.lock().map_err(|_| ())?;
    if inner.shutting_down.load(Ordering::SeqCst) || active.is_some() {
        Err(())
    } else {
        Ok(active)
    }
}

#[cfg(windows)]
fn active_job_for_shutdown(inner: &VerifyInner) -> Result<Option<Arc<OwnedJob>>, ()> {
    inner
        .active_job
        .lock()
        .map(|active| active.clone())
        .map_err(|_| ())
}

struct VerifyGuard<'a>(&'a AtomicBool);

impl<'a> VerifyGuard<'a> {
    fn acquire(value: &'a AtomicBool) -> Option<Self> {
        value
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .ok()
            .map(|_| Self(value))
    }
}

impl Drop for VerifyGuard<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

#[derive(Clone, Copy)]
enum VerifyError {
    ValidationFailed,
    StaleSelection,
    NotFound,
    Busy,
    Unavailable,
    #[cfg(not(windows))]
    UnsupportedPlatform,
}

impl VerifyError {
    fn fields(self) -> (&'static str, bool, &'static str) {
        match self {
            Self::ValidationFailed => {
                ("validation_failed", false, "codex.verify.validation_failed")
            }
            Self::StaleSelection => ("stale_selection", true, "codex.verify.stale_selection"),
            Self::NotFound => ("not_found", false, "codex.verify.not_found"),
            Self::Busy => ("busy", true, "codex.verify.busy"),
            Self::Unavailable => ("unavailable", true, "codex.verify.unavailable"),
            #[cfg(not(windows))]
            Self::UnsupportedPlatform => (
                "unsupported_platform",
                false,
                "codex.verify.unsupported_platform",
            ),
        }
    }
}

#[derive(Clone, Copy)]
struct VerifyEvidence {
    probe_status: &'static str,
    failure_code: Option<&'static str>,
    output_diagnostic: Option<OutputDiagnostic>,
    cli_error_count: u32,
    route_eligible: bool,
    process_completed: bool,
    marker_received: bool,
    ingress_peer_observed: bool,
    selection_unchanged: bool,
    runtime_generation_unchanged: bool,
}

impl VerifyEvidence {
    fn initial() -> Self {
        Self {
            probe_status: "blocked",
            failure_code: None,
            output_diagnostic: None,
            cli_error_count: 0,
            route_eligible: false,
            process_completed: false,
            marker_received: false,
            ingress_peer_observed: false,
            selection_unchanged: false,
            runtime_generation_unchanged: false,
        }
    }
}

fn error_reply(request_id: &str, fault: VerifyError) -> CodexVerifyReply {
    let safe_id = if valid_uuid(request_id) {
        request_id.to_owned()
    } else {
        ZERO_UUID.to_owned()
    };
    let (code, retryable, message_key) = fault.fields();
    CodexVerifyReply::Error(CodexVerifyError {
        schema_version: SCHEMA_VERSION,
        request_id: safe_id,
        status: "error",
        error: CodexVerifyErrorDetail {
            code,
            retryable,
            message_key,
            diagnostic_id: None,
        },
    })
}

pub(crate) fn failure_reply(request_id: String) -> CodexVerifyReply {
    error_reply(&request_id, VerifyError::Unavailable)
}

fn snapshot_reply(
    request: &CodexVerifyRequest,
    mut evidence: VerifyEvidence,
    endpoint: Option<&ReadyStableEndpoint>,
    generation: Option<&str>,
    selection_unchanged: bool,
    runtime_generation_unchanged: bool,
) -> CodexVerifyReply {
    evidence.selection_unchanged = selection_unchanged;
    evidence.runtime_generation_unchanged = runtime_generation_unchanged;
    let endpoint_id = endpoint.map(|value| value.endpoint_id.clone());
    let runtime_generation = generation.map(str::to_owned);
    let proof_flags_ok = evidence.route_eligible
        && evidence.process_completed
        && evidence.marker_received
        && evidence.ingress_peer_observed
        && evidence.selection_unchanged
        && evidence.runtime_generation_unchanged;
    let identifiers_ok = endpoint_id.as_deref().is_some_and(valid_uuid)
        && runtime_generation.as_deref().is_some_and(valid_uuid);
    let passed_is_valid = evidence.probe_status == "passed"
        && evidence.failure_code.is_none()
        && evidence.output_diagnostic.is_none()
        && evidence.cli_error_count == 0
        && proof_flags_ok
        && identifiers_ok;
    let warned_is_valid = evidence.probe_status == "response_verified_cli_errors"
        && evidence.failure_code.is_none()
        && evidence.output_diagnostic == Some(OutputDiagnostic::UnexpectedError)
        && evidence.cli_error_count > 0
        && evidence.cli_error_count <= MAX_EVENTS as u32
        && proof_flags_ok
        && identifiers_ok;
    if (evidence.probe_status == "passed" && !passed_is_valid)
        || (evidence.probe_status == "response_verified_cli_errors" && !warned_is_valid)
    {
        evidence.probe_status = "failed";
        evidence.failure_code = Some("state_changed");
    } else if evidence.probe_status == "blocked"
        && (evidence.output_diagnostic.is_some() || evidence.cli_error_count != 0)
    {
        evidence.probe_status = "failed";
        evidence.failure_code.get_or_insert("state_changed");
    } else if evidence.cli_error_count > 0 && evidence.output_diagnostic.is_none() {
        evidence.probe_status = "failed";
        evidence.failure_code.get_or_insert("state_changed");
    }
    if evidence.probe_status != "passed"
        && evidence.probe_status != "response_verified_cli_errors"
        && evidence.failure_code.is_none()
    {
        evidence.failure_code = Some("process_failed");
        evidence.probe_status = "failed";
    }
    CodexVerifyReply::Snapshot(CodexVerifySnapshot {
        schema_version: SCHEMA_VERSION,
        request_id: request.request_id.clone(),
        status: "snapshot",
        installation_id: request.installation_id.clone(),
        scan_generation: request.scan_generation,
        probe_status: evidence.probe_status,
        failure_code: evidence.failure_code,
        output_diagnostic: evidence.output_diagnostic,
        cli_error_count: evidence.cli_error_count,
        scope: "new_process_only",
        endpoint_id,
        runtime_generation,
        route_eligible: evidence.route_eligible,
        process_completed: evidence.process_completed,
        marker_received: evidence.marker_received,
        ingress_peer_observed: evidence.ingress_peer_observed,
        selection_unchanged: evidence.selection_unchanged,
        runtime_generation_unchanged: evidence.runtime_generation_unchanged,
        saved_config_adoption: "unknown",
        auth_general: "not_tested",
        subagent: "not_tested",
    })
}

fn valid_uuid(value: &str) -> bool {
    value.len() == 36 && Uuid::parse_str(value).is_ok_and(|parsed| parsed.to_string() == value)
}

fn normalize_runtime_generation(value: &str) -> Option<String> {
    // RuntimeManager's trusted generation contract is 32 lowercase hex bytes.
    if value.len() != 32
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return None;
    }

    let normalized = format!(
        "{}-{}-{}-{}-{}",
        &value[..8],
        &value[8..12],
        &value[12..16],
        &value[16..20],
        &value[20..]
    );
    valid_uuid(&normalized).then_some(normalized)
}

#[cfg(windows)]
fn ready_runtime(runtime: &RuntimeManager) -> Option<(ReadyStableEndpoint, String)> {
    let endpoint = runtime.ready_stable_endpoint().ok()?;
    let observation = runtime.codex_connection_observation();
    if !observation.endpoint_ready
        || !observation.engine_ready
        || observation.endpoint_id.as_deref() != Some(endpoint.endpoint_id.as_str())
        || observation.endpoint_port != Some(endpoint.port)
        || endpoint.host != "127.0.0.1"
    {
        return None;
    }
    let generation = observation.runtime_generation?;
    normalize_runtime_generation(&generation).map(|generation| (endpoint, generation))
}

#[cfg(windows)]
fn same_runtime(
    runtime: &RuntimeManager,
    endpoint: &ReadyStableEndpoint,
    generation: &str,
) -> bool {
    ready_runtime(runtime).is_some_and(|(current, current_generation)| {
        current == *endpoint && current_generation == generation
    })
}

#[cfg(windows)]
fn same_selection(
    plans: &PlanManager,
    request: &CodexVerifyRequest,
    expected: &InstallationSummary,
    expected_path: &Path,
) -> bool {
    plans
        .discovery
        .select_installation_with_path(&request.installation_id, request.scan_generation)
        .is_ok_and(|(current, path)| {
            current.installation_id == expected.installation_id
                && current.client_kind == expected.client_kind
                && current.surface == expected.surface
                && current.version == expected.version
                && current.source == expected.source
                && current.status == expected.status
                && path == expected_path
        })
}

#[cfg(windows)]
fn codex_route_eligible(
    selected: &InstallationSummary,
    scan_generation: u64,
    endpoint: &ReadyStableEndpoint,
    bytes: &[u8],
    presence: super::ConfigPresence,
) -> bool {
    if selected.client_kind != "codex"
        || selected.surface != "cli"
        || selected.version.as_deref() != Some(CODEX_VERSION)
        || !super::super::plan::supported_version(ClientKind::Codex, CODEX_VERSION)
    {
        return false;
    }
    let Ok(now) = SystemTime::now().duration_since(UNIX_EPOCH) else {
        return false;
    };
    if now.as_secs() > MAX_SAFE_INTEGER - 300 {
        return false;
    }
    let context = PlanContext {
        request_id: Uuid::new_v4().to_string(),
        installation_id: selected.installation_id.clone(),
        client_kind: ClientKind::Codex,
        config_source: ConfigSource::CodexToml,
        surface: selected.surface.to_owned(),
        version: selected.version.clone(),
        scope: ConfigScope::UserConfig,
        scan_generation,
        endpoint: StableEndpoint {
            endpoint_id: endpoint.endpoint_id.clone(),
            host: endpoint.host.to_owned(),
            port: endpoint.port,
        },
        now_unix_seconds: now.as_secs(),
    };
    preview_config_with_presence(&context, bytes, presence)
        .is_ok_and(|plan| plan.status == PlanStatus::Preview && plan.blockers.is_empty())
}

#[cfg(windows)]
fn create_run_workspace(root: &Path) -> Result<(PathBuf, PinnedDirectoryChain), ()> {
    super::validate_lexical_path(root).map_err(|_| ())?;
    super::ensure_fixed_volume(root).map_err(|_| ())?;
    let mut missing = Vec::new();
    let mut existing = root.to_path_buf();
    loop {
        match fs::symlink_metadata(&existing) {
            Ok(metadata) if metadata.is_dir() && !is_reparse(&metadata) => break,
            Ok(_) => return Err(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                missing.push(existing.clone());
                existing = existing.parent().ok_or(())?.to_path_buf();
            }
            Err(_) => return Err(()),
        }
    }
    let mut pins = super::pin_existing_directory_chain(&existing).map_err(|_| ())?;
    let canonical_existing = fs::canonicalize(&existing).map_err(|_| ())?;
    if !super::same_path(&existing, &canonical_existing) {
        return Err(());
    }
    for component in missing.into_iter().rev() {
        pins.validate().map_err(|_| ())?;
        if fs::create_dir(&component).is_err() {
            return Err(());
        }
        let metadata = fs::symlink_metadata(&component).map_err(|_| ())?;
        if !metadata.is_dir() || is_reparse(&metadata) {
            return Err(());
        }
        let canonical = fs::canonicalize(&component).map_err(|_| ())?;
        if !super::same_path(&component, &canonical) {
            return Err(());
        }
        pins = super::pin_existing_directory_chain(&component).map_err(|_| ())?;
    }
    pins.validate().map_err(|_| ())?;
    let leaf = root.join(Uuid::new_v4().to_string());
    fs::create_dir(&leaf).map_err(|_| ())?;
    let metadata = fs::symlink_metadata(&leaf).map_err(|_| ())?;
    if !metadata.is_dir() || is_reparse(&metadata) {
        return Err(());
    }
    let canonical_leaf = fs::canonicalize(&leaf).map_err(|_| ())?;
    if !super::same_path(&leaf, &canonical_leaf) || !canonical_leaf.starts_with(&canonical_existing)
    {
        return Err(());
    }
    let leaf_pins = super::pin_existing_directory_chain(&leaf).map_err(|_| ())?;
    leaf_pins.validate().map_err(|_| ())?;
    Ok((canonical_leaf, leaf_pins))
}

#[cfg(windows)]
fn no_ancestor_codex_config(workspace: &Path, roots: &ConfigRootInputs) -> bool {
    let global_config = roots
        .codex_home
        .clone()
        .or_else(|| roots.user_profile.clone().map(|path| path.join(".codex")))
        .map(|path| path.join("config.toml"));
    let global_canonical = global_config
        .as_deref()
        .and_then(|path| fs::canonicalize(path).ok());
    let Ok(workspace) = fs::canonicalize(workspace) else {
        return false;
    };
    let mut current = Some(workspace.as_path());
    while let Some(directory) = current {
        let codex_dir = directory.join(".codex");
        match fs::symlink_metadata(&codex_dir) {
            Ok(metadata) if metadata.is_dir() && !is_reparse(&metadata) => {
                let config = codex_dir.join("config.toml");
                match fs::symlink_metadata(&config) {
                    Ok(metadata) if metadata.is_file() && !is_reparse(&metadata) => {
                        let Some(global) = global_canonical.as_deref() else {
                            return false;
                        };
                        let Ok(config) = fs::canonicalize(config) else {
                            return false;
                        };
                        if !super::same_path(&config, global) {
                            return false;
                        }
                    }
                    Ok(_) => return false,
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                    Err(_) => return false,
                }
            }
            Ok(_) => return false,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return false,
        }
        current = directory.parent();
    }
    true
}

#[cfg(windows)]
fn is_reparse(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    use windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(windows)]
fn read_bounded_output(output: impl Read) -> Result<Vec<u8>, OutputDiagnostic> {
    let mut bytes = Vec::with_capacity(64 * 1024);
    output
        .take((MAX_OUTPUT_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| OutputDiagnostic::ReaderFailed)?;
    if bytes.len() > MAX_OUTPUT_BYTES {
        return Err(OutputDiagnostic::OutputOversize);
    }
    Ok(bytes)
}

fn reader_error_diagnostic(error: mpsc::RecvTimeoutError) -> OutputDiagnostic {
    match error {
        mpsc::RecvTimeoutError::Timeout => OutputDiagnostic::ReaderTimeout,
        mpsc::RecvTimeoutError::Disconnected => OutputDiagnostic::ReaderFailed,
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct CliCompletion {
    valid: bool,
    marker_received: bool,
    terminal_received: bool,
    cli_error_count: u32,
    diagnostic: Option<OutputDiagnostic>,
}

fn decode_cli_output(bytes: &[u8], nonce: &str) -> CliCompletion {
    if bytes.len() > MAX_OUTPUT_BYTES {
        return invalid_completion(OutputDiagnostic::OutputOversize, 0);
    }
    if nonce.len() > 128 {
        return invalid_completion(OutputDiagnostic::InvalidShape, 0);
    }
    let mut result = CliCompletion {
        valid: true,
        ..CliCompletion::default()
    };
    let mut line_count = 0usize;
    let mut marker_count = 0usize;
    let mut terminal_count = 0usize;
    let mut fatal = false;
    for line in bytes.split(|byte| *byte == b'\n') {
        let line = line.strip_suffix(b"\r").unwrap_or(line);
        if line.is_empty() {
            continue;
        }
        line_count += 1;
        if line.len() > MAX_LINE_BYTES || line_count > MAX_EVENTS {
            return invalid_completion(OutputDiagnostic::LineOrEventLimit, result.cli_error_count);
        }
        let value = match deserialize_unique_json(line) {
            Ok(value) => value,
            Err(()) => {
                return invalid_completion(OutputDiagnostic::InvalidJson, result.cli_error_count)
            }
        };
        let Some(object) = value.as_object() else {
            return invalid_completion(OutputDiagnostic::InvalidShape, result.cli_error_count);
        };
        let Some(event_type) = object.get("type").and_then(Value::as_str) else {
            return invalid_completion(OutputDiagnostic::InvalidShape, result.cli_error_count);
        };
        match event_type {
            "item_started" | "item_completed" | "response_item" | "item.started"
            | "item.completed" => {
                let Some(item) = object.get("item").and_then(Value::as_object) else {
                    return invalid_completion(
                        OutputDiagnostic::InvalidShape,
                        result.cli_error_count,
                    );
                };
                let Some(item_type) = item.get("type").and_then(Value::as_str) else {
                    return invalid_completion(
                        OutputDiagnostic::InvalidShape,
                        result.cli_error_count,
                    );
                };
                match item_type {
                    "agent_message" | "agentMessage" => {
                        if exact_agent_text(item).is_none() {
                            set_fatal_diagnostic(
                                &mut result.diagnostic,
                                &mut fatal,
                                OutputDiagnostic::InvalidShape,
                            );
                        } else if exact_agent_text(item) == Some(nonce) {
                            marker_count += 1;
                        }
                    }
                    "reasoning" | "user_message" | "userMessage" => {}
                    "function_call"
                    | "function_call_output"
                    | "command_execution"
                    | "web_search_call"
                    | "mcp_tool_call"
                    | "tool_call"
                    | "tool_call_output" => {
                        set_fatal_diagnostic(
                            &mut result.diagnostic,
                            &mut fatal,
                            OutputDiagnostic::UnexpectedTool,
                        );
                    }
                    "error" => result.cli_error_count += 1,
                    _ => set_fatal_diagnostic(
                        &mut result.diagnostic,
                        &mut fatal,
                        OutputDiagnostic::UnexpectedItem,
                    ),
                }
            }
            "event_msg" | "event.msg" => {
                let Some(payload) = object.get("payload").and_then(Value::as_object) else {
                    return invalid_completion(
                        OutputDiagnostic::InvalidShape,
                        result.cli_error_count,
                    );
                };
                let Some(payload_type) = payload.get("type").and_then(Value::as_str) else {
                    return invalid_completion(
                        OutputDiagnostic::InvalidShape,
                        result.cli_error_count,
                    );
                };
                match payload_type {
                    "task_complete" | "turn_completed" => {
                        terminal_count += 1;
                    }
                    "turn_failed" | "task_failed" | "task_aborted" => {
                        set_fatal_diagnostic(
                            &mut result.diagnostic,
                            &mut fatal,
                            OutputDiagnostic::UnexpectedError,
                        );
                    }
                    "error" => set_fatal_diagnostic(
                        &mut result.diagnostic,
                        &mut fatal,
                        OutputDiagnostic::UnexpectedError,
                    ),
                    "task_started" | "turn_started" | "agent_message_delta" => {}
                    _ => set_fatal_diagnostic(
                        &mut result.diagnostic,
                        &mut fatal,
                        OutputDiagnostic::UnexpectedEvent,
                    ),
                }
            }
            "turn_completed" | "turn.completed" => terminal_count += 1,
            "turn_failed" | "turn.failed" | "task_failed" | "task_aborted" | "turn_aborted" => {
                set_fatal_diagnostic(
                    &mut result.diagnostic,
                    &mut fatal,
                    OutputDiagnostic::UnexpectedError,
                )
            }
            "error" => result.cli_error_count += 1,
            "thread_started" | "thread.started" | "turn_started" | "turn.started" => {}
            _ => set_fatal_diagnostic(
                &mut result.diagnostic,
                &mut fatal,
                OutputDiagnostic::UnexpectedEvent,
            ),
        }
    }
    result.marker_received = marker_count == 1;
    result.terminal_received = terminal_count == 1;
    if result.diagnostic.is_none() {
        result.diagnostic = if marker_count > 1 {
            Some(OutputDiagnostic::MarkerMultiple)
        } else if terminal_count > 1 {
            Some(OutputDiagnostic::TerminalMultiple)
        } else if marker_count == 0 {
            Some(OutputDiagnostic::MarkerMissing)
        } else if terminal_count == 0 {
            Some(OutputDiagnostic::TerminalMissing)
        } else if result.cli_error_count > 0 {
            Some(OutputDiagnostic::UnexpectedError)
        } else {
            None
        };
    }
    result.valid = marker_count == 1 && terminal_count == 1 && !fatal;
    result
}

fn invalid_completion(diagnostic: OutputDiagnostic, cli_error_count: u32) -> CliCompletion {
    CliCompletion {
        cli_error_count,
        diagnostic: Some(diagnostic),
        ..CliCompletion::default()
    }
}

fn set_fatal_diagnostic(
    slot: &mut Option<OutputDiagnostic>,
    fatal: &mut bool,
    diagnostic: OutputDiagnostic,
) {
    *fatal = true;
    if slot.is_none() {
        *slot = Some(diagnostic);
    }
}

fn exact_agent_text(item: &Map<String, Value>) -> Option<&str> {
    if let Some(text) = item.get("text").and_then(Value::as_str) {
        return Some(text);
    }
    let content = item.get("content")?.as_array()?;
    if content.len() != 1 {
        return None;
    }
    let block = content[0].as_object()?;
    matches!(
        block.get("type").and_then(Value::as_str),
        Some("text" | "output_text")
    )
    .then(|| block.get("text").and_then(Value::as_str))
    .flatten()
}

fn deserialize_unique_json(bytes: &[u8]) -> Result<Value, ()> {
    let mut deserializer = serde_json::Deserializer::from_slice(bytes);
    let value = UniqueValue::deserialize(&mut deserializer).map_err(|_| ())?;
    deserializer.end().map_err(|_| ())?;
    Ok(value.0)
}

struct UniqueValue(Value);

impl<'de> Deserialize<'de> for UniqueValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(UniqueValueVisitor)
    }
}

struct UniqueValueVisitor;

impl<'de> Visitor<'de> for UniqueValueVisitor {
    type Value = UniqueValue;

    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("bounded JSON with unique object keys")
    }

    fn visit_bool<E>(self, value: bool) -> Result<Self::Value, E> {
        Ok(UniqueValue(Value::Bool(value)))
    }

    fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E> {
        Ok(UniqueValue(Value::Number(value.into())))
    }

    fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E> {
        Ok(UniqueValue(Value::Number(value.into())))
    }

    fn visit_f64<E>(self, value: f64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        serde_json::Number::from_f64(value)
            .map(Value::Number)
            .map(UniqueValue)
            .ok_or_else(|| E::custom("non-finite JSON number"))
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(UniqueValue(Value::String(value.to_owned())))
    }

    fn visit_string<E>(self, value: String) -> Result<Self::Value, E> {
        Ok(UniqueValue(Value::String(value)))
    }

    fn visit_none<E>(self) -> Result<Self::Value, E> {
        Ok(UniqueValue(Value::Null))
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E> {
        Ok(UniqueValue(Value::Null))
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut values = Vec::new();
        while let Some(value) = sequence.next_element::<UniqueValue>()? {
            values.push(value.0);
            if values.len() > MAX_EVENTS {
                return Err(de::Error::custom("array limit exceeded"));
            }
        }
        Ok(UniqueValue(Value::Array(values)))
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut values = Map::new();
        let mut keys = HashSet::new();
        while let Some(key) = map.next_key::<String>()? {
            if !keys.insert(key.clone()) || keys.len() > MAX_EVENTS {
                return Err(de::Error::custom("duplicate key or object limit exceeded"));
            }
            let value = map.next_value::<UniqueValue>()?;
            values.insert(key, value.0);
        }
        Ok(UniqueValue(Value::Object(values)))
    }
}

#[cfg(windows)]
struct OwnedJob(windows_sys::Win32::Foundation::HANDLE);

#[cfg(windows)]
unsafe impl Send for OwnedJob {}
#[cfg(windows)]
unsafe impl Sync for OwnedJob {}

#[cfg(windows)]
impl OwnedJob {
    fn create() -> Result<Self, ()> {
        use std::mem::size_of;
        use std::ptr::null;
        use windows_sys::Win32::System::JobObjects::{
            CreateJobObjectW, JobObjectExtendedLimitInformation, SetInformationJobObject,
            JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        };
        let handle = unsafe { CreateJobObjectW(null(), std::ptr::null()) };
        if handle.is_null() {
            return Err(());
        }
        let mut information: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { std::mem::zeroed() };
        information.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        let configured = unsafe {
            SetInformationJobObject(
                handle,
                JobObjectExtendedLimitInformation,
                &information as *const _ as *const _,
                size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
        };
        if configured == 0 {
            unsafe { windows_sys::Win32::Foundation::CloseHandle(handle) };
            return Err(());
        }
        Ok(Self(handle))
    }

    fn assign(&self, child: &Child) -> Result<(), ()> {
        use std::os::windows::io::AsRawHandle;
        let assigned = unsafe {
            windows_sys::Win32::System::JobObjects::AssignProcessToJobObject(
                self.0,
                child.as_raw_handle() as windows_sys::Win32::Foundation::HANDLE,
            )
        };
        (assigned != 0).then_some(()).ok_or(())
    }

    fn terminate_and_wait_empty(&self, timeout: Duration) -> bool {
        use std::mem::size_of;
        use std::ptr::null_mut;
        use windows_sys::Win32::System::JobObjects::{
            JobObjectBasicAccountingInformation, QueryInformationJobObject, TerminateJobObject,
            JOBOBJECT_BASIC_ACCOUNTING_INFORMATION,
        };
        unsafe { TerminateJobObject(self.0, 0xC000_013A) };
        let deadline = Instant::now() + timeout;
        loop {
            let mut information = JOBOBJECT_BASIC_ACCOUNTING_INFORMATION::default();
            let queried = unsafe {
                QueryInformationJobObject(
                    self.0,
                    JobObjectBasicAccountingInformation,
                    &mut information as *mut _ as *mut _,
                    size_of::<JOBOBJECT_BASIC_ACCOUNTING_INFORMATION>() as u32,
                    null_mut(),
                )
            };
            if queried != 0 && information.ActiveProcesses == 0 {
                return true;
            }
            if Instant::now() >= deadline {
                return false;
            }
            thread::sleep(Duration::from_millis(20));
        }
    }
}

#[cfg(windows)]
impl Drop for OwnedJob {
    fn drop(&mut self) {
        unsafe { windows_sys::Win32::Foundation::CloseHandle(self.0) };
    }
}

#[cfg(test)]
mod tests {
    use super::{
        decode_cli_output, snapshot_reply, CodexVerifyRequest, OutputDiagnostic, VerifyEvidence,
    };
    use crate::runtime::manager::ReadyStableEndpoint;
    use serde_json::json;
    use std::sync::atomic::AtomicBool;

    const NONCE: &str = "HEADROOM_LOCAL_VERIFY_0123456789abcdef0123456789abcdef";
    const REQUEST: &str = "7a54d50f-964e-4ce0-bd3f-bbc08b0f5c5e";
    const INSTALLATION: &str = "652ba4c8-5f97-4e88-a597-b65f516d6be3";
    const ENDPOINT: &str = "ed7b249f-706c-43f7-8c06-12502e8fba4d";
    const MANAGER_GENERATION: &str = "845d57c4818f409b8a7f8c01986476fc";

    fn output(agent_text: &str, terminal: &str) -> Vec<u8> {
        let events = [
            json!({"type":"item_completed","item":{"type":"agent_message","text":agent_text}}),
            json!({"type":"event_msg","payload":{"type":terminal,"last_agent_message":agent_text}}),
        ];
        events
            .iter()
            .map(|event| serde_json::to_string(event).unwrap())
            .collect::<Vec<_>>()
            .join("\n")
            .into_bytes()
    }

    fn jsonl(events: &[serde_json::Value]) -> Vec<u8> {
        events
            .iter()
            .map(|event| serde_json::to_string(event).unwrap())
            .collect::<Vec<_>>()
            .join("\n")
            .into_bytes()
    }

    #[test]
    fn exact_nonce_and_terminal_are_required() {
        let valid = decode_cli_output(&output(NONCE, "task_complete"), NONCE);
        assert!(valid.valid && valid.marker_received && valid.terminal_received);
        assert!(
            !decode_cli_output(
                &output(
                    "prefix HEADROOM_LOCAL_VERIFY_0123456789abcdef0123456789abcdef",
                    "task_complete"
                ),
                NONCE
            )
            .valid
        );
        assert!(!decode_cli_output(&output(NONCE, "turn_failed"), NONCE).valid);
    }

    #[test]
    fn generic_error_notification_can_coexist_with_exact_completed_response() {
        let bytes = jsonl(&[
            json!({"type":"error","message":"unclassified CLI notice"}),
            json!({"type":"item.completed","item":{"type":"error","message":"another notice"}}),
            json!({"type":"item.completed","item":{"type":"agent_message","text":NONCE}}),
            json!({"type":"turn.completed"}),
        ]);
        let completion = decode_cli_output(&bytes, NONCE);
        assert!(completion.valid);
        assert!(completion.marker_received && completion.terminal_received);
        assert_eq!(completion.cli_error_count, 2);
        assert_eq!(
            completion.diagnostic,
            Some(OutputDiagnostic::UnexpectedError)
        );
    }

    #[test]
    fn fatal_event_after_generic_error_still_fails_with_fatal_reason() {
        let bytes = jsonl(&[
            json!({"type":"error","message":"unclassified CLI notice"}),
            json!({"type":"item.completed","item":{"type":"function_call"}}),
            json!({"type":"item.completed","item":{"type":"agent_message","text":NONCE}}),
            json!({"type":"turn.completed"}),
        ]);
        let completion = decode_cli_output(&bytes, NONCE);
        assert!(!completion.valid);
        assert_eq!(
            completion.diagnostic,
            Some(OutputDiagnostic::UnexpectedTool)
        );
        assert_eq!(completion.cli_error_count, 1);

        let failed_turn = jsonl(&[
            json!({"type":"error","message":"unclassified CLI notice"}),
            json!({"type":"item.completed","item":{"type":"agent_message","text":NONCE}}),
            json!({"type":"turn.failed"}),
        ]);
        let completion = decode_cli_output(&failed_turn, NONCE);
        assert!(!completion.valid);
        assert_eq!(
            completion.diagnostic,
            Some(OutputDiagnostic::UnexpectedError)
        );
        assert_eq!(completion.cli_error_count, 1);

        let missing_marker = jsonl(&[
            json!({"type":"error","message":"unclassified CLI notice"}),
            json!({"type":"turn.completed"}),
        ]);
        let completion = decode_cli_output(&missing_marker, NONCE);
        assert!(!completion.valid);
        assert_eq!(completion.diagnostic, Some(OutputDiagnostic::MarkerMissing));
        assert_eq!(completion.cli_error_count, 1);

        let duplicate_terminal = jsonl(&[
            json!({"type":"error","message":"unclassified CLI notice"}),
            json!({"type":"item.completed","item":{"type":"agent_message","text":NONCE}}),
            json!({"type":"turn.completed"}),
            json!({"type":"turn.completed"}),
        ]);
        let completion = decode_cli_output(&duplicate_terminal, NONCE);
        assert!(!completion.valid);
        assert_eq!(
            completion.diagnostic,
            Some(OutputDiagnostic::TerminalMultiple)
        );
        assert_eq!(completion.cli_error_count, 1);
    }

    #[test]
    fn pinned_cli_four_event_shape_decodes_exact_marker_and_terminal() {
        let bytes = jsonl(&[
            json!({"type":"thread.started","thread_id":"019a0000-0000-7000-8000-000000000019"}),
            json!({"type":"turn.started"}),
            json!({"type":"item.completed","item":{"id":"msg_fixture","type":"agent_message","text":NONCE}}),
            json!({"type":"turn.completed"}),
        ]);
        let completion = decode_cli_output(&bytes, NONCE);
        assert!(completion.valid);
        assert!(completion.marker_received);
        assert!(completion.terminal_received);
        assert_eq!(completion.diagnostic, None);

        let newline_marker = jsonl(&[
            json!({"type":"item.completed","item":{"type":"agent_message","text":format!("{NONCE}\n")}}),
            json!({"type":"turn.completed"}),
        ]);
        assert_eq!(
            decode_cli_output(&newline_marker, NONCE).diagnostic,
            Some(OutputDiagnostic::MarkerMissing)
        );
    }

    #[test]
    fn decoder_reports_only_fixed_output_failure_reasons() {
        let message = |text: &str| json!({"type":"item.completed","item":{"type":"agent_message","text":text}});
        let terminal = json!({"type":"turn.completed"});
        let cases = [
            (b"{".to_vec(), OutputDiagnostic::InvalidJson),
            (jsonl(&[json!([])]), OutputDiagnostic::InvalidShape),
            (
                jsonl(&[json!({"type":"error"})]),
                OutputDiagnostic::MarkerMissing,
            ),
            (
                jsonl(&[json!({"type":"item.completed","item":{"type":"function_call"}})]),
                OutputDiagnostic::UnexpectedTool,
            ),
            (
                jsonl(&[json!({"type":"item.completed","item":{"type":"unknown_item"}})]),
                OutputDiagnostic::UnexpectedItem,
            ),
            (
                jsonl(&[json!({"type":"unrecognized.event"})]),
                OutputDiagnostic::UnexpectedEvent,
            ),
            (
                jsonl(&[message("different"), terminal.clone()]),
                OutputDiagnostic::MarkerMissing,
            ),
            (
                jsonl(&[message(NONCE), message(NONCE), terminal.clone()]),
                OutputDiagnostic::MarkerMultiple,
            ),
            (jsonl(&[message(NONCE)]), OutputDiagnostic::TerminalMissing),
            (
                jsonl(&[message(NONCE), terminal.clone(), terminal]),
                OutputDiagnostic::TerminalMultiple,
            ),
        ];
        for (bytes, expected) in cases {
            let result = decode_cli_output(&bytes, NONCE);
            assert!(!result.valid);
            assert_eq!(result.diagnostic, Some(expected));
        }
        let error_only = decode_cli_output(&jsonl(&[json!({"type":"error"})]), NONCE);
        assert!(!error_only.valid);
        assert_eq!(error_only.diagnostic, Some(OutputDiagnostic::MarkerMissing));
        assert_eq!(error_only.cli_error_count, 1);
        assert_eq!(
            decode_cli_output(&vec![b'x'; super::MAX_OUTPUT_BYTES + 1], NONCE).diagnostic,
            Some(OutputDiagnostic::OutputOversize)
        );
        let limited_events = vec![json!({"type":"thread.started"}); super::MAX_EVENTS + 1];
        assert_eq!(
            decode_cli_output(&jsonl(&limited_events), NONCE).diagnostic,
            Some(OutputDiagnostic::LineOrEventLimit)
        );

        let unexpected_then_missing =
            jsonl(&[json!({"type":"item.completed","item":{"type":"function_call"}})]);
        assert_eq!(
            decode_cli_output(&unexpected_then_missing, NONCE).diagnostic,
            Some(OutputDiagnostic::UnexpectedTool),
            "a concrete unsupported item must not be replaced by a generic missing-marker diagnosis"
        );
    }

    #[test]
    fn reader_failures_map_to_fixed_diagnostics() {
        assert_eq!(
            super::reader_error_diagnostic(std::sync::mpsc::RecvTimeoutError::Timeout),
            OutputDiagnostic::ReaderTimeout
        );
        assert_eq!(
            super::reader_error_diagnostic(std::sync::mpsc::RecvTimeoutError::Disconnected),
            OutputDiagnostic::ReaderFailed
        );
    }

    #[cfg(windows)]
    #[test]
    fn bounded_stdout_reader_distinguishes_io_failure_from_oversize() {
        struct FailedReader;
        impl std::io::Read for FailedReader {
            fn read(&mut self, _buffer: &mut [u8]) -> std::io::Result<usize> {
                Err(std::io::Error::other("fixture"))
            }
        }

        assert_eq!(
            super::read_bounded_output(FailedReader),
            Err(OutputDiagnostic::ReaderFailed)
        );
        assert_eq!(
            super::read_bounded_output(std::io::Cursor::new(vec![0; super::MAX_OUTPUT_BYTES + 1])),
            Err(OutputDiagnostic::OutputOversize)
        );
    }

    #[test]
    fn tool_items_and_duplicate_keys_fail_closed() {
        let tool = br#"{"type":"item_completed","item":{"type":"function_call"}}"#;
        assert!(!decode_cli_output(tool, NONCE).valid);
        let duplicate = format!(
            "{{\"type\":\"item_completed\",\"type\":\"turn_completed\",\"item\":{{\"type\":\"agent_message\",\"text\":\"{NONCE}\"}}}}\n{{\"type\":\"turn_completed\"}}"
        );
        assert!(!decode_cli_output(duplicate.as_bytes(), NONCE).valid);
    }

    #[test]
    fn size_and_line_caps_fail_closed() {
        assert!(!decode_cli_output(&vec![b'x'; super::MAX_OUTPUT_BYTES + 1], NONCE).valid);
        let line = format!("{{\"type\":\"{}\"}}", "x".repeat(super::MAX_LINE_BYTES));
        assert!(!decode_cli_output(line.as_bytes(), NONCE).valid);
    }

    #[cfg(windows)]
    #[test]
    fn debug_manager_workspace_root_supports_initial_creation() {
        use std::path::PathBuf;

        let manager = super::CodexVerifyManager::new(None);
        let Some(root) = manager.inner.workspace_root.clone() else {
            panic!("debug workspace root must be configured");
        };
        let root_existed = std::fs::symlink_metadata(&root).is_ok();
        let project_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("project root")
            .to_path_buf();
        let project_boundary = std::fs::canonicalize(&project_root).expect("project boundary");
        let created = super::create_run_workspace(&root);
        let created_ok = created.is_ok();

        if let Ok((workspace, pins)) = created {
            drop(pins);
            let root_canonical = std::fs::canonicalize(&root).ok();
            let leaf_is_owned = std::fs::symlink_metadata(&workspace)
                .is_ok_and(|metadata| metadata.is_dir() && !super::is_reparse(&metadata))
                && workspace
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(super::valid_uuid)
                && std::fs::canonicalize(&workspace).is_ok_and(|canonical| {
                    super::super::same_path(&workspace, &canonical)
                        && root_canonical
                            .as_ref()
                            .is_some_and(|root| canonical.starts_with(root))
                });
            if leaf_is_owned {
                let _ = std::fs::remove_dir(&workspace);
            }
        }
        if !root_existed {
            let safe_empty_root = std::fs::symlink_metadata(&root)
                .is_ok_and(|metadata| metadata.is_dir() && !super::is_reparse(&metadata))
                && std::fs::canonicalize(&root).is_ok_and(|canonical| {
                    super::super::same_path(&root, &canonical)
                        && canonical.starts_with(&project_boundary)
                })
                && std::fs::read_dir(&root).is_ok_and(|mut entries| entries.next().is_none());
            if safe_empty_root {
                let _ = std::fs::remove_dir(&root);
            }
        }
        assert!(created_ok, "manager debug workspace must be creatable");
    }

    #[test]
    fn runtime_generation_matches_manager_shape_and_normalizes_for_ipc() {
        assert_eq!(
            super::normalize_runtime_generation(MANAGER_GENERATION).as_deref(),
            Some("845d57c4-818f-409b-8a7f-8c01986476fc")
        );
        for invalid in [
            "845d57c4818f409b8a7f8c01986476f",      // short
            "845d57c4818f409b8a7f8c01986476fca",    // long
            "845d57c4818f409b8a7f8c01986476fg",     // non-hex
            "845D57C4818F409B8A7F8C01986476FC",     // uppercase
            "845d57c4-818f-409b-8a7f-8c01986476fc", // not manager wire shape
        ] {
            assert_eq!(
                super::normalize_runtime_generation(invalid),
                None,
                "{invalid}"
            );
        }
    }

    #[test]
    fn passed_snapshot_requires_every_proof_flag_and_identifiers() {
        let request = CodexVerifyRequest {
            schema_version: 1,
            request_id: REQUEST.to_owned(),
            installation_id: INSTALLATION.to_owned(),
            scan_generation: 3,
        };
        let endpoint = ReadyStableEndpoint {
            endpoint_id: ENDPOINT.to_owned(),
            host: "127.0.0.1",
            port: 51778,
        };
        let evidence = VerifyEvidence {
            probe_status: "passed",
            failure_code: None,
            output_diagnostic: None,
            cli_error_count: 0,
            route_eligible: true,
            process_completed: true,
            marker_received: true,
            ingress_peer_observed: true,
            selection_unchanged: true,
            runtime_generation_unchanged: true,
        };
        let reply = snapshot_reply(
            &request,
            evidence,
            Some(&endpoint),
            Some("845d57c4-818f-409b-8a7f-8c01986476fc"),
            true,
            true,
        );
        let super::CodexVerifyReply::Snapshot(snapshot) = reply else {
            panic!()
        };
        assert_eq!(snapshot.probe_status, "passed");
        assert_eq!(snapshot.output_diagnostic, None);
        assert_eq!(snapshot.cli_error_count, 0);
        assert_eq!(snapshot.saved_config_adoption, "unknown");
        assert_eq!(snapshot.auth_general, "not_tested");
        assert_eq!(snapshot.subagent, "not_tested");
        let wire = serde_json::to_value(&snapshot).expect("snapshot serializes");
        assert_eq!(
            wire.get("output_diagnostic"),
            Some(&serde_json::Value::Null)
        );

        let failed = snapshot_reply(
            &request,
            VerifyEvidence {
                marker_received: false,
                ..evidence
            },
            Some(&endpoint),
            Some("845d57c4-818f-409b-8a7f-8c01986476fc"),
            true,
            true,
        );
        let super::CodexVerifyReply::Snapshot(snapshot) = failed else {
            panic!()
        };
        assert_ne!(snapshot.probe_status, "passed");
    }

    #[test]
    fn verified_response_with_generic_errors_requires_complete_unchanged_proof() {
        let request = CodexVerifyRequest {
            schema_version: 1,
            request_id: REQUEST.to_owned(),
            installation_id: INSTALLATION.to_owned(),
            scan_generation: 3,
        };
        let endpoint = ReadyStableEndpoint {
            endpoint_id: ENDPOINT.to_owned(),
            host: "127.0.0.1",
            port: 51778,
        };
        let evidence = VerifyEvidence {
            probe_status: "response_verified_cli_errors",
            failure_code: None,
            output_diagnostic: Some(OutputDiagnostic::UnexpectedError),
            cli_error_count: 2,
            route_eligible: true,
            process_completed: true,
            marker_received: true,
            ingress_peer_observed: true,
            selection_unchanged: true,
            runtime_generation_unchanged: true,
        };
        let response = snapshot_reply(
            &request,
            evidence,
            Some(&endpoint),
            Some("845d57c4-818f-409b-8a7f-8c01986476fc"),
            true,
            true,
        );
        let super::CodexVerifyReply::Snapshot(snapshot) = response else {
            panic!()
        };
        assert_eq!(snapshot.probe_status, "response_verified_cli_errors");
        assert_eq!(snapshot.failure_code, None);
        assert_eq!(
            snapshot.output_diagnostic,
            Some(OutputDiagnostic::UnexpectedError)
        );
        assert_eq!(snapshot.cli_error_count, 2);

        let incomplete = snapshot_reply(
            &request,
            VerifyEvidence {
                marker_received: false,
                ..evidence
            },
            Some(&endpoint),
            Some("845d57c4-818f-409b-8a7f-8c01986476fc"),
            true,
            true,
        );
        let super::CodexVerifyReply::Snapshot(snapshot) = incomplete else {
            panic!()
        };
        assert_eq!(snapshot.probe_status, "failed");
        assert_eq!(snapshot.failure_code, Some("state_changed"));

        let incomplete_output = decode_cli_output(&jsonl(&[json!({"type":"error"})]), NONCE);
        let incomplete = snapshot_reply(
            &request,
            VerifyEvidence {
                probe_status: "blocked",
                failure_code: Some("process_output_invalid"),
                output_diagnostic: incomplete_output.diagnostic,
                cli_error_count: incomplete_output.cli_error_count,
                route_eligible: true,
                process_completed: true,
                marker_received: incomplete_output.marker_received,
                ingress_peer_observed: true,
                selection_unchanged: true,
                runtime_generation_unchanged: true,
            },
            Some(&endpoint),
            Some("845d57c4-818f-409b-8a7f-8c01986476fc"),
            true,
            true,
        );
        let super::CodexVerifyReply::Snapshot(snapshot) = incomplete else {
            panic!()
        };
        assert_eq!(snapshot.probe_status, "failed");
        assert_eq!(snapshot.failure_code, Some("process_output_invalid"));
        assert_eq!(
            snapshot.output_diagnostic,
            Some(OutputDiagnostic::MarkerMissing)
        );
        assert_eq!(snapshot.cli_error_count, 1);
    }

    #[cfg(windows)]
    #[test]
    fn verifier_busy_guard_releases_after_request_finishes() {
        let busy = AtomicBool::new(false);
        let first = super::VerifyGuard::acquire(&busy).expect("first lease");
        assert!(super::VerifyGuard::acquire(&busy).is_none());
        drop(first);
        assert!(super::VerifyGuard::acquire(&busy).is_some());
    }

    #[cfg(windows)]
    #[test]
    fn shutdown_waits_for_start_slot_to_publish_owned_job() {
        use std::sync::mpsc;
        use std::sync::Arc;
        use std::time::Duration;

        let manager = super::CodexVerifyManager::new(None);
        let mut slot = super::reserve_start_slot(&manager.inner).expect("start slot");
        let job = Arc::new(super::OwnedJob::create().expect("empty owned job"));
        let inner = Arc::clone(&manager.inner);
        let (started_tx, started_rx) = mpsc::sync_channel(1);
        let shutdown = std::thread::spawn(move || {
            inner
                .shutting_down
                .store(true, std::sync::atomic::Ordering::SeqCst);
            started_tx.send(()).expect("started signal");
            super::active_job_for_shutdown(&inner)
                .expect("active-job lock")
                .is_some()
        });

        started_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("shutdown started");
        assert!(matches!(
            manager.inner.active_job.try_lock(),
            Err(std::sync::TryLockError::WouldBlock)
        ));
        *slot = Some(Arc::clone(&job));
        drop(slot);

        assert!(shutdown.join().expect("shutdown result sees published job"));
        assert!(job.terminate_and_wait_empty(Duration::from_secs(1)));
    }

    #[cfg(windows)]
    #[test]
    fn verifier_does_not_reserve_process_slot_after_shutdown() {
        let manager = super::CodexVerifyManager::new(None);
        assert!(manager.shutdown());
        assert!(super::reserve_start_slot(&manager.inner).is_err());
        assert!(manager.inner.active_job.lock().unwrap().is_none());
    }
}
