use super::opencode_routes::{
    endpoint_before, optional_object, optional_string, validate_key, validate_stable_endpoint,
    OpenCodeDefaultRouteCoverage, OpenCodeProtocol, OpenCodeRoute, OpenCodeRouteAnalysis,
    OpenCodeRouteAnalysisError, OpenCodeRouteBefore, OpenCodeRouteCoverage,
    OpenCodeRuntimeEffective, MAX_ELIGIBLE_ROUTES, MAX_KEY_BYTES, MAX_MODEL_COUNT,
    MAX_MODEL_REFERENCE_BYTES, MAX_PROVIDER_COUNT, MAX_TRANSPORT_NESTING_DEPTH,
    MAX_VARIANTS_PER_MODEL, RESERVED_CUSTOM_LOADERS,
};
use serde_json::{Map, Value};
use std::collections::{BTreeSet, HashSet};

const OPENAI_PACKAGE: &str = "@opencode/ai/providers/openai";
const OPENAI_COMPATIBLE_PACKAGE: &str = "@opencode/ai/providers/openai-compatible";
const MAX_POLICIES: usize = 128;
const LEGACY_ROOT_KEYS: &[&str] = &[
    "provider",
    "agent",
    "mode",
    "enabled_providers",
    "disabled_providers",
    "small_model",
    "plugin",
    "tools",
    "permission",
    "experimental",
    "command",
];
const POLICY_ACTIONS: &[&str] = &[
    "provider.use",
    "tool.use",
    "integration.connect",
    "integration.use",
    "mcp.use",
    "skill.use",
    "plugin.use",
    "websearch.use",
];
const TRANSPORT_OVERRIDE_KEYS: &[&str] = &[
    "baseURL",
    "baseUrl",
    "endpoint",
    "url",
    "fetch",
    "headers",
    "package",
    "npm",
    "api",
    "transport",
    "options",
];
const DIRECT_PROVIDER_ROUTE_KEYS: &[&str] = &[
    "baseURL",
    "baseUrl",
    "endpoint",
    "url",
    "fetch",
    "npm",
    "api",
    "transport",
    "options",
];
const DIRECT_MODEL_ROUTE_KEYS: &[&str] = &[
    "baseURL",
    "baseUrl",
    "endpoint",
    "url",
    "fetch",
    "npm",
    "api",
    "transport",
    "options",
];
const V2_RESERVED_PROVIDER_IDS: &[&str] = &[
    // Provider plug-in module stems from the pinned V2 tree. V1's loader IDs are
    // also excluded below; these names must not be treated as custom aliases.
    "amazon-bedrock",
    "azure",
    "cerebras",
    "chatgpt",
    "claude-agent",
    "cloudflare-ai-gateway",
    "cloudflare-workers-ai",
    "databricks",
    "digitalocean",
    "github-copilot",
    "gitlab",
    "google-vertex",
    "kilo",
    "llmgateway",
    "lmstudio",
    "local",
    "modal",
    "neon",
    "nvidia",
    "ollama",
    "openai-context-caps",
    "opencode-watches",
    "opencode",
    "openrouter",
    "poe",
    "prompt-cache-key",
    "sap-ai-core",
    "snowflake-cortex",
    "vllm",
    "zenmux",
];

struct ProviderFacts<'a> {
    package: Option<&'a str>,
    base_url: Option<&'a Value>,
    excluded: bool,
}

