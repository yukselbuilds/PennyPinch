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

    pub(crate) fn definition(&self) -> serde_json::Value {
        serde_json::json!({"type": "local", "command": self.0, "enabled": true, "timeout": 15000})
    }
}

pub(crate) fn can_install(root: &serde_json::Value) -> bool {
    match root.get("mcp") {
        None => true,
        Some(serde_json::Value::Object(mcp)) => !mcp.contains_key("headroom"),
        _ => false,
    }
}

pub(crate) fn definition_fingerprint(value: &serde_json::Value, literal: &[u8]) -> Option<String> {
    let definition: Definition = serde_json::from_value(value.clone()).ok()?;
    if definition.r#type != "local"
        || !definition.enabled
        || definition.timeout != 15000
        || definition.command.len() != 8
        || definition.command[1] != "-I"
        || definition.command[2] != "-B"
        || definition.command.iter().enumerate().any(|(index, arg)| {
            (index < 7 && arg.is_empty()) || arg.len() > 2048 || arg.chars().any(char::is_control)
        })
    {
        return None;
    }
    let mut hash = Sha256::new();
    hash.update(b"headroom-opencode-retrieval/v1\0");
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
}
