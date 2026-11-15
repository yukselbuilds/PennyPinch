use serde::Serialize;
use serde_json::{Map, Value};
use std::collections::{BTreeSet, HashSet};
use std::fmt;

const CANONICAL_OPENAI_BASE: &str = "https://api.openai.com/v1";
pub(super) const MAX_PROVIDER_COUNT: usize = 32;
pub(super) const MAX_ELIGIBLE_ROUTES: usize = 16;
pub(super) const MAX_MODEL_COUNT: usize = 128;
pub(super) const MAX_VARIANTS_PER_MODEL: usize = 32;
pub(super) const MAX_KEY_BYTES: usize = 128;
pub(super) const MAX_MODEL_REFERENCE_BYTES: usize = 512;
pub(super) const MAX_TRANSPORT_NESTING_DEPTH: usize = 8;
const TRANSPORT_OVERRIDE_KEYS: &[&str] = &["baseURL", "endpoint", "fetch", "headers", "npm", "api"];

// These IDs have V1-specific loaders in the pinned 1.19.3 provider.ts. Treating
// them as generic OpenAI-compatible aliases would select different SDK behavior.
pub(super) const RESERVED_CUSTOM_LOADERS: &[&str] = &[
    "anthropic",
    "opencode",
    "openai",
    "meta",
    "xai",
    "github-copilot",
    "azure",
    "azure-cognitive-services",
    "amazon-bedrock",
    "llmgateway",
    "openrouter",
    "nvidia",
    "vercel",
    "google-vertex",
    "google-vertex-anthropic",
    "sap-ai-core",
    "zenmux",
    "gitlab",
    "cloudflare-workers-ai",
    "cloudflare-ai-gateway",
    "cerebras",
    "kilo",
    "snowflake-cortex",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum OpenCodeProtocol {
    Responses,
    ChatCompletions,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum OpenCodeRouteBefore {
    Missing,
    CanonicalOpenAiBase,
    SameStableEndpoint,
}

#[derive(Clone, Eq, PartialEq)]
pub(crate) struct OpenCodeRoute {
    pub(crate) provider_key: String,
    pub(crate) protocol: OpenCodeProtocol,
    pub(crate) before: OpenCodeRouteBefore,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum OpenCodeRuntimeEffective {
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum OpenCodeDefaultRouteCoverage {
    Covered,
    Excluded,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub(crate) struct OpenCodeRouteCoverage {
    pub(crate) eligible_count: u8,
    pub(crate) excluded_count: u8,
    pub(crate) already_same_count: u8,
    pub(crate) responses_count: u8,
    pub(crate) chat_completions_count: u8,
    pub(crate) partial: bool,
    pub(crate) configured_default_route: OpenCodeDefaultRouteCoverage,
    pub(crate) runtime_effective: OpenCodeRuntimeEffective,
}

#[derive(Clone, Eq, PartialEq)]
pub(crate) struct OpenCodeRouteAnalysis {
    pub(crate) routes: Vec<OpenCodeRoute>,
    pub(crate) coverage: OpenCodeRouteCoverage,
}

impl fmt::Debug for OpenCodeRouteAnalysis {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OpenCodeRouteAnalysis")
            .field("route_count", &self.routes.len())
            .field("coverage", &self.coverage)
            .finish()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum OpenCodeRouteAnalysisError {
    InvalidSchema,
    InvalidEndpoint,
    LimitExceeded,
    UnsupportedConfig,
}

pub(crate) fn analyze_opencode_v1_routes(
    root: &Map<String, Value>,
    stable_endpoint: &str,
) -> Result<OpenCodeRouteAnalysis, OpenCodeRouteAnalysisError> {
    validate_stable_endpoint(stable_endpoint)?;
    if root.contains_key("providers")
        || root.contains_key("settings")
        || root.contains_key("agents")
    {
        return Err(OpenCodeRouteAnalysisError::UnsupportedConfig);
    }

    let providers = object_field(root, "provider")?;
    if providers.is_some_and(|items| items.len() > MAX_PROVIDER_COUNT) {
        return Err(OpenCodeRouteAnalysisError::LimitExceeded);
    }

    let mut candidates = BTreeSet::new();
    if let Some(providers) = providers {
        for (name, value) in providers {
            add_provider_key(&mut candidates, name)?;
            if value.as_object().is_none() {
                return Err(OpenCodeRouteAnalysisError::InvalidSchema);
            }
        }
    }

    let enabled = string_set_field(root, "enabled_providers", &mut candidates)?;
    let disabled = string_set_field(root, "disabled_providers", &mut BTreeSet::new())?;
    add_model_reference(root.get("model"), &mut candidates)?;
    add_model_reference(root.get("small_model"), &mut candidates)?;
    add_configured_model_references(root.get("agent"), &mut candidates)?;
    // V1's deprecated mode blocks also carry model selectors and must not be
    // silently omitted from route inventory.
    add_configured_model_references(root.get("mode"), &mut candidates)?;
    if candidates.len() > MAX_PROVIDER_COUNT {
        return Err(OpenCodeRouteAnalysisError::LimitExceeded);
    }

    let model_count = validate_provider_shapes(providers)?;
    if model_count > MAX_MODEL_COUNT {
        return Err(OpenCodeRouteAnalysisError::LimitExceeded);
    }
    let plugin_configured = match root.get("plugin") {
        None => false,
        Some(Value::Array(plugins)) if plugins.len() <= MAX_MODEL_COUNT => !plugins.is_empty(),
        Some(Value::Array(_)) => return Err(OpenCodeRouteAnalysisError::LimitExceeded),
        Some(_) => return Err(OpenCodeRouteAnalysisError::InvalidSchema),
    };

    let mut routes = Vec::new();
    let mut excluded_count = 0usize;
    let mut already_same_count = 0usize;
    let mut responses_count = 0usize;
    let mut chat_completions_count = 0usize;
    for provider_key in candidates {
        if disabled
            .as_ref()
            .is_some_and(|disabled| disabled.contains(&provider_key))
            || enabled
                .as_ref()
                .is_some_and(|enabled| !enabled.contains(&provider_key))
        {
            excluded_count += 1;
            continue;
        }
        let provider = providers.and_then(|items| items.get(&provider_key));
        let Some((protocol, before)) = classify_provider(&provider_key, provider, stable_endpoint)?
        else {
            excluded_count += 1;
            continue;
        };
        if routes.len() >= MAX_ELIGIBLE_ROUTES {
            return Err(OpenCodeRouteAnalysisError::LimitExceeded);
        }
        if before == OpenCodeRouteBefore::SameStableEndpoint {
            already_same_count += 1;
        }
        match protocol {
            OpenCodeProtocol::Responses => responses_count += 1,
            OpenCodeProtocol::ChatCompletions => chat_completions_count += 1,
        }
        routes.push(OpenCodeRoute {
            provider_key,
            protocol,
            before,
        });
    }

    Ok(OpenCodeRouteAnalysis {
        coverage: OpenCodeRouteCoverage {
            eligible_count: routes.len() as u8,
            excluded_count: excluded_count as u8,
            already_same_count: already_same_count as u8,
            responses_count: responses_count as u8,
            chat_completions_count: chat_completions_count as u8,
            partial: excluded_count > 0 || plugin_configured,
            configured_default_route: configured_default_route(root, &routes)?,
            runtime_effective: OpenCodeRuntimeEffective::Unknown,
        },
        routes,
    })
}

pub(super) fn validate_stable_endpoint(value: &str) -> Result<(), OpenCodeRouteAnalysisError> {
    let Some(port) = value
        .strip_prefix("http://127.0.0.1:")
        .and_then(|value| value.strip_suffix("/v1"))
    else {
        return Err(OpenCodeRouteAnalysisError::InvalidEndpoint);
    };
    if port.is_empty() || !port.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(OpenCodeRouteAnalysisError::InvalidEndpoint);
    }
    let port = port
        .parse::<u16>()
        .map_err(|_| OpenCodeRouteAnalysisError::InvalidEndpoint)?;
    if port == 0 {
        return Err(OpenCodeRouteAnalysisError::InvalidEndpoint);
    }
    Ok(())
}

fn configured_default_route(
    root: &Map<String, Value>,
    routes: &[OpenCodeRoute],
) -> Result<OpenCodeDefaultRouteCoverage, OpenCodeRouteAnalysisError> {
    let Some(value) = root.get("model") else {
        return Ok(OpenCodeDefaultRouteCoverage::Unknown);
    };
    let model = value
        .as_str()
        .ok_or(OpenCodeRouteAnalysisError::InvalidSchema)?;
    let (provider, _) = model
        .split_once('/')
        .ok_or(OpenCodeRouteAnalysisError::InvalidSchema)?;
    if routes.iter().any(|route| route.provider_key == provider) {
        Ok(OpenCodeDefaultRouteCoverage::Covered)
    } else {
        Ok(OpenCodeDefaultRouteCoverage::Excluded)
    }
}

fn add_provider_key(
    candidates: &mut BTreeSet<String>,
    value: &str,
) -> Result<(), OpenCodeRouteAnalysisError> {
    if value.is_empty() || value.len() > MAX_KEY_BYTES || value.chars().any(char::is_control) {
        return Err(OpenCodeRouteAnalysisError::LimitExceeded);
    }
    candidates.insert(value.to_owned());
    if candidates.len() > MAX_PROVIDER_COUNT {
        return Err(OpenCodeRouteAnalysisError::LimitExceeded);
    }
    Ok(())
}

fn object_field<'a>(
    root: &'a Map<String, Value>,
    key: &str,
) -> Result<Option<&'a Map<String, Value>>, OpenCodeRouteAnalysisError> {
    match root.get(key) {
        None => Ok(None),
        Some(Value::Object(value)) => Ok(Some(value)),
        Some(_) => Err(OpenCodeRouteAnalysisError::InvalidSchema),
    }
}

fn string_set_field(
    root: &Map<String, Value>,
    key: &str,
    candidates: &mut BTreeSet<String>,
) -> Result<Option<HashSet<String>>, OpenCodeRouteAnalysisError> {
    let Some(value) = root.get(key) else {
        return Ok(None);
    };
    let items = value
        .as_array()
        .ok_or(OpenCodeRouteAnalysisError::InvalidSchema)?;
    if items.len() > MAX_PROVIDER_COUNT {
        return Err(OpenCodeRouteAnalysisError::LimitExceeded);
    }
    let mut set = HashSet::with_capacity(items.len());
    for item in items {
        let name = item
            .as_str()
            .ok_or(OpenCodeRouteAnalysisError::InvalidSchema)?;
        add_provider_key(candidates, name)?;
        set.insert(name.to_owned());
    }
    Ok(Some(set))
}

fn add_model_reference(
    value: Option<&Value>,
    candidates: &mut BTreeSet<String>,
) -> Result<(), OpenCodeRouteAnalysisError> {
    let Some(value) = value else {
        return Ok(());
    };
    let model = value
        .as_str()
        .ok_or(OpenCodeRouteAnalysisError::InvalidSchema)?;
    if model.len() > MAX_MODEL_REFERENCE_BYTES {
        return Err(OpenCodeRouteAnalysisError::LimitExceeded);
    }
    let Some((provider, model_id)) = model.split_once('/') else {
        return Err(OpenCodeRouteAnalysisError::InvalidSchema);
    };
    if model_id.is_empty() {
        return Err(OpenCodeRouteAnalysisError::InvalidSchema);
    }
    add_provider_key(candidates, provider)
}

fn add_configured_model_references(
    value: Option<&Value>,
    candidates: &mut BTreeSet<String>,
) -> Result<(), OpenCodeRouteAnalysisError> {
    let Some(value) = value else {
        return Ok(());
    };
    let entries = value
        .as_object()
        .ok_or(OpenCodeRouteAnalysisError::InvalidSchema)?;
    if entries.len() > MAX_PROVIDER_COUNT {
        return Err(OpenCodeRouteAnalysisError::LimitExceeded);
    }
    for (name, config) in entries {
        validate_key(name)?;
        let config = config
            .as_object()
            .ok_or(OpenCodeRouteAnalysisError::InvalidSchema)?;
        add_model_reference(config.get("model"), candidates)?;
    }
    Ok(())
}

fn validate_provider_shapes(
    providers: Option<&Map<String, Value>>,
) -> Result<usize, OpenCodeRouteAnalysisError> {
    let mut model_count = 0usize;
    for provider in providers
        .into_iter()
        .flat_map(|providers| providers.values())
    {
        let provider = provider
            .as_object()
            .ok_or(OpenCodeRouteAnalysisError::InvalidSchema)?;
        optional_string(provider.get("npm"))?;
        optional_string(provider.get("api"))?;
        if let Some(options) = provider.get("options") {
            let options = options
                .as_object()
                .ok_or(OpenCodeRouteAnalysisError::InvalidSchema)?;
            optional_string(options.get("baseURL"))?;
            optional_string(options.get("endpoint"))?;
        }
        if let Some(models) = provider.get("models") {
            let models = models
                .as_object()
                .ok_or(OpenCodeRouteAnalysisError::InvalidSchema)?;
            model_count = model_count.saturating_add(models.len());
            if model_count > MAX_MODEL_COUNT {
                return Err(OpenCodeRouteAnalysisError::LimitExceeded);
            }
            for (model_id, model) in models {
                validate_key(model_id)?;
                validate_model_shape(model)?;
            }
        }
    }
    Ok(model_count)
}

fn validate_model_shape(value: &Value) -> Result<(), OpenCodeRouteAnalysisError> {
    let model = value
        .as_object()
        .ok_or(OpenCodeRouteAnalysisError::InvalidSchema)?;
    if let Some(id) = optional_string(model.get("id"))? {
        if id.is_empty() || id.len() > MAX_MODEL_REFERENCE_BYTES {
            return Err(OpenCodeRouteAnalysisError::LimitExceeded);
        }
    }
    if let Some(provider) = model.get("provider") {
        let provider = provider
            .as_object()
            .ok_or(OpenCodeRouteAnalysisError::InvalidSchema)?;
        optional_string(provider.get("npm"))?;
        optional_string(provider.get("api"))?;
        if let Some(options) = provider.get("options") {
            let options = options
                .as_object()
                .ok_or(OpenCodeRouteAnalysisError::InvalidSchema)?;
            optional_string(options.get("baseURL"))?;
            optional_string(options.get("endpoint"))?;
        }
    }
    if let Some(options) = model.get("options") {
        let options = options
            .as_object()
            .ok_or(OpenCodeRouteAnalysisError::InvalidSchema)?;
        optional_string(options.get("baseURL"))?;
        optional_string(options.get("endpoint"))?;
    }
    if let Some(headers) = model.get("headers") {
        headers
            .as_object()
            .ok_or(OpenCodeRouteAnalysisError::InvalidSchema)?;
    }
    if let Some(variants) = model.get("variants") {
        let variants = variants
            .as_object()
            .ok_or(OpenCodeRouteAnalysisError::InvalidSchema)?;
        if variants.len() > MAX_VARIANTS_PER_MODEL {
            return Err(OpenCodeRouteAnalysisError::LimitExceeded);
        }
        for value in variants.values() {
            let variant = value
                .as_object()
                .ok_or(OpenCodeRouteAnalysisError::InvalidSchema)?;
            optional_string(variant.get("baseURL"))?;
            optional_string(variant.get("endpoint"))?;
            optional_string(variant.get("npm"))?;
            optional_string(variant.get("api"))?;
        }
    }
    Ok(())
}

pub(super) fn validate_key(value: &str) -> Result<(), OpenCodeRouteAnalysisError> {
    if value.is_empty() || value.len() > MAX_KEY_BYTES || value.chars().any(char::is_control) {
        Err(OpenCodeRouteAnalysisError::LimitExceeded)
    } else {
        Ok(())
    }
}

fn classify_provider(
    provider_key: &str,
    value: Option<&Value>,
    stable_endpoint: &str,
) -> Result<Option<(OpenCodeProtocol, OpenCodeRouteBefore)>, OpenCodeRouteAnalysisError> {
    if provider_key == "openai" {
        return classify_builtin_openai(value, stable_endpoint);
    }
    if RESERVED_CUSTOM_LOADERS.contains(&provider_key) {
        return Ok(None);
    }
    classify_openai_compatible(value, stable_endpoint)
}

fn classify_builtin_openai(
    value: Option<&Value>,
    stable_endpoint: &str,
) -> Result<Option<(OpenCodeProtocol, OpenCodeRouteBefore)>, OpenCodeRouteAnalysisError> {
    let Some(value) = value else {
        return Ok(Some((
            OpenCodeProtocol::Responses,
            OpenCodeRouteBefore::Missing,
        )));
    };
    let provider = value
        .as_object()
        .ok_or(OpenCodeRouteAnalysisError::InvalidSchema)?;
    let npm = optional_string(provider.get("npm"))?;
    if npm.is_some_and(|npm| npm != "@ai-sdk/openai") {
        return Ok(None);
    }
    if !public_or_same_endpoint(optional_string(provider.get("api"))?, stable_endpoint) {
        return Ok(None);
    }
    if !models_match_npm(provider, "@ai-sdk/openai", stable_endpoint)? {
        return Ok(None);
    }
    let Some(options) = optional_object(provider.get("options"))? else {
        return Ok(Some((
            OpenCodeProtocol::Responses,
            OpenCodeRouteBefore::Missing,
        )));
    };
    if !safe_headers(options.get("headers"))? || options.contains_key("endpoint") {
        return Ok(None);
    }
    if contains_transport_override(options, 0, &["baseURL", "headers"])? {
        return Ok(None);
    }
    let before = endpoint_before(options.get("baseURL"), stable_endpoint)?;
    Ok(before.map(|before| (OpenCodeProtocol::Responses, before)))
}

fn classify_openai_compatible(
    value: Option<&Value>,
    stable_endpoint: &str,
) -> Result<Option<(OpenCodeProtocol, OpenCodeRouteBefore)>, OpenCodeRouteAnalysisError> {
    let Some(value) = value else {
        return Ok(None);
    };
    let provider = value
        .as_object()
        .ok_or(OpenCodeRouteAnalysisError::InvalidSchema)?;
    let npm = optional_string(provider.get("npm"))?;
    if npm != Some("@ai-sdk/openai-compatible") {
        return Ok(None);
    }
    if !public_or_same_endpoint(optional_string(provider.get("api"))?, stable_endpoint) {
        return Ok(None);
    }
    let models = optional_object(provider.get("models"))?;
    if models.is_none_or(Map::is_empty)
        || !models_match_npm(provider, "@ai-sdk/openai-compatible", stable_endpoint)?
    {
        return Ok(None);
    }
    let Some(options) = optional_object(provider.get("options"))? else {
        return Ok(None);
    };
    if !safe_headers(options.get("headers"))? || options.contains_key("endpoint") {
        return Ok(None);
    }
    if contains_transport_override(options, 0, &["baseURL", "headers"])? {
        return Ok(None);
    }
    let Some(base_url) = options.get("baseURL") else {
        return Ok(None);
    };
    let before = endpoint_before(Some(base_url), stable_endpoint)?;
    Ok(before.map(|before| (OpenCodeProtocol::ChatCompletions, before)))
}

fn models_match_npm(
    provider: &Map<String, Value>,
    expected_npm: &str,
    stable_endpoint: &str,
) -> Result<bool, OpenCodeRouteAnalysisError> {
    let Some(models) = optional_object(provider.get("models"))? else {
        return Ok(true);
    };
    for model in models.values() {
        let model = model
            .as_object()
            .ok_or(OpenCodeRouteAnalysisError::InvalidSchema)?;
        let model_provider = optional_object(model.get("provider"))?;
        if let Some(model_provider) = model_provider {
            if optional_string(model_provider.get("npm"))?.is_some_and(|npm| npm != expected_npm)
                || !public_or_same_endpoint(
                    optional_string(model_provider.get("api"))?,
                    stable_endpoint,
                )
                || contains_transport_override(model_provider, 0, &["npm", "api"])?
            {
                return Ok(false);
            }
        }
        let options = optional_object(model.get("options"))?;
        let model_options_unsafe = if let Some(options) = options {
            options.contains_key("baseURL")
                || options.contains_key("endpoint")
                || options.contains_key("headers")
                || contains_transport_override(options, 0, &[])?
        } else {
            false
        };
        if model_options_unsafe || !safe_headers(model.get("headers"))? {
            return Ok(false);
        }
        if let Some(variants) = optional_object(model.get("variants"))? {
            for variant in variants.values() {
                let variant = variant
                    .as_object()
                    .ok_or(OpenCodeRouteAnalysisError::InvalidSchema)?;
                if contains_transport_override(variant, 0, &[])? {
                    return Ok(false);
                }
            }
        }
    }
    Ok(true)
}

fn contains_transport_override(
    value: &Map<String, Value>,
    depth: usize,
    allowed_root_keys: &[&str],
) -> Result<bool, OpenCodeRouteAnalysisError> {
    if depth > MAX_TRANSPORT_NESTING_DEPTH {
        return Err(OpenCodeRouteAnalysisError::LimitExceeded);
    }
    for (key, child) in value {
        if TRANSPORT_OVERRIDE_KEYS.contains(&key.as_str())
            && (depth > 0 || !allowed_root_keys.contains(&key.as_str()))
        {
            return Ok(true);
        }
        if value_contains_transport_override(child, depth + 1)? {
            return Ok(true);
        }
    }
    Ok(false)
}

fn value_contains_transport_override(
    value: &Value,
    depth: usize,
) -> Result<bool, OpenCodeRouteAnalysisError> {
    if depth > MAX_TRANSPORT_NESTING_DEPTH {
        return Err(OpenCodeRouteAnalysisError::LimitExceeded);
    }
    match value {
        Value::Object(object) => contains_transport_override(object, depth, &[]),
        Value::Array(items) => {
            for child in items {
                if value_contains_transport_override(child, depth + 1)? {
                    return Ok(true);
                }
            }
            Ok(false)
        }
        _ => Ok(false),
    }
}

pub(super) fn optional_object<'a>(
    value: Option<&'a Value>,
) -> Result<Option<&'a Map<String, Value>>, OpenCodeRouteAnalysisError> {
    match value {
        None => Ok(None),
        Some(Value::Object(value)) => Ok(Some(value)),
        Some(_) => Err(OpenCodeRouteAnalysisError::InvalidSchema),
    }
}

pub(super) fn optional_string<'a>(
    value: Option<&'a Value>,
) -> Result<Option<&'a str>, OpenCodeRouteAnalysisError> {
    match value {
        None => Ok(None),
        Some(Value::String(value)) => Ok(Some(value)),
        Some(_) => Err(OpenCodeRouteAnalysisError::InvalidSchema),
    }
}

