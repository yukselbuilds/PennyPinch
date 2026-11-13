//! Versioned, metadata-only Codex views. Preview, title and turn fields are never retained.
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

pub(super) const VERSION: &str = "0.167.0-alpha.4.1";
pub(super) const MAX_ROWS: usize = 128;
pub(super) const PAGE_SIZE: usize = 64;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ThreadPage {
    pub data: Vec<Thread>,
    pub next_cursor: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct LoadedPage {
    pub data: Vec<String>,
    pub next_cursor: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Thread {
    id: String,
    cwd: String,
    created_at: i64,
    updated_at: i64,
    model: Option<String>,
    model_provider: String,
    reasoning_effort: Option<String>,
    parent_thread_id: Option<String>,
    source: Source,
    status: Status,
}

#[derive(Deserialize)]
pub(super) struct ThreadRead {
    pub thread: Thread,
}

pub(super) fn missing_loaded(
    threads: &ThreadPage,
    loaded: &LoadedPage,
) -> Result<Vec<String>, &'static str> {
    if threads.data.len() > PAGE_SIZE
        || loaded.data.len() > PAGE_SIZE
        || loaded.data.iter().any(|v| !id(v))
    {
        return Err("invalid_response");
    }
    Ok(loaded
        .data
        .iter()
        .filter(|v| !threads.data.iter().any(|t| &t.id == *v))
        .cloned()
        .collect())
}

pub(super) fn metadata_request(
    request_id: u64,
    thread_id: &str,
) -> Result<serde_json::Value, &'static str> {
    if !id(thread_id) || !(4..4 + PAGE_SIZE as u64).contains(&request_id) {
        return Err("invalid_response");
    }
    Ok(
        serde_json::json!({"id":request_id,"method":"thread/read","params":{"threadId":thread_id,"includeTurns":false}}),
    )
}

pub(super) fn append_read(
    page: &mut ThreadPage,
    read: ThreadRead,
    expected: &str,
) -> Result<(), &'static str> {
    if read.thread.id != expected || page.data.len() >= MAX_ROWS {
        return Err("invalid_response");
    }
    page.data.push(read.thread);
    Ok(())
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Source {
    Label(String),
    Child {
        #[serde(rename = "subAgent")]
        _child: serde::de::IgnoredAny,
    },
    Other(serde::de::IgnoredAny),
}

#[derive(Deserialize)]
#[serde(tag = "type")]
enum Status {
    #[serde(rename = "notLoaded")]
    NotLoaded,
    #[serde(rename = "idle")]
    Idle,
    #[serde(rename = "systemError")]
    Error,
    #[serde(rename = "active")]
    Active {
        #[serde(rename = "activeFlags")]
        flags: Vec<String>,
    },
    #[serde(other)]
    Unknown,
}

pub(super) struct Snapshot {
    pub rows: Vec<super::Row>,
    pub partial: bool,
    pub loaded_count: usize,
}

fn id(value: &str) -> bool {
    uuid::Uuid::parse_str(value).is_ok_and(|v| !v.is_nil() && v.to_string() == value)
}

fn key(scope: &str, id: &str) -> String {
    let mut hasher = Sha256::new();
    for part in ["codex-session", scope, id] {
        hasher.update((part.len() as u64).to_be_bytes());
        hasher.update(part.as_bytes());
    }
    format!("{:x}", hasher.finalize())
}

fn label(value: &str) -> Option<String> {
    crate::metrics::attribution::safe_label(value).then(|| value.to_owned())
}

fn project(cwd: &str) -> Option<String> {
    let name = cwd
        .trim_end_matches(['/', '\\'])
        .rsplit(['/', '\\'])
        .next()?;
    (!name.is_empty() && !name.chars().any(char::is_control))
        .then(|| name.chars().take(80).collect())
}

fn time(value: i64) -> Option<String> {
    (value >= 0)
        .then(|| chrono::DateTime::from_timestamp(value, 0))
        .flatten()
        .filter(|v| v.format("%Y").to_string().len() == 4)
        .map(|v| v.to_rfc3339_opts(chrono::SecondsFormat::Secs, true))
}