pub(crate) fn analyze_opencode_v2_routes(
    root: &Map<String, Value>,
    stable_endpoint: &str,
) -> Result<OpenCodeRouteAnalysis, OpenCodeRouteAnalysisError> {
    validate_stable_endpoint(stable_endpoint)?;
    if LEGACY_ROOT_KEYS.iter().any(|key| root.contains_key(*key)) {
        return Err(OpenCodeRouteAnalysisError::UnsupportedConfig);
    }

    let providers = optional_object(root.get("providers"))?;
    if providers.is_some_and(|providers| providers.len() > MAX_PROVIDER_COUNT) {
        return Err(OpenCodeRouteAnalysisError::LimitExceeded);
    }

    let mut candidates = BTreeSet::new();
    let default_provider = match root.get("model") {
        Some(value) => Some(add_model_selector(value, &mut candidates)?),
        None => None,
    };

    let mut model_count = 0usize;
    let mut provider_facts = std::collections::BTreeMap::new();
    if let Some(providers) = providers {
        for (provider_key, value) in providers {
            validate_key(provider_key)?;
            candidates.insert(provider_key.clone());
            let (count, facts) = inspect_provider(provider_key, value)?;
            model_count = model_count.saturating_add(count);
            if model_count > MAX_MODEL_COUNT {
                return Err(OpenCodeRouteAnalysisError::LimitExceeded);
            }
            provider_facts.insert(provider_key.clone(), facts);
        }
    }

    inspect_agents(root.get("agents"), &mut candidates)?;
    if candidates.len() > MAX_PROVIDER_COUNT {
        return Err(OpenCodeRouteAnalysisError::LimitExceeded);
    }

    validate_plugins(root.get("plugins"))?;
    let policies_present = validate_policies(root.get("policies"))?;

    let mut routes = Vec::new();
    let mut excluded_count = 0usize;
    let mut already_same_count = 0usize;
    let mut responses_count = 0usize;
    let mut chat_completions_count = 0usize;

    for provider_key in candidates {
        let facts = provider_facts.get(&provider_key);
        let Some((protocol, before)) = classify_provider(&provider_key, facts, stable_endpoint)?
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

    let configured_default_route = match default_provider {
        None => OpenCodeDefaultRouteCoverage::Unknown,
        Some(provider) if routes.iter().any(|route| route.provider_key == provider) => {
            OpenCodeDefaultRouteCoverage::Covered
        }
        Some(_) => OpenCodeDefaultRouteCoverage::Excluded,
    };

    Ok(OpenCodeRouteAnalysis {
        coverage: OpenCodeRouteCoverage {
            eligible_count: routes.len() as u8,
            excluded_count: excluded_count as u8,
            already_same_count: already_same_count as u8,
            responses_count: responses_count as u8,
            chat_completions_count: chat_completions_count as u8,
            partial: excluded_count > 0 || policies_present,
            configured_default_route,
            runtime_effective: OpenCodeRuntimeEffective::Unknown,
        },
        routes,
    })
}

fn classify_provider(
    provider_key: &str,
    facts: Option<&ProviderFacts<'_>>,
    stable_endpoint: &str,
) -> Result<Option<(OpenCodeProtocol, OpenCodeRouteBefore)>, OpenCodeRouteAnalysisError> {
    if provider_key != "openai"
        && (RESERVED_CUSTOM_LOADERS.contains(&provider_key)
            || V2_RESERVED_PROVIDER_IDS.contains(&provider_key))
    {
        return Ok(None);
    }

    let package = facts.and_then(|facts| facts.package);
    let (protocol, custom_alias) = if provider_key == "openai" {
        if package.is_some_and(|package| package != OPENAI_PACKAGE) {
            return Ok(None);
        }
        (OpenCodeProtocol::Responses, false)
    } else if package == Some(OPENAI_PACKAGE) {
        (OpenCodeProtocol::Responses, true)
    } else if package == Some(OPENAI_COMPATIBLE_PACKAGE) {
        (OpenCodeProtocol::ChatCompletions, true)
    } else {
        return Ok(None);
    };

    // Creating a provider entry changes its activation in V2.
    let Some(facts) = facts else {
        return Ok(None);
    };
    if facts.excluded {
        return Ok(None);
    }

    let before = endpoint_before(facts.base_url, stable_endpoint)?;
    if custom_alias && before == Some(OpenCodeRouteBefore::Missing) {
        return Ok(None);
    }
    Ok(before.map(|before| (protocol, before)))
}

fn inspect_provider<'a>(
    provider_key: &str,
    value: &'a Value,
) -> Result<(usize, ProviderFacts<'a>), OpenCodeRouteAnalysisError> {
    let provider = value
        .as_object()
        .ok_or(OpenCodeRouteAnalysisError::InvalidSchema)?;

    optional_string(provider.get("name"))?;
    if let Some(env) = provider.get("env") {
        let env = env
            .as_array()
            .ok_or(OpenCodeRouteAnalysisError::InvalidSchema)?;
        if env.len() > MAX_PROVIDER_COUNT {
            return Err(OpenCodeRouteAnalysisError::LimitExceeded);
        }
        for name in env {
            let name = name
                .as_str()
                .ok_or(OpenCodeRouteAnalysisError::InvalidSchema)?;
            validate_key(name)?;
        }
    }

    let package = optional_string(provider.get("package"))?;
    if package.is_some_and(|package| package.len() > MAX_MODEL_REFERENCE_BYTES) {
        return Err(OpenCodeRouteAnalysisError::LimitExceeded);
    }

    let mut excluded = false;
    if let Some(canonical) = optional_string(provider.get("canonical"))? {
        if canonical.len() > MAX_KEY_BYTES {
            return Err(OpenCodeRouteAnalysisError::LimitExceeded);
        }
        excluded = true;
    }
    if provider.contains_key("compatibility") {
        optional_object(provider.get("compatibility"))?;
        excluded = true;
    }

    let headers_empty = validate_string_record(provider.get("headers"))?;
    if !headers_empty {
        excluded = true;
    }
    if provider.contains_key("body") {
        optional_object(provider.get("body"))?;
        excluded = true;
    }
    if DIRECT_PROVIDER_ROUTE_KEYS
        .iter()
        .any(|key| provider.contains_key(*key))
    {
        excluded = true;
    }

    let settings = optional_object(provider.get("settings"))?;
    let base_url = settings.and_then(|settings| settings.get("baseURL"));
    if let Some(base_url) = base_url {
        if base_url.as_str().is_none() {
            return Err(OpenCodeRouteAnalysisError::InvalidSchema);
        }
    }
    if let Some(settings_value) = provider.get("settings") {
        if has_transport_override(settings_value, 0, &["baseURL"])? {
            excluded = true;
        }
    }

    let models = optional_object(provider.get("models"))?;
    let model_count = models.map_or(0, Map::len);
    if model_count > MAX_MODEL_COUNT {
        return Err(OpenCodeRouteAnalysisError::LimitExceeded);
    }
    let effective_package = package.or((provider_key == "openai").then_some(OPENAI_PACKAGE));
    if let Some(models) = models {
        for (model_id, model) in models {
            validate_key(model_id)?;
            if !inspect_model(model, effective_package)? {
                excluded = true;
            }
        }
    }

    Ok((
        model_count,
        ProviderFacts {
            package,
            base_url,
            excluded,
        },
    ))
}

