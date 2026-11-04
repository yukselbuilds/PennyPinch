use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use time::{format_description::well_known::Rfc3339, OffsetDateTime, UtcOffset};
use uuid::Uuid;

pub const MAX_TOKEN_COUNT: u64 = 9_007_199_254_740_991;
pub const MAX_OBSERVATIONS: usize = 10_000;
pub const MAX_SNAPSHOT_BYTES: usize = 2 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    Proxy,
    Mcp,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CounterKind {
    LocalEstimate,
    ProviderTokenizer,
    Unknown,
}

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct Basis {
    pub kind: CounterKind,
    pub tokenizer: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Component {
    MessageInput,
    WholeInput,
    ToolSchema,
    NovelInput,
    CacheInput,
    Output,
    Unknown,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Reading {
    pub tokens: Option<u64>,
    pub basis: Basis,
    pub component: Component,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Optimizer {
    Headroom,
    Rtk,
    Cache,
    Output,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct ProviderUsage {
    pub input: Option<u64>,
    pub output: Option<u64>,
    pub cached_input: Option<u64>,
}

// Origins and bases come from a trusted collector, never from arbitrary engine tags.
#[derive(Clone, Debug, Deserialize)]
pub struct Observation {
    pub runtime_generation: String,
    pub request_id: String,
    pub source: Source,
    pub proxy_request_id: Option<String>,
    pub timestamp_utc: Option<String>,
    pub optimizer: Optimizer,
    pub before: Reading,
    pub after: Reading,
    pub provider_usage: ProviderUsage,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MeasurementStatus {
    Measured,
    MissingCounter,
    UnknownBasis,
    Incomparable,
    Uncorrelated,
    MissingTimestamp,
    Conflict,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NormalizedEvent {
    pub schema_version: u8,
    pub runtime_generation: String,
    pub origin_id: String,
    pub event_id: String,
    pub observed_via: Vec<Source>,
    pub timestamp_utc: Option<String>,
    pub optimizer: Optimizer,
    pub before: Reading,
    pub after: Reading,
    pub provider_usage: ProviderUsage,
    pub status: MeasurementStatus,
    pub saved_tokens: Option<i64>,
    pub saved_percent: Option<f64>,
    pub main_metric_eligible: bool,
}

#[derive(Debug, Serialize)]
pub struct NormalizedBatch {
    pub schema_version: u8,
    pub events: Vec<NormalizedEvent>,
    pub observations: usize,
    pub duplicate_observations: usize,
    pub conflicting_events: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NormalizeError {
    TooLarge,
    InvalidPayload,
    InvalidIdentity,
    InvalidTimestamp,
    InvalidCounter,
    InvalidBasis,
    UnsupportedEngineVersion,
}

#[derive(Debug, Serialize)]
pub struct EngineProjection {
    pub batch: NormalizedBatch,
    pub reported_request_count: Option<u64>,
    pub unidentified_rows: usize,
    pub rolling_window: bool,
    pub complete_traffic: bool,
    pub aggregate_only: bool,
}

fn valid_label(value: &str) -> bool {
    !value.is_empty() && value.len() <= 128 && value.bytes().all(|b| b.is_ascii_graphic())
}

fn generation(value: &str) -> Result<String, NormalizeError> {
    let parsed = Uuid::parse_str(value).map_err(|_| NormalizeError::InvalidIdentity)?;
    if parsed.is_nil() {
        return Err(NormalizeError::InvalidIdentity);
    }
    Ok(parsed.to_string())
}

fn validate_count(value: Option<u64>) -> Result<(), NormalizeError> {
    if value.is_some_and(|n| n > MAX_TOKEN_COUNT) {
        return Err(NormalizeError::InvalidCounter);
    }
    Ok(())
}

fn validate_reading(value: &Reading) -> Result<(), NormalizeError> {
    validate_count(value.tokens)?;
    if value
        .basis
        .tokenizer
        .as_deref()
        .is_some_and(|id| !valid_label(id))
        || (value.basis.kind == CounterKind::Unknown && value.basis.tokenizer.is_some())
    {
        return Err(NormalizeError::InvalidBasis);
    }
    Ok(())
}

fn opaque_id(parts: &[&str]) -> String {
    let mut hash = Sha256::new();
    for part in parts {
        hash.update((part.len() as u64).to_be_bytes());
        hash.update(part.as_bytes());
    }
    format!("{:x}", hash.finalize())
}

fn input_component(component: Component) -> bool {
    matches!(component, Component::MessageInput | Component::WholeInput)
}

fn slot(optimizer: Optimizer, component: Component) -> String {
    // Overlapping message/whole-input observations must conflict, never add twice.
    let optimizer = match optimizer {
        Optimizer::Headroom => "headroom",
        Optimizer::Rtk => "rtk",
        Optimizer::Cache => "cache",
        Optimizer::Output => "output",
    };
    let component = match component {
        Component::MessageInput | Component::WholeInput | Component::Unknown => "input",
        Component::ToolSchema => "tool_schema",
        Component::NovelInput => "novel_input",
        Component::CacheInput => "cache_input",
        Component::Output => "output",
    };
    format!("{optimizer}:{component}")
}

fn normalize_one(value: &Observation) -> Result<NormalizedEvent, NormalizeError> {
    let runtime_generation = generation(&value.runtime_generation)?;
    if !valid_label(&value.request_id)
        || value
            .proxy_request_id
            .as_deref()
            .is_some_and(|id| !valid_label(id))
        || (value.source == Source::Proxy
            && value
                .proxy_request_id
                .as_ref()
                .is_some_and(|id| id != &value.request_id))
    {
        return Err(NormalizeError::InvalidIdentity);
    }
    validate_reading(&value.before)?;
    validate_reading(&value.after)?;
    for count in [
        value.provider_usage.input,
        value.provider_usage.output,
        value.provider_usage.cached_input,
    ] {
        validate_count(count)?;
    }
    let timestamp_utc = value
        .timestamp_utc
        .as_deref()
        .map(|stamp| {
            let parsed = OffsetDateTime::parse(stamp, &Rfc3339)
                .map_err(|_| NormalizeError::InvalidTimestamp)?;
            parsed
                .to_offset(UtcOffset::UTC)
                .format(&Rfc3339)
                .map_err(|_| NormalizeError::InvalidTimestamp)
        })
        .transpose()?;
    let proxy_origin = value.source == Source::Proxy || value.proxy_request_id.is_some();
    let origin_namespace = if proxy_origin {
        "proxy"
    } else {
        "mcp-unlinked"
    };
    let origin_request = value
        .proxy_request_id
        .as_deref()
        .unwrap_or(&value.request_id);
    let origin_id = opaque_id(&[&runtime_generation, origin_namespace, origin_request]);
    let status = if value.before.tokens.is_none() || value.after.tokens.is_none() {
        MeasurementStatus::MissingCounter
    } else if value.before.basis.kind == CounterKind::Unknown
        || value.after.basis.kind == CounterKind::Unknown
        || value.before.basis.tokenizer.is_none()
        || value.after.basis.tokenizer.is_none()
    {
        MeasurementStatus::UnknownBasis
    } else if value.before.basis != value.after.basis
        || value.before.component == Component::Unknown
        || value.before.component != value.after.component
    {
        MeasurementStatus::Incomparable
    } else if !proxy_origin {
        MeasurementStatus::Uncorrelated
    } else if timestamp_utc.is_none() {
        MeasurementStatus::MissingTimestamp
    } else {
        MeasurementStatus::Measured
    };
    let (saved_tokens, saved_percent) = if status == MeasurementStatus::Measured {
        let before = value.before.tokens.unwrap();
        let after = value.after.tokens.unwrap();
        let saved = before as i64 - after as i64;
        (
            Some(saved),
            (before > 0).then(|| 100.0 * saved as f64 / before as f64),
        )
    } else {
        (None, None)
    };
    Ok(NormalizedEvent {
        schema_version: 1,
        runtime_generation,
        event_id: opaque_id(&[&origin_id, &slot(value.optimizer, value.before.component)]),
        origin_id,
        observed_via: vec![value.source],
        timestamp_utc,
        optimizer: value.optimizer,
        before: value.before.clone(),
        after: value.after.clone(),
        provider_usage: value.provider_usage.clone(),
        status,
        saved_tokens,
        saved_percent,
        main_metric_eligible: status == MeasurementStatus::Measured
            && value.optimizer == Optimizer::Headroom
            && input_component(value.before.component),
    })
}

pub fn normalize(observations: &[Observation]) -> Result<NormalizedBatch, NormalizeError> {
    if observations.len() > MAX_OBSERVATIONS {
        return Err(NormalizeError::TooLarge);
    }
    let mut events: BTreeMap<String, NormalizedEvent> = BTreeMap::new();
    let mut duplicate_observations = 0;
    let mut conflicting_events = 0;
    for value in observations {
        let incoming = normalize_one(value)?;
        if let Some(current) = events.get_mut(&incoming.event_id) {
            duplicate_observations += 1;
            if merge_event(current, &incoming) {
                conflicting_events += 1;
            }
        } else {
            events.insert(incoming.event_id.clone(), incoming);
        }
    }
    Ok(NormalizedBatch {
        schema_version: 1,
        events: events.into_values().collect(),
        observations: observations.len(),
        duplicate_observations,
        conflicting_events,
    })
}

pub(crate) fn merge_event(current: &mut NormalizedEvent, incoming: &NormalizedEvent) -> bool {
    let mut comparison = incoming.clone();
    comparison.observed_via = current.observed_via.clone();
    let new_conflict = current.status != MeasurementStatus::Conflict && *current != comparison;
    if new_conflict {
        current.status = MeasurementStatus::Conflict;
        current.before = unverified_reading(None);
        current.after = unverified_reading(None);
        current.provider_usage = ProviderUsage::default();
        current.saved_tokens = None;
        current.saved_percent = None;
        current.main_metric_eligible = false;
    }
    if current.status == MeasurementStatus::Conflict
        && current.timestamp_utc != incoming.timestamp_utc
    {
        current.timestamp_utc = None;
    }
    for source in &incoming.observed_via {
        if !current.observed_via.contains(source) {
            current.observed_via.push(*source);
        }
    }
    current.observed_via.sort_by_key(|s| match s {
        Source::Proxy => 0,
        Source::Mcp => 1,
    });
    new_conflict
}

/// Revalidate the bounded schema at the persistent-store trust boundary.
pub(crate) fn validate_event(value: &NormalizedEvent) -> Result<(), NormalizeError> {
    let valid_hash = |s: &str| {
        s.len() == 64
            && s.bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    };
    if value.schema_version != 1
        || generation(&value.runtime_generation)? != value.runtime_generation
        || !valid_hash(&value.origin_id)
        || !valid_hash(&value.event_id)
        || value.observed_via.is_empty()
        || value.observed_via.len() > 2
        || (value.observed_via.len() == 2 && value.observed_via != [Source::Proxy, Source::Mcp])
    {
        return Err(NormalizeError::InvalidIdentity);
    }
    if value.status != MeasurementStatus::Conflict
        && value.event_id
            != opaque_id(&[
                &value.origin_id,
                &slot(value.optimizer, value.before.component),
            ])
    {
        return Err(NormalizeError::InvalidIdentity);
    }
    validate_reading(&value.before)?;
    validate_reading(&value.after)?;
    for count in [
        value.provider_usage.input,
        value.provider_usage.output,
        value.provider_usage.cached_input,
    ] {
        validate_count(count)?;
    }
    if let Some(stamp) = &value.timestamp_utc {
        let parsed =
            OffsetDateTime::parse(stamp, &Rfc3339).map_err(|_| NormalizeError::InvalidTimestamp)?;
        if parsed.offset() != UtcOffset::UTC {
            return Err(NormalizeError::InvalidTimestamp);
        }
    }
    if value.status == MeasurementStatus::Measured {
        let (Some(before), Some(after)) = (value.before.tokens, value.after.tokens) else {
            return Err(NormalizeError::InvalidCounter);
        };
        let saved = before as i64 - after as i64;
        let percent = (before > 0).then(|| 100.0 * saved as f64 / before as f64);
        if value.timestamp_utc.is_none()
            || value.before.basis != value.after.basis
            || value.before.basis.kind == CounterKind::Unknown
            || value.before.basis.tokenizer.is_none()
            || value.before.component == Component::Unknown
            || value.before.component != value.after.component
            || value.saved_tokens != Some(saved)
            || value.saved_percent != percent
            || value.main_metric_eligible
                != (value.optimizer == Optimizer::Headroom
                    && input_component(value.before.component))
        {
            return Err(NormalizeError::InvalidBasis);
        }
    } else if value.main_metric_eligible
        || value.saved_tokens.is_some()
        || value.saved_percent.is_some()
    {
        return Err(NormalizeError::InvalidBasis);
    }
    Ok(())
}

#[derive(Deserialize)]
struct RequestCounts {
    total: Option<u64>,
}

#[derive(Deserialize)]
struct EngineSnapshot {
    recent_requests: Option<Rows>,
    request_logs: Option<Rows>,
    requests: Option<RequestCounts>,
}

#[derive(Deserialize)]
struct RawRow {
    request_id: Option<String>,
    timestamp: Option<String>,
    input_tokens_original: Option<u64>,
    input_tokens_optimized: Option<u64>,
}

struct Rows(Vec<RawRow>);

impl<'de> Deserialize<'de> for Rows {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct BoundedRows;
        impl<'de> serde::de::Visitor<'de> for BoundedRows {
            type Value = Rows;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a bounded request list")
            }
            fn visit_seq<A: serde::de::SeqAccess<'de>>(self, mut seq: A) -> Result<Rows, A::Error> {
                let mut values = Vec::new();
                while let Some(value) = seq.next_element()? {
                    if values.len() == MAX_OBSERVATIONS {
                        return Err(serde::de::Error::custom("request limit"));
                    }
                    values.push(value);
                }
                Ok(Rows(values))
            }
        }
        deserializer.deserialize_seq(BoundedRows)
    }
}

fn unverified_reading(tokens: Option<u64>) -> Reading {
    Reading {
        tokens,
        basis: Basis {
            kind: CounterKind::Unknown,
            tokenizer: None,
        },
        component: Component::Unknown,
    }
}

pub fn project_headroom_041(
    payload: &[u8],
    runtime_generation: &str,
    engine_version: &str,
    source: Source,
) -> Result<EngineProjection, NormalizeError> {
    if engine_version != "0.41.0" {
        return Err(NormalizeError::UnsupportedEngineVersion);
    }
    generation(runtime_generation)?;
    if payload.len() > MAX_SNAPSHOT_BYTES {
        return Err(NormalizeError::TooLarge);
    }
    let snapshot: EngineSnapshot =
        serde_json::from_slice(payload).map_err(|_| NormalizeError::InvalidPayload)?;
    let reported_request_count = snapshot.requests.as_ref().and_then(|r| r.total);
    validate_count(reported_request_count)?;
    let rows = snapshot.recent_requests.or(snapshot.request_logs);
    if rows.is_none() && snapshot.requests.is_none() {
        return Err(NormalizeError::InvalidPayload);
    }
    let aggregate_only = rows.is_none();
    let mut unidentified_rows = 0;
    let mut observations = Vec::new();
    for row in rows.map(|rows| rows.0).unwrap_or_default() {
        let Some(request_id) = row.request_id.filter(|id| !id.is_empty()) else {
            unidentified_rows += 1;
            continue;
        };
        observations.push(Observation {
            runtime_generation: runtime_generation.into(),
            request_id,
            source,
            // A raw MCP ID/tag cannot attest ownership of a proxy request.
            proxy_request_id: None,
            timestamp_utc: row.timestamp,
            optimizer: Optimizer::Headroom,
            before: unverified_reading(row.input_tokens_original),
            after: unverified_reading(row.input_tokens_optimized),
            // Legacy logger defaults cannot prove provider-usage provenance either.
            provider_usage: ProviderUsage::default(),
        });
    }
    Ok(EngineProjection {
        batch: normalize(&observations)?,
        reported_request_count,
        unidentified_rows,
        rolling_window: !aggregate_only,
        complete_traffic: false,
        aggregate_only,
    })
}
