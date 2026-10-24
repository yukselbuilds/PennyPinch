use super::codex::CurrentConfigValue;
use super::*;
use crate::integrations::plan::{
    ClientKind, ConfigPresence, ConfigSource, ConnectionPlan, EndpointBefore, PlanStatus,
};
use crate::integrations::transaction::{
    apply_missing_endpoint, undo_owned_endpoint, ApplyResult, TransactionError,
};
use crate::integrations::transaction_fs::{
    FileSnapshot, FileToken, FileTransaction, TransactionFsError, MAX_TRANSACTION_FILE_BYTES,
};
use crate::state::ownership::{
    config_path_binding, OwnershipError, OwnershipRecord, OwnershipState,
};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

const ZERO_UUID: &str = "00000000-0000-0000-0000-000000000000";
const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ApplyConnectionRequest {
    pub schema_version: u8,
    pub request_id: String,
    pub plan_id: String,
    pub expected_config_fingerprint: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct UndoConnectionRequest {
    pub schema_version: u8,
    pub request_id: String,
    pub client_kind: String,
    pub owner_id: String,
    pub expected_revision: u64,
    pub expected_record_fingerprint: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct OwnershipSnapshotRequest {
    pub schema_version: u8,
    pub request_id: String,
    pub client_kind: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(untagged)]
pub(crate) enum ConnectionActionReply {
    Success(ConnectionActionSuccess),
    Error(ConnectionActionError),
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct ConnectionActionSuccess {
    pub schema_version: u8,
    pub request_id: String,
    pub status: &'static str,
    pub client_kind: &'static str,
    pub ownership: Option<OwnershipSummary>,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct OwnershipSummary {
    pub state: &'static str,
    pub owner_id: String,
    pub revision: u64,
    pub record_fingerprint: String,
    pub config_source: &'static str,
    pub endpoint_id: String,
    pub endpoint_port: u16,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct ConnectionActionError {
    pub schema_version: u8,
    pub request_id: String,
    pub status: &'static str,
    pub error: ConnectionActionErrorDetail,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct ConnectionActionErrorDetail {
    pub code: &'static str,
    pub retryable: bool,
    pub message_key: &'static str,
    pub diagnostic_id: Option<String>,
}

impl PlanManager {
    pub(crate) fn apply_connection(
        &self,
        request: ApplyConnectionRequest,
    ) -> ConnectionActionReply {
        apply_connection(self, request)
    }

    pub(crate) fn undo_connection(&self, request: UndoConnectionRequest) -> ConnectionActionReply {
        undo_connection(self, request)
    }

    pub(crate) fn ownership_snapshot(
        &self,
        request: OwnershipSnapshotRequest,
    ) -> ConnectionActionReply {
        ownership_snapshot(self, request)
    }

    pub(super) fn codex_config_observation(
        &self,
        expected_installation: &InstallationSummary,
        scan_generation: u64,
    ) -> (Result<Option<OwnershipSummary>, ()>, CurrentConfigValue) {
        codex_config_observation(self, expected_installation, scan_generation)
    }

    pub(crate) fn action_failure_reply(raw_request_id: String) -> ConnectionActionReply {
        let safe_id = request_id(&raw_request_id);
        error_reply(safe_id, ActionFault::Unavailable)
    }
}

fn codex_config_observation(
    manager: &PlanManager,
    expected_installation: &InstallationSummary,
    scan_generation: u64,
) -> (Result<Option<OwnershipSummary>, ()>, CurrentConfigValue) {
    let unavailable = || (Err(()), CurrentConfigValue::Unavailable);
    let Some(_guard) = PreviewGuard::try_acquire(&manager.preview_in_flight) else {
        return unavailable();
    };
    let Some(store) = manager.ownership_store.as_ref() else {
        return unavailable();
    };
    if !current_codex_selection(manager, expected_installation, scan_generation) {
        return unavailable();
    }

    let mut transaction = match FileTransaction::begin() {
        Ok(value) => value,
        Err(_) => return unavailable(),
    };
    let snapshot = match store.read_snapshot_in_transaction(&mut transaction, ClientKind::Codex) {
        Ok(value) => value,
        Err(_) => {
            let _ = transaction.rollback();
            return unavailable();
        }
    };
    let Some(snapshot) = snapshot else {
        if transaction.rollback().is_err()
            || !current_codex_selection(manager, expected_installation, scan_generation)
        {
            return unavailable();
        }
        return (Ok(None), CurrentConfigValue::NotOwned);
    };
    let owner = match summary(&snapshot.record, snapshot.record_fingerprint) {
        Ok(value) => value,
        Err(_) => {
            let _ = transaction.rollback();
            return unavailable();
        }
    };
    if snapshot.record.state() != OwnershipState::Active {
        if transaction.rollback().is_err()
            || !current_codex_selection(manager, expected_installation, scan_generation)
        {
            return unavailable();
        }
        return (Ok(Some(owner)), CurrentConfigValue::NotOwned);
    }

    let supported_cli = expected_installation.surface == "cli"
        && expected_installation
            .version
            .as_deref()
            .is_some_and(|version| {
                super::super::plan::supported_version(ClientKind::Codex, version)
            });
    if !supported_cli {
        if transaction.rollback().is_err()
            || !current_codex_selection(manager, expected_installation, scan_generation)
        {
            return unavailable();
        }
        return (Ok(Some(owner)), CurrentConfigValue::NotRechecked);
    }

    if snapshot.record.client_kind() != Some(ClientKind::Codex)
        || snapshot.record.config_source() != Some(ConfigSource::CodexToml)
    {
        let _ = transaction.rollback();
        return (Ok(Some(owner)), CurrentConfigValue::Unavailable);
    }

    let roots = (manager.roots_provider)();
    let path = match target_for_plan(&roots, ClientKind::Codex, ConfigSource::CodexToml) {
        Ok((path, None)) => path,
        _ => {
            let _ = transaction.rollback();
            return (Ok(Some(owner)), CurrentConfigValue::Unavailable);
        }
    };
    let binding = match config_path_binding(&path) {
        Ok(value) => value,
        Err(_) => {
            let _ = transaction.rollback();
            return (Ok(Some(owner)), CurrentConfigValue::Unavailable);
        }
    };
    if snapshot.record.config_path_binding_sha256() != binding {
        let _ = transaction.rollback();
        return (Ok(Some(owner)), CurrentConfigValue::Unavailable);
    }

    let config = match super::read_codex_config_pinned(&roots) {
        Ok(value) => value,
        Err(_) => {
            let _ = transaction.rollback();
            return (Ok(Some(owner)), CurrentConfigValue::Unavailable);
        }
    };
    let mut current_value =
        classify_codex_config(&config.bytes, config.presence, owner.endpoint_port);
    let rollback = transaction.rollback();
    #[cfg(test)]
    if rollback.is_ok() {
        if let Some(hook) = &manager.after_codex_config_rollback {
            hook();
        }
    }
    if rollback.is_err() {
        current_value = CurrentConfigValue::Unavailable;
    } else if config.validate().is_err()
        || !same_target_roots(
            &roots,
            &(manager.roots_provider)(),
            ClientKind::Codex,
            ConfigSource::CodexToml,
        )
        || !current_codex_selection(manager, expected_installation, scan_generation)
    {
        current_value = CurrentConfigValue::Unavailable;
    }
    (Ok(Some(owner)), current_value)
}

fn current_codex_selection(
    manager: &PlanManager,
    expected: &InstallationSummary,
    scan_generation: u64,
) -> bool {
    manager
        .discovery
        .select_installation(&expected.installation_id, scan_generation)
        .is_ok_and(|selected| same_installation(expected, &Ok(selected)))
}

fn classify_codex_config(
    bytes: &[u8],
    presence: ConfigPresence,
    endpoint_port: u16,
) -> CurrentConfigValue {
    if presence == ConfigPresence::Missing {
        return CurrentConfigValue::Missing;
    }
    let Ok(document) = super::super::config_document::parse_document(ClientKind::Codex, bytes)
    else {
        return CurrentConfigValue::Unavailable;
    };
    let Some(root) = document.as_object() else {
        return CurrentConfigValue::Unavailable;
    };
    if !super::super::plan::codex_route_supported(root) {
        return CurrentConfigValue::UnsupportedRoute;
    }
    match document.get("openai_base_url") {
        None => CurrentConfigValue::Missing,
        Some(serde_json::Value::String(value)) => {
            let expected = format!("http://127.0.0.1:{endpoint_port}/v1");
            if value == &expected {
                CurrentConfigValue::MatchesOwnedEndpoint
            } else {
                CurrentConfigValue::Different
            }
        }
        Some(_) => CurrentConfigValue::Unavailable,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ActionFault {
    Validation,
    UnsupportedSchema,
    Busy,
    NotFound,
    Expired,
    Conflict,
    InvalidConfig,
    UnsafePath,
    Unavailable,
    Unsupported,
    CommitUnknown,
}

impl ActionFault {
    fn detail(self) -> ConnectionActionErrorDetail {
        let (code, retryable, message_key) = match self {
            Self::Validation => ("validation_failed", false, "connection.validation_failed"),
            Self::UnsupportedSchema => {
                ("unsupported_schema", false, "connection.unsupported_schema")
            }
            Self::Busy => ("busy", true, "connection.busy"),
            Self::NotFound => ("not_found", false, "connection.not_found"),
            Self::Expired => ("plan_expired", true, "connection.plan_expired"),
            Self::Conflict => ("config_conflict", true, "connection.config_conflict"),
            Self::InvalidConfig => ("config_invalid", false, "connection.config_invalid"),
            Self::UnsafePath => ("config_unsafe", false, "connection.config_unsafe"),
            Self::Unavailable => ("config_unavailable", true, "connection.config_unavailable"),
            Self::Unsupported => (
                "unsupported_filesystem",
                false,
                "connection.unsupported_filesystem",
            ),
            Self::CommitUnknown => ("commit_unknown", true, "connection.commit_unknown"),
        };
        ConnectionActionErrorDetail {
            code,
            retryable,
            message_key,
            diagnostic_id: None,
        }
    }
}

fn error_reply(request_id: String, fault: ActionFault) -> ConnectionActionReply {
    ConnectionActionReply::Error(ConnectionActionError {
        schema_version: 1,
        request_id,
        status: "error",
        error: fault.detail(),
    })
}

fn success_reply(
    request_id: String,
    status: &'static str,
    kind: ClientKind,
    ownership: Option<OwnershipSummary>,
) -> ConnectionActionReply {
    ConnectionActionReply::Success(ConnectionActionSuccess {
        schema_version: 1,
        request_id,
        status,
        client_kind: client_kind_code(kind),
        ownership,
    })
}

fn request_id(value: &str) -> String {
    if valid_uuid(value) {
        value.to_owned()
    } else {
        ZERO_UUID.to_owned()
    }
}

fn valid_uuid(value: &str) -> bool {
    value.len() == 36 && Uuid::parse_str(value).is_ok_and(|parsed| parsed.to_string() == value)
}

fn valid_hash(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn parse_client_kind(value: &str) -> Option<ClientKind> {
    match value {
        "codex" => Some(ClientKind::Codex),
        "claude_code" => Some(ClientKind::ClaudeCode),
        "opencode_v1" | "opencode" => Some(ClientKind::OpenCodeV1),
        _ => None,
    }
}

fn client_kind_code(kind: ClientKind) -> &'static str {
    match kind {
        ClientKind::Codex => "codex",
        ClientKind::ClaudeCode => "claude_code",
        ClientKind::OpenCodeV1 => "opencode_v1",
    }
}

fn config_source_code(source: ConfigSource) -> &'static str {
    match source {
        ConfigSource::CodexToml => "codex_toml",
        ConfigSource::ClaudeJson => "claude_json",
        ConfigSource::OpenCodeJson => "opencode_json",
        ConfigSource::OpenCodeJsonc => "opencode_jsonc",
        ConfigSource::OpenCodeMissing => "opencode_missing",
    }
}

fn summary(record: &OwnershipRecord, fingerprint: String) -> Result<OwnershipSummary, ActionFault> {
    let source = record.config_source().ok_or(ActionFault::Unavailable)?;
    Ok(OwnershipSummary {
        state: match record.state() {
            OwnershipState::Active => "active",
            OwnershipState::Released => "released",
        },
        owner_id: record.owner_id().to_owned(),
        revision: record.revision(),
        record_fingerprint: fingerprint,
        config_source: config_source_code(source),
        endpoint_id: record.endpoint_id().to_owned(),
        endpoint_port: record.endpoint_port(),
    })
}

fn map_transaction(error: TransactionFsError) -> ActionFault {
    match error {
        TransactionFsError::Conflict => ActionFault::Conflict,
        TransactionFsError::UnsafePath | TransactionFsError::UnsafeFile => ActionFault::UnsafePath,
        TransactionFsError::TooLarge | TransactionFsError::TooManyFiles => {
            ActionFault::InvalidConfig
        }
        TransactionFsError::Unsupported => ActionFault::Unsupported,
        TransactionFsError::CommitFailed | TransactionFsError::RollbackFailed => {
            ActionFault::CommitUnknown
        }
        TransactionFsError::Expired | TransactionFsError::Io => ActionFault::Unavailable,
    }
}

fn map_ownership(error: OwnershipError) -> ActionFault {
    match error {
        OwnershipError::Invalid => ActionFault::InvalidConfig,
        OwnershipError::Conflict => ActionFault::Conflict,
        OwnershipError::UnsafePath => ActionFault::UnsafePath,
        OwnershipError::TooLarge => ActionFault::InvalidConfig,
        OwnershipError::Unsupported => ActionFault::Unsupported,
        OwnershipError::Unavailable => ActionFault::Unavailable,
    }
}

fn map_transaction_error(error: TransactionError) -> ActionFault {
    match error {
        TransactionError::InvalidPlan => ActionFault::Validation,
        TransactionError::InvalidConfig => ActionFault::InvalidConfig,
        TransactionError::InvalidReceipt => ActionFault::InvalidConfig,
        TransactionError::Conflict => ActionFault::Conflict,
    }
}

fn map_config_io_error(error: ConfigIoError) -> ActionFault {
    match error {
        ConfigIoError::InvalidRoot | ConfigIoError::UnsafePath => ActionFault::UnsafePath,
        ConfigIoError::Unavailable => ActionFault::Unavailable,
        ConfigIoError::TooLarge => ActionFault::InvalidConfig,
        ConfigIoError::Ambiguous => ActionFault::Conflict,
    }
}

fn config_kind(plan: &ConnectionPlan) -> Result<ClientKind, ActionFault> {
    parse_client_kind(plan.client_kind).ok_or(ActionFault::Validation)
}

fn apply_connection(
    manager: &PlanManager,
    request: ApplyConnectionRequest,
) -> ConnectionActionReply {
    let correlation = request_id(&request.request_id);
    if !valid_uuid(&request.request_id) {
        return error_reply(correlation, ActionFault::Validation);
    }
    if request.schema_version != 1 {
        return error_reply(correlation, ActionFault::UnsupportedSchema);
    }
    if !valid_uuid(&request.plan_id) || !valid_hash(&request.expected_config_fingerprint) {
        return error_reply(correlation, ActionFault::Validation);
    }
    let Some(_guard) = PreviewGuard::try_acquire(&manager.preview_in_flight) else {
        return error_reply(correlation, ActionFault::Busy);
    };
    let lease = match manager.leases.lock() {
        Ok(mut leases) => leases.remove(&request.plan_id),
        Err(_) => return error_reply(correlation, ActionFault::Unavailable),
    };
    let Some(lease) = lease else {
        return error_reply(correlation, ActionFault::NotFound);
    };
    if lease.expires <= Instant::now()
        || lease.plan.expires_at_unix_seconds <= unix_seconds().unwrap_or(u64::MAX)
    {
        return error_reply(correlation, ActionFault::Expired);
    }
    if lease.plan.config_fingerprint != request.expected_config_fingerprint {
        return error_reply(correlation, ActionFault::Conflict);
    }
    if !applicable_plan(&lease.plan, &lease.installation, &lease.endpoint) {
        return error_reply(correlation, ActionFault::Conflict);
    }
    let Some(store) = manager.ownership_store.as_ref() else {
        return error_reply(correlation, ActionFault::Unavailable);
    };
    let kind = match config_kind(&lease.plan) {
        Ok(value) => value,
        Err(error) => return error_reply(correlation, error),
    };
    let roots = (manager.roots_provider)();
    let (target_path, alternate_path) =
        match target_for_plan(&roots, kind, lease.plan.config_source) {
            Ok(value) => value,
            Err(error) => return error_reply(correlation, error),
        };
    let (leased_target, leased_alternate) =
        match target_for_plan(&lease.roots, kind, lease.plan.config_source) {
            Ok(value) => value,
            Err(error) => return error_reply(correlation, error),
        };
    if !same_path(&target_path, &leased_target)
        || !same_optional_path(alternate_path.as_deref(), leased_alternate.as_deref())
        || validate_live_plan(manager, &lease).is_err()
    {
        return error_reply(correlation, ActionFault::Conflict);
    }

    let mut transaction = match FileTransaction::begin() {
        Ok(value) => value,
        Err(error) => return error_reply(correlation, map_transaction(error)),
    };
    if let Some(alternate) = &alternate_path {
        if let Err(error) = transaction.reserve_absent(alternate) {
            return error_reply(correlation, map_transaction(error));
        }
    }
    let (config_token, before, identity, fingerprint_path) =
        match open_config_for_apply(&mut transaction, &target_path, &lease.plan) {
            Ok(value) => value,
            Err(error) => return error_reply(correlation, error),
        };
    let identity = identity.map(|value| {
        (
            value.volume_serial,
            (value.file_index >> 32) as u32,
            value.file_index as u32,
        )
    });
    let current_fingerprint = crate::integrations::plan::fingerprint_config(
        kind,
        lease.plan.config_source,
        lease.plan.config_presence,
        &before,
        identity,
        Some(&fingerprint_path),
    );
    if current_fingerprint != lease.plan.config_fingerprint {
        return error_reply(correlation, ActionFault::Conflict);
    }
    let binding_path = fingerprint_path.as_path();
    let binding = match config_path_binding(binding_path) {
        Ok(value) => value,
        Err(error) => return error_reply(correlation, map_ownership(error)),
    };
    let ownership = match store.read_in_transaction(&mut transaction, kind, &binding) {
        Ok(value) => value,
        Err(error) => return error_reply(correlation, map_ownership(error)),
    };
    let patch = match apply_missing_endpoint(&lease.plan, &before) {
        Ok(value) => value,
        Err(error) => return error_reply(correlation, map_transaction_error(error)),
    };
    if let ApplyResult::Unchanged = patch {
        if let Err(error) = transaction.rollback() {
            return error_reply(correlation, map_transaction(error));
        }
        if let Err(error) = validate_live_plan(manager, &lease) {
            return error_reply(correlation, error);
        }
        return unchanged_apply(correlation, kind, ownership, &lease.plan);
    }
    if ownership
        .current()
        .is_some_and(|record| record.state() == OwnershipState::Active)
    {
        return error_reply(correlation, ActionFault::Conflict);
    }
    if let Err(error) = validate_live_plan(manager, &lease) {
        return error_reply(correlation, error);
    }

    if let Err(error) = store.prepare_for_apply() {
        return error_reply(correlation, map_ownership(error));
    }
    let ApplyResult::Patched {
        bytes: after,
        receipt,
    } = patch
    else {
        return error_reply(correlation, ActionFault::Conflict);
    };
    if let Err(error) = transaction.replace_contents(config_token, &after) {
        return error_reply(correlation, map_transaction(error));
    }
    let now = match unix_seconds() {
        Some(value) => value,
        None => return error_reply(correlation, ActionFault::Unavailable),
    };
    let revision = match ownership.current() {
        Some(current) if current.state() == OwnershipState::Released => {
            match current
                .revision()
                .checked_add(1)
                .filter(|value| *value <= MAX_SAFE_INTEGER)
            {
                Some(value) => value,
                None => return error_reply(correlation, ActionFault::Conflict),
            }
        }
        Some(_) => return error_reply(correlation, ActionFault::Conflict),
        None => 1,
    };
    let owner_id = Uuid::new_v4().to_string();
    let record = match OwnershipRecord::active_from_patch(
        &owner_id,
        &lease.plan.endpoint_id,
        binding_path,
        revision,
        now,
        &before,
        &after,
        &receipt,
    ) {
        Ok(value) => value,
        Err(error) => return error_reply(correlation, map_ownership(error)),
    };
    let fingerprint = match record.encoded_fingerprint() {
        Ok(value) => value,
        Err(error) => return error_reply(correlation, map_ownership(error)),
    };
    let ownership_summary = match summary(&record, fingerprint) {
        Ok(value) => Some(value),
        Err(error) => return error_reply(correlation, error),
    };
    if let Err(error) = store.stage_in_transaction(&mut transaction, ownership, &record) {
        return error_reply(correlation, map_ownership(error));
    }
    if let Err(error) = validate_live_plan(manager, &lease) {
        return error_reply(correlation, error);
    }
    #[cfg(test)]
    if manager.fail_action_before_commit {
        return error_reply(correlation, ActionFault::Unavailable);
    }
    if let Err(error) = transaction.seal() {
        return error_reply(correlation, map_transaction(error));
    }
    if let Err(error) = validate_live_plan(manager, &lease) {
        return error_reply(correlation, error);
    }
    if let Err(error) = transaction.commit() {
        return error_reply(correlation, map_transaction(error));
    }
    success_reply(correlation, "applied", kind, ownership_summary)
}

fn undo_connection(manager: &PlanManager, request: UndoConnectionRequest) -> ConnectionActionReply {
    let correlation = request_id(&request.request_id);
    let Some(kind) = parse_client_kind(&request.client_kind) else {
        return error_reply(correlation, ActionFault::Validation);
    };
    if !valid_uuid(&request.request_id) {
        return error_reply(correlation, ActionFault::Validation);
    }
    if request.schema_version != 1 {
        return error_reply(correlation, ActionFault::UnsupportedSchema);
    }
    if client_kind_code(kind) != request.client_kind
        || !valid_uuid(&request.owner_id)
        || request.expected_revision == 0
        || request.expected_revision > MAX_SAFE_INTEGER
        || !valid_hash(&request.expected_record_fingerprint)
    {
        return error_reply(correlation, ActionFault::Validation);
    }
    let Some(_guard) = PreviewGuard::try_acquire(&manager.preview_in_flight) else {
        return error_reply(correlation, ActionFault::Busy);
    };
    let Some(store) = manager.ownership_store.as_ref() else {
        return error_reply(correlation, ActionFault::Unavailable);
    };
    let roots = (manager.roots_provider)();
    let first_config = match read_user_config(kind, &roots) {
        Ok(value) if value.presence == ConfigPresence::Present => value,
        Ok(_) => return error_reply(correlation, ActionFault::Conflict),
        Err(error) => return error_reply(correlation, map_config_io_error(error)),
    };
    let (target_path, alternate_path) = match target_for_plan(&roots, kind, first_config.source) {
        Ok(value) => value,
        Err(error) => return error_reply(correlation, error),
    };
    let mut transaction = match FileTransaction::begin() {
        Ok(value) => value,
        Err(error) => return error_reply(correlation, map_transaction(error)),
    };
    if let Some(alternate) = &alternate_path {
        if let Err(error) = transaction.reserve_absent(alternate) {
            return error_reply(correlation, map_transaction(error));
        }
    }
    let (config_token, snapshot) =
        match transaction.open_existing(&target_path, MAX_TRANSACTION_FILE_BYTES) {
            Ok(value) => value,
            Err(error) => return error_reply(correlation, map_transaction(error)),
        };
    let canonical_target = match transaction.canonical_target(config_token) {
        Ok(value) => value,
        Err(error) => return error_reply(correlation, map_transaction(error)),
    };
    let identity = Some((
        snapshot.identity.volume_serial,
        (snapshot.identity.file_index >> 32) as u32,
        snapshot.identity.file_index as u32,
    ));
    let current_fingerprint = crate::integrations::plan::fingerprint_config(
        kind,
        first_config.source,
        ConfigPresence::Present,
        &snapshot.bytes,
        identity,
        Some(&canonical_target),
    );
    if current_fingerprint != first_config.fingerprint {
        return error_reply(correlation, ActionFault::Conflict);
    }
    let binding = match config_path_binding(&canonical_target) {
        Ok(value) => value,
        Err(error) => return error_reply(correlation, map_ownership(error)),
    };
    let ownership = match store.read_in_transaction(&mut transaction, kind, &binding) {
        Ok(value) => value,
        Err(error) => return error_reply(correlation, map_ownership(error)),
    };
    let Some(record) = ownership.current() else {
        return error_reply(correlation, ActionFault::NotFound);
    };
    if record.state() != OwnershipState::Active
        || record.owner_id() != request.owner_id
        || record.revision() != request.expected_revision
        || ownership.record_fingerprint() != Some(request.expected_record_fingerprint.as_str())
        || record.client_kind() != Some(kind)
        || record.config_source() != Some(first_config.source)
    {
        return error_reply(correlation, ActionFault::Conflict);
    }
    let receipt = match record.patch_receipt() {
        Ok(value) => value,
        Err(error) => return error_reply(correlation, map_ownership(error)),
    };
    let after = match undo_owned_endpoint(&receipt, &snapshot.bytes) {
        Ok(value) => value,
        Err(error) => return error_reply(correlation, map_transaction_error(error)),
    };
    if let Err(error) = transaction.replace_contents(config_token, &after) {
        return error_reply(correlation, map_transaction(error));
    }
    let now = match unix_seconds() {
        Some(value) => value,
        None => return error_reply(correlation, ActionFault::Unavailable),
    };
    let released = match record.released(now) {
        Ok(value) => value,
        Err(error) => return error_reply(correlation, map_ownership(error)),
    };
    let fingerprint = match released.encoded_fingerprint() {
        Ok(value) => value,
        Err(error) => return error_reply(correlation, map_ownership(error)),
    };
    let ownership_summary = match summary(&released, fingerprint) {
        Ok(value) => Some(value),
        Err(error) => return error_reply(correlation, error),
    };
    if let Err(error) = store.stage_in_transaction(&mut transaction, ownership, &released) {
        return error_reply(correlation, map_ownership(error));
    }
    if !same_target_roots(
        &roots,
        &(manager.roots_provider)(),
        kind,
        first_config.source,
    ) {
        return error_reply(correlation, ActionFault::Conflict);
    }
    #[cfg(test)]
    if manager.fail_undo_before_commit {
        return error_reply(correlation, ActionFault::Unavailable);
    }
    if let Err(error) = transaction.seal() {
        return error_reply(correlation, map_transaction(error));
    }
    if !same_target_roots(
        &roots,
        &(manager.roots_provider)(),
        kind,
        first_config.source,
    ) {
        return error_reply(correlation, ActionFault::Conflict);
    }
    if let Err(error) = transaction.commit() {
        return error_reply(correlation, map_transaction(error));
    }
    success_reply(correlation, "released", kind, ownership_summary)
}

fn ownership_snapshot(
    manager: &PlanManager,
    request: OwnershipSnapshotRequest,
) -> ConnectionActionReply {
    let correlation = request_id(&request.request_id);
    let Some(kind) = parse_client_kind(&request.client_kind) else {
        return error_reply(correlation, ActionFault::Validation);
    };
    if !valid_uuid(&request.request_id) {
        return error_reply(correlation, ActionFault::Validation);
    }
    if request.schema_version != 1 {
        return error_reply(correlation, ActionFault::UnsupportedSchema);
    }
    if client_kind_code(kind) != request.client_kind {
        return error_reply(correlation, ActionFault::Validation);
    }
    let Some(_guard) = PreviewGuard::try_acquire(&manager.preview_in_flight) else {
        return error_reply(correlation, ActionFault::Busy);
    };
    let Some(store) = manager.ownership_store.as_ref() else {
        return error_reply(correlation, ActionFault::Unavailable);
    };
    let mut transaction = match FileTransaction::begin() {
        Ok(value) => value,
        Err(error) => return error_reply(correlation, map_transaction(error)),
    };
    let snapshot = match store.read_snapshot_in_transaction(&mut transaction, kind) {
        Ok(value) => value,
        Err(error) => return error_reply(correlation, map_ownership(error)),
    };
    if let Err(error) = transaction.rollback() {
        return error_reply(correlation, map_transaction(error));
    }
    let ownership = match snapshot {
        Some(value) => match summary(&value.record, value.record_fingerprint) {
            Ok(value) => Some(value),
            Err(error) => return error_reply(correlation, error),
        },
        None => None,
    };
    success_reply(correlation, "snapshot", kind, ownership)
}

fn applicable_plan(
    plan: &ConnectionPlan,
    installation: &InstallationSummary,
    endpoint: &ReadyStableEndpoint,
) -> bool {
    let Some(kind) = parse_client_kind(plan.client_kind) else {
        return false;
    };
    let supported_version = installation
        .version
        .as_deref()
        .is_some_and(|version| super::super::plan::supported_version(kind, version));
    plan.schema_version == 1
        && valid_uuid(&plan.plan_id)
        && valid_uuid(&plan.installation_id)
        && valid_uuid(&plan.endpoint_id)
        && plan.installation_id == installation.installation_id
        && plan.client_kind == client_kind_code(kind)
        && parse_client_kind(installation.client_kind) == Some(kind)
        && plan.scope == "user_config"
        && plan.scan_generation > 0
        && plan.scan_generation <= MAX_SAFE_INTEGER
        && plan.status == PlanStatus::Preview
        && plan.blockers.is_empty()
        && plan.restart_required
        && plan.runtime_effective == super::super::plan::DeclarationState::Unknown
        && plan.endpoint_field.after_headroom
        && matches!(
            plan.endpoint_field.before,
            EndpointBefore::Missing | EndpointBefore::SameStableEndpoint
        )
        && plan.endpoint_host == endpoint.host
        && plan.endpoint_port == endpoint.port
        && plan.endpoint_id == endpoint.endpoint_id
        && endpoint.host == "127.0.0.1"
        && endpoint.port != 0
        && installation.surface == "cli"
        && supported_version
        && installation.status == "detected"
}

fn validate_live_plan(manager: &PlanManager, lease: &PlanLease) -> Result<(), ActionFault> {
    let plan = &lease.plan;
    if lease.expires <= Instant::now()
        || plan.expires_at_unix_seconds <= unix_seconds().unwrap_or(u64::MAX)
    {
        return Err(ActionFault::Expired);
    }
    let selected = manager
        .discovery
        .select_installation(&plan.installation_id, plan.scan_generation)
        .map_err(|_| ActionFault::Conflict)?;
    if !same_installation(&lease.installation, &Ok(selected)) {
        return Err(ActionFault::Conflict);
    }
    let endpoint = (manager.endpoint_provider)().map_err(|_| ActionFault::Conflict)?;
    if !same_endpoint(&lease.endpoint, &Ok(endpoint.clone()))
        || !same_endpoint(&endpoint, &Ok(lease.endpoint.clone()))
    {
        return Err(ActionFault::Conflict);
    }
    let kind = config_kind(plan)?;
    let current_roots = (manager.roots_provider)();
    if !same_target_roots(&lease.roots, &current_roots, kind, plan.config_source) {
        return Err(ActionFault::Conflict);
    }
    Ok(())
}

fn target_for_plan(
    roots: &ConfigRootInputs,
    kind: ClientKind,
    source: ConfigSource,
) -> Result<(PathBuf, Option<PathBuf>), ActionFault> {
    match (roots.target(kind).map_err(map_config_io_error)?, source) {
        (
            ConfigTarget::Single {
                path,
                source: actual,
            },
            expected,
        ) if actual == expected => Ok((path, None)),
        (ConfigTarget::OpenCode { json, jsonc }, ConfigSource::OpenCodeJson) => {
            Ok((json, Some(jsonc)))
        }
        (ConfigTarget::OpenCode { json, jsonc }, ConfigSource::OpenCodeJsonc) => {
            Ok((jsonc, Some(json)))
        }
        (ConfigTarget::OpenCode { json, jsonc }, ConfigSource::OpenCodeMissing) => {
            Ok((json, Some(jsonc)))
        }
        _ => Err(ActionFault::Conflict),
    }
}

fn same_target_roots(
    expected: &ConfigRootInputs,
    actual: &ConfigRootInputs,
    kind: ClientKind,
    source: ConfigSource,
) -> bool {
    let (Ok((expected_path, expected_alt)), Ok((actual_path, actual_alt))) = (
        target_for_plan(expected, kind, source),
        target_for_plan(actual, kind, source),
    ) else {
        return false;
    };
    same_path(&expected_path, &actual_path)
        && same_optional_path(expected_alt.as_deref(), actual_alt.as_deref())
}

fn same_optional_path(left: Option<&Path>, right: Option<&Path>) -> bool {
    match (left, right) {
        (None, None) => true,
        (Some(left), Some(right)) => same_path(left, right),
        _ => false,
    }
}

fn open_config_for_apply(
    transaction: &mut FileTransaction,
    path: &Path,
    plan: &ConnectionPlan,
) -> Result<
    (
        FileToken,
        Vec<u8>,
        Option<crate::integrations::transaction_fs::FileIdentity>,
        PathBuf,
    ),
    ActionFault,
> {
    match plan.config_presence {
        ConfigPresence::Present => {
            let (token, FileSnapshot { bytes, identity }) = transaction
                .open_existing(path, MAX_TRANSACTION_FILE_BYTES)
                .map_err(map_transaction)?;
            let canonical = transaction
                .canonical_target(token)
                .map_err(map_transaction)?;
            Ok((token, bytes, Some(identity), canonical))
        }
        ConfigPresence::Missing => {
            let (token, _) = transaction
                .create_missing(path, MAX_TRANSACTION_FILE_BYTES)
                .map_err(map_transaction)?;
            Ok((
                token,
                empty_document(config_kind(plan)?),
                None,
                path.to_path_buf(),
            ))
        }
    }
}

fn unchanged_apply(
    request_id: String,
    kind: ClientKind,
    ownership: crate::state::ownership::OwnershipRead,
    plan: &ConnectionPlan,
) -> ConnectionActionReply {
    let record = ownership.current();
    match record {
        Some(record)
            if record.state() == OwnershipState::Active
                && record.endpoint_id() == plan.endpoint_id
                && record.endpoint_port() == plan.endpoint_port =>
        {
            if record.config_source() != Some(plan.config_source) {
                return error_reply(request_id, ActionFault::Conflict);
            }
            let fingerprint = ownership
                .record_fingerprint()
                .unwrap_or_default()
                .to_owned();
            match summary(record, fingerprint) {
                Ok(summary) => success_reply(request_id, "unchanged", kind, Some(summary)),
                Err(error) => error_reply(request_id, error),
            }
        }
        Some(record) if record.state() == OwnershipState::Active => {
            error_reply(request_id, ActionFault::Conflict)
        }
        Some(_) | None => success_reply(request_id, "unchanged", kind, None),
    }
}

fn unix_seconds() -> Option<u64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|value| value.as_secs())
        .filter(|value| *value <= MAX_SAFE_INTEGER)
}

#[cfg(test)]
mod action_fault_tests {
    use super::ActionFault;
    use std::collections::HashSet;

    #[test]
    fn fixed_action_error_codes_are_unique_and_have_one_message_key() {
        let faults = [
            ActionFault::Validation,
            ActionFault::UnsupportedSchema,
            ActionFault::Busy,
            ActionFault::NotFound,
            ActionFault::Expired,
            ActionFault::Conflict,
            ActionFault::InvalidConfig,
            ActionFault::UnsafePath,
            ActionFault::Unavailable,
            ActionFault::Unsupported,
            ActionFault::CommitUnknown,
        ];
        let mut codes = HashSet::new();
        for fault in faults {
            let detail = fault.detail();
            assert!(codes.insert(detail.code));
            assert_eq!(detail.message_key, format!("connection.{}", detail.code));
        }
    }
}