fn inspect_model(
    value: &Value,
    provider_package: Option<&str>,
) -> Result<bool, OpenCodeRouteAnalysisError> {
    let model = value
        .as_object()
        .ok_or(OpenCodeRouteAnalysisError::InvalidSchema)?;
    let mut safe = true;

    if let Some(model_id) = optional_string(model.get("modelID"))? {
        validate_nonempty_bounded(model_id, MAX_MODEL_REFERENCE_BYTES)?;
    }
    if let Some(model_type) = optional_string(model.get("type"))? {
        if model_type != "chat" {
            safe = false;
        }
    }
    if let Some(package) = optional_string(model.get("package"))? {
        if package.len() > MAX_MODEL_REFERENCE_BYTES {
            return Err(OpenCodeRouteAnalysisError::LimitExceeded);
        }
        if provider_package != Some(package) {
            safe = false;
        }
    }
    if model.contains_key("compatibility") {
        optional_object(model.get("compatibility"))?;
        safe = false;
    }
    if DIRECT_MODEL_ROUTE_KEYS
        .iter()
        .any(|key| model.contains_key(*key))
    {
        safe = false;
    }

    if !validate_string_record(model.get("headers"))? {
        safe = false;
    }
    if model.contains_key("body") {
        optional_object(model.get("body"))?;
        safe = false;
    }
    if let Some(settings_value) = model.get("settings") {
        optional_object(Some(settings_value))?;
        if has_transport_override(settings_value, 0, &[])? {
            safe = false;
        }
    }

    if let Some(variants) = model.get("variants") {
        let variants = variants
            .as_array()
            .ok_or(OpenCodeRouteAnalysisError::InvalidSchema)?;
        if variants.len() > MAX_VARIANTS_PER_MODEL {
            return Err(OpenCodeRouteAnalysisError::LimitExceeded);
        }
        let mut variant_ids = HashSet::with_capacity(variants.len());
        for value in variants {
            let variant = value
                .as_object()
                .ok_or(OpenCodeRouteAnalysisError::InvalidSchema)?;
            let variant_id = optional_string(variant.get("id"))?
                .ok_or(OpenCodeRouteAnalysisError::InvalidSchema)?;
            validate_nonempty_bounded(variant_id, MAX_KEY_BYTES)?;
            if !variant_ids.insert(variant_id) {
                return Err(OpenCodeRouteAnalysisError::InvalidSchema);
            }
            if DIRECT_MODEL_ROUTE_KEYS
                .iter()
                .any(|key| variant.contains_key(*key))
                || variant.contains_key("package")
            {
                safe = false;
            }
            if !validate_string_record(variant.get("headers"))? {
                safe = false;
            }
            if variant.contains_key("body") {
                optional_object(variant.get("body"))?;
                safe = false;
            }
            if let Some(settings_value) = variant.get("settings") {
                optional_object(Some(settings_value))?;
                if has_transport_override(settings_value, 0, &[])? {
                    safe = false;
                }
            }
        }
    }

    Ok(safe)
}