pub(super) fn normalize(
    scope: &str,
    threads: ThreadPage,
    loaded: LoadedPage,
    closed: &BTreeSet<String>,
) -> Result<Snapshot, &'static str> {
    if scope.len() != 64
        || threads.data.len() > MAX_ROWS
        || loaded.data.len() > MAX_ROWS
        || closed.len() > MAX_ROWS
        || loaded.data.iter().chain(closed).any(|v| !id(v))
    {
        return Err("invalid_response");
    }
    let loaded_set: BTreeSet<_> = loaded.data.iter().cloned().collect();
    if loaded_set.len() != loaded.data.len() {
        return Err("invalid_response");
    }
    let mut rows = BTreeMap::new();
    for thread in threads.data {
        if !id(&thread.id)
            || thread.cwd.len() > 4096
            || thread
                .parent_thread_id
                .as_ref()
                .is_some_and(|v| !id(v) || v == &thread.id)
        {
            return Err("invalid_response");
        }
        let created_at = time(thread.created_at).ok_or("invalid_response")?;
        let updated_at = time(thread.updated_at).ok_or("invalid_response")?;
        let source = match &thread.source {
            Source::Label(v) if ["cli", "vscode", "exec", "appServer"].contains(&v.as_str()) => {
                v.as_str()
            }
            Source::Child { .. } => "subagent",
            _ => "unknown",
        };
        let role = if thread.parent_thread_id.is_some() || source == "subagent" {
            "subagent"
        } else if source != "unknown" {
            "main"
        } else {
            "unknown"
        };
        let is_loaded = loaded_set.contains(&thread.id);
        let state = if closed.contains(&thread.id) && !is_loaded {
            "ended"
        } else {
            match (&thread.status, is_loaded) {
                (Status::Idle, true) => "open",
                (Status::Active { flags }, true) if flags.is_empty() => "working",
                (Status::Active { flags }, true)
                    if flags.iter().all(|v| {
                        ["waitingOnApproval", "waitingOnUserInput"].contains(&v.as_str())
                    }) =>
                {
                    "waiting"
                }
                (Status::Error, true) => "error",
                (Status::NotLoaded, false) => "stored",
                (Status::NotLoaded, true) => "loaded",
                _ => "unknown",
            }
        };
        let session_key = key(scope, &thread.id);
        let row = super::Row {
            id_hint: session_key[..8].to_owned(),
            key: session_key,
            client_kind: "codex",
            project: project(&thread.cwd),
            model: thread.model.as_deref().and_then(label),
            provider: label(&thread.model_provider),
            reasoning: thread.reasoning_effort.as_deref().and_then(label),
            created_at: Some(created_at),
            updated_at: Some(updated_at),
            state,
            role,
            parent_key: thread.parent_thread_id.as_deref().map(|v| key(scope, v)),
            traffic_assignment: "unverified",
        };
        if rows.insert(thread.id, row).is_some() {
            return Err("invalid_response");
        }
    }
    for missing in loaded_set.iter().chain(closed) {
        rows.entry(missing.clone()).or_insert_with(|| {
            let session_key = key(scope, missing);
            super::Row {
                id_hint: session_key[..8].to_owned(),
                key: session_key,
                client_kind: "codex",
                project: None,
                model: None,
                provider: None,
                reasoning: None,
                created_at: None,
                updated_at: None,
                state: if loaded_set.contains(missing) {
                    "loaded"
                } else {
                    "ended"
                },
                role: "unknown",
                parent_key: None,
                traffic_assignment: "unverified",
            }
        });
    }
    if rows.len() > MAX_ROWS * 2 {
        return Err("invalid_response");
    }
    let mut rows: Vec<_> = rows.into_values().collect();
    rows.sort_by(|a, b| {
        let rank = |s: &str| match s {
            "working" | "waiting" => 0,
            "open" | "loaded" => 1,
            "error" | "unknown" => 2,
            "ended" => 3,
            _ => 4,
        };
        (rank(a.state), std::cmp::Reverse(&a.updated_at), &a.key).cmp(&(
            rank(b.state),
            std::cmp::Reverse(&b.updated_at),
            &b.key,
        ))
    });
    Ok(Snapshot {
        rows,
        partial: threads.next_cursor.is_some() || loaded.next_cursor.is_some(),
        loaded_count: loaded_set.len(),
    })
}

pub(super) fn requests() -> [serde_json::Value; 4] {
    [
        // This version reserves codex-backend for non-originating readers. An ordinary
        // client name changes shared originator/gateway-login state. Verify the peer first.
        serde_json::json!({"id":1,"method":"initialize","params":{
            "clientInfo":{"name":"codex-backend","title":"Headroom metadata observer","version":"0.1.0"},
            "capabilities":{"experimentalApi":false,"requestAttestation":false}}}),
        serde_json::json!({"method":"initialized"}),
        serde_json::json!({"id":2,"method":"thread/list","params":{
            "limit":PAGE_SIZE,"sortKey":"updated_at","sortDirection":"desc","useStateDbOnly":true,
            "currentAccountOnly":false,"modelProviders":[],"sourceKinds":["cli","vscode","exec","appServer",
                "subAgent","subAgentReview","subAgentCompact","subAgentThreadSpawn","subAgentOther","unknown"]}}),
        serde_json::json!({"id":3,"method":"thread/loaded/list","params":{"limit":PAGE_SIZE}}),
    ]
}
