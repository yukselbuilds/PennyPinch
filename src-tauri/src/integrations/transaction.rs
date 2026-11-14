use serde::de::{MapAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use sha2::{Digest, Sha256};
use std::fmt;
use std::ops::Range;

use super::config_document;
use super::opencode_retrieval::{self, RetrievalChange, RetrievalReceipt};
use super::plan::{
    ClientKind, ConfigPresence, ConfigSource, ConnectionPlan, EndpointBefore, EndpointField,
    PlanStatus,
};

const ENDPOINT_SUFFIX_CODEX: &str = "/v1";
const ENDPOINT_SUFFIX_CLAUDE: &str = "";
const ENDPOINT_SUFFIX_OPENCODE: &str = "/v1";
const CANONICAL_OPENAI_BASE: &str = "https://api.openai.com/v1";
const MAX_OPENCODE_ROUTE_RECEIPTS: usize = 16;
const MAX_CANONICAL_URL_LITERAL_BYTES: usize = 512;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct OpenCodeRouteReceipt {
    pub(crate) provider_key_sha256: String,
    pub(crate) original_base_url_literal: Option<String>,
    pub(crate) created_ancestor_mask: u8,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PatchReceipt {
    client_kind: ClientKind,
    config_source: ConfigSource,
    endpoint_port: u16,
    created_ancestor_mask: u8,
    config_was_missing: bool,
    opencode_routes: Option<Vec<OpenCodeRouteReceipt>>,
    opencode_retrieval: Option<RetrievalReceipt>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PatchReceiptMetadata {
    pub(crate) client_kind: ClientKind,
    pub(crate) config_source: ConfigSource,
    pub(crate) endpoint_port: u16,
    pub(crate) created_ancestor_mask: u8,
    pub(crate) config_was_missing: bool,
    pub(crate) opencode_routes: Option<Vec<OpenCodeRouteReceipt>>,
    pub(crate) opencode_retrieval: Option<RetrievalReceipt>,
}

impl PatchReceipt {
    pub(crate) fn metadata(&self) -> PatchReceiptMetadata {
        PatchReceiptMetadata {
            client_kind: self.client_kind,
            config_source: self.config_source,
            endpoint_port: self.endpoint_port,
            created_ancestor_mask: self.created_ancestor_mask,
            config_was_missing: self.config_was_missing,
            opencode_routes: self.opencode_routes.clone(),
            opencode_retrieval: self.opencode_retrieval.clone(),
        }
    }

    pub(crate) fn from_metadata(metadata: PatchReceiptMetadata) -> Result<Self, TransactionError> {
        if !valid_receipt_metadata(&metadata) {
            return Err(TransactionError::InvalidReceipt);
        }
        Ok(Self {
            client_kind: metadata.client_kind,
            config_source: metadata.config_source,
            endpoint_port: metadata.endpoint_port,
            created_ancestor_mask: metadata.created_ancestor_mask,
            config_was_missing: metadata.config_was_missing,
            opencode_routes: metadata.opencode_routes,
            opencode_retrieval: metadata.opencode_retrieval,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ApplyResult {
    Unchanged,
    Patched {
        bytes: Vec<u8>,
        receipt: PatchReceipt,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TransactionError {
    InvalidPlan,
    InvalidConfig,
    InvalidReceipt,
    Conflict,
}

pub(crate) fn apply_missing_endpoint(
    plan: &ConnectionPlan,
    current: &[u8],
) -> Result<ApplyResult, TransactionError> {
    let Some(command) = &plan.opencode_retrieval_command else {
        if plan.opencode_retrieval.is_some() {
            return Err(TransactionError::InvalidPlan);
        }
        return apply_endpoint_only(plan, current);
    };
    if plan.client_kind != "opencode_v1"
        || plan.opencode_retrieval != Some(RetrievalChange::InstallLocalBridge)
    {
        return Err(TransactionError::InvalidPlan);
    }
    let parsed = config_document::parse_document(ClientKind::OpenCodeV1, current)
        .map_err(|_| TransactionError::InvalidConfig)?;
    if !opencode_retrieval::can_install(&parsed) {
        return Err(TransactionError::Conflict);
    }
    let (bytes, mut receipt) = match apply_endpoint_only(plan, current)? {
        ApplyResult::Patched { bytes, receipt } => (bytes, receipt),
        ApplyResult::Unchanged => (
            current.to_vec(),
            PatchReceipt {
                client_kind: ClientKind::OpenCodeV1,
                config_source: plan.config_source,
                endpoint_port: plan.endpoint_port,
                created_ancestor_mask: 0,
                config_was_missing: false,
                opencode_routes: Some(Vec::new()),
                opencode_retrieval: None,
            },
        ),
    };
    let definition = command.definition();
    let literal = serde_json::to_string(&definition).map_err(|_| TransactionError::InvalidPlan)?;
    let (bytes, mask) = patch_json_path_literal(
        ClientKind::OpenCodeV1,
        &bytes,
        &["mcp"],
        "headroom",
        &literal,
    )?;
    receipt.opencode_retrieval = Some(RetrievalReceipt {
        definition_sha256: opencode_retrieval::definition_fingerprint(
            &definition,
            literal.as_bytes(),
        )
        .ok_or(TransactionError::InvalidPlan)?,
        created_mcp_object: mask == 1,
    });
    let receipt = PatchReceipt::from_metadata(receipt.metadata())?;
    config_document::parse_document(ClientKind::OpenCodeV1, &bytes)
        .map_err(|_| TransactionError::InvalidConfig)?;
    Ok(ApplyResult::Patched { bytes, receipt })
}

fn apply_endpoint_only(
    plan: &ConnectionPlan,
    current: &[u8],
) -> Result<ApplyResult, TransactionError> {
    validate_plan(plan)?;
    let kind = client_kind(plan.client_kind).ok_or(TransactionError::InvalidPlan)?;
    let expected = endpoint_value(kind, plan.endpoint_port);
    if plan.endpoint_host != "127.0.0.1"
        || plan.endpoint_port == 0
        || plan.endpoint_field.field != endpoint_field(kind)
        || !plan.endpoint_field.after_headroom
        || plan.endpoint_field.proposed_value != expected
    {
        return Err(TransactionError::InvalidPlan);
    }
    if plan.config_presence == ConfigPresence::Missing && current != empty_document(kind) {
        return Err(TransactionError::Conflict);
    }

    let parsed = config_document::parse_document(kind, current)
        .map_err(|_| TransactionError::InvalidConfig)?;
    if kind == ClientKind::OpenCodeV1 {
        return apply_opencode_routes(plan, current, &expected, &parsed);
    }
    let actual = endpoint_value_in(&parsed, kind)?;
    if actual == Some(expected.as_str()) {
        return Ok(ApplyResult::Unchanged);
    }
    if actual.is_some() || plan.endpoint_field.before != EndpointBefore::Missing {
        return Err(TransactionError::Conflict);
    }

    let mut mask = 0u8;
    let bytes = match kind {
        ClientKind::Codex => patch_codex(current, &expected)?,
        ClientKind::ClaudeCode | ClientKind::OpenCodeV1 => {
            let (bytes, created) = patch_json(kind, current, &expected)?;
            mask = created;
            bytes
        }
    };
    config_document::parse_document(kind, &bytes).map_err(|_| TransactionError::InvalidConfig)?;
    Ok(ApplyResult::Patched {
        bytes,
        receipt: PatchReceipt::from_metadata(PatchReceiptMetadata {
            client_kind: kind,
            config_source: plan.config_source,
            endpoint_port: plan.endpoint_port,
            created_ancestor_mask: mask,
            config_was_missing: plan.config_presence == ConfigPresence::Missing,
            opencode_routes: None,
            opencode_retrieval: None,
        })?,
    })
}

pub(crate) fn undo_owned_endpoint(
    receipt: &PatchReceipt,
    current: &[u8],
) -> Result<Vec<u8>, TransactionError> {
    let Some(bridge) = &receipt.opencode_retrieval else {
        return undo_endpoint_only(receipt, current);
    };
    if !valid_receipt_metadata(&receipt.metadata()) {
        return Err(TransactionError::InvalidReceipt);
    }
    let parsed = config_document::parse_document(ClientKind::OpenCodeV1, current)
        .map_err(|_| TransactionError::InvalidConfig)?;
    let object = json_object_at(ClientKind::OpenCodeV1, current, &["mcp", "headroom"])?
        .ok_or(TransactionError::Conflict)?;
    if parsed
        .get("mcp")
        .and_then(|mcp| mcp.get("headroom"))
        .and_then(|value| {
            opencode_retrieval::definition_fingerprint(
                value,
                &current[object.range.start..object.range.end],
            )
        })
        .as_ref()
        != Some(&bridge.definition_sha256)
    {
        return Err(TransactionError::Conflict);
    }
    let mut bytes = remove_json_property(ClientKind::OpenCodeV1, current, &["mcp"], "headroom")?;
    if bridge.created_mcp_object {
        let object = json_object_at(ClientKind::OpenCodeV1, &bytes, &["mcp"])?
            .ok_or(TransactionError::Conflict)?;
        if object_is_empty_and_comment_free(&bytes, &object) {
            bytes = remove_json_property(ClientKind::OpenCodeV1, &bytes, &[], "mcp")?;
        }
    }
    // Both transformations stay in memory until the existing file transaction commits.
    undo_endpoint_only(receipt, &bytes)
}

fn undo_endpoint_only(receipt: &PatchReceipt, current: &[u8]) -> Result<Vec<u8>, TransactionError> {
    if let Some(routes) = &receipt.opencode_routes {
        if receipt.client_kind != ClientKind::OpenCodeV1 {
            return Err(TransactionError::InvalidReceipt);
        }
        return undo_opencode_routes(receipt, routes, current);
    }
    let expected = endpoint_value(receipt.client_kind, receipt.endpoint_port);
    let parsed = config_document::parse_document(receipt.client_kind, current)
        .map_err(|_| TransactionError::InvalidConfig)?;
    if endpoint_value_in(&parsed, receipt.client_kind)? != Some(expected.as_str()) {
        return Err(TransactionError::Conflict);
    }

    let result = match receipt.client_kind {
        ClientKind::Codex => remove_codex_assignment(current, &expected)?,
        ClientKind::ClaudeCode | ClientKind::OpenCodeV1 => {
            let (ancestors, leaf) = json_path(receipt.client_kind);
            let mut bytes = remove_json_property(receipt.client_kind, current, &ancestors, leaf)?;
            for index in (0..ancestors.len()).rev() {
                if receipt.created_ancestor_mask & (1 << index) == 0 {
                    continue;
                }
                let object_path = &ancestors[..=index];
                let Some(object) = json_object_at(receipt.client_kind, &bytes, object_path)? else {
                    return Err(TransactionError::Conflict);
                };
                if !object_is_empty_and_comment_free(&bytes, &object) {
                    break;
                }
                bytes = remove_json_property(
                    receipt.client_kind,
                    &bytes,
                    &ancestors[..index],
                    ancestors[index],
                )?;
            }
            bytes
        }
    };
    config_document::parse_document(receipt.client_kind, &result)
        .map_err(|_| TransactionError::InvalidConfig)?;
    Ok(result)
}

fn apply_opencode_routes(
    plan: &ConnectionPlan,
    current: &[u8],
    expected: &str,
    parsed: &serde_json::Value,
) -> Result<ApplyResult, TransactionError> {
    use super::opencode_routes::{analyze_opencode_v1_routes, OpenCodeRouteBefore};

    let planned = plan
        .opencode_routes
        .as_ref()
        .ok_or(TransactionError::InvalidPlan)?;
    if plan.config_presence != ConfigPresence::Present
        || plan.endpoint_field.before != EndpointBefore::ProviderRoutes
        || plan.opencode_coverage != Some(planned.coverage)
        || planned.routes.is_empty()
        || planned.routes.len() > MAX_OPENCODE_ROUTE_RECEIPTS
    {
        return Err(TransactionError::InvalidPlan);
    }
    let root = parsed.as_object().ok_or(TransactionError::InvalidConfig)?;
    let current_analysis =
        analyze_opencode_v1_routes(root, expected).map_err(|_| TransactionError::Conflict)?;
    if current_analysis != *planned || plan.opencode_coverage != Some(current_analysis.coverage) {
        return Err(TransactionError::Conflict);
    }
    if planned
        .routes
        .iter()
        .all(|route| route.before == OpenCodeRouteBefore::SameStableEndpoint)
    {
        return Ok(ApplyResult::Unchanged);
    }

    let mut output = current.to_vec();
    let mut receipts = Vec::with_capacity(planned.routes.len());
    for route in &planned.routes {
        let ancestors = ["provider", route.provider_key.as_str(), "options"];
        let value =
            json_string_value_and_token(ClientKind::OpenCodeV1, &output, &ancestors, "baseURL")?;
        let (next, original_literal, mask) = match route.before {
            OpenCodeRouteBefore::SameStableEndpoint => {
                if value.as_ref().map(|(decoded, _)| decoded.as_str()) != Some(expected) {
                    return Err(TransactionError::Conflict);
                }
                continue;
            }
            OpenCodeRouteBefore::Missing => {
                if value.is_some() {
                    return Err(TransactionError::Conflict);
                }
                let (bytes, mask) = patch_json_path(
                    ClientKind::OpenCodeV1,
                    &output,
                    &ancestors,
                    "baseURL",
                    expected,
                )?;
                (bytes, None, mask)
            }
            OpenCodeRouteBefore::CanonicalOpenAiBase => {
                let (decoded, literal) = value.ok_or(TransactionError::Conflict)?;
                if decoded != CANONICAL_OPENAI_BASE
                    || literal.len() > MAX_CANONICAL_URL_LITERAL_BYTES
                {
                    return Err(TransactionError::Conflict);
                }
                let bytes = replace_json_string_value(
                    ClientKind::OpenCodeV1,
                    &output,
                    &ancestors,
                    "baseURL",
                    expected,
                )?;
                (bytes, Some(literal), 0)
            }
        };
        output = next;
        receipts.push(OpenCodeRouteReceipt {
            provider_key_sha256: provider_key_digest(&route.provider_key),
            original_base_url_literal: original_literal,
            created_ancestor_mask: mask,
        });
    }
    if receipts.is_empty() {
        return Ok(ApplyResult::Unchanged);
    }

    let updated = config_document::parse_document(ClientKind::OpenCodeV1, &output)
        .map_err(|_| TransactionError::InvalidConfig)?;
    let updated_root = updated.as_object().ok_or(TransactionError::InvalidConfig)?;
    let updated_analysis = analyze_opencode_v1_routes(updated_root, expected)
        .map_err(|_| TransactionError::InvalidConfig)?;
    if updated_analysis.routes.len() != planned.routes.len()
        || updated_analysis
            .routes
            .iter()
            .zip(&planned.routes)
            .any(|(updated, planned)| {
                updated.provider_key != planned.provider_key
                    || updated.protocol != planned.protocol
                    || updated.before != OpenCodeRouteBefore::SameStableEndpoint
            })
    {
        return Err(TransactionError::InvalidConfig);
    }

    Ok(ApplyResult::Patched {
        bytes: output,
        receipt: PatchReceipt::from_metadata(PatchReceiptMetadata {
            client_kind: ClientKind::OpenCodeV1,
            config_source: plan.config_source,
            endpoint_port: plan.endpoint_port,
            created_ancestor_mask: 0,
            config_was_missing: false,
            opencode_routes: Some(receipts),
            opencode_retrieval: None,
        })?,
    })
}

fn undo_opencode_routes(
    receipt: &PatchReceipt,
    routes: &[OpenCodeRouteReceipt],
    current: &[u8],
) -> Result<Vec<u8>, TransactionError> {
    if receipt.client_kind != ClientKind::OpenCodeV1
        || receipt.created_ancestor_mask != 0
        || !(valid_opencode_route_receipts(routes)
            || routes.is_empty() && receipt.opencode_retrieval.is_some())
    {
        return Err(TransactionError::InvalidReceipt);
    }
    let expected = endpoint_value(ClientKind::OpenCodeV1, receipt.endpoint_port);
    let parsed = config_document::parse_document(ClientKind::OpenCodeV1, current)
        .map_err(|_| TransactionError::InvalidConfig)?;
    let provider_map = parsed
        .get("provider")
        .and_then(serde_json::Value::as_object)
        .ok_or(TransactionError::Conflict)?;
    if provider_map.len() > 32 {
        return Err(TransactionError::Conflict);
    }

    // Resolve every digest and verify every owned endpoint before transforming
    // any bytes. A missing, ambiguous, or user-changed route aborts the whole undo.
    let mut resolved = Vec::with_capacity(routes.len());
    for route in routes {
        let mut matching = provider_map
            .keys()
            .filter(|key| provider_key_digest(key) == route.provider_key_sha256);
        let key = matching.next().ok_or(TransactionError::Conflict)?.clone();
        if matching.next().is_some() {
            return Err(TransactionError::Conflict);
        }
        let ancestors = ["provider", key.as_str(), "options"];
        let value =
            json_string_value_and_token(ClientKind::OpenCodeV1, current, &ancestors, "baseURL")?;
        if value.as_ref().map(|(decoded, _)| decoded.as_str()) != Some(expected.as_str()) {
            return Err(TransactionError::Conflict);
        }
        resolved.push((route, key));
    }

    let mut output = current.to_vec();
    for (route, key) in resolved.iter().rev() {
        let ancestors = ["provider", key.as_str(), "options"];
        if let Some(original_literal) = &route.original_base_url_literal {
            output = replace_json_string_token(
                ClientKind::OpenCodeV1,
                &output,
                &ancestors,
                "baseURL",
                original_literal,
            )?;
        } else {
            output = remove_json_property(ClientKind::OpenCodeV1, &output, &ancestors, "baseURL")?;
            for index in (0..ancestors.len()).rev() {
                if route.created_ancestor_mask & (1 << index) == 0 {
                    continue;
                }
                let object_path = &ancestors[..=index];
                let Some(object) = json_object_at(ClientKind::OpenCodeV1, &output, object_path)?
                else {
                    return Err(TransactionError::Conflict);
                };
                if !object_is_empty_and_comment_free(&output, &object) {
                    break;
                }
                output = remove_json_property(
                    ClientKind::OpenCodeV1,
                    &output,
                    &ancestors[..index],
                    ancestors[index],
                )?;
            }
        }
    }

    config_document::parse_document(ClientKind::OpenCodeV1, &output)
        .map_err(|_| TransactionError::InvalidConfig)?;
    for (route, key) in resolved {
        let ancestors = ["provider", key.as_str(), "options"];
        let restored =
            json_string_value_and_token(ClientKind::OpenCodeV1, &output, &ancestors, "baseURL")?;
        match &route.original_base_url_literal {
            Some(literal) => {
                if restored
                    .as_ref()
                    .map(|(decoded, token)| decoded == CANONICAL_OPENAI_BASE && token == literal)
                    != Some(true)
                {
                    return Err(TransactionError::InvalidConfig);
                }
            }
            None if restored.is_none() => {}
            None => return Err(TransactionError::InvalidConfig),
        }
    }
    Ok(output)
}

fn provider_key_digest(provider_key: &str) -> String {
    let mut digest = Sha256::new();
    digest.update(b"headroom-opencode-provider-key/v1\0");
    digest.update((provider_key.len() as u64).to_le_bytes());
    digest.update(provider_key.as_bytes());
    digest
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn valid_opencode_route_receipts(routes: &[OpenCodeRouteReceipt]) -> bool {
    use std::collections::HashSet;

    if routes.is_empty() || routes.len() > MAX_OPENCODE_ROUTE_RECEIPTS {
        return false;
    }
    let mut digests = HashSet::with_capacity(routes.len());
    routes.iter().all(|route| {
        let mask_valid = matches!(route.created_ancestor_mask, 0 | 4 | 6 | 7);
        let literal_valid = route
            .original_base_url_literal
            .as_ref()
            .is_none_or(|literal| {
                literal.len() <= MAX_CANONICAL_URL_LITERAL_BYTES
                    && serde_json::from_str::<String>(literal).ok().as_deref()
                        == Some(CANONICAL_OPENAI_BASE)
                    && route.created_ancestor_mask == 0
            });
        valid_sha256(&route.provider_key_sha256)
            && digests.insert(route.provider_key_sha256.as_str())
            && mask_valid
            && literal_valid
    })
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn json_string_value_and_token(
    kind: ClientKind,
    current: &[u8],
    parent_path: &[&str],
    key: &str,
) -> Result<Option<(String, String)>, TransactionError> {
    let text = std::str::from_utf8(current).map_err(|_| TransactionError::InvalidConfig)?;
    let root = parse_json_ast(kind, text)?;
    let Some(parent) = object_ref_at(&root, parent_path) else {
        return Ok(None);
    };
    let Some(property) = parent.get(key) else {
        return Ok(None);
    };
    let Some(literal) = property.value.as_string_lit() else {
        return Err(TransactionError::Conflict);
    };
    let token = text
        .get(literal.range.start..literal.range.end)
        .ok_or(TransactionError::InvalidConfig)?;
    Ok(Some((literal.value.to_string(), token.to_owned())))
}

fn replace_json_string_value(
    kind: ClientKind,
    current: &[u8],
    parent_path: &[&str],
    key: &str,
    replacement: &str,
) -> Result<Vec<u8>, TransactionError> {
    let token = serde_json::to_string(replacement).map_err(|_| TransactionError::InvalidConfig)?;
    replace_json_string_token(kind, current, parent_path, key, &token)
}

fn replace_json_string_token(
    kind: ClientKind,
    current: &[u8],
    parent_path: &[&str],
    key: &str,
    token: &str,
) -> Result<Vec<u8>, TransactionError> {
    let text = std::str::from_utf8(current).map_err(|_| TransactionError::InvalidConfig)?;
    let root = parse_json_ast(kind, text)?;
    let parent = object_ref_at(&root, parent_path).ok_or(TransactionError::Conflict)?;
    let property = parent.get(key).ok_or(TransactionError::Conflict)?;
    let literal = property
        .value
        .as_string_lit()
        .ok_or(TransactionError::Conflict)?;
    let range = literal.range.start..literal.range.end;
    if range.end > text.len()
        || !text.is_char_boundary(range.start)
        || !text.is_char_boundary(range.end)
    {
        return Err(TransactionError::InvalidConfig);
    }
    let mut output = Vec::with_capacity(current.len() + token.len());
    output.extend_from_slice(text[..range.start].as_bytes());
    output.extend_from_slice(token.as_bytes());
    output.extend_from_slice(text[range.end..].as_bytes());
    Ok(output)
}

fn validate_plan(plan: &ConnectionPlan) -> Result<(), TransactionError> {
    if plan.schema_version != 1
        || plan.status != PlanStatus::Preview
        || !plan.blockers.is_empty()
        || !plan.restart_required
        || plan.scan_generation == 0
        || plan.scan_generation > 9_007_199_254_740_991
        || plan.expires_at_unix_seconds > 8_640_000_000_000_000
        || plan.config_fingerprint.len() != 64
        || !plan
            .config_fingerprint
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(TransactionError::InvalidPlan);
    }
    let kind = client_kind(plan.client_kind).ok_or(TransactionError::InvalidPlan)?;
    let scope_matches_client = matches!(
        (kind, plan.scope),
        (ClientKind::Codex, "user_config")
            | (
                ClientKind::ClaudeCode,
                "user_config" | "project_local_config"
            )
            | (ClientKind::OpenCodeV1, "user_config")
    );
    if !scope_matches_client {
        return Err(TransactionError::InvalidPlan);
    }
    let source_presence_matches = match (kind, plan.config_source, plan.config_presence) {
        (ClientKind::Codex, ConfigSource::CodexToml, _) => true,
        (ClientKind::ClaudeCode, ConfigSource::ClaudeJson, _) => true,
        (ClientKind::OpenCodeV1, ConfigSource::OpenCodeJson, ConfigPresence::Present)
        | (ClientKind::OpenCodeV1, ConfigSource::OpenCodeJsonc, ConfigPresence::Present)
        | (ClientKind::OpenCodeV1, ConfigSource::OpenCodeMissing, ConfigPresence::Missing) => true,
        _ => false,
    };
    if !source_presence_matches {
        return Err(TransactionError::InvalidPlan);
    }
    Ok(())
}

fn valid_receipt_metadata(metadata: &PatchReceiptMetadata) -> bool {
    if metadata.endpoint_port == 0 {
        return false;
    }
    if let Some(bridge) = &metadata.opencode_retrieval {
        if metadata.client_kind != ClientKind::OpenCodeV1
            || metadata.opencode_routes.is_none()
            || !valid_sha256(&bridge.definition_sha256)
        {
            return false;
        }
    }
    match metadata.client_kind {
        ClientKind::Codex => {
            metadata.config_source == ConfigSource::CodexToml
                && metadata.created_ancestor_mask == 0
                && metadata.opencode_routes.is_none()
        }
        ClientKind::ClaudeCode => {
            metadata.config_source == ConfigSource::ClaudeJson
                && metadata.created_ancestor_mask <= 1
                && metadata.opencode_routes.is_none()
        }
        ClientKind::OpenCodeV1 => {
            if let Some(routes) = &metadata.opencode_routes {
                return matches!(
                    metadata.config_source,
                    ConfigSource::OpenCodeJson | ConfigSource::OpenCodeJsonc
                ) && !metadata.config_was_missing
                    && metadata.created_ancestor_mask == 0
                    && (valid_opencode_route_receipts(routes)
                        || routes.is_empty() && metadata.opencode_retrieval.is_some());
            }
            let allowed_mask = matches!(metadata.created_ancestor_mask, 0 | 4 | 6 | 7);
            let source_presence = match metadata.config_source {
                ConfigSource::OpenCodeMissing => metadata.config_was_missing,
                ConfigSource::OpenCodeJson | ConfigSource::OpenCodeJsonc => {
                    !metadata.config_was_missing
                }
                _ => false,
            };
            allowed_mask
                && source_presence
                && (metadata.config_source != ConfigSource::OpenCodeMissing
                    || metadata.created_ancestor_mask == 7)
        }
    }
}

#[cfg(test)]
mod receipt_tests {
    use super::{
        apply_missing_endpoint, OpenCodeRouteReceipt, PatchReceipt, PatchReceiptMetadata,
        TransactionError, CANONICAL_OPENAI_BASE, MAX_OPENCODE_ROUTE_RECEIPTS,
    };
    use crate::integrations::plan::{
        preview_config, ClientKind, ConfigPresence, ConfigScope, ConfigSource, PlanContext,
        StableEndpoint,
    };

    fn open_code_plan(source: ConfigSource) -> crate::integrations::plan::ConnectionPlan {
        preview_config(
            &PlanContext {
                request_id: "019a0000-0000-7000-8000-d00000000001".to_owned(),
                installation_id: "019a0000-0000-7000-8000-d00000000002".to_owned(),
                client_kind: ClientKind::OpenCodeV1,
                config_source: source,
                surface: "cli".to_owned(),
                version: Some("1.19.3".to_owned()),
                scope: ConfigScope::UserConfig,
                scan_generation: 1,
                endpoint: StableEndpoint {
                    endpoint_id: "019a0000-0000-7000-8000-d00000000003".to_owned(),
                    host: "127.0.0.1".to_owned(),
                    port: 45123,
                },
                now_unix_seconds: 1_800_000_000,
            },
            b"{}",
        )
        .expect("fixture preview")
    }

    #[test]
    fn opencode_missing_source_cannot_claim_a_present_config() {
        let mut forged = open_code_plan(ConfigSource::OpenCodeJson);
        forged.config_source = ConfigSource::OpenCodeMissing;
        forged.config_presence = ConfigPresence::Present;
        assert!(matches!(
            apply_missing_endpoint(&forged, b"{}"),
            Err(TransactionError::InvalidPlan)
        ));
    }

    #[test]
    fn receipt_metadata_rejects_inconsistent_sources_and_masks() {
        let base = PatchReceiptMetadata {
            client_kind: ClientKind::OpenCodeV1,
            config_source: ConfigSource::OpenCodeMissing,
            endpoint_port: 45123,
            created_ancestor_mask: 7,
            config_was_missing: true,
            opencode_routes: None,
            opencode_retrieval: None,
        };
        assert!(PatchReceipt::from_metadata(base.clone()).is_ok());
        assert!(PatchReceipt::from_metadata(PatchReceiptMetadata {
            config_was_missing: false,
            ..base.clone()
        })
        .is_err());
        assert!(PatchReceipt::from_metadata(PatchReceiptMetadata {
            created_ancestor_mask: 3,
            ..base.clone()
        })
        .is_err());
        assert!(PatchReceipt::from_metadata(PatchReceiptMetadata {
            client_kind: ClientKind::Codex,
            config_source: ConfigSource::CodexToml,
            created_ancestor_mask: 0,
            ..base
        })
        .is_ok());
    }

    #[test]
    fn opencode_route_receipts_require_bounded_unique_safe_rows() {
        let row = OpenCodeRouteReceipt {
            provider_key_sha256: "a".repeat(64),
            original_base_url_literal: Some(serde_json::to_string(CANONICAL_OPENAI_BASE).unwrap()),
            created_ancestor_mask: 0,
        };
        let base = PatchReceiptMetadata {
            client_kind: ClientKind::OpenCodeV1,
            config_source: ConfigSource::OpenCodeJsonc,
            endpoint_port: 45123,
            created_ancestor_mask: 0,
            config_was_missing: false,
            opencode_routes: Some(vec![row.clone()]),
            opencode_retrieval: None,
        };
        assert!(PatchReceipt::from_metadata(base.clone()).is_ok());

        for invalid_rows in [
            Vec::new(),
            vec![row.clone(), row.clone()],
            vec![OpenCodeRouteReceipt {
                provider_key_sha256: "A".repeat(64),
                ..row.clone()
            }],
            vec![OpenCodeRouteReceipt {
                original_base_url_literal: Some(
                    serde_json::to_string("https://user.example/v1").unwrap(),
                ),
                ..row.clone()
            }],
            vec![OpenCodeRouteReceipt {
                original_base_url_literal: Some("\"https://api.openai.com/v1\" trailing".into()),
                ..row.clone()
            }],
            vec![OpenCodeRouteReceipt {
                created_ancestor_mask: 4,
                ..row.clone()
            }],
        ] {
            let mut metadata = base.clone();
            metadata.opencode_routes = Some(invalid_rows);
            assert!(PatchReceipt::from_metadata(metadata).is_err());
        }

        let mut oversized = base.clone();
        oversized.opencode_routes = Some(
            (0..=MAX_OPENCODE_ROUTE_RECEIPTS)
                .map(|index| OpenCodeRouteReceipt {
                    provider_key_sha256: format!("{index:064x}"),
                    original_base_url_literal: None,
                    created_ancestor_mask: 0,
                })
                .collect(),
        );
        assert!(PatchReceipt::from_metadata(oversized).is_err());

        let mut cross_client = base;
        cross_client.client_kind = ClientKind::ClaudeCode;
        cross_client.config_source = ConfigSource::ClaudeJson;
        assert!(PatchReceipt::from_metadata(cross_client).is_err());
    }

    #[test]
    fn legacy_opencode_receipt_without_route_rows_remains_valid() {
        let receipt = PatchReceipt::from_metadata(PatchReceiptMetadata {
            client_kind: ClientKind::OpenCodeV1,
            config_source: ConfigSource::OpenCodeJsonc,
            endpoint_port: 45123,
            created_ancestor_mask: 0,
            config_was_missing: false,
            opencode_routes: None,
            opencode_retrieval: None,
        })
        .expect("legacy schema-1 receipt without route metadata remains readable");
        let current = br#"{"provider":{"openai":{"npm":"@ai-sdk/openai","options":{"baseURL":"http://127.0.0.1:45123/v1"}}}}"#;
        let restored = super::undo_owned_endpoint(&receipt, current).unwrap();
        let parsed =
            super::config_document::parse_document(ClientKind::OpenCodeV1, &restored).unwrap();
        assert!(parsed["provider"]["openai"]["options"]
            .get("baseURL")
            .is_none());
    }
}

fn client_kind(name: &str) -> Option<ClientKind> {
    match name {
        "codex" => Some(ClientKind::Codex),
        "claude_code" => Some(ClientKind::ClaudeCode),
        "opencode_v1" => Some(ClientKind::OpenCodeV1),
        _ => None,
    }
}

fn endpoint_field(kind: ClientKind) -> EndpointField {
    match kind {
        ClientKind::Codex => EndpointField::CodexOpenaiBaseUrl,
        ClientKind::ClaudeCode => EndpointField::ClaudeAnthropicBaseUrl,
        ClientKind::OpenCodeV1 => EndpointField::OpenCodeV1OpenAiBaseUrl,
    }
}

fn endpoint_value(kind: ClientKind, port: u16) -> String {
    let suffix = match kind {
        ClientKind::Codex => ENDPOINT_SUFFIX_CODEX,
        ClientKind::ClaudeCode => ENDPOINT_SUFFIX_CLAUDE,
        ClientKind::OpenCodeV1 => ENDPOINT_SUFFIX_OPENCODE,
    };
    format!("http://127.0.0.1:{port}{suffix}")
}

fn empty_document(kind: ClientKind) -> &'static [u8] {
    match kind {
        ClientKind::Codex => b"",
        ClientKind::ClaudeCode | ClientKind::OpenCodeV1 => b"{}",
    }
}

fn endpoint_value_in<'a>(
    root: &'a serde_json::Value,
    kind: ClientKind,
) -> Result<Option<&'a str>, TransactionError> {
    let value = match kind {
        ClientKind::Codex => root.get("openai_base_url"),
        ClientKind::ClaudeCode => root
            .get("env")
            .and_then(|env| env.get("ANTHROPIC_BASE_URL")),
        ClientKind::OpenCodeV1 => root
            .get("provider")
            .and_then(|providers| providers.get("openai"))
            .and_then(|provider| provider.get("options"))
            .and_then(|options| options.get("baseURL")),
    };
    match value {
        None => Ok(None),
        Some(serde_json::Value::String(value)) => Ok(Some(value)),
        Some(_) => Err(TransactionError::Conflict),
    }
}

fn patch_codex(current: &[u8], endpoint: &str) -> Result<Vec<u8>, TransactionError> {
    let text = std::str::from_utf8(current).map_err(|_| TransactionError::InvalidConfig)?;
    let newline = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let quoted = serde_json::to_string(endpoint).map_err(|_| TransactionError::InvalidConfig)?;
    let assignment = format!("openai_base_url = {quoted}{newline}");
    let mut output = Vec::with_capacity(current.len() + assignment.len());
    output.extend_from_slice(assignment.as_bytes());
    output.extend_from_slice(current);
    Ok(output)
}

#[derive(Debug)]
struct TomlEndpoint {
    key_span: Range<usize>,
    value_span: Range<usize>,
    value: toml::Value,
}

#[derive(Debug, Default)]
struct TomlRootFields {
    endpoint: Option<TomlEndpoint>,
}

impl<'de> Deserialize<'de> for TomlRootFields {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct RootVisitor;
        impl<'de> Visitor<'de> for RootVisitor {
            type Value = TomlRootFields;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a TOML root table")
            }

            fn visit_map<M>(self, mut map: M) -> Result<Self::Value, M::Error>
            where
                M: MapAccess<'de>,
            {
                let mut result = TomlRootFields::default();
                while let Some((key, value)) =
                    map.next_entry::<toml::Spanned<String>, toml::Spanned<toml::Value>>()?
                {
                    if key.get_ref() == "openai_base_url" {
                        result.endpoint = Some(TomlEndpoint {
                            key_span: key.span(),
                            value_span: value.span(),
                            value: value.into_inner(),
                        });
                    }
                }
                Ok(result)
            }
        }
        deserializer.deserialize_map(RootVisitor)
    }
}

fn toml_root_fields(bytes: &[u8]) -> Result<TomlRootFields, TransactionError> {
    let text = std::str::from_utf8(bytes).map_err(|_| TransactionError::InvalidConfig)?;
    toml::from_str(text).map_err(|_| TransactionError::InvalidConfig)
}

fn remove_codex_assignment(current: &[u8], expected: &str) -> Result<Vec<u8>, TransactionError> {
    let fields = toml_root_fields(current)?;
    let field = fields.endpoint.ok_or(TransactionError::Conflict)?;
    if field.value.as_str() != Some(expected) {
        return Err(TransactionError::Conflict);
    }
    let text = std::str::from_utf8(current).map_err(|_| TransactionError::InvalidConfig)?;
    let start = text[..field.key_span.start]
        .rfind('\n')
        .map_or(0, |index| index + 1);
    let value_line_end = text[field.value_span.end..]
        .find('\n')
        .map_or(text.len(), |index| field.value_span.end + index);
    let suffix = &text[field.value_span.end..value_line_end];
    let end = if suffix.trim().is_empty() && value_line_end < text.len() {
        value_line_end + 1
    } else if suffix.trim().is_empty() {
        value_line_end
    } else {
        field.value_span.end
    };
    let mut output = Vec::with_capacity(current.len());
    output.extend_from_slice(&current[..start]);
    output.extend_from_slice(&current[end..]);
    Ok(output)
}

fn json_path(kind: ClientKind) -> (&'static [&'static str], &'static str) {
    match kind {
        ClientKind::ClaudeCode => (&["env"], "ANTHROPIC_BASE_URL"),
        ClientKind::OpenCodeV1 => (&["provider", "openai", "options"], "baseURL"),
        ClientKind::Codex => unreachable!("TOML has no JSON path"),
    }
}

fn json_parse_options(kind: ClientKind) -> jsonc_parser::ParseOptions {
    let jsonc = kind == ClientKind::OpenCodeV1;
    jsonc_parser::ParseOptions {
        allow_comments: jsonc,
        allow_loose_object_property_names: false,
        allow_trailing_commas: jsonc,
        allow_missing_commas: false,
        allow_single_quoted_strings: false,
        allow_hexadecimal_numbers: false,
        allow_unary_plus_numbers: false,
        allow_bare_decimal_point_numbers: false,
        allow_non_finite_numbers: false,
        allow_extended_string_escapes: false,
    }
}

fn parse_json_ast<'a>(
    kind: ClientKind,
    text: &'a str,
) -> Result<jsonc_parser::ast::Object<'a>, TransactionError> {
    let parsed = jsonc_parser::parse_to_ast(
        text,
        &jsonc_parser::CollectOptions::default(),
        &json_parse_options(kind),
    )
    .map_err(|_| TransactionError::InvalidConfig)?;
    let root = parsed.value.ok_or(TransactionError::InvalidConfig)?;
    match root {
        jsonc_parser::ast::Value::Object(object) => Ok(object),
        _ => Err(TransactionError::InvalidConfig),
    }
}

fn patch_json(
    kind: ClientKind,
    current: &[u8],
    endpoint: &str,
) -> Result<(Vec<u8>, u8), TransactionError> {
    let (ancestors, leaf) = json_path(kind);
    patch_json_path(kind, current, ancestors, leaf, endpoint)
}

fn patch_json_path(
    kind: ClientKind,
    current: &[u8],
    ancestors: &[&str],
    leaf: &str,
    endpoint: &str,
) -> Result<(Vec<u8>, u8), TransactionError> {
    let literal = serde_json::to_string(endpoint).map_err(|_| TransactionError::InvalidConfig)?;
    patch_json_path_literal(kind, current, ancestors, leaf, &literal)
}

fn patch_json_path_literal(
    kind: ClientKind,
    current: &[u8],
    ancestors: &[&str],
    leaf: &str,
    literal: &str,
) -> Result<(Vec<u8>, u8), TransactionError> {
    let text = std::str::from_utf8(current).map_err(|_| TransactionError::InvalidConfig)?;
    let root = parse_json_ast(kind, text)?;
    let mut object = &root;
    let mut missing = None;
    for (index, segment) in ancestors.iter().enumerate() {
        match object.get(segment) {
            None => {
                missing = Some(index);
                break;
            }
            Some(property) => match &property.value {
                jsonc_parser::ast::Value::Object(child) => object = child,
                _ => return Err(TransactionError::Conflict),
            },
        }
    }

    let mut mask = 0u8;
    let (parent, key, value) = if let Some(index) = missing {
        let mut nested = format!("{}: {}", json_string(leaf)?, literal);
        for segment in ancestors[index + 1..].iter().rev() {
            nested = format!("{}: {{{nested}}}", json_string(segment)?);
        }
        for bit in index..ancestors.len() {
            mask |= 1 << bit;
        }
        (object, ancestors[index], format!("{{{nested}}}"))
    } else {
        (object, leaf, literal.to_owned())
    };
    let key = json_string(key)?;
    let bytes = insert_json_property(text, parent, &key, &value)?;
    Ok((bytes, mask))
}

fn remove_json_property(
    kind: ClientKind,
    current: &[u8],
    parent_path: &[&str],
    key: &str,
) -> Result<Vec<u8>, TransactionError> {
    let text = std::str::from_utf8(current).map_err(|_| TransactionError::InvalidConfig)?;
    let root = parse_json_ast(kind, text)?;
    let parent = object_ref_at(&root, parent_path).ok_or(TransactionError::Conflict)?;
    let index = parent
        .properties
        .iter()
        .position(|property| property.name.as_str() == key)
        .ok_or(TransactionError::Conflict)?;
    let property = &parent.properties[index];
    let commas = comma_ranges(text, kind)?;
    let comma = if index + 1 < parent.properties.len() {
        let next = &parent.properties[index + 1];
        commas
            .into_iter()
            .find(|range| range.start >= property.range.end && range.end <= next.range.start)
            .ok_or(TransactionError::InvalidConfig)?
    } else if index > 0 {
        let previous = &parent.properties[index - 1];
        commas
            .into_iter()
            .find(|range| range.start >= previous.range.end && range.end <= property.range.start)
            .ok_or(TransactionError::InvalidConfig)?
    } else {
        commas
            .into_iter()
            .find(|range| range.start >= property.range.end && range.end < parent.range.end - 1)
            .unwrap_or(property.range.start..property.range.start)
    };
    let mut removals = vec![property.range.start..property.range.end];
    if !comma.is_empty() {
        removals.push(comma);
    }
    removals.sort_by_key(|range| range.start);
    if removals.windows(2).any(|pair| pair[0].end > pair[1].start) {
        return Err(TransactionError::InvalidConfig);
    }
    let removed_len = removals.iter().map(|range| range.len()).sum::<usize>();
    let mut output = Vec::with_capacity(current.len().saturating_sub(removed_len));
    let mut cursor = 0;
    for range in removals {
        output.extend_from_slice(&current[cursor..range.start]);
        cursor = range.end;
    }
    output.extend_from_slice(&current[cursor..]);
    Ok(output)
}

fn json_object_at<'a>(
    kind: ClientKind,
    current: &'a [u8],
    path: &[&str],
) -> Result<Option<jsonc_parser::ast::Object<'a>>, TransactionError> {
    let text = std::str::from_utf8(current).map_err(|_| TransactionError::InvalidConfig)?;
    let root = parse_json_ast(kind, text)?;
    Ok(object_ref_at(&root, path).cloned())
}