fn inspect_agents(
    value: Option<&Value>,
    candidates: &mut BTreeSet<String>,
) -> Result<(), OpenCodeRouteAnalysisError> {
    let Some(agents) = optional_object(value)? else {
        return Ok(());
    };
    if agents.len() > MAX_PROVIDER_COUNT {
        return Err(OpenCodeRouteAnalysisError::LimitExceeded);
    }
    for (name, value) in agents {
        validate_key(name)?;
        let agent = value
            .as_object()
            .ok_or(OpenCodeRouteAnalysisError::InvalidSchema)?;
        if let Some(model) = agent.get("model") {
            add_model_selector(model, candidates)?;
        }
        validate_agent_request(agent.get("request"))?;
    }
    Ok(())
}

fn validate_agent_request(value: Option<&Value>) -> Result<(), OpenCodeRouteAnalysisError> {
    let Some(value) = value else {
        return Ok(());
    };
    let request = value
        .as_object()
        .ok_or(OpenCodeRouteAnalysisError::InvalidSchema)?;
    let headers_empty = validate_string_record(request.get("headers"))?;
    let body = optional_object(request.get("body"))?;
    if !headers_empty || body.is_some_and(|body| !body.is_empty()) {
        return Err(OpenCodeRouteAnalysisError::UnsupportedConfig);
    }
    if has_transport_override(value, 0, &["headers", "body"])? {
        return Err(OpenCodeRouteAnalysisError::UnsupportedConfig);
    }
    Ok(())
}

fn add_model_selector(
    value: &Value,
    candidates: &mut BTreeSet<String>,
) -> Result<String, OpenCodeRouteAnalysisError> {
    let (provider, model, variant, encoded_length) = match value {
        Value::String(reference) => {
            if reference.len() > MAX_MODEL_REFERENCE_BYTES {
                return Err(OpenCodeRouteAnalysisError::LimitExceeded);
            }
            let Some((provider, model_ref)) = reference.split_once('/') else {
                return Err(OpenCodeRouteAnalysisError::InvalidSchema);
            };
            let (model, variant) = match model_ref.split_once('#') {
                Some((model, variant)) => (model, Some(variant)),
                None => (model_ref, None),
            };
            if variant.is_some_and(|variant| variant.contains('#')) {
                return Err(OpenCodeRouteAnalysisError::InvalidSchema);
            }
            (provider, model, variant, reference.len())
        }
        Value::Object(selection) => {
            if selection
                .keys()
                .any(|key| !["providerID", "model", "variant"].contains(&key.as_str()))
            {
                return Err(OpenCodeRouteAnalysisError::InvalidSchema);
            }
            let provider = optional_string(selection.get("providerID"))?
                .ok_or(OpenCodeRouteAnalysisError::InvalidSchema)?;
            let model = optional_string(selection.get("model"))?
                .ok_or(OpenCodeRouteAnalysisError::InvalidSchema)?;
            let variant = optional_string(selection.get("variant"))?;
            let encoded_length = provider.len()
                + model.len()
                + variant.map_or(0, str::len)
                + 1
                + usize::from(variant.is_some());
            (provider, model, variant, encoded_length)
        }
        _ => return Err(OpenCodeRouteAnalysisError::InvalidSchema),
    };

    if encoded_length > MAX_MODEL_REFERENCE_BYTES {
        return Err(OpenCodeRouteAnalysisError::LimitExceeded);
    }
    validate_key(provider)?;
    validate_nonempty_bounded(model, MAX_MODEL_REFERENCE_BYTES)?;
    if provider.contains('/') || provider.contains('#') || model.contains('#') {
        return Err(OpenCodeRouteAnalysisError::InvalidSchema);
    }
    if let Some(variant) = variant {
        validate_nonempty_bounded(variant, MAX_MODEL_REFERENCE_BYTES)?;
        if variant.contains('#') {
            return Err(OpenCodeRouteAnalysisError::InvalidSchema);
        }
    }
    candidates.insert(provider.to_owned());
    if candidates.len() > MAX_PROVIDER_COUNT {
        return Err(OpenCodeRouteAnalysisError::LimitExceeded);
    }
    Ok(provider.to_owned())
}

