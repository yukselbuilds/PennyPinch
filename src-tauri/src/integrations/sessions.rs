use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use uuid::Uuid;

#[cfg(windows)]
#[path = "sessions_windows.rs"]
mod platform;
#[path = "sessions_protocol.rs"]
mod protocol;

#[derive(Clone, Serialize)]
pub(crate) struct Row {
    pub key: String,
    pub id_hint: String,
    pub client_kind: &'static str,
    pub project: Option<String>,
    pub model: Option<String>,
    pub provider: Option<String>,
    pub reasoning: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub state: &'static str,
    pub role: &'static str,
    pub parent_key: Option<String>,
    pub traffic_assignment: &'static str,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Operation {
    Read,
    OpenCodexCenter,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Request {
    pub schema_version: u8,
    pub request_id: String,
    pub operation: Operation,
    pub snapshot_id: Option<String>,
    pub confirm_open: bool,
}

#[derive(Serialize)]
struct Scope {
    client_kind: &'static str,
    state: &'static str,
    reason: Option<&'static str>,
    server_version: Option<&'static str>,
    loaded_count: Option<usize>,
    partial: bool,
}
impl Scope {
    fn unknown(client_kind: &'static str, reason: &'static str) -> Self {
        Self {
            client_kind,
            state: "unknown",
            reason: Some(reason),
            server_version: None,
            loaded_count: None,
            partial: false,
        }
    }
}

#[derive(Serialize)]
pub(crate) struct Snapshot {
    snapshot_id: String,
    observed_at_utc: String,
    scope: &'static str,
    clients: Vec<Scope>,
    rows: Vec<Row>,
    can_open_codex: bool,
    traffic_assignment: &'static str,
}

#[derive(Serialize)]
pub(crate) struct Reply {
    pub schema_version: u8,
    pub request_id: String,
    pub status: &'static str,
    pub snapshot: Option<Snapshot>,
    pub error: Option<&'static str>,
}

#[derive(Clone, Default)]
pub(crate) struct SessionManager(Arc<Mutex<Inner>>);
#[derive(Default)]
struct Inner {
    ticket: Option<(String, Instant)>,
    #[cfg(windows)]
    reader: platform::Reader,
}

impl SessionManager {
    pub(crate) fn execute(&self, request: Request) -> Reply {
        if !valid_request(&request) {
            return failure(&request.request_id, "validation_failed");
        }
        let Ok(mut inner) = self.0.try_lock() else {
            return failure(&request.request_id, "busy");
        };
        if matches!(request.operation, Operation::OpenCodexCenter) {
            let fresh = inner.ticket.as_ref().is_some_and(|(id, at)| {
                Some(id) == request.snapshot_id.as_ref() && at.elapsed() < Duration::from_secs(45)
            });
            // Consume consent even if launch fails; replay cannot open another terminal.
            inner.ticket = None;
            if !fresh {
                return failure(&request.request_id, "stale_snapshot");
            }
            #[cfg(windows)]
            return match inner.reader.open_center() {
                Ok(opened) => {
                    drop(opened); // Close our handle only. The user's client survives Headroom.
                    Reply {
                        schema_version: 1,
                        request_id: request.request_id,
                        status: "launched",
                        snapshot: None,
                        error: None,
                    }
                }
                Err(reason) => failure(&request.request_id, reason),
            };
            #[cfg(not(windows))]
            return failure(&request.request_id, "unavailable");
        }
        inner.ticket = None;
        let mut snapshot = Snapshot {
            snapshot_id: Uuid::new_v4().to_string(),
            observed_at_utc: chrono::Utc::now()
                .to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            scope: "current_cli_environment",
            clients: Vec::new(),
            rows: Vec::new(),
            can_open_codex: false,
            traffic_assignment: "unverified",
        };
        #[cfg(windows)]
        match inner.reader.read() {
            Ok(value) => {
                snapshot.clients.push(Scope {
                    client_kind: "codex",
                    state: "observed",
                    reason: None,
                    server_version: Some(protocol::VERSION),
                    loaded_count: Some(value.loaded_count),
                    partial: value.partial,
                });
                snapshot.rows = value.rows;
                snapshot.can_open_codex = inner.reader.can_open_center();
            }
            Err(reason) => snapshot.clients.push(Scope::unknown("codex", reason)),
        }
        #[cfg(not(windows))]
        snapshot
            .clients
            .push(Scope::unknown("codex", "unsupported_metadata"));
        snapshot
            .clients
            .push(Scope::unknown("claude_code", "unsupported_metadata"));
        snapshot
            .clients
            .push(Scope::unknown("opencode", "unsupported_metadata"));
        if snapshot.can_open_codex {
            inner.ticket = Some((snapshot.snapshot_id.clone(), Instant::now()));
        }
        snapshot.observed_at_utc =
            chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        Reply {
            schema_version: 1,
            request_id: request.request_id,
            status: "snapshot",
            snapshot: Some(snapshot),
            error: None,
        }
    }
}

fn valid_request(request: &Request) -> bool {
    let uuid = |v: &str| Uuid::parse_str(v).is_ok_and(|id| !id.is_nil() && id.to_string() == v);
    request.schema_version == 1
        && uuid(&request.request_id)
        && match request.operation {
            Operation::Read => request.snapshot_id.is_none() && !request.confirm_open,
            Operation::OpenCodexCenter => {
                request.confirm_open && request.snapshot_id.as_deref().is_some_and(uuid)
            }
        }
}

pub(crate) fn failure(id: &str, reason: &'static str) -> Reply {
    Reply {
        schema_version: 1,
        request_id: id.into(),
        status: "error",
        snapshot: None,
        error: Some(reason),
    }
}

#[cfg(test)]
#[path = "sessions_tests.rs"]
mod tests;
