//! Bounded local evidence, never an automatic repair or provider verification.
use super::manager::RuntimeManager;
use crate::integrations::config_io::PlanManager;
use crate::integrations::discovery::DiscoveryManager;
use crate::integrations::plan::{supported_installation, ClientKind};
use crate::metrics::activity::{ActivityData, ActivityFilter, ActivityQueryRequest, ActivityReply};
use crate::metrics::collector::OverviewManager;
use crate::metrics::normalize::Optimizer;
use crate::metrics::request_metadata::Outcome;
use crate::metrics::store::Period;
use chrono::{SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use uuid::Uuid;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Request {
    pub schema_version: u8,
    pub request_id: String,
}
#[derive(Serialize)]
#[serde(untagged)]
pub(crate) enum Reply {
    Snapshot(Snapshot),
    Error(Failure),
}
#[derive(Serialize)]
pub(crate) struct Snapshot {
    schema_version: u8,
    request_id: String,
    status: &'static str,
    observed_at_utc: String,
    runtime: Runtime,
    traffic: Traffic,
    clients: Vec<Client>,
}
#[derive(Serialize)]
struct Runtime {
    engine_status: &'static str,
    ingress_status: &'static str,
    error_code: Option<&'static str>,
}
#[derive(Serialize)]
struct Traffic {
    status: &'static str,
    period: &'static str,
    last_observed_utc: Option<String>,
    outcome: Outcome,
    error_layer: &'static str,
}
#[derive(Serialize)]
struct Client {
    client_kind: &'static str,
    detected_count: usize,
    installation_status: &'static str,
    compatibility: &'static str,
    config_state: &'static str,
    config_scope: &'static str,
    endpoint_binding: &'static str,
}
#[derive(Serialize)]
pub(crate) struct Failure {
    schema_version: u8,
    request_id: String,
    status: &'static str,
    error: Error,
}
#[derive(Serialize)]
struct Error {
    code: &'static str,
    retryable: bool,
    message_key: String,
    diagnostic_id: Option<()>,
}

#[derive(Clone, Default)]
pub(crate) struct RecoveryManager(Arc<AtomicBool>);
struct Guard<'a>(&'a AtomicBool);
impl Drop for Guard<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

pub(crate) fn failure(id: String, code: &'static str) -> Reply {
    let id = if Uuid::parse_str(&id).is_ok_and(|u| u.to_string() == id) {
        id
    } else {
        Uuid::nil().to_string()
    };
    Reply::Error(Failure {
        schema_version: 1,
        request_id: id,
        status: "error",
        error: Error {
            code,
            retryable: code == "busy",
            message_key: format!("recovery.{code}"),
            diagnostic_id: None,
        },
    })
}

