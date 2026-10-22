use serde::de::{MapAccess, Visitor};
use serde::{Deserialize, Deserializer};
use std::fmt;
use std::ops::Range;

use super::config_document;
use super::plan::{
    ClientKind, ConfigPresence, ConfigSource, ConnectionPlan, EndpointBefore, EndpointField,
    PlanStatus,
};

const ENDPOINT_SUFFIX_CODEX: &str = "/v1";
const ENDPOINT_SUFFIX_CLAUDE: &str = "";
const ENDPOINT_SUFFIX_OPENCODE: &str = "/v1";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PatchReceipt {
    client_kind: ClientKind,
    config_source: ConfigSource,
    endpoint_port: u16,
    created_ancestor_mask: u8,
    config_was_missing: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PatchReceiptMetadata {
    pub(crate) client_kind: ClientKind,
    pub(crate) config_source: ConfigSource,
    pub(crate) endpoint_port: u16,
    pub(crate) created_ancestor_mask: u8,
    pub(crate) config_was_missing: bool,
}

impl PatchReceipt {
    pub(crate) fn metadata(&self) -> PatchReceiptMetadata {
        PatchReceiptMetadata {
            client_kind: self.client_kind,
            config_source: self.config_source,
            endpoint_port: self.endpoint_port,
            created_ancestor_mask: self.created_ancestor_mask,
            config_was_missing: self.config_was_missing,
        }
    }

    pub(crate) fn from_metadata(metadata: PatchReceiptMetadata) -> Result<Self, TransactionError> {
        if !valid_receipt_metadata(metadata) {
            return Err(TransactionError::InvalidReceipt);
        }
        Ok(Self {
            client_kind: metadata.client_kind,
            config_source: metadata.config_source,
            endpoint_port: metadata.endpoint_port,
            created_ancestor_mask: metadata.created_ancestor_mask,
            config_was_missing: metadata.config_was_missing,
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
        })?,
    })
}

pub(crate) fn undo_owned_endpoint(
    receipt: &PatchReceipt,
    current: &[u8],
) -> Result<Vec<u8>, TransactionError> {
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

fn validate_plan(plan: &ConnectionPlan) -> Result<(), TransactionError> {
    if plan.schema_version != 1
        || plan.scope != "user_config"
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

fn valid_receipt_metadata(metadata: PatchReceiptMetadata) -> bool {
    if metadata.endpoint_port == 0 {
        return false;
    }
    match metadata.client_kind {
        ClientKind::Codex => {
            metadata.config_source == ConfigSource::CodexToml && metadata.created_ancestor_mask == 0
        }
        ClientKind::ClaudeCode => {
            metadata.config_source == ConfigSource::ClaudeJson
                && metadata.created_ancestor_mask <= 1
        }
        ClientKind::OpenCodeV1 => {
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
    use super::{apply_missing_endpoint, PatchReceipt, PatchReceiptMetadata, TransactionError};
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
        };
        assert!(PatchReceipt::from_metadata(base).is_ok());
        assert!(PatchReceipt::from_metadata(PatchReceiptMetadata {
            config_was_missing: false,
            ..base
        })
        .is_err());
        assert!(PatchReceipt::from_metadata(PatchReceiptMetadata {
            created_ancestor_mask: 3,
            ..base
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
    let text = std::str::from_utf8(current).map_err(|_| TransactionError::InvalidConfig)?;
    let root = parse_json_ast(kind, text)?;
    let (ancestors, leaf) = json_path(kind);
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
        let mut nested = format!(
            "{}: {}",
            json_string(leaf)?,
            serde_json::to_string(endpoint).map_err(|_| TransactionError::InvalidConfig)?
        );
        for segment in ancestors[index + 1..].iter().rev() {
            nested = format!("{}: {{{nested}}}", json_string(segment)?);
        }
        for bit in index..ancestors.len() {
            mask |= 1 << bit;
        }
        (object, ancestors[index], format!("{{{nested}}}"))
    } else {
        (
            object,
            leaf,
            serde_json::to_string(endpoint).map_err(|_| TransactionError::InvalidConfig)?,
        )
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
