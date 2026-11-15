use super::plan::OpenCodeSchema;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::Path;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RetrievalChange {
    InstallLocalBridge,
    Conflict,
}

#[derive(Clone, Eq, PartialEq)]
pub(crate) struct RetrievalCommand(Vec<String>);

impl std::fmt::Debug for RetrievalCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("RetrievalCommand([redacted])")
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Definition {
    r#type: String,
    command: Vec<String>,
    enabled: bool,
    timeout: u32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Timeouts {
    startup: u32,
    catalog: u32,
    execution: u32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DefinitionV2 {
    r#type: String,
    command: Vec<String>,
    disabled: bool,
    codemode: bool,
    protocol: String,
    timeout: Timeouts,
}

impl RetrievalCommand {
    pub(crate) fn new(
        python: &Path,
        script: &Path,
        endpoint_record: &Path,
        endpoint_id: &str,
        owner_executable: &Path,
        desktop_use_gate: Option<&Path>,
    ) -> Result<Self, ()> {
        if uuid::Uuid::parse_str(endpoint_id)
            .ok()
            .is_none_or(|id| id.to_string() != endpoint_id)
        {
            return Err(());
        }
        let path = |value: &Path| -> Result<String, ()> {
            if !value.is_absolute() {
                return Err(());
            }
            let text = value.to_str().ok_or(())?;
            if text.len() > 2048 || text.chars().any(char::is_control) {
                return Err(());
            }
            Ok(text.to_owned())
        };
        Ok(Self(vec![
            path(python)?,
            "-I".to_owned(),
            "-B".to_owned(),
            path(script)?,
            path(endpoint_record)?,
            endpoint_id.to_owned(),
            path(owner_executable)?,
            desktop_use_gate.map(path).transpose()?.unwrap_or_default(),
        ]))
    }

    pub(crate) fn definition(&self, schema: Option<OpenCodeSchema>) -> serde_json::Value {
        if schema.is_some() {
            serde_json::json!({"type":"local","command":self.0,"disabled":false,"codemode":false,
                "protocol":"legacy","timeout":{"startup":15000,"catalog":15000,"execution":15000}})
        } else {
            serde_json::json!({"type": "local", "command": self.0, "enabled": true, "timeout": 15000})
        }
    }
}

pub(crate) fn can_install(root: &serde_json::Value) -> bool {
    match root.get("mcp") {
        None => true,
        Some(serde_json::Value::Object(mcp)) => !mcp.contains_key("headroom"),
        _ => false,
    }
}

pub(crate) fn can_install_for_schema(
    root: &serde_json::Value,
    schema: Option<OpenCodeSchema>,
) -> bool {
    if !can_install(root) {
        return false;
    }
    if schema.is_none() {
        return true;
    }
    match root.get("mcp").and_then(|mcp| mcp.get("servers")) {
        None => true,
        Some(serde_json::Value::Object(servers)) => {
            !servers.contains_key("headroom")
                && !matches!(
                    servers.get("type").and_then(serde_json::Value::as_str),
                    Some("local" | "remote")
                )
                && !servers
                    .get("enabled")
                    .is_some_and(serde_json::Value::is_boolean)
        }
        _ => false,
    }
}

pub(crate) fn definition_fingerprint(
    value: &serde_json::Value,
    literal: &[u8],
    schema: Option<OpenCodeSchema>,
) -> Option<String> {
    let command = if schema.is_some() {
        let definition: DefinitionV2 = serde_json::from_value(value.clone()).ok()?;
        if definition.r#type != "local"
            || definition.disabled
            || definition.codemode
            || definition.protocol != "legacy"
            || definition.timeout.startup != 15000
            || definition.timeout.catalog != 15000
            || definition.timeout.execution != 15000
        {
            return None;
        }
        definition.command
    } else {
        let definition: Definition = serde_json::from_value(value.clone()).ok()?;
        if definition.r#type != "local" || !definition.enabled || definition.timeout != 15000 {
            return None;
        }
        definition.command
    };
    if command.len() != 8
        || command[1] != "-I"
        || command[2] != "-B"
        || command.iter().enumerate().any(|(index, arg)| {
            (index < 7 && arg.is_empty()) || arg.len() > 2048 || arg.chars().any(char::is_control)
        })
    {
        return None;
    }
    let mut hash = Sha256::new();
    hash.update(if schema.is_some() {
        b"headroom-opencode-retrieval/v2\0"
    } else {
        b"headroom-opencode-retrieval/v1\0"
    });
    // Include the literal so Undo cannot silently erase user comments or formatting edits.
    hash.update(literal);
    Some(
        hash.finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect(),
    )
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RetrievalReceipt {
    pub(crate) definition_sha256: String,
    pub(crate) created_mcp_object: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub(crate) created_servers_object: bool,
}

/// Fixed ASCII MCP names, with the pinned V2 Windows wildcard semantics.
fn matches(input: &str, pattern: &str) -> bool {
    let input = input.replace('\\', "/");
    let pattern = pattern.replace('\\', "/");
    let (input, pattern) = if cfg!(windows) {
        (input.to_ascii_lowercase(), pattern.to_ascii_lowercase())
    } else {
        (input, pattern)
    };
    let test = |pattern: &str| {
        let mut row = vec![false; input.len() + 1];
        row[0] = true;
        let mut next = row.clone();
        for token in pattern.bytes() {
            for index in 0..row.len() {
                next[index] = if token == b'*' {
                    row[index] || (index > 0 && next[index - 1])
                } else {
                    index > 0
                        && row[index - 1]
                        && (token == b'?' || token == input.as_bytes()[index - 1])
                };
            }
            std::mem::swap(&mut row, &mut next);
        }
        row[input.len()]
    };
    test(&pattern) || pattern.strip_suffix(" *").is_some_and(test)
}

fn permission_allows(value: Option<&serde_json::Value>) -> bool {
    let Some(value) = value else {
        return true;
    };
    let Some(rules) = value.as_array().filter(|rules| rules.len() <= 128) else {
        return false;
    };
    let mut allowed = true;
    for rule in rules {
        let (Some(action), Some(resource), Some(effect)) = (
            rule.get("action").and_then(serde_json::Value::as_str),
            rule.get("resource").and_then(serde_json::Value::as_str),
            rule.get("effect").and_then(serde_json::Value::as_str),
        ) else {
            return false;
        };
        if action.len() > 2048
            || resource.len() > 2048
            || !["allow", "deny", "ask"].contains(&effect)
        {
            return false;
        }
        if matches("headroom_retrieve", action) && matches("*", resource) {
            allowed = effect != "deny";
        }
    }
    allowed
}

pub(crate) fn retrieval_available(root: &serde_json::Value) -> bool {
    if !permission_allows(root.get("permissions")) {
        return false;
    }
    if let Some(value) = root.get("agents") {
        let Some(agents) = value.as_object().filter(|agents| agents.len() <= 32) else {
            return false;
        };
        for agent in agents.values() {
            if !agent.is_object() {
                return false;
            }
            if agent.get("disabled") == Some(&serde_json::Value::Bool(true)) {
                continue;
            }
            if !permission_allows(agent.get("permissions")) {
                return false;
            }
        }
    }
    let Some(value) = root.get("policies") else {
        return true;
    };
    let Some(rules) = value.as_array().filter(|rules| rules.len() <= 128) else {
        return false;
    };
    let (mut mcp, mut tool) = (true, true);
    for rule in rules {
        let (Some(action), Some(resource), Some(effect)) = (
            rule.get("action").and_then(serde_json::Value::as_str),
            rule.get("resource").and_then(serde_json::Value::as_str),
            rule.get("effect").and_then(serde_json::Value::as_str),
        ) else {
            return false;
        };
        if resource.len() > 2048 || !["allow", "deny"].contains(&effect) {
            return false;
        }
        let mcp_pattern = match action {
            "mcp.use" => Some(resource),
            "integration.use" => resource.strip_prefix("mcp:"),
            _ => None,
        };
        if mcp_pattern.is_some_and(|pattern| matches("headroom", pattern)) {
            mcp = effect == "allow";
        }
        if action == "tool.use" && matches("headroom_retrieve:*", resource) {
            tool = effect == "allow";
        }
    }
    mcp && tool
}
