use super::{
    read_candidate, CandidatePin, CandidateRead, ConfigIoError, ConfigRead, ConfigRootInputs,
    ConfigTarget, MAX_CONFIG_BYTES,
};
use crate::integrations::plan::{
    preview_config_with_presence, AuthDeclaration, ClientKind, ConfigPresence, ConfigScope,
    ConfigSource, DeclarationState, HookShellEffect, PlanBlocker, PlanContext,
};
use serde::Serialize;
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
#[cfg(not(test))]
use std::env;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

const MAX_LAYER_BYTES: usize = MAX_CONFIG_BYTES;
const ROUTE_ENV_NAMES: &[&str] = &[
    "ANTHROPIC_BASE_URL",
    "CLAUDE_CODE_USE_BEDROCK",
    "CLAUDE_CODE_USE_VERTEX",
    "CLAUDE_CODE_USE_FOUNDRY",
    "ANTHROPIC_BEDROCK_BASE_URL",
    "ANTHROPIC_VERTEX_BASE_URL",
    "ANTHROPIC_FOUNDRY_BASE_URL",
];
#[cfg(all(windows, not(test)))]
const MANAGED_SETTINGS_PATH: &str = r"C:\Program Files\ClaudeCode\managed-settings.json";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ClaudeLayersError {
    Io(ConfigIoError),
    TooLarge,
    InvalidDocument,
    DuplicateKey,
}

