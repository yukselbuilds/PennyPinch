//! Read-only, allowlisted activity projections of the same period selected for Overview.
use super::attribution::{self, Agent, Attribution, Client};
use super::collector::CollectionSnapshot;
use super::normalize::{
    opaque_id, Component, CounterKind, MeasurementStatus, NormalizedEvent, Optimizer,
    ProviderUsage, Reading, Source,
};
use super::query::{aggregate_event_refs, MetricSummary};
use super::request_metadata::{Outcome, RequestMetadata};
use super::store::{
    Coverage, MetricsError, MetricsFailure, MetricsQueryRequest, MetricsStore, Period,
};
use chrono::{DateTime, SecondsFormat, TimeZone, Utc};
use serde::{Deserialize, Serialize};

pub const LIST_LIMIT: usize = 100;
pub const DETAIL_LIMIT: usize = 32;
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ClientFilter {
    #[default]
    Any,
    Unknown,
    Codex,
    ClaudeCode,
    #[serde(rename = "opencode")]
    OpenCode,
}
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ModelFilter {
    Any {},
    Unknown {},
    Exact { value: String },
}
impl Default for ModelFilter {
    fn default() -> Self {
        Self::Any {}
    }
}
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OutcomeFilter {
    #[default]
    Any,
    Unknown,
    NoErrorReported,
    ErrorReported,
}
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MeasurementFilter {
    #[default]
    Any,
    Measured,
    Unmeasurable,
    Conflict,
}
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ActivityFilter {
    pub client: ClientFilter,
    pub model: ModelFilter,
    pub outcome: OutcomeFilter,
    pub measurement: MeasurementFilter,
}
impl ActivityFilter {
    fn valid(&self) -> bool {
        match &self.model {
            ModelFilter::Exact { value } => attribution::safe_label(value),
            _ => true,
        }
    }
    fn matches(&self, row: &ActivityRow) -> bool {
        let client = match self.client {
            ClientFilter::Any => true,
            ClientFilter::Unknown => row.identity.client.is_none(),
            ClientFilter::Codex => row.identity.client == Some(Client::Codex),
            ClientFilter::ClaudeCode => row.identity.client == Some(Client::ClaudeCode),
            ClientFilter::OpenCode => row.identity.client == Some(Client::OpenCode),
        };
        let model = match &self.model {
            ModelFilter::Any {} => true,
            ModelFilter::Unknown {} => row.identity.model.is_none(),
            ModelFilter::Exact { value } => row.identity.model.as_ref() == Some(value),
        };
        let outcome = match self.outcome {
            OutcomeFilter::Any => true,
            OutcomeFilter::Unknown => row.metadata.outcome == Outcome::Unknown,
            OutcomeFilter::NoErrorReported => row.metadata.outcome == Outcome::NoErrorReported,
            OutcomeFilter::ErrorReported => row.metadata.outcome == Outcome::ErrorReported,
        };
        client
            && model
            && outcome
            && (self.measurement == MeasurementFilter::Any || self.measurement == row.measurement)
    }
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActivityQueryRequest {
    pub schema_version: u8,
    pub request_id: String,
    pub period: Period,
    pub filter: ActivityFilter,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActivityDetailRequest {
    pub schema_version: u8,
    pub request_id: String,
    pub period: Period,
    pub origin_id: String,
}
fn valid_request(version: u8, id: &str, period: Period) -> bool {
    MetricsQueryRequest {
        schema_version: version,
        request_id: id.to_owned(),
        period,
    }
    .is_valid()
}
impl ActivityQueryRequest {
    pub(crate) fn is_valid(&self) -> bool {
        valid_request(self.schema_version, &self.request_id, self.period) && self.filter.valid()
    }
}
impl ActivityDetailRequest {
    pub(crate) fn is_valid(&self) -> bool {
        valid_request(self.schema_version, &self.request_id, self.period)
            && attribution::hash(&self.origin_id)
    }
}

#[derive(Debug, Serialize)]
pub struct ActivityIdentity {
    pub model: Option<String>,
    pub provider: Option<String>,
    pub client: Option<Client>,
    pub upstream_id: Option<String>,
    pub agent: Option<Agent>,
    pub conflicted: bool,
}
impl From<Attribution> for ActivityIdentity {
    fn from(v: Attribution) -> Self {
        Self {
            model: v.model,
            provider: v.provider,
            client: v.route.as_ref().map(|r| r.client),
            upstream_id: v.route.as_ref().map(|r| r.upstream_id.clone()),
            agent: v.route.and_then(|r| r.agent),
            conflicted: v.conflicted,
        }
    }
}
#[derive(Debug, Serialize)]
pub struct ActivityRow {
    pub origin_id: String,
    pub timestamp_utc: Option<String>,
    pub identity: ActivityIdentity,
    pub metadata: RequestMetadata,
    pub totals: MetricSummary,
    pub measurement: MeasurementFilter,
    pub events: u64,
    pub layers: Vec<Optimizer>,
}
fn event_stamp(e: &NormalizedEvent) -> Option<DateTime<Utc>> {
    e.timestamp_utc
        .as_deref()
        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        .map(|s| s.with_timezone(&Utc))
}
fn utc(stamp: DateTime<Utc>) -> String {
    stamp.to_rfc3339_opts(SecondsFormat::AutoSi, true)
}
fn row(id: &str, identity: Attribution, events: &[&NormalizedEvent]) -> ActivityRow {
    let mut metadata = RequestMetadata::default();
    for e in events.iter().filter(|e| e.optimizer == Optimizer::Headroom) {
        metadata.merge(&e.metadata);
    }
    let totals: MetricSummary = aggregate_event_refs(events.iter().copied()).into();
    let measurement = if events
        .iter()
        .any(|e| e.status == MeasurementStatus::Conflict)
    {
        MeasurementFilter::Conflict
    } else if totals.comparable_events > 0 {
        MeasurementFilter::Measured
    } else {
        MeasurementFilter::Unmeasurable
    };
    ActivityRow {
        origin_id: id.into(),
        timestamp_utc: events.iter().filter_map(|e| event_stamp(e)).max().map(utc),
        identity: identity.into(),
        metadata,
        totals,
        measurement,
        events: events.len() as u64,
        layers: [
            Optimizer::Headroom,
            Optimizer::Rtk,
            Optimizer::Cache,
            Optimizer::Output,
        ]
        .into_iter()
        .filter(|o| events.iter().any(|e| e.optimizer == *o))
        .collect(),
    }
}
#[derive(Debug, Serialize)]
pub struct ActivityData {
    pub monitoring_session_id: String,
    pub period: Period,
    pub from_utc: Option<String>,
    pub to_utc: String,
    pub retention_days: u16,
    pub coverage: Coverage,
    pub matched_requests: u64,
    pub shown_requests: u64,
    pub limit: usize,
    pub has_more: bool,
    pub unplaced_requests: u64,
    pub rows: Vec<ActivityRow>,
}
#[derive(Debug, Serialize)]
pub struct ActivityReading {
    pub tokens: Option<u64>,
    pub basis_kind: CounterKind,
    pub basis_id: Option<String>,
    pub component: Component,
}
impl From<&Reading> for ActivityReading {
    fn from(v: &Reading) -> Self {
        Self {
            tokens: v.tokens,
            basis_kind: v.basis.kind,
            basis_id: v
                .basis
                .tokenizer
                .as_deref()
                .map(|s| opaque_id(&["basis", s])),
            component: v.component,
        }
    }
}
#[derive(Debug, Serialize)]
pub struct MeasurementDetail {
    pub event_id: String,
    pub timestamp_utc: Option<String>,
    pub observed_via: Vec<Source>,
    pub optimizer: Optimizer,
    pub status: MeasurementStatus,
    pub before: ActivityReading,
    pub after: ActivityReading,
    pub saved_tokens: Option<i64>,
    pub saved_percent: Option<f64>,
    pub main_metric_eligible: bool,
    pub provider_usage: ProviderUsage,
    pub metadata: RequestMetadata,
}
impl From<&NormalizedEvent> for MeasurementDetail {
    fn from(e: &NormalizedEvent) -> Self {
        Self {
            event_id: e.event_id.clone(),
            timestamp_utc: e.timestamp_utc.clone(),
            observed_via: e.observed_via.clone(),
            optimizer: e.optimizer,
            status: e.status,
            before: (&e.before).into(),
            after: (&e.after).into(),
            saved_tokens: e.saved_tokens,
            saved_percent: e.saved_percent,
            main_metric_eligible: e.main_metric_eligible,
            provider_usage: e.provider_usage.clone(),
            metadata: e.metadata.clone(),
        }
    }
}
#[derive(Debug, Serialize)]
pub struct ActivityDetailData {
    pub period: Period,
    pub row: ActivityRow,
    pub measurements: Vec<MeasurementDetail>,
    pub omitted_measurements: u64,
}
#[derive(Debug, Serialize)]
pub struct ActivitySnapshot<T> {
    pub schema_version: u8,
    pub request_id: String,
    pub status: &'static str,
    pub observed_at_utc: String,
    pub data: T,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub collection: Option<CollectionSnapshot>,
}
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum ActivityReply<T> {
    Snapshot(ActivitySnapshot<T>),
    Error(MetricsFailure),
}
pub type ActivityListReply = ActivityReply<ActivityData>;
pub type ActivityDetailReply = ActivityReply<ActivityDetailData>;
impl<T> ActivityReply<T> {
    pub(crate) fn error(id: String, code: MetricsError) -> Self {
        Self::Error(MetricsStore::error_detail(id, code))
    }
}

impl MetricsStore {
    pub(crate) fn activity_at<Tz: TimeZone>(
        &self,
        period: Period,
        filter: &ActivityFilter,
        now: DateTime<Tz>,
    ) -> Result<ActivityData, MetricsError> {
        if !filter.valid() {
            return Err(MetricsError::InvalidInput);
        }
        let s = self.select_period(period, now)?;
        let mut rows = Vec::new();
        let mut unplaced_requests = 0;
        for (id, (identity, events)) in attribution::group_origins(&s.events) {
            let r = row(id, identity, &events);
            if filter.matches(&r) {
                if events.iter().any(|e| e.timestamp_utc.is_none()) {
                    unplaced_requests += 1;
                }
                let stamp = events.iter().filter_map(|e| event_stamp(e)).max();
                rows.push((stamp, r));
            }
        }
        rows.sort_by(|(a, ra), (b, rb)| b.cmp(a).then_with(|| ra.origin_id.cmp(&rb.origin_id)));
        let matched_requests = rows.len() as u64;
        rows.truncate(LIST_LIMIT);
        Ok(ActivityData {
            monitoring_session_id: s.monitoring_session_id,
            period,
            from_utc: s.from.map(utc),
            to_utc: utc(s.now),
            retention_days: s.retention_days,
            coverage: s.coverage,
            matched_requests,
            shown_requests: rows.len() as u64,
            limit: LIST_LIMIT,
            has_more: matched_requests > LIST_LIMIT as u64,
            unplaced_requests,
            rows: rows.into_iter().map(|(_, r)| r).collect(),
        })
    }
    pub(crate) fn activity_detail_at<Tz: TimeZone>(
        &self,
        period: Period,
        origin: &str,
        now: DateTime<Tz>,
    ) -> Result<ActivityDetailData, MetricsError> {
        if !attribution::hash(origin) {
            return Err(MetricsError::InvalidInput);
        }
        let s = self.select_period(period, now)?;
        let mut origins = attribution::group_origins(&s.events);
        let (identity, mut events) = origins.remove(origin).ok_or(MetricsError::NotFound)?;
        let row = row(origin, identity, &events);
        events.sort_by(|a, b| {
            event_stamp(b)
                .cmp(&event_stamp(a))
                .then_with(|| a.event_id.cmp(&b.event_id))
        });
        let omitted_measurements = events.len().saturating_sub(DETAIL_LIMIT) as u64;
        Ok(ActivityDetailData {
            period,
            row,
            measurements: events
                .into_iter()
                .take(DETAIL_LIMIT)
                .map(MeasurementDetail::from)
                .collect(),
            omitted_measurements,
        })
    }
}
