use super::super::discovery::{InstallationSelectionError, InstallationSummary};
use super::super::plan::CODEX_FIXTURE_CLI_VERSIONS;
use super::connection_actions::OwnershipSummary;
use super::PlanManager;
use crate::runtime::manager::{CodexRuntimeObservation, RuntimeManager};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

const ZERO_UUID: &str = "00000000-0000-0000-0000-000000000000";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum CurrentConfigValue {
    MatchesOwnedEndpoint,
    Missing,
    Different,
    UnsupportedRoute,
    Unavailable,
    NotOwned,
    NotRechecked,
}

impl CurrentConfigValue {
    fn as_str(self) -> &'static str {
        match self {
            Self::MatchesOwnedEndpoint => "matches_owned_endpoint",
            Self::Missing => "missing",
            Self::Different => "different",
            Self::UnsupportedRoute => "unsupported_route",
            Self::Unavailable => "unavailable",
            Self::NotOwned => "not_owned",
            Self::NotRechecked => "not_rechecked",
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CodexStatusRequest {
    pub schema_version: u8,
    pub request_id: String,
    pub installation_id: String,
    pub scan_generation: u64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(untagged)]
pub(crate) enum CodexStatusReply {
    Snapshot(CodexStatusSnapshot),
    Error(CodexStatusError),
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct CodexStatusSnapshot {
    pub schema_version: u8,
    pub request_id: String,
    pub status: &'static str,
    pub installation: CodexInstallationStatus,
    pub config_scope: &'static str,
    pub ownership: CodexOwnershipStatus,
    pub runtime: CodexRuntimeStatus,
    pub policy: CodexPolicyStatus,
    pub auth_scope: CodexAuthScope,
    pub coverage: CodexCoverage,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct CodexInstallationStatus {
    pub installation_id: String,
    pub surface: &'static str,
    pub version: Option<String>,
    pub compatibility: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct CodexOwnershipStatus {
    pub state: &'static str,
    pub endpoint_binding: &'static str,
    pub config_source: Option<&'static str>,
    pub endpoint_id: Option<String>,
    pub endpoint_port: Option<u16>,
    pub current_config_value: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct CodexRuntimeStatus {
    pub ingress_status: &'static str,
    pub stable_ingress_ready: bool,
    pub engine_status: &'static str,
    pub engine_ready: bool,
    pub runtime_generation: Option<String>,
    pub active_connection_count: Option<u64>,
    pub error_code: Option<&'static str>,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct CodexPolicyStatus {
    pub model_changed_by_headroom: bool,
    pub reasoning_changed_by_headroom: bool,
    pub auth_changed_by_headroom: bool,
    pub restart_required_after_apply: bool,
    pub runtime_effective: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct CodexAuthScope {
    pub current_method: &'static str,
    pub chatgpt_auth: &'static str,
    pub api_key_auth: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct CodexCoverage {
    pub traffic_verification: &'static str,
    pub main_agent: &'static str,
    pub subagent: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct CodexStatusError {
    pub schema_version: u8,
    pub request_id: String,
    pub status: &'static str,
    pub error: CodexStatusErrorDetail,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct CodexStatusErrorDetail {
    pub code: &'static str,
    pub retryable: bool,
    pub message_key: &'static str,
    pub diagnostic_id: Option<String>,
}

pub(crate) fn connection_status(
    manager: &PlanManager,
    runtime: &RuntimeManager,
    request: CodexStatusRequest,
) -> CodexStatusReply {
    let request_id = if valid_uuid(&request.request_id) {
        request.request_id.clone()
    } else {
        ZERO_UUID.to_owned()
    };
    if request.schema_version != 1
        || !valid_uuid(&request.request_id)
        || !valid_uuid(&request.installation_id)
        || request.scan_generation == 0
        || request.scan_generation > 9_007_199_254_740_991
    {
        return error_reply(request_id, StatusFault::Validation);
    }
    let selection = manager
        .discovery
        .select_installation(&request.installation_id, request.scan_generation);
    let snapshot_request_id = request_id.clone();
    let scan_generation = request.scan_generation;
    snapshot_for_selection(request_id, selection, move |installation| {
        let (ownership, current_config_value) =
            manager.codex_config_observation(&installation, scan_generation);
        build_snapshot(
            snapshot_request_id,
            installation,
            ownership,
            runtime.codex_connection_observation(),
            current_config_value,
        )
    })
}

pub(super) fn snapshot_for_selection<F>(
    request_id: String,
    selection: Result<InstallationSummary, InstallationSelectionError>,
    build: F,
) -> CodexStatusReply
where
    F: FnOnce(InstallationSummary) -> CodexStatusSnapshot,
{
    let installation = match selection {
        Ok(value) if value.client_kind == "codex" => value,
        Ok(_) => return error_reply(request_id, StatusFault::ClientMismatch),
        Err(InstallationSelectionError::StaleScanGeneration)
        | Err(InstallationSelectionError::NotScanned) => {
            return error_reply(request_id, StatusFault::StaleSelection)
        }
        Err(InstallationSelectionError::InstallationNotFound) => {
            return error_reply(request_id, StatusFault::NotFound)
        }
        Err(InstallationSelectionError::InvalidInstallationId)
        | Err(InstallationSelectionError::InvalidScanGeneration) => {
            return error_reply(request_id, StatusFault::Validation)
        }
    };
    CodexStatusReply::Snapshot(build(installation))
}

pub(crate) fn failure_reply(raw_request_id: String) -> CodexStatusReply {
    let request_id = if valid_uuid(&raw_request_id) {
        raw_request_id
    } else {
        ZERO_UUID.to_owned()
    };
    error_reply(request_id, StatusFault::Unavailable)
}

pub(super) fn build_snapshot(
    request_id: String,
    installation: InstallationSummary,
    ownership: Result<Option<OwnershipSummary>, ()>,
    runtime: CodexRuntimeObservation,
    current_config_value: CurrentConfigValue,
) -> CodexStatusSnapshot {
    let fixture_scope = installation.surface == "cli"
        && installation
            .version
            .as_deref()
            .is_some_and(|version| CODEX_FIXTURE_CLI_VERSIONS.contains(&version));
    let compatibility = if installation.surface == "desktop" {
        "not_tested"
    } else if fixture_scope {
        "fixture_only"
    } else {
        "not_verified"
    };
    let (
        ownership_state,
        endpoint_binding,
        owner_config_source,
        owner_endpoint_id,
        owner_endpoint_port,
    ) = match ownership {
        Ok(Some(owner)) if owner.state == "active" => {
            let endpoint_binding = match (&runtime.endpoint_id, runtime.endpoint_port) {
                (Some(endpoint_id), Some(endpoint_port))
                    if owner.endpoint_id == *endpoint_id
                        && owner.endpoint_port == endpoint_port
                        && owner.config_source == "codex_toml" =>
                {
                    "same"
                }
                (Some(endpoint_id), Some(endpoint_port))
                    if owner.endpoint_id != *endpoint_id
                        || owner.endpoint_port != endpoint_port =>
                {
                    "different"
                }
                _ => "unknown",
            };
            (
                "active",
                endpoint_binding,
                Some(owner.config_source),
                Some(owner.endpoint_id),
                Some(owner.endpoint_port),
            )
        }
        Ok(Some(owner)) if owner.state == "released" => (
            "released",
            "not_owned",
            Some(owner.config_source),
            Some(owner.endpoint_id),
            Some(owner.endpoint_port),
        ),
        Ok(Some(_)) => ("unavailable", "unknown", None, None, None),
        Ok(None) => ("none", "not_owned", None, None, None),
        Err(()) => ("unavailable", "unknown", None, None, None),
    };
    CodexStatusSnapshot {
        schema_version: 1,
        request_id,
        status: "snapshot",
        installation: CodexInstallationStatus {
            installation_id: installation.installation_id,
            surface: installation.surface,
            version: installation.version,
            compatibility,
        },
        config_scope: "user_config",
        ownership: CodexOwnershipStatus {
            state: ownership_state,
            endpoint_binding,
            config_source: owner_config_source,
            endpoint_id: owner_endpoint_id,
            endpoint_port: owner_endpoint_port,
            current_config_value: current_config_value.as_str(),
        },
        runtime: CodexRuntimeStatus {
            ingress_status: runtime.endpoint_status,
            stable_ingress_ready: runtime.endpoint_ready,
            engine_status: runtime.engine_status,
            engine_ready: runtime.engine_ready,
            runtime_generation: runtime.runtime_generation,
            active_connection_count: runtime.active_connection_count,
            error_code: runtime.error_code,
        },
        policy: CodexPolicyStatus {
            model_changed_by_headroom: false,
            reasoning_changed_by_headroom: false,
            auth_changed_by_headroom: false,
            restart_required_after_apply: true,
            runtime_effective: "unknown",
        },
        auth_scope: CodexAuthScope {
            current_method: "not_read",
            chatgpt_auth: "not_tested",
            api_key_auth: if fixture_scope {
                "fixture_only"
            } else {
                "not_tested"
            },
        },
        coverage: CodexCoverage {
            traffic_verification: "not_verified",
            main_agent: "unknown",
            subagent: "not_tested",
        },
    }
}

fn error_reply(request_id: String, fault: StatusFault) -> CodexStatusReply {
    let (code, retryable, message_key) = match fault {
        StatusFault::Validation => ("validation_failed", false, "codex.status.validation_failed"),
        StatusFault::StaleSelection => ("stale_selection", true, "codex.status.stale_selection"),
        StatusFault::NotFound => ("not_found", false, "codex.status.not_found"),
        StatusFault::ClientMismatch => ("client_mismatch", false, "codex.status.client_mismatch"),
        StatusFault::Unavailable => ("unavailable", true, "codex.status.unavailable"),
    };
    CodexStatusReply::Error(CodexStatusError {
        schema_version: 1,
        request_id,
        status: "error",
        error: CodexStatusErrorDetail {
            code,
            retryable,
            message_key,
            diagnostic_id: None,
        },
    })
}

#[derive(Clone, Copy)]
enum StatusFault {
    Validation,
    StaleSelection,
    NotFound,
    ClientMismatch,
    Unavailable,
}

fn valid_uuid(value: &str) -> bool {
    value.len() == 36 && Uuid::parse_str(value).is_ok_and(|parsed| parsed.to_string() == value)
}