impl From<ConfigIoError> for ClaudeLayersError {
    fn from(value: ConfigIoError) -> Self {
        Self::Io(value)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ClaudeLayerSource {
    UserSettings,
    SharedProjectSettings,
    LocalProjectSettings,
    LocalManagedSettings,
    ProcessEnvironment,
}

impl ClaudeLayerSource {
    fn code(self) -> &'static str {
        match self {
            Self::UserSettings => "user_settings",
            Self::SharedProjectSettings => "shared_project_settings",
            Self::LocalProjectSettings => "local_project_settings",
            Self::LocalManagedSettings => "local_managed_settings",
            Self::ProcessEnvironment => "process_environment",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ClaudeLayerAvailability {
    Present,
    Missing,
    NotObserved,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ClaudeRouteDeclaration {
    Absent,
    HeadroomEndpoint,
    OtherConfiguredRedacted,
    Unknown,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct ClaudeLayerSummary {
    pub source: ClaudeLayerSource,
    pub availability: ClaudeLayerAvailability,
    pub route: ClaudeRouteDeclaration,
    pub model: DeclarationState,
    pub reasoning: DeclarationState,
    pub auth: AuthDeclaration,
    pub hooks: HookShellEffect,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct ClaudeSettingsSnapshot {
    pub schema_version: u8,
    pub request_id: String,
    pub status: &'static str,
    pub scope: &'static str,
    pub layers: Vec<ClaudeLayerSummary>,
    pub model_declared_in_observed_files: DeclarationState,
    pub reasoning_declared_in_observed_files: DeclarationState,
    pub auth_declared_in_observed_files: AuthDeclaration,
    pub hooks_declared_in_observed_files: HookShellEffect,
    pub runtime_effective: &'static str,
    pub remote_managed_policy: &'static str,
    pub other_session_overrides: &'static str,
}

pub(super) struct ClaudeLayersObservation {
    pub fingerprint: String,
    pub route_blockers: Vec<PlanBlocker>,
    pub snapshot: ClaudeSettingsSnapshot,
    pins: Vec<CandidatePin>,
}

impl ClaudeLayersObservation {
    pub(super) fn validate_pins(&self) -> Result<(), ConfigIoError> {
        for pin in &self.pins {
            pin.validate()?;
        }
        Ok(())
    }
}

pub(super) fn observe(
    roots: &ConfigRootInputs,
    scope: ConfigScope,
    target_read: Option<&ConfigRead>,
    context: &PlanContext,
) -> Result<ClaudeLayersObservation, ClaudeLayersError> {
    let target = roots.target(ClientKind::ClaudeCode)?;
    let ConfigTarget::Single {
        path: target_path,
        source: target_source,
    } = target
    else {
        return Err(ConfigIoError::InvalidRoot.into());
    };
    if target_source != ConfigSource::ClaudeJson {
        return Err(ConfigIoError::InvalidRoot.into());
    }

    let mut summaries = Vec::with_capacity(5);
    let mut pins = Vec::with_capacity(3);
    let mut route_blockers = Vec::new();
    let mut fingerprint = Sha256::new();
    fingerprint.update(b"headroom-claude-layers/v1\0");
    fingerprint.update(scope_code(scope).as_bytes());
    fingerprint.update([0]);

    let (user_path, project_shared_path, project_local_path) = match scope {
        ConfigScope::UserConfig => (Some(user_settings_path(roots)?), None, None),
        ConfigScope::ProjectLocalConfig => {
            let project = roots
                .claude_project_directory
                .as_ref()
                .ok_or(ConfigIoError::InvalidRoot)?;
            let claude_dir = project.join(".claude");
            let metadata =
                std::fs::symlink_metadata(&claude_dir).map_err(|_| ConfigIoError::Unavailable)?;
            if !metadata.is_dir() || super::is_reparse(&metadata) {
                return Err(ConfigIoError::UnsafePath.into());
            }
            let user = user_settings_path(roots)?;
            (
                Some(user),
                Some(claude_dir.join("settings.json")),
                Some(claude_dir.join("settings.local.json")),
            )
        }
    };

    for (source, path) in [
        (ClaudeLayerSource::UserSettings, user_path),
        (
            ClaudeLayerSource::SharedProjectSettings,
            project_shared_path,
        ),
        (ClaudeLayerSource::LocalProjectSettings, project_local_path),
    ] {
        if let Some(path) = path {
            if same_path(&path, &target_path) {
                if let Some(target_read) = target_read {
                    summaries.push(summary_from_bytes(
                        source,
                        target_read.presence,
                        &target_read.bytes,
                        context,
                    )?);
                } else {
                    summaries.push(not_observed(source));
                }
                continue;
            }
            let file = read_layer_file(&path, source, context)?;
            append_layer_fingerprint(&mut fingerprint, source, &file.fingerprint);
            route_blockers.extend(file.route_blockers.iter().copied());
            summaries.push(file.summary);
            if let Some(pin) = file.pin {
                pins.push(pin);
            }
        } else {
            summaries.push(not_observed(source));
        }
    }

    if let Some(target_read) = target_read {
        let source = if scope == ConfigScope::ProjectLocalConfig {
            ClaudeLayerSource::LocalProjectSettings
        } else {
            ClaudeLayerSource::UserSettings
        };
        if !summaries.iter().any(|layer| layer.source == source) {
            summaries.push(summary_from_bytes(
                source,
                target_read.presence,
                &target_read.bytes,
                context,
            )?);
        }
    }

    let managed_path = managed_settings_path(roots);
    match managed_path {
        Some(path) => {
            let file = read_layer_file(&path, ClaudeLayerSource::LocalManagedSettings, context)?;
            append_layer_fingerprint(
                &mut fingerprint,
                ClaudeLayerSource::LocalManagedSettings,
                &file.fingerprint,
            );
            route_blockers.extend(file.route_blockers.iter().copied());
            summaries.push(file.summary);
            if let Some(pin) = file.pin {
                pins.push(pin);
            }
        }
        None => summaries.push(not_observed(ClaudeLayerSource::LocalManagedSettings)),
    }

    let environment = route_environment(roots);
    let (environment_summary, environment_fingerprint, environment_blockers) =
        summarize_environment(&environment, context)?;
    append_layer_fingerprint(
        &mut fingerprint,
        ClaudeLayerSource::ProcessEnvironment,
        &environment_fingerprint,
    );
    route_blockers.extend(environment_blockers);
    summaries.push(environment_summary);

    deduplicate_blockers(&mut route_blockers);
    let snapshot = ClaudeSettingsSnapshot {
        schema_version: 1,
        request_id: context.request_id.clone(),
        status: "snapshot",
        scope: scope_code(scope),
        model_declared_in_observed_files: aggregate_declaration(&summaries, |layer| layer.model),
        reasoning_declared_in_observed_files: aggregate_declaration(&summaries, |layer| {
            layer.reasoning
        }),
        auth_declared_in_observed_files: aggregate_auth(&summaries),
        hooks_declared_in_observed_files: aggregate_hooks(&summaries),
        layers: summaries,
        runtime_effective: "unknown",
        remote_managed_policy: "unknown",
        other_session_overrides: "unknown",
    };

    Ok(ClaudeLayersObservation {
        fingerprint: hex(&fingerprint.finalize()),
        route_blockers,
        snapshot,
        pins,
    })
}

struct ReadLayer {
    fingerprint: String,
    summary: ClaudeLayerSummary,
    route_blockers: Vec<PlanBlocker>,
    pin: Option<CandidatePin>,
}

fn read_layer_file(
    path: &Path,
    source: ClaudeLayerSource,
    context: &PlanContext,
) -> Result<ReadLayer, ClaudeLayersError> {
    match read_candidate(path)? {
        CandidateRead::Present {
            bytes,
            identity,
            target_path,
            pin,
        } => {
            if bytes.len() > MAX_LAYER_BYTES {
                return Err(ClaudeLayersError::TooLarge);
            }
            let fingerprint = super::super::plan::fingerprint_config(
                ClientKind::ClaudeCode,
                ConfigSource::ClaudeJson,
                ConfigPresence::Present,
                &bytes,
                Some((identity.volume, identity.high, identity.low)),
                Some(&target_path),
            );
            let summary = summary_from_bytes(source, ConfigPresence::Present, &bytes, context)?;
            let route_blockers = route_blockers(context, &bytes)?;
            Ok(ReadLayer {
                fingerprint,
                summary,
                route_blockers,
                pin: Some(pin),
            })
        }
        CandidateRead::Missing { target_path, pin } => Ok(ReadLayer {
            fingerprint: super::super::plan::fingerprint_config(
                ClientKind::ClaudeCode,
                ConfigSource::ClaudeJson,
                ConfigPresence::Missing,
                b"{}",
                None,
                Some(&target_path),
            ),
            summary: ClaudeLayerSummary {
                source,
                availability: ClaudeLayerAvailability::Missing,
                route: ClaudeRouteDeclaration::Absent,
                model: DeclarationState::Absent,
                reasoning: DeclarationState::Absent,
                auth: AuthDeclaration::Unknown,
                hooks: HookShellEffect::Absent,
            },
            route_blockers: Vec::new(),
            pin,
        }),
    }
}

fn summary_from_bytes(
    source: ClaudeLayerSource,
    presence: ConfigPresence,
    bytes: &[u8],
    context: &PlanContext,
) -> Result<ClaudeLayerSummary, ClaudeLayersError> {
    if presence == ConfigPresence::Missing {
        return Ok(ClaudeLayerSummary {
            source,
            availability: ClaudeLayerAvailability::Missing,
            route: ClaudeRouteDeclaration::Absent,
            model: DeclarationState::Absent,
            reasoning: DeclarationState::Absent,
            auth: AuthDeclaration::Unknown,
            hooks: HookShellEffect::Absent,
        });
    }
    if bytes.len() > MAX_LAYER_BYTES {
        return Err(ClaudeLayersError::TooLarge);
    }
    let parsed = super::super::config_document::parse_document(ClientKind::ClaudeCode, bytes)
        .map_err(|error| match error {
            crate::integrations::plan::PlanError::ConfigTooLarge => ClaudeLayersError::TooLarge,
            crate::integrations::plan::PlanError::DuplicateKey => ClaudeLayersError::DuplicateKey,
            _ => ClaudeLayersError::InvalidDocument,
        })?;
    let root = parsed
        .as_object()
        .ok_or(ClaudeLayersError::InvalidDocument)?;
    let env = root.get("env").and_then(Value::as_object);
    let route = if root.contains_key("env") && env.is_none() {
        ClaudeRouteDeclaration::Unknown
    } else {
        route_declaration(env.and_then(|vars| vars.get("ANTHROPIC_BASE_URL")), context)
    };
    let model = declaration(root.get("model"));
    let reasoning = declaration(root.get("effortLevel"));
    let auth = if root.contains_key("apiKeyHelper") {
        AuthDeclaration::HelperDeclared
    } else if env.is_some_and(|vars| {
        vars.contains_key("ANTHROPIC_API_KEY") || vars.contains_key("ANTHROPIC_AUTH_TOKEN")
    }) {
        AuthDeclaration::SecretConfigured
    } else {
        AuthDeclaration::Unknown
    };
    let hooks = match root.get("hooks") {
        None => HookShellEffect::Absent,
        Some(Value::Null) => HookShellEffect::Unknown,
        Some(_) => HookShellEffect::PresentUnchanged,
    };
    Ok(ClaudeLayerSummary {
        source,
        availability: ClaudeLayerAvailability::Present,
        route,
        model,
        reasoning,
        auth,
        hooks,
    })
}

fn route_declaration(value: Option<&Value>, context: &PlanContext) -> ClaudeRouteDeclaration {
    match value {
        None => ClaudeRouteDeclaration::Absent,
        Some(Value::String(value)) if value == &endpoint_value(context) => {
            ClaudeRouteDeclaration::HeadroomEndpoint
        }
        Some(Value::String(value)) if !value.is_empty() => {
            ClaudeRouteDeclaration::OtherConfiguredRedacted
        }
        Some(_) => ClaudeRouteDeclaration::Unknown,
    }
}

fn route_blockers(
    context: &PlanContext,
    bytes: &[u8],
) -> Result<Vec<PlanBlocker>, ClaudeLayersError> {
    let plan =
        preview_config_with_presence(context, bytes, ConfigPresence::Present).map_err(|error| {
            match error {
                crate::integrations::plan::PlanError::ConfigTooLarge => ClaudeLayersError::TooLarge,
                crate::integrations::plan::PlanError::DuplicateKey => {
                    ClaudeLayersError::DuplicateKey
                }
                _ => ClaudeLayersError::InvalidDocument,
            }
        })?;
    Ok(plan
        .blockers
        .into_iter()
        .filter(|blocker| {
            matches!(
                blocker,
                PlanBlocker::CustomGateway
                    | PlanBlocker::UnsupportedRoute
                    | PlanBlocker::UnsupportedSchema
            )
        })
        .collect())
}

fn summarize_environment(
    environment: &[(String, OsString)],
    context: &PlanContext,
) -> Result<(ClaudeLayerSummary, String, Vec<PlanBlocker>), ClaudeLayersError> {
    let mut map = Map::new();
    let mut environment_hash = Sha256::new();
    environment_hash.update(b"headroom-claude-route-env/v1\0");
    for (name, value) in environment {
        let Some(name) = ROUTE_ENV_NAMES.iter().find(|known| **known == name) else {
            return Err(ClaudeLayersError::InvalidDocument);
        };
        let value_bytes = os_value_bytes(value);
        environment_hash.update((name.len() as u64).to_le_bytes());
        environment_hash.update(name.as_bytes());
        environment_hash.update((value_bytes.len() as u64).to_le_bytes());
        environment_hash.update(&value_bytes);
        if *name == "ANTHROPIC_BASE_URL" {
            let string = value.to_str().map(str::to_owned);
            map.insert(
                "ANTHROPIC_BASE_URL".to_owned(),
                string.map(Value::String).unwrap_or(Value::Null),
            );
        } else {
            map.insert((*name).to_owned(), Value::String("set".to_owned()));
        }
    }
    let env_value = Value::Object(Map::from_iter([("env".to_owned(), Value::Object(map))]));
    let bytes = serde_json::to_vec(&env_value).map_err(|_| ClaudeLayersError::InvalidDocument)?;
    let route = match env_value
        .get("env")
        .and_then(Value::as_object)
        .and_then(|vars| vars.get("ANTHROPIC_BASE_URL"))
    {
        None => ClaudeRouteDeclaration::Absent,
        value => route_declaration(value, context),
    };
    let blockers = if environment.is_empty() {
        Vec::new()
    } else {
        route_blockers(context, &bytes)?
    };
    Ok((
        ClaudeLayerSummary {
            source: ClaudeLayerSource::ProcessEnvironment,
            availability: ClaudeLayerAvailability::Present,
            route,
            model: DeclarationState::Unknown,
            reasoning: DeclarationState::Unknown,
            auth: AuthDeclaration::Unknown,
            hooks: HookShellEffect::Unknown,
        },
        hex(&environment_hash.finalize()),
        blockers,
    ))
}

fn user_settings_path(roots: &ConfigRootInputs) -> Result<PathBuf, ClaudeLayersError> {
    let root = match &roots.claude_config_dir {
        Some(path) => path.clone(),
        None => roots
            .user_profile
            .as_ref()
            .ok_or(ConfigIoError::InvalidRoot)?
            .join(".claude"),
    };
    Ok(root.join("settings.json"))
}

fn managed_settings_path(roots: &ConfigRootInputs) -> Option<PathBuf> {
    #[cfg(test)]
    {
        roots.claude_test_managed_settings.clone()
    }
    #[cfg(all(not(test), windows))]
    {
        Some(PathBuf::from(MANAGED_SETTINGS_PATH))
    }
    #[cfg(all(not(test), not(windows)))]
    {
        let _ = roots;
        None
    }
}

fn route_environment(roots: &ConfigRootInputs) -> Vec<(String, OsString)> {
    #[cfg(test)]
    {
        return roots
            .claude_test_route_environment
            .as_ref()
            .map(|items| {
                items
                    .iter()
                    .map(|(name, value)| (name.clone(), OsString::from(value)))
                    .collect()
            })
            .unwrap_or_default();
    }
    #[cfg(not(test))]
    {
        ROUTE_ENV_NAMES
            .iter()
            .filter_map(|name| env::var_os(name).map(|value| ((*name).to_owned(), value)))
            .collect()
    }
}

fn endpoint_value(context: &PlanContext) -> String {
    format!("http://127.0.0.1:{}", context.endpoint.port)
}

fn scope_code(scope: ConfigScope) -> &'static str {
    match scope {
        ConfigScope::UserConfig => "user_config",
        ConfigScope::ProjectLocalConfig => "project_local_config",
    }
}

fn declaration(value: Option<&Value>) -> DeclarationState {
    match value {
        None => DeclarationState::Absent,
        Some(Value::String(value)) if !value.is_empty() => DeclarationState::Present,
        Some(_) => DeclarationState::Unknown,
    }
}

fn not_observed(source: ClaudeLayerSource) -> ClaudeLayerSummary {
    ClaudeLayerSummary {
        source,
        availability: ClaudeLayerAvailability::NotObserved,
        route: ClaudeRouteDeclaration::Unknown,
        model: DeclarationState::Unknown,
        reasoning: DeclarationState::Unknown,
        auth: AuthDeclaration::Unknown,
        hooks: HookShellEffect::Unknown,
    }
}

fn aggregate_declaration(
    layers: &[ClaudeLayerSummary],
    select: impl Fn(&ClaudeLayerSummary) -> DeclarationState,
) -> DeclarationState {
    let mut present = false;
    let mut unknown = false;
    for layer in layers.iter().filter(|layer| {
        layer.source != ClaudeLayerSource::ProcessEnvironment
            && layer.availability != ClaudeLayerAvailability::NotObserved
    }) {
        match select(layer) {
            DeclarationState::Present => present = true,
            DeclarationState::Unknown => unknown = true,
            DeclarationState::Absent => {}
        }
    }
    if unknown {
        DeclarationState::Unknown
    } else if present {
        DeclarationState::Present
    } else {
        DeclarationState::Absent
    }
}

fn aggregate_auth(layers: &[ClaudeLayerSummary]) -> AuthDeclaration {
    let mut declaration = None;
    for layer in layers.iter().filter(|layer| {
        layer.source != ClaudeLayerSource::ProcessEnvironment
            && layer.availability != ClaudeLayerAvailability::NotObserved
    }) {
        if layer.auth == AuthDeclaration::Unknown {
            continue;
        }
        if declaration.is_some_and(|existing| existing != layer.auth) {
            return AuthDeclaration::Unknown;
        }
        declaration = Some(layer.auth);
    }
    declaration.unwrap_or(AuthDeclaration::Unknown)
}

fn aggregate_hooks(layers: &[ClaudeLayerSummary]) -> HookShellEffect {
    let mut present = false;
    let mut unknown = false;
    for layer in layers.iter().filter(|layer| {
        layer.source != ClaudeLayerSource::ProcessEnvironment
            && layer.availability != ClaudeLayerAvailability::NotObserved
    }) {
        match layer.hooks {
            HookShellEffect::PresentUnchanged => present = true,
            HookShellEffect::Unknown => unknown = true,
            HookShellEffect::Absent => {}
        }
    }
    if unknown {
        HookShellEffect::Unknown
    } else if present {
        HookShellEffect::PresentUnchanged
    } else {
        HookShellEffect::Absent
    }
}

fn append_layer_fingerprint(hash: &mut Sha256, source: ClaudeLayerSource, fingerprint: &str) {
    let source = source.code().as_bytes();
    hash.update((source.len() as u64).to_le_bytes());
    hash.update(source);
    hash.update((fingerprint.len() as u64).to_le_bytes());
    hash.update(fingerprint.as_bytes());
}

fn deduplicate_blockers(blockers: &mut Vec<PlanBlocker>) {
    let mut unique = Vec::with_capacity(blockers.len());
    for blocker in blockers.drain(..) {
        if !unique.contains(&blocker) {
            unique.push(blocker);
        }
    }
    *blockers = unique;
}

fn os_value_bytes(value: &OsString) -> Vec<u8> {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        value.encode_wide().flat_map(u16::to_le_bytes).collect()
    }
    #[cfg(not(windows))]
    {
        use std::os::unix::ffi::OsStrExt;
        value.as_os_str().as_bytes().to_vec()
    }
}

fn same_path(left: &Path, right: &Path) -> bool {
    super::same_path(left, right)
}

fn hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write;
        let _ = write!(output, "{byte:02x}");
    }
    output
}

#[cfg(test)]
#[path = "claude_layers_tests.rs"]
mod tests;
