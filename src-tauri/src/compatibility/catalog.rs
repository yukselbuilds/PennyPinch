//! Historical scoped evidence, never a current account or conversation verification.
use crate::integrations::plan::{supported_installation, ClientKind};
use crate::integrations::{config_io::PlanManager, discovery::DiscoveryManager};
use crate::runtime::manager::{CatalogRuntime, RuntimeManager};
use chrono::{NaiveDate, SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashSet};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, OnceLock,
};
use uuid::Uuid;

const SOURCE: &str = include_str!("../../../compatibility/clients.json");
#[cfg(not(target_os = "macos"))]
const MANIFEST: &str = crate::runtime::manager::ACTIVE_MANIFEST;
const STATUSES: [&str; 7] = [
    "not_tested",
    "unknown",
    "environment_blocked",
    "local_fixture_partial",
    "local_fixture_pass",
    "live_verified",
    "unsupported",
];

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Request {
    pub schema_version: u8,
    pub request_id: String,
}

#[derive(Deserialize)]
struct Catalog {
    schema_version: u8,
    last_updated: String,
    allowed_statuses: Vec<String>,
    release: Release,
    records: Vec<RawRecord>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Release {
    channel: String,
    app_version: String,
    engine_version: String,
}
#[derive(Deserialize)]
struct RawRecord {
    id: String,
    product: String,
    version: String,
    surface: String,
    status: String,
    evidence: Vec<String>,
    evidence_date: String,
    capabilities: BTreeMap<String, RawCapability>,
    #[serde(rename = "match")]
    target: Target,
}
#[derive(Deserialize)]
struct RawCapability {
    status: String,
    evidence: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Target {
    client_kind: String,
    surface: String,
    versions: Vec<String>,
    auth_method: String,
    platform: String,
    engine_version: String,
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
    catalog_revision: String,
    catalog_date: String,
    channel: &'static str,
    app_version: &'static str,
    engine: CatalogRuntime,
    automatic_updates: bool,
    updater_available: bool,
    adapters: Vec<Adapter>,
    records: Vec<Record>,
    scan_generation: Option<u64>,
    context_fingerprint: Option<String>,
    installations: Vec<Installation>,
}
#[derive(Serialize)]
struct Adapter {
    client_kind: &'static str,
    fingerprint: String,
}
#[derive(Serialize)]
struct Record {
    id: String,
    client_kind: String,
    product: String,
    version: String,
    surface: String,
    tested_versions: Vec<String>,
    capability_counts: Vec<CapabilityCount>,
    auth_method: String,
    platform: String,
    engine_version: String,
    evidence_status: String,
    evidence_date: String,
    evidence: Vec<String>,
    scope: String,
}
#[derive(Serialize)]
struct CapabilityCount {
    status: &'static str,
    count: usize,
}
#[derive(Serialize)]
struct Installation {
    installation_id: String,
    client_kind: &'static str,
    surface: &'static str,
    version: Option<String>,
    freshness: &'static str,
    config_supported: bool,
    config_state: &'static str,
    historical_evidence_ids: Vec<String>,
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
    diagnostic_id: Option<String>,
}

#[derive(Clone, Default)]
pub(crate) struct CatalogManager(Arc<AtomicBool>);
struct Guard<'a>(&'a AtomicBool);
impl Drop for Guard<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn bounded(value: &str, max: usize) -> bool {
    !value.is_empty() && value.len() <= max && !value.chars().any(char::is_control)
}
fn date(value: &str) -> bool {
    NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .is_ok_and(|d| d.format("%Y-%m-%d").to_string() == value)
}
fn evidence_path(value: &str) -> bool {
    value.len() <= 160
        && (value.starts_with("docs/proofs/") || value.starts_with("docs/reviews/"))
        && value.ends_with(".md")
        && value
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"/-_.".contains(&c))
        && !value.contains("..")
}