fn validate_plugins(value: Option<&Value>) -> Result<(), OpenCodeRouteAnalysisError> {
    let Some(value) = value else {
        return Ok(());
    };
    let plugins = value
        .as_array()
        .ok_or(OpenCodeRouteAnalysisError::InvalidSchema)?;
    if plugins.is_empty() {
        Ok(())
    } else {
        Err(OpenCodeRouteAnalysisError::UnsupportedConfig)
    }
}

fn validate_policies(value: Option<&Value>) -> Result<bool, OpenCodeRouteAnalysisError> {
    let Some(value) = value else {
        return Ok(false);
    };
    let policies = value
        .as_array()
        .ok_or(OpenCodeRouteAnalysisError::InvalidSchema)?;
    if policies.len() > MAX_POLICIES {
        return Err(OpenCodeRouteAnalysisError::LimitExceeded);
    }
    for value in policies {
        let policy = value
            .as_object()
            .ok_or(OpenCodeRouteAnalysisError::InvalidSchema)?;
        let action = optional_string(policy.get("action"))?
            .ok_or(OpenCodeRouteAnalysisError::InvalidSchema)?;
        let resource = optional_string(policy.get("resource"))?
            .ok_or(OpenCodeRouteAnalysisError::InvalidSchema)?;
        let effect = optional_string(policy.get("effect"))?
            .ok_or(OpenCodeRouteAnalysisError::InvalidSchema)?;
        if !POLICY_ACTIONS.contains(&action) || !["allow", "deny"].contains(&effect) {
            return Err(OpenCodeRouteAnalysisError::InvalidSchema);
        }
        if resource.len() > MAX_MODEL_REFERENCE_BYTES {
            return Err(OpenCodeRouteAnalysisError::LimitExceeded);
        }
    }
    Ok(!policies.is_empty())
}

fn validate_string_record(value: Option<&Value>) -> Result<bool, OpenCodeRouteAnalysisError> {
    let Some(value) = value else {
        return Ok(true);
    };
    let record = value
        .as_object()
        .ok_or(OpenCodeRouteAnalysisError::InvalidSchema)?;
    if record.len() > MAX_MODEL_COUNT {
        return Err(OpenCodeRouteAnalysisError::LimitExceeded);
    }
    for (key, value) in record {
        validate_key(key)?;
        if value.as_str().is_none() {
            return Err(OpenCodeRouteAnalysisError::InvalidSchema);
        }
    }
    Ok(record.is_empty())
}

fn validate_nonempty_bounded(
    value: &str,
    maximum: usize,
) -> Result<(), OpenCodeRouteAnalysisError> {
    if value.is_empty() {
        return Err(OpenCodeRouteAnalysisError::InvalidSchema);
    }
    if value.len() > maximum || value.chars().any(char::is_control) {
        return Err(OpenCodeRouteAnalysisError::LimitExceeded);
    }
    Ok(())
}

fn has_transport_override(
    value: &Value,
    depth: usize,
    allowed_root_keys: &[&str],
) -> Result<bool, OpenCodeRouteAnalysisError> {
    if depth > MAX_TRANSPORT_NESTING_DEPTH {
        return Err(OpenCodeRouteAnalysisError::LimitExceeded);
    }
    let mut found = false;
    match value {
        Value::Object(object) => {
            for (key, child) in object {
                if TRANSPORT_OVERRIDE_KEYS.contains(&key.as_str())
                    && (depth > 0 || !allowed_root_keys.contains(&key.as_str()))
                {
                    found = true;
                }
                if has_transport_override(child, depth + 1, &[])? {
                    found = true;
                }
            }
        }
        Value::Array(items) => {
            for child in items {
                if has_transport_override(child, depth + 1, &[])? {
                    found = true;
                }
            }
        }
        _ => {}
    }
    Ok(found)
}
