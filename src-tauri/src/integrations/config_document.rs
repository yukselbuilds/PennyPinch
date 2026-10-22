use std::collections::HashSet;

use jsonc_parser::tokens::Token;
use jsonc_parser::{
    parse_to_ast, CollectOptions, CommentCollectionStrategy, ParseOptions, Scanner, ScannerOptions,
};
use serde_json::Value;

use super::plan::{ClientKind, PlanError};

const MAX_DOCUMENT_BYTES: usize = 256 * 1024;
const MAX_TREE_DEPTH: usize = 64;
const MAX_TREE_NODES: usize = 32_768;

pub(super) fn parse_document(kind: ClientKind, bytes: &[u8]) -> Result<Value, PlanError> {
    if bytes.len() > MAX_DOCUMENT_BYTES {
        return Err(PlanError::ConfigTooLarge);
    }

    let text = std::str::from_utf8(bytes).map_err(|_| PlanError::InvalidConfig)?;
    match kind {
        ClientKind::Codex => parse_toml(text),
        ClientKind::ClaudeCode => parse_json(text, false, false),
        ClientKind::OpenCodeV1 => parse_json(text, true, true),
    }
}

fn parse_toml(text: &str) -> Result<Value, PlanError> {
    let value = toml::from_str::<toml::Value>(text).map_err(|_| PlanError::InvalidConfig)?;
    let json = serde_json::to_value(value).map_err(|_| PlanError::InvalidConfig)?;
    validate_json_tree(&json)?;
    if !json.is_object() {
        return Err(PlanError::InvalidConfig);
    }
    Ok(json)
}

fn parse_json(
    text: &str,
    allow_comments: bool,
    allow_trailing_commas: bool,
) -> Result<Value, PlanError> {
    preflight_json_nesting(text)?;
    let collect = CollectOptions {
        comments: CommentCollectionStrategy::Off,
        tokens: false,
    };
    let options = ParseOptions {
        allow_comments,
        allow_loose_object_property_names: false,
        allow_trailing_commas,
        allow_missing_commas: false,
        allow_single_quoted_strings: false,
        allow_hexadecimal_numbers: false,
        allow_unary_plus_numbers: false,
        allow_bare_decimal_point_numbers: false,
        allow_non_finite_numbers: false,
        allow_extended_string_escapes: false,
    };
    let parsed = parse_to_ast(text, &collect, &options).map_err(|_| PlanError::InvalidConfig)?;
    let value = parsed.value.ok_or(PlanError::InvalidConfig)?;
    validate_ast_tree(&value, 0, &mut 0usize)?;
    let json = Value::from(value);
    if !json.is_object() {
        return Err(PlanError::InvalidConfig);
    }
    Ok(json)
}

fn preflight_json_nesting(text: &str) -> Result<(), PlanError> {
    let scanner_options = ScannerOptions {
        allow_single_quoted_strings: false,
        allow_hexadecimal_numbers: false,
        allow_unary_plus_numbers: false,
        allow_bare_decimal_point_numbers: false,
        allow_non_finite_numbers: false,
        allow_extended_string_escapes: false,
    };
    let mut scanner = Scanner::new(text, &scanner_options);
    let mut depth = 0usize;
    let mut token_count = 0usize;

    while let Some(token) = scanner.scan().map_err(|_| PlanError::InvalidConfig)? {
        token_count += 1;
        if token_count > MAX_TREE_NODES * 3 {
            return Err(PlanError::InvalidConfig);
        }
        match token {
            Token::OpenBrace | Token::OpenBracket => {
                depth += 1;
                if depth > MAX_TREE_DEPTH {
                    return Err(PlanError::InvalidConfig);
                }
            }
            Token::CloseBrace | Token::CloseBracket => {
                if depth == 0 {
                    return Err(PlanError::InvalidConfig);
                }
                depth -= 1;
            }
            _ => {}
        }
    }
    if depth != 0 {
        return Err(PlanError::InvalidConfig);
    }
    Ok(())
}

fn validate_ast_tree(
    value: &jsonc_parser::ast::Value<'_>,
    depth: usize,
    nodes: &mut usize,
) -> Result<(), PlanError> {
    *nodes += 1;
    if depth > MAX_TREE_DEPTH || *nodes > MAX_TREE_NODES {
        return Err(PlanError::InvalidConfig);
    }

    match value {
        jsonc_parser::ast::Value::Object(object) => {
            let mut names = HashSet::with_capacity(object.properties.len());
            for property in &object.properties {
                if !names.insert(property.name.as_str()) {
                    return Err(PlanError::DuplicateKey);
                }
                validate_ast_tree(&property.value, depth + 1, nodes)?;
            }
        }
        jsonc_parser::ast::Value::Array(array) => {
            for item in &array.elements {
                validate_ast_tree(item, depth + 1, nodes)?;
            }
        }
        jsonc_parser::ast::Value::StringLit(_)
        | jsonc_parser::ast::Value::NumberLit(_)
        | jsonc_parser::ast::Value::BooleanLit(_)
        | jsonc_parser::ast::Value::NullKeyword(_) => {}
    }
    Ok(())
}

fn validate_json_tree(value: &Value) -> Result<(), PlanError> {
    fn visit(value: &Value, depth: usize, nodes: &mut usize) -> Result<(), PlanError> {
        *nodes += 1;
        if depth > MAX_TREE_DEPTH || *nodes > MAX_TREE_NODES {
            return Err(PlanError::InvalidConfig);
        }
        match value {
            Value::Object(object) => {
                for child in object.values() {
                    visit(child, depth + 1, nodes)?;
                }
            }
            Value::Array(items) => {
                for child in items {
                    visit(child, depth + 1, nodes)?;
                }
            }
            _ => {}
        }
        Ok(())
    }

    visit(value, 0, &mut 0)
}