#[cfg(any(target_os = "macos", test))]
fn macos_manifest_pin_available(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
        && value.bytes().any(|c| c != b'0')
}
fn parse(source: &str) -> Result<Catalog, ()> {
    if source.len() > 192 * 1024 {
        return Err(());
    }
    let value: Catalog = serde_json::from_str(source).map_err(|_| ())?;
    let mut ids = HashSet::new();
    let allowed: HashSet<_> = value.allowed_statuses.iter().map(String::as_str).collect();
    if value.schema_version != 1
        || !date(&value.last_updated)
        || value.release.channel != "development"
        || value.release.app_version != env!("CARGO_PKG_VERSION")
        || !bounded(&value.release.engine_version, 64)
        || allowed != STATUSES.into_iter().collect()
        || allowed.len() != value.allowed_statuses.len()
        || value.records.is_empty()
        || value.records.len() > 32
    {
        return Err(());
    }
    for row in &value.records {
        let m = &row.target;
        if !bounded(&row.id, 96)
            || !row
                .id
                .bytes()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
            || !ids.insert(row.id.as_str())
            || !bounded(&row.product, 120)
            || !bounded(&row.version, 160)
            || !bounded(&row.surface, 320)
            || !STATUSES.contains(&row.status.as_str())
            || !date(&row.evidence_date)
            || row.evidence_date > value.last_updated
            || row.evidence.is_empty()
            || row.evidence.len() > 8
            || !row.evidence.iter().all(|p| evidence_path(p))
            || row.capabilities.is_empty()
            || row.capabilities.len() > 32
            || !row
                .capabilities
                .values()
                .all(|c| STATUSES.contains(&c.status.as_str()) && evidence_path(&c.evidence))
            || !["engine", "codex", "claude_code", "opencode"].contains(&m.client_kind.as_str())
            || !["runtime", "cli", "desktop", "cli_desktop", "other"].contains(&m.surface.as_str())
            || ![
                "none",
                "synthetic",
                "chatgpt",
                "api_key",
                "oauth",
                "unknown",
            ]
            .contains(&m.auth_method.as_str())
            || ![
                "windows_10_x64",
                "windows_11_x64",
                "macos",
                "linux",
                "unknown",
            ]
            .contains(&m.platform.as_str())
            || !bounded(&m.engine_version, 64)
            || m.versions.len() > 8
            || !m.versions.iter().all(|v| bounded(v, 64))
            || (row.status == "live_verified"
                && (m.versions.is_empty()
                    || m.platform == "unknown"
                    || !["chatgpt", "api_key", "oauth"].contains(&m.auth_method.as_str())))
        {
            return Err(());
        }
    }
    Ok(value)
}

fn adapters() -> Vec<Adapter> {
    let shared = [
        include_bytes!("../integrations/plan.rs").as_slice(),
        include_bytes!("../integrations/config_document.rs").as_slice(),
        include_bytes!("../integrations/config_io.rs").as_slice(),
    ];
    [
        (
            "codex",
            include_bytes!("../integrations/codex_binary.rs").as_slice(),
        ),
        (
            "claude_code",
            include_bytes!("../integrations/config_io/claude_layers.rs").as_slice(),
        ),
        (
            "opencode",
            include_bytes!("../integrations/opencode_routes.rs").as_slice(),
        ),
    ]
    .into_iter()
    .map(|(client_kind, specific)| {
        let mut d = Sha256::new();
        d.update(b"headroom-config-adapter/v1\0");
        d.update(client_kind.as_bytes());
        for source in shared.into_iter().chain([specific]) {
            d.update((source.len() as u64).to_le_bytes());
            d.update(source);
        }
        Adapter {
            client_kind,
            fingerprint: format!("{:x}", d.finalize()),
        }
    })
    .collect()
}

pub(crate) fn failure(request_id: String, code: &'static str) -> Reply {
    let request_id = Uuid::parse_str(&request_id)
        .ok()
        .filter(|id| !id.is_nil() && id.to_string() == request_id)
        .map_or_else(|| Uuid::nil().to_string(), |id| id.to_string());
    Reply::Error(Failure {
        schema_version: 1,
        request_id,
        status: "error",
        error: Error {
            code,
            retryable: code == "busy",
            message_key: format!("compatibility.{code}"),
            diagnostic_id: None,
        },
    })
}