fn public_or_same_endpoint(value: Option<&str>, stable_endpoint: &str) -> bool {
    value.is_none_or(|value| value == CANONICAL_OPENAI_BASE || value == stable_endpoint)
}

fn safe_headers(value: Option<&Value>) -> Result<bool, OpenCodeRouteAnalysisError> {
    match value {
        None => Ok(true),
        Some(Value::Object(headers)) => Ok(headers.is_empty()),
        Some(_) => Err(OpenCodeRouteAnalysisError::InvalidSchema),
    }
}

pub(super) fn endpoint_before(
    value: Option<&Value>,
    stable_endpoint: &str,
) -> Result<Option<OpenCodeRouteBefore>, OpenCodeRouteAnalysisError> {
    match value {
        None => Ok(Some(OpenCodeRouteBefore::Missing)),
        Some(Value::String(value)) if value == stable_endpoint => {
            Ok(Some(OpenCodeRouteBefore::SameStableEndpoint))
        }
        Some(Value::String(value)) if value == CANONICAL_OPENAI_BASE => {
            Ok(Some(OpenCodeRouteBefore::CanonicalOpenAiBase))
        }
        Some(Value::String(_)) => Ok(None),
        Some(_) => Err(OpenCodeRouteAnalysisError::InvalidSchema),
    }
}
