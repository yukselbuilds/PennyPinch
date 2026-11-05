//! Attribution is separate from token comparability and never inferred from model names.
use super::normalize::{opaque_id, NormalizeError, NormalizedEvent};
use super::query::PeriodTotals;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Client {
    Codex,
    ClaudeCode,
    #[serde(rename = "opencode")]
    OpenCode,
}
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentRole {
    Unknown,
    Main,
    Subagent,
}
#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Agent {
    pub id: String,
    pub parent_id: Option<String>,
    pub role: AgentRole,
}
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Route {
    pub origin_id: String,
    pub upstream_id: String,
    pub client: Client,
    pub agent: Option<Agent>,
}
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Attribution {
    pub model: Option<String>,
    pub provider: Option<String>,
    pub route: Option<Route>,
    pub conflicted: bool,
}

impl Attribution {
    pub fn is_unknown(&self) -> bool {
        self == &Self::default()
    }
    pub(crate) fn observed(model: Option<&str>, provider: Option<&str>) -> Self {
        Self {
            model: model.filter(|s| safe_label(s)).map(str::to_owned),
            provider: provider.filter(|s| safe_label(s)).map(str::to_owned),
            ..Self::default()
        }
    }
}

pub(crate) fn safe_label(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-/:+@".contains(&b))
        && !value.contains(":/")
        && !value.starts_with('/')
        && !lower.starts_with("sk-")
        && !lower.starts_with("ghp_")
        && !lower.starts_with("github_pat_")
        && lower != "unknown"
}
fn identity(value: &str) -> bool {
    !value.is_empty() && value.len() <= 128 && value.bytes().all(|b| b.is_ascii_graphic())
}
pub(crate) fn hash(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// App-owned request-route collectors only; engine model/header/tag claims are not witnesses.
pub(crate) fn native_route(
    generation: &str,
    request_id: &str,
    upstream_key: &str,
    client: Client,
    agent: Option<(&str, AgentRole, Option<&str>)>,
) -> Result<Route, NormalizeError> {
    let parsed = uuid::Uuid::parse_str(generation).map_err(|_| NormalizeError::InvalidIdentity)?;
    if parsed.is_nil()
        || !identity(request_id)
        || !hash(upstream_key)
        || agent.is_some_and(|(id, role, parent)| {
            !identity(id)
                || parent.is_some_and(|p| !identity(p) || p == id)
                || role == AgentRole::Main && parent.is_some()
        })
    {
        return Err(NormalizeError::InvalidIdentity);
    }
    let generation = parsed.to_string();
    Ok(Route {
        origin_id: opaque_id(&[&generation, "proxy", request_id]),
        upstream_id: opaque_id(&["upstream", &generation, upstream_key]),
        client,
        agent: agent.map(|(id, role, parent)| Agent {
            id: opaque_id(&["agent", &generation, id]),
            parent_id: parent.map(|id| opaque_id(&["agent", &generation, id])),
            role,
        }),
    })
}

pub(crate) fn validate(value: &Attribution, origin: &str) -> Result<(), NormalizeError> {
    if value.model.as_deref().is_some_and(|s| !safe_label(s))
        || value.provider.as_deref().is_some_and(|s| !safe_label(s))
        || value.conflicted
            && (value.model.is_some() || value.provider.is_some() || value.route.is_some())
    {
        return Err(NormalizeError::InvalidIdentity);
    }
    if let Some(route) = &value.route {
        if route.origin_id != origin
            || !hash(&route.upstream_id)
            || route.agent.as_ref().is_some_and(|a| {
                !hash(&a.id)
                    || a.parent_id
                        .as_deref()
                        .is_some_and(|p| !hash(p) || p == a.id)
                    || a.role == AgentRole::Main && a.parent_id.is_some()
            })
        {
            return Err(NormalizeError::InvalidIdentity);
        }
    }
    Ok(())
}

fn enrich<T: Clone + PartialEq>(current: &mut Option<T>, incoming: &Option<T>) -> bool {
    match (&current, incoming) {
        (Some(a), Some(b)) if a != b => false,
        (None, Some(_)) => {
            *current = incoming.clone();
            true
        }
        _ => true,
    }
}
pub(crate) fn merge(current: &mut Attribution, incoming: &Attribution) {
    if current.conflicted {
        return;
    }
    let mut compatible = !incoming.conflicted
        && enrich(&mut current.model, &incoming.model)
        && enrich(&mut current.provider, &incoming.provider);
    if let Some(next) = &incoming.route {
        if let Some(route) = &mut current.route {
            compatible &= route.origin_id == next.origin_id
                && route.client == next.client
                && route.upstream_id == next.upstream_id;
            if let Some(agent) = &next.agent {
                if let Some(before) = &mut route.agent {
                    compatible &=
                        before.id == agent.id && enrich(&mut before.parent_id, &agent.parent_id);
                    if before.role == AgentRole::Unknown {
                        before.role = agent.role;
                    } else if agent.role != AgentRole::Unknown && agent.role != before.role {
                        compatible = false;
                    }
                    if before.role == AgentRole::Main && before.parent_id.is_some() {
                        compatible = false;
                    }
                } else {
                    route.agent = Some(agent.clone());
                }
            }
        } else {
            current.route = Some(next.clone());
        }
    }
    if !compatible {
        *current = Attribution {
            conflicted: true,
            ..Attribution::default()
        };
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CoverageStatus {
    Unknown,
    Partial,
    Verified,
}
#[derive(Clone, Debug, Default, Serialize)]
pub struct Counts {
    pub requests: u64,
    pub events: u64,
    pub model_observed: u64,
    pub client_verified: u64,
    pub agent_verified: u64,
    pub conflicts: u64,
}
impl Counts {
    fn add(&mut self, other: &Self) {
        self.requests += other.requests;
        self.events += other.events;
        self.model_observed += other.model_observed;
        self.client_verified += other.client_verified;
        self.agent_verified += other.agent_verified;
        self.conflicts += other.conflicts;
    }
}
#[derive(Debug, Serialize)]
pub struct AttributionRow {
    pub group_id: String,
    pub model: Option<String>,
    pub provider: Option<String>,
    pub client: Option<Client>,
    pub upstream_id: Option<String>,
    pub agent: Option<Agent>,
    pub conflicted: bool,
    pub is_other: bool,
    pub coverage: Counts,
    pub totals: PeriodTotals,
}
#[derive(Debug, Serialize)]
pub struct AttributionSummary {
    pub scope: &'static str,
    pub coverage: Counts,
    pub client_status: CoverageStatus,
    pub agent_status: CoverageStatus,
    pub groups: Vec<AttributionRow>,
    pub omitted_groups: u64,
}
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
struct Group {
    model: Option<String>,
    provider: Option<String>,
    client: Option<Client>,
    upstream_id: Option<String>,
    agent: Option<Agent>,
    conflicted: bool,
}
type Bucket<'a> = (Counts, Vec<&'a NormalizedEvent>);
fn row(key: Option<Group>, bucket: Bucket<'_>) -> AttributionRow {
    let id = key
        .as_ref()
        .map(|key| serde_json::to_string(key).expect("fixed metadata representation"))
        .unwrap_or_else(|| "other".into());
    let unknown = Group {
        model: None,
        provider: None,
        client: None,
        upstream_id: None,
        agent: None,
        conflicted: false,
    };
    let is_other = key.is_none();
    let key = key.unwrap_or(unknown);
    AttributionRow {
        group_id: opaque_id(&["attribution", &id]),
        model: key.model,
        provider: key.provider,
        client: key.client,
        upstream_id: key.upstream_id,
        agent: key.agent,
        conflicted: key.conflicted,
        is_other,
        coverage: bucket.0,
        totals: super::query::aggregate_event_refs(bucket.1),
    }
}
fn status(known: u64, total: u64) -> CoverageStatus {
    if known == 0 {
        CoverageStatus::Unknown
    } else if known == total {
        CoverageStatus::Verified
    } else {
        CoverageStatus::Partial
    }
}
pub(crate) fn group_origins(
    events: &[NormalizedEvent],
) -> BTreeMap<&str, (Attribution, Vec<&NormalizedEvent>)> {
    let mut origins: BTreeMap<&str, (Attribution, Vec<&NormalizedEvent>)> = BTreeMap::new();
    for event in events {
        let origin = origins.entry(&event.origin_id).or_default();
        merge(&mut origin.0, &event.attribution);
        origin.1.push(event);
    }
    origins
}
pub(crate) fn aggregate(events: &[NormalizedEvent]) -> AttributionSummary {
    let origins = group_origins(events);
    let mut groups: BTreeMap<Group, Bucket<'_>> = BTreeMap::new();
    let mut total = Counts::default();
    for (value, events) in origins.into_values() {
        let route = value.route.as_ref();
        let counts = Counts {
            requests: 1,
            events: events.len() as u64,
            model_observed: u64::from(value.model.is_some()),
            client_verified: u64::from(route.is_some()),
            agent_verified: u64::from(
                route
                    .and_then(|r| r.agent.as_ref())
                    .is_some_and(|a| a.role != AgentRole::Unknown),
            ),
            conflicts: u64::from(value.conflicted),
        };
        total.add(&counts);
        let key = Group {
            model: value.model,
            provider: value.provider,
            client: route.map(|r| r.client),
            upstream_id: route.map(|r| r.upstream_id.clone()),
            agent: route.and_then(|r| r.agent.clone()),
            conflicted: value.conflicted,
        };
        let bucket = groups.entry(key).or_default();
        bucket.0.add(&counts);
        bucket.1.extend(events);
    }
    let folded = if groups.len() > 64 {
        groups.len() - 63
    } else {
        0
    };
    let visible = if folded > 0 { 63 } else { 64 };
    let mut rows = Vec::new();
    let mut other: Bucket<'_> = Default::default();
    for (index, (key, bucket)) in groups.into_iter().enumerate() {
        if index < visible {
            rows.push(row(Some(key), bucket));
        } else {
            other.0.add(&bucket.0);
            other.1.extend(bucket.1);
        }
    }
    if folded > 0 {
        rows.push(row(None, other));
    }
    AttributionSummary {
        scope: "observed_requests",
        client_status: status(total.client_verified, total.requests),
        agent_status: status(total.agent_verified, total.requests),
        coverage: total,
        groups: rows,
        omitted_groups: folded as u64,
    }
}