impl RecoveryManager {
    pub(crate) fn read(
        &self,
        request: Request,
        runtime: &RuntimeManager,
        discovery: &DiscoveryManager,
        plans: &PlanManager,
        overview: &OverviewManager,
    ) -> Reply {
        if request.schema_version != 1
            || !Uuid::parse_str(&request.request_id)
                .is_ok_and(|u| u.to_string() == request.request_id && !u.is_nil())
        {
            return failure(request.request_id, "invalid_input");
        }
        if self
            .0
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return failure(request.request_id, "busy");
        }
        let _guard = Guard(&self.0);
        let scan = discovery.recovery_scan();
        let mut clients = Vec::with_capacity(3);
        let mut bindings = Vec::with_capacity(3);
        for (name, kind) in [
            ("codex", ClientKind::Codex),
            ("claude_code", ClientKind::ClaudeCode),
            ("opencode", ClientKind::OpenCodeV1),
        ] {
            let found: Vec<_> = scan
                .as_ref()
                .map(|s| {
                    s.installations
                        .iter()
                        .filter(|(i, _)| i.client_kind == name)
                        .collect()
                })
                .unwrap_or_default();
            let installation_status = if scan.is_none() {
                "unknown"
            } else if found.is_empty() {
                "not_found"
            } else if found.iter().any(|(_, fresh)| *fresh == "stale") {
                "stale"
            } else if found.iter().all(|(_, fresh)| *fresh == "current") {
                "current"
            } else {
                "unknown"
            };
            let supported = installation_status == "current"
                && found
                    .iter()
                    .all(|(i, _)| supported_installation(kind, i.surface, i.version.as_deref()));
            let config = plans.recovery_config(
                kind,
                if supported {
                    found.first().map(|(i, _)| i)
                } else {
                    None
                },
                scan.as_ref().map_or(0, |s| s.generation),
            );
            bindings.push(config.endpoint);
            clients.push(Client {
                client_kind: name,
                detected_count: found.len(),
                installation_status,
                compatibility: if supported {
                    "config_supported"
                } else {
                    "not_verified"
                },
                config_state: config.state,
                config_scope: config.scope,
                endpoint_binding: "unknown",
            });
        }
        let traffic = match overview.read_activity(ActivityQueryRequest {
            schema_version: 1,
            request_id: Uuid::new_v4().to_string(),
            period: Period::SevenDays,
            filter: ActivityFilter::default(),
        }) {
            ActivityReply::Snapshot(s) => traffic_from_data(s.data),
            ActivityReply::Error(_) => traffic(None, false),
        };
        let observed = runtime.codex_connection_observation();
        let current_scan = discovery.recovery_scan();
        let scan_unchanged = match (&scan, &current_scan) {
            (Some(before), Some(after)) => before.generation == after.generation,
            _ => false,
        };
        for (client, binding) in clients.iter_mut().zip(bindings) {
            let updated = current_scan.as_ref().is_some_and(|s| {
                s.installations
                    .iter()
                    .any(|(i, fresh)| i.client_kind == client.client_kind && *fresh == "stale")
            });
            revalidate_client(client, scan_unchanged, updated);
            client.endpoint_binding = match binding {
                None if client.config_state == "not_owned" => "not_owned",
                Some((id, port)) => match (&observed.endpoint_id, observed.endpoint_port) {
                    (Some(current), Some(current_port))
                        if *current == id && current_port == port =>
                    {
                        "same"
                    }
                    (Some(_), Some(_)) => "different",
                    _ => "unknown",
                },
                _ => "unknown",
            };
        }
        Reply::Snapshot(Snapshot {
            schema_version: 1,
            request_id: request.request_id,
            status: "snapshot",
            observed_at_utc: Utc::now().to_rfc3339_opts(SecondsFormat::AutoSi, true),
            runtime: Runtime {
                engine_status: match observed.engine_status {
                    s @ ("unavailable" | "stopped" | "starting" | "ready" | "draining"
                    | "failed" | "unknown") => s,
                    _ => "unknown",
                },
                ingress_status: match observed.endpoint_status {
                    s @ ("unavailable" | "starting" | "ready" | "unknown") => s,
                    _ => "unavailable",
                },
                error_code: observed.error_code,
            },
            traffic,
            clients,
        })
    }
}

fn revalidate_client(client: &mut Client, same_scan: bool, updated: bool) {
    if !same_scan || updated {
        // A new scan may contain new rows; counts belong to the first scan.
        client.installation_status = if same_scan && updated {
            "stale"
        } else {
            "unknown"
        };
        client.compatibility = "not_verified";
        if client.config_state == "matches_owned_endpoint" {
            client.config_state = "not_rechecked";
        }
    }
}

fn traffic(value: Option<(String, Outcome)>, complete: bool) -> Traffic {
    let (status, last, outcome) = match value {
        Some((at, outcome)) => ("observed", Some(at), outcome),
        None if complete => ("none", None, Outcome::Unknown),
        _ => ("unavailable", None, Outcome::Unknown),
    };
    Traffic {
        status,
        period: "seven_days",
        last_observed_utc: last,
        outcome,
        error_layer: "unknown",
    }
}

fn traffic_from_data(data: ActivityData) -> Traffic {
    let last = data
        .rows
        .into_iter()
        .filter(|r| r.layers.contains(&Optimizer::Headroom))
        .filter_map(|r| {
            let at = r.timestamp_utc?;
            let stamp = chrono::DateTime::parse_from_rfc3339(&at).ok()?;
            let outcome = if r.metadata.conflicted {
                Outcome::Unknown
            } else {
                r.metadata.outcome
            };
            Some((stamp, at, outcome))
        })
        .max_by_key(|(stamp, _, _)| *stamp)
        .map(|(_, at, outcome)| (at, outcome));
    traffic(last, !data.has_more && data.unplaced_requests == 0)
}

#[cfg(test)]
#[path = "recovery_tests.rs"]
mod tests;