impl CatalogManager {
    pub(crate) fn read(
        &self,
        request: Request,
        runtime: &RuntimeManager,
        discovery: &DiscoveryManager,
        plans: &PlanManager,
    ) -> Reply {
        if request.schema_version != 1
            || !Uuid::parse_str(&request.request_id)
                .is_ok_and(|id| !id.is_nil() && id.to_string() == request.request_id)
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
        static CATALOG: OnceLock<Result<Catalog, ()>> = OnceLock::new();
        let Ok(catalog) = CATALOG.get_or_init(|| parse(SOURCE)) else {
            return failure(request.request_id, "unavailable");
        };
        let engine = runtime.catalog_runtime();
        #[cfg(not(target_os = "macos"))]
        {
            let Ok(manifest) = serde_json::from_str::<serde_json::Value>(MANIFEST) else {
                return failure(request.request_id, "unavailable");
            };
            if engine.manifest_sha256 != digest(MANIFEST.as_bytes())
                || manifest["engine"]["version"] != engine.pinned_version
            {
                return failure(request.request_id, "unavailable");
            }
        }
        // macOS supplies its compiled payload pin here; MacRuntime validates the actual resources before start.
        #[cfg(target_os = "macos")]
        if !macos_manifest_pin_available(engine.manifest_sha256) {
            return failure(request.request_id, "unavailable");
        }
        if catalog.release.engine_version != engine.pinned_version {
            return failure(request.request_id, "unavailable");
        }
        let revision = digest(SOURCE.as_bytes());
        let adapters = adapters();
        let scan = discovery.recovery_scan();
        let mut context = Sha256::new();
        context.update(revision.as_bytes());
        for adapter in &adapters {
            context.update(adapter.fingerprint.as_bytes());
        }
        context.update(engine.pinned_version.as_bytes());
        context.update(
            engine
                .active_version
                .as_deref()
                .unwrap_or("none")
                .as_bytes(),
        );
        let mut context_known = scan.is_some();
        let mut installations = Vec::new();
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
            let supported = !found.is_empty()
                && found.iter().all(|(i, f)| {
                    *f == "current" && supported_installation(kind, i.surface, i.version.as_deref())
                });
            let config = plans.recovery_config(
                kind,
                if supported {
                    found.first().map(|(i, _)| i)
                } else {
                    None
                },
                scan.as_ref().map_or(0, |s| s.generation),
            );
            if ["busy", "unavailable", "not_rechecked"].contains(&config.state) {
                context_known = false;
            }
            for value in [
                name,
                config.state,
                config.scope,
                config.context_fingerprint.as_deref().unwrap_or("none"),
            ] {
                context.update(value.as_bytes());
                context.update([0]);
            }
            for (i, freshness) in found {
                let config_supported = *freshness == "current"
                    && supported_installation(kind, i.surface, i.version.as_deref());
                if *freshness != "current" {
                    context_known = false;
                }
                for value in [
                    i.installation_id.as_str(),
                    i.surface,
                    i.version.as_deref().unwrap_or("none"),
                    freshness,
                ] {
                    context.update(value.as_bytes());
                    context.update([0]);
                }
                let historical_evidence_ids = if *freshness == "current" {
                    catalog
                        .records
                        .iter()
                        .filter(|r| {
                            r.target.client_kind == name
                                && (r.target.surface == i.surface
                                    || r.target.surface == "cli_desktop")
                                && r.target.engine_version == engine.pinned_version
                                && i.version
                                    .as_ref()
                                    .is_some_and(|v| r.target.versions.contains(v))
                        })
                        .map(|r| r.id.clone())
                        .collect()
                } else {
                    Vec::new()
                };
                installations.push(Installation {
                    installation_id: i.installation_id.clone(),
                    client_kind: name,
                    surface: i.surface,
                    version: i.version.clone(),
                    freshness,
                    config_supported,
                    config_state: config.state,
                    historical_evidence_ids,
                });
            }
        }
        let end = discovery.recovery_scan();
        let same_scan = scan.as_ref().zip(end.as_ref()).is_some_and(|(a, b)| {
            a.generation == b.generation
                && a.installations.len() == b.installations.len()
                && a.installations
                    .iter()
                    .zip(&b.installations)
                    .all(|((a, f), (b, g))| a.installation_id == b.installation_id && f == g)
        });
        if !same_scan {
            context_known = false;
            for i in &mut installations {
                i.freshness = "unknown";
                i.config_supported = false;
                i.config_state = "not_rechecked";
                i.historical_evidence_ids.clear();
            }
        }
        Reply::Snapshot(Snapshot {
            schema_version: 1,
            request_id: request.request_id,
            status: "snapshot",
            observed_at_utc: Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true),
            catalog_revision: revision,
            catalog_date: catalog.last_updated.clone(),
            channel: "development",
            app_version: env!("CARGO_PKG_VERSION"),
            engine,
            automatic_updates: false,
            updater_available: false,
            adapters,
            scan_generation: if same_scan {
                scan.as_ref().map(|s| s.generation)
            } else {
                None
            },
            context_fingerprint: context_known.then(|| format!("{:x}", context.finalize())),
            installations,
            records: catalog
                .records
                .iter()
                .map(|r| Record {
                    id: r.id.clone(),
                    client_kind: r.target.client_kind.clone(),
                    product: r.product.clone(),
                    version: r.version.clone(),
                    surface: r.target.surface.clone(),
                    tested_versions: r.target.versions.clone(),
                    capability_counts: STATUSES
                        .into_iter()
                        .filter_map(|status| {
                            let count = r
                                .capabilities
                                .values()
                                .filter(|c| c.status == status)
                                .count();
                            (count > 0).then_some(CapabilityCount { status, count })
                        })
                        .collect(),
                    auth_method: r.target.auth_method.clone(),
                    platform: r.target.platform.clone(),
                    engine_version: r.target.engine_version.clone(),
                    evidence_status: r.status.clone(),
                    evidence_date: r.evidence_date.clone(),
                    evidence: r.evidence.clone(),
                    scope: r.surface.clone(),
                })
                .collect(),
        })
    }
}

#[cfg(test)]
#[path = "catalog_tests.rs"]
mod tests;
