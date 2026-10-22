use serde_json::Value;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use serde::Serialize;
#[cfg(windows)]
use std::os::windows::ffi::OsStrExt;
use std::path::Path;

const PLAN_LIFETIME_SECONDS: u64 = 300;
const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;
pub(super) const CODEX_FIXTURE_CLI_VERSIONS: &[&str] =
    &["0.163.0-alpha.10.2", "0.163.0-alpha.11.1"];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ClientKind {
    Codex,
    ClaudeCode,
    OpenCodeV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ConfigScope {
    UserConfig,
}

#[derive(Clone, Debug)]
pub(crate) struct StableEndpoint {
    pub endpoint_id: String,
    pub host: String,
    pub port: u16,
}

#[derive(Clone, Debug)]
pub(crate) struct PlanContext {
    pub request_id: String,
    pub installation_id: String,
    pub client_kind: ClientKind,
    pub config_source: ConfigSource,
    pub surface: String,
    pub version: Option<String>,
    pub scope: ConfigScope,
    pub scan_generation: u64,
    pub endpoint: StableEndpoint,
    pub now_unix_seconds: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum PlanStatus {
    Preview,
    Blocked,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DeclarationState {
    Present,
    Absent,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AuthDeclaration {
    Unknown,
    MethodDeclared,
    SecretConfigured,
    HelperDeclared,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum HookShellEffect {
    Absent,
    PresentUnchanged,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum EndpointBefore {
    Missing,
    SameStableEndpoint,
    OtherConfiguredRedacted,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum EndpointField {
    CodexOpenaiBaseUrl,
    ClaudeAnthropicBaseUrl,
    #[serde(rename = "opencode_v1_openai_base_url")]
    OpenCodeV1OpenAiBaseUrl,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct PlannedEndpointField {
    pub field: EndpointField,
    pub before: EndpointBefore,
    pub proposed_value: String,
    pub after_headroom: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum PlanBlocker {
    UnsupportedSchema,
    CustomGateway,
    UnsupportedRoute,
    ClientVersionUnknown,
    UnsupportedClientVersion,
    UnsupportedClientSurface,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ConfigPresence {
    Present,
    Missing,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ConfigSource {
    #[serde(rename = "codex_toml")]
    CodexToml,
    #[serde(rename = "claude_json")]
    ClaudeJson,
    #[serde(rename = "opencode_json")]
    OpenCodeJson,
    #[serde(rename = "opencode_jsonc")]
    OpenCodeJsonc,
    #[serde(rename = "opencode_missing")]
    OpenCodeMissing,
}

impl ConfigSource {
    pub(super) fn code(self) -> &'static str {
        match self {
            Self::CodexToml => "codex_toml",
            Self::ClaudeJson => "claude_json",
            Self::OpenCodeJson => "opencode_json",
            Self::OpenCodeJsonc => "opencode_jsonc",
            Self::OpenCodeMissing => "opencode_missing",
        }
    }

    fn matches_client(self, kind: ClientKind) -> bool {
        matches!(
            (kind, self),
            (ClientKind::Codex, Self::CodexToml)
                | (ClientKind::ClaudeCode, Self::ClaudeJson)
                | (ClientKind::OpenCodeV1, Self::OpenCodeJson)
                | (ClientKind::OpenCodeV1, Self::OpenCodeJsonc)
                | (ClientKind::OpenCodeV1, Self::OpenCodeMissing)
        )
    }
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct ConnectionPlan {
    pub schema_version: u8,
    pub request_id: String,
    pub plan_id: String,
    pub expires_at_unix_seconds: u64,
    pub installation_id: String,
    pub client_kind: &'static str,
    pub scope: &'static str,
    pub scan_generation: u64,
    pub config_fingerprint: String,
    pub config_presence: ConfigPresence,
    pub config_source: ConfigSource,
    pub endpoint_id: String,
    pub endpoint_host: &'static str,
    pub endpoint_port: u16,
    pub endpoint_field: PlannedEndpointField,
    pub status: PlanStatus,
    pub model: DeclarationState,
    pub reasoning: DeclarationState,
    pub model_unchanged: bool,
    pub provider_unchanged: bool,
    pub reasoning_unchanged: bool,
    pub auth_unchanged: bool,
    pub agent_count_unchanged: bool,
    pub runtime_effective: DeclarationState,
    pub auth: AuthDeclaration,
    pub hook_shell: HookShellEffect,
    pub restart_required: bool,
    pub blockers: Vec<PlanBlocker>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PlanError {
    InvalidContext,
    ConfigTooLarge,
    DuplicateKey,
    InvalidConfig,
}

pub(crate) fn preview_config(
    context: &PlanContext,
    config_bytes: &[u8],
) -> Result<ConnectionPlan, PlanError> {
    preview_config_with_presence(context, config_bytes, ConfigPresence::Present)
}

pub(crate) fn preview_config_with_presence(
    context: &PlanContext,
    config_bytes: &[u8],
    config_presence: ConfigPresence,
) -> Result<ConnectionPlan, PlanError> {
    if !valid_uuid(&context.request_id)
        || !valid_uuid(&context.installation_id)
        || !valid_uuid(&context.endpoint.endpoint_id)
        || context.scope != ConfigScope::UserConfig
        || !context.config_source.matches_client(context.client_kind)
        || context.scan_generation == 0
        || context.scan_generation > MAX_SAFE_INTEGER
        || context.endpoint.host != "127.0.0.1"
        || context.endpoint.port == 0
        || context.now_unix_seconds > MAX_SAFE_INTEGER - PLAN_LIFETIME_SECONDS
    {
        return Err(PlanError::InvalidContext);
    }

    let config = super::config_document::parse_document(context.client_kind, config_bytes)?;
    let root = config.as_object().ok_or(PlanError::InvalidConfig)?;
    let proposed_value = proposed_endpoint_value(context.client_kind, &context.endpoint);
    let mut blockers = Vec::new();
    let (field, before) = match context.client_kind {
        ClientKind::Codex => analyze_codex(root, &proposed_value, &mut blockers),
        ClientKind::ClaudeCode => analyze_claude(root, &proposed_value, &mut blockers),
        ClientKind::OpenCodeV1 => analyze_opencode_v1(root, &proposed_value, &mut blockers),
    };

    if context.surface != "cli" {
        add_blocker(&mut blockers, PlanBlocker::UnsupportedClientSurface);
    }
    match context.version.as_deref() {
        None => add_blocker(&mut blockers, PlanBlocker::ClientVersionUnknown),
        Some(version) if !supported_version(context.client_kind, version) => {
            add_blocker(&mut blockers, PlanBlocker::UnsupportedClientVersion);
        }
        Some(_) => {}
    }

    let (model, reasoning, auth, hook_shell) = match context.client_kind {
        ClientKind::Codex => (
            declaration(root.get("model")),
            declaration(root.get("model_reasoning_effort")),
            codex_auth(root),
            hook_effect(root, &["hooks", "notify", "shell"]),
        ),
        ClientKind::ClaudeCode => (
            declaration(root.get("model")),
            declaration(root.get("effortLevel")),
            claude_auth(root),
            hook_effect(root, &["hooks", "statusLine", "shell"]),
        ),
        ClientKind::OpenCodeV1 => (
            declaration(root.get("model")),
            DeclarationState::Unknown,
            opencode_auth(root),
            hook_effect(root, &["plugin", "command", "shell"]),
        ),
    };

    if before == EndpointBefore::OtherConfiguredRedacted {
        add_blocker(&mut blockers, PlanBlocker::CustomGateway);
    } else if before == EndpointBefore::Unknown {
        add_blocker(&mut blockers, PlanBlocker::UnsupportedRoute);
    }

    let status = if blockers.is_empty() {
        PlanStatus::Preview
    } else {
        PlanStatus::Blocked
    };
    let after_headroom = status == PlanStatus::Preview;
    let config_fingerprint = fingerprint_config(
        context.client_kind,
        context.config_source,
        config_presence,
        config_bytes,
        None,
        None,
    );
    let plan_id = Uuid::new_v4().to_string();

    Ok(ConnectionPlan {
        schema_version: 1,
        request_id: context.request_id.clone(),
        plan_id,
        expires_at_unix_seconds: context.now_unix_seconds + PLAN_LIFETIME_SECONDS,
        installation_id: context.installation_id.clone(),
        client_kind: client_name(context.client_kind),
        scope: "user_config",
        scan_generation: context.scan_generation,
        config_fingerprint,
        config_presence,
        config_source: context.config_source,
        endpoint_id: context.endpoint.endpoint_id.clone(),
        endpoint_host: "127.0.0.1",
        endpoint_port: context.endpoint.port,
        endpoint_field: PlannedEndpointField {
            field,
            before,
            proposed_value,
            after_headroom,
        },
        status,
        model,
        reasoning,
        model_unchanged: true,
        provider_unchanged: true,
        reasoning_unchanged: true,
        auth_unchanged: true,
        agent_count_unchanged: true,
        runtime_effective: DeclarationState::Unknown,
        auth,
        hook_shell,
        restart_required: true,
        blockers: unique_blockers(blockers),
    })
}

pub(super) fn supported_version(kind: ClientKind, version: &str) -> bool {
    match kind {
        ClientKind::Codex => CODEX_FIXTURE_CLI_VERSIONS.contains(&version),
        ClientKind::ClaudeCode => version == "2.1.158",
        ClientKind::OpenCodeV1 => version == "1.19.3",
    }
}

pub(super) fn fingerprint_config(
    kind: ClientKind,
    source: ConfigSource,
    presence: ConfigPresence,
    bytes: &[u8],
    file_identity: Option<(u32, u32, u32)>,
    target_path: Option<&Path>,
) -> String {
    let mut digest = Sha256::new();
    digest.update(b"headroom-config-fingerprint/v1\0user_config\0");
    digest.update(client_name(kind).as_bytes());
    digest.update([0]);
    digest.update(source.code().as_bytes());
    digest.update([0]);
    if presence == ConfigPresence::Present {
        digest.update(b"present\0");
        digest.update((bytes.len() as u64).to_le_bytes());
        digest.update(Sha256::digest(bytes));
    } else {
        digest.update(b"missing\0");
    }
    if let Some((volume, high, low)) = file_identity {
        digest.update(b"identity\0");
        digest.update(volume.to_le_bytes());
        digest.update(high.to_le_bytes());
        digest.update(low.to_le_bytes());
    } else {
        digest.update(b"no-identity\0");
    }
    if let Some(path) = target_path {
        digest.update(b"target\0");
        #[cfg(windows)]
        for unit in path.as_os_str().encode_wide() {
            digest.update(unit.to_le_bytes());
        }
        #[cfg(not(windows))]
        digest.update(path.to_string_lossy().as_bytes());
    } else {
        digest.update(b"no-target\0");
    }
    hex_digest(&digest.finalize())
}

fn analyze_codex(
    root: &serde_json::Map<String, Value>,
    target: &str,
    blockers: &mut Vec<PlanBlocker>,
) -> (EndpointField, EndpointBefore) {
    if let Some(provider) = root.get("model_provider") {
        match provider.as_str() {
            Some("openai") => {}
            Some(_) => add_blocker(blockers, PlanBlocker::CustomGateway),
            None => add_blocker(blockers, PlanBlocker::UnsupportedRoute),
        }
    }
    if root
        .get("model_providers")
        .and_then(Value::as_object)
        .is_some_and(|providers| !providers.is_empty())
    {
        add_blocker(blockers, PlanBlocker::CustomGateway);
    }
    if root
        .get("model_providers")
        .is_some_and(|providers| !providers.is_object())
    {
        add_blocker(blockers, PlanBlocker::UnsupportedSchema);
    }
    if root.contains_key("profile")
        || root.contains_key("profiles")
        || root.contains_key("experimental_realtime_ws_base_url")
    {
        add_blocker(blockers, PlanBlocker::UnsupportedRoute);
    }
    (
        EndpointField::CodexOpenaiBaseUrl,
        endpoint_before(root.get("openai_base_url"), target),
    )
}

fn analyze_claude(
    root: &serde_json::Map<String, Value>,
    target: &str,
    blockers: &mut Vec<PlanBlocker>,
) -> (EndpointField, EndpointBefore) {
    let env = root.get("env").and_then(Value::as_object);
    let before = endpoint_before(env.and_then(|vars| vars.get("ANTHROPIC_BASE_URL")), target);
    if root.get("env").is_some() && env.is_none() {
        add_blocker(blockers, PlanBlocker::UnsupportedSchema);
    }
    if env.is_some_and(|vars| {
        vars.keys().any(|name| {
            name.starts_with("CLAUDE_CODE_USE_")
                || name.starts_with("ANTHROPIC_BEDROCK_")
                || name.starts_with("ANTHROPIC_VERTEX_")
                || name.starts_with("ANTHROPIC_FOUNDRY_")
        })
    }) {
        add_blocker(blockers, PlanBlocker::UnsupportedRoute);
    }
    (EndpointField::ClaudeAnthropicBaseUrl, before)
}

fn analyze_opencode_v1(
    root: &serde_json::Map<String, Value>,
    target: &str,
    blockers: &mut Vec<PlanBlocker>,
) -> (EndpointField, EndpointBefore) {
    if root.contains_key("providers") || root.contains_key("settings") {
        add_blocker(blockers, PlanBlocker::UnsupportedRoute);
    }
    let model_provider = root
        .get("model")
        .and_then(Value::as_str)
        .and_then(|model| model.split_once('/').map(|(provider, _)| provider));
    if model_provider != Some("openai") {
        add_blocker(blockers, PlanBlocker::UnsupportedRoute);
    }

    let provider_map = root.get("provider").and_then(Value::as_object);
    if root.get("provider").is_some() && provider_map.is_none() {
        add_blocker(blockers, PlanBlocker::UnsupportedSchema);
    }
    if provider_map.is_some_and(|providers| providers.keys().any(|name| name != "openai")) {
        add_blocker(blockers, PlanBlocker::UnsupportedRoute);
    }
    let openai_provider = provider_map
        .and_then(|providers| providers.get("openai"))
        .and_then(Value::as_object);
    if provider_map.is_some_and(|providers| providers.contains_key("openai"))
        && openai_provider.is_none()
    {
        add_blocker(blockers, PlanBlocker::UnsupportedSchema);
    }
    if openai_provider.is_some_and(|provider| {
        provider.get("npm").is_some_and(|npm| !npm.is_null())
            || provider.get("models").is_some_and(|models| match models {
                Value::Object(models) => !models.is_empty(),
                Value::Null => false,
                _ => true,
            })
    }) {
        add_blocker(blockers, PlanBlocker::UnsupportedRoute);
    }
    if root.contains_key("agent") || root.contains_key("agents") {
        add_blocker(blockers, PlanBlocker::UnsupportedRoute);
    }
    let options = openai_provider
        .and_then(|provider| provider.get("options"))
        .and_then(Value::as_object);
    if openai_provider.is_some_and(|provider| provider.contains_key("options")) && options.is_none()
    {
        add_blocker(blockers, PlanBlocker::UnsupportedSchema);
    }
    if options.is_some_and(|options| options.contains_key("headers")) {
        add_blocker(blockers, PlanBlocker::UnsupportedRoute);
    }
    (
        EndpointField::OpenCodeV1OpenAiBaseUrl,
        endpoint_before(options.and_then(|options| options.get("baseURL")), target),
    )
}

fn endpoint_before(value: Option<&Value>, target: &str) -> EndpointBefore {
    match value {
        None => EndpointBefore::Missing,
        Some(Value::String(value)) if value == target => EndpointBefore::SameStableEndpoint,
        Some(Value::String(value)) if !value.is_empty() => EndpointBefore::OtherConfiguredRedacted,
        Some(_) => EndpointBefore::Unknown,
    }
}

fn proposed_endpoint_value(kind: ClientKind, endpoint: &StableEndpoint) -> String {
    let suffix = match kind {
        ClientKind::Codex | ClientKind::OpenCodeV1 => "/v1",
        ClientKind::ClaudeCode => "",
    };
    format!("http://127.0.0.1:{}{suffix}", endpoint.port)
}

fn declaration(value: Option<&Value>) -> DeclarationState {
    match value {
        Some(Value::String(value)) if !value.is_empty() => DeclarationState::Present,
        Some(_) => DeclarationState::Unknown,
        None => DeclarationState::Absent,
    }
}

fn codex_auth(root: &serde_json::Map<String, Value>) -> AuthDeclaration {
    match root.get("forced_login_method").and_then(Value::as_str) {
        Some("chatgpt" | "api") => AuthDeclaration::MethodDeclared,
        _ => AuthDeclaration::Unknown,
    }
}

fn claude_auth(root: &serde_json::Map<String, Value>) -> AuthDeclaration {
    if root.contains_key("apiKeyHelper") {
        return AuthDeclaration::HelperDeclared;
    }
    let Some(env) = root.get("env").and_then(Value::as_object) else {
        return AuthDeclaration::Unknown;
    };
    if env.contains_key("ANTHROPIC_API_KEY") || env.contains_key("ANTHROPIC_AUTH_TOKEN") {
        AuthDeclaration::SecretConfigured
    } else {
        AuthDeclaration::Unknown
    }
}

fn opencode_auth(root: &serde_json::Map<String, Value>) -> AuthDeclaration {
    let options = root
        .get("provider")
        .and_then(Value::as_object)
        .and_then(|providers| providers.get("openai"))
        .and_then(Value::as_object)
        .and_then(|provider| provider.get("options"))
        .and_then(Value::as_object);
    if options.is_some_and(|options| options.contains_key("apiKey")) {
        AuthDeclaration::SecretConfigured
    } else {
        AuthDeclaration::Unknown
    }
}

fn hook_effect(root: &serde_json::Map<String, Value>, keys: &[&str]) -> HookShellEffect {
    let mut present = false;
    for key in keys {
        match root.get(*key) {
            Some(Value::Null) => return HookShellEffect::Unknown,
            Some(_) => present = true,
            None => {}
        }
    }
    if present {
        HookShellEffect::PresentUnchanged
    } else {
        HookShellEffect::Absent
    }
}

fn add_blocker(blockers: &mut Vec<PlanBlocker>, blocker: PlanBlocker) {
    if !blockers.contains(&blocker) {
        blockers.push(blocker);
    }
}

fn unique_blockers(blockers: Vec<PlanBlocker>) -> Vec<PlanBlocker> {
    let mut unique = Vec::with_capacity(blockers.len());
    for blocker in blockers {
        add_blocker(&mut unique, blocker);
    }
    unique
}

fn client_name(kind: ClientKind) -> &'static str {
    match kind {
        ClientKind::Codex => "codex",
        ClientKind::ClaudeCode => "claude_code",
        ClientKind::OpenCodeV1 => "opencode_v1",
    }
}

fn valid_uuid(value: &str) -> bool {
    value.len() == 36 && Uuid::parse_str(value).is_ok_and(|parsed| parsed.to_string() == value)
}

fn hex_digest(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

#[cfg(test)]
mod wire_name_tests {
    use super::{ConfigSource, EndpointField};

    #[test]
    fn connection_plan_wire_enums_use_fixed_contract_names() {
        assert_eq!(
            serde_json::to_value(ConfigSource::OpenCodeJsonc).unwrap(),
            "opencode_jsonc"
        );
        assert_eq!(
            serde_json::to_value(EndpointField::OpenCodeV1OpenAiBaseUrl).unwrap(),
            "opencode_v1_openai_base_url"
        );
    }
}