fn object_is_empty_and_comment_free(bytes: &[u8], object: &jsonc_parser::ast::Object<'_>) -> bool {
    let interior = &bytes[object.range.start + 1..object.range.end.saturating_sub(1)];
    interior.iter().all(u8::is_ascii_whitespace)
}

fn object_ref_at<'ast, 'text>(
    root: &'ast jsonc_parser::ast::Object<'text>,
    path: &[&str],
) -> Option<&'ast jsonc_parser::ast::Object<'text>> {
    let mut current = root;
    for segment in path {
        let property = current.get(segment)?;
        match &property.value {
            jsonc_parser::ast::Value::Object(object) => current = object,
            _ => return None,
        }
    }
    Some(current)
}

fn json_string(value: &str) -> Result<String, TransactionError> {
    serde_json::to_string(value).map_err(|_| TransactionError::InvalidConfig)
}

fn insert_json_property(
    text: &str,
    object: &jsonc_parser::ast::Object<'_>,
    key: &str,
    value: &str,
) -> Result<Vec<u8>, TransactionError> {
    let (offset, insertion) = if let Some(last) = object.properties.last() {
        (last.range.end, format!(",{key}: {value}"))
    } else {
        (object.range.start + 1, format!("{key}: {value}"))
    };
    if offset > text.len() || !text.is_char_boundary(offset) {
        return Err(TransactionError::InvalidConfig);
    }
    let mut output = Vec::with_capacity(text.len() + insertion.len());
    output.extend_from_slice(text[..offset].as_bytes());
    output.extend_from_slice(insertion.as_bytes());
    output.extend_from_slice(text[offset..].as_bytes());
    Ok(output)
}

fn comma_ranges(text: &str, kind: ClientKind) -> Result<Vec<Range<usize>>, TransactionError> {
    use jsonc_parser::tokens::Token;
    let options = json_parse_options(kind);
    let scanner_options = jsonc_parser::ScannerOptions {
        allow_single_quoted_strings: options.allow_single_quoted_strings,
        allow_hexadecimal_numbers: options.allow_hexadecimal_numbers,
        allow_unary_plus_numbers: options.allow_unary_plus_numbers,
        allow_bare_decimal_point_numbers: options.allow_bare_decimal_point_numbers,
        allow_non_finite_numbers: options.allow_non_finite_numbers,
        allow_extended_string_escapes: options.allow_extended_string_escapes,
    };
    let mut scanner = jsonc_parser::Scanner::new(text, &scanner_options);
    let mut commas = Vec::new();
    while let Some(token) = scanner
        .scan()
        .map_err(|_| TransactionError::InvalidConfig)?
    {
        if matches!(token, Token::Comma) {
            commas.push(scanner.token_start()..scanner.token_end());
        }
    }
    Ok(commas)
}
