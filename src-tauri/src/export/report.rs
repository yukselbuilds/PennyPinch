use crate::metrics::collector::{CollectionSnapshot, OverviewSnapshot};
use crate::metrics::query::AggregateStatus;
use crate::metrics::store::Period;
use crate::runtime::manager::RuntimeReply;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

pub(crate) const MAX_REPORT_BYTES: usize = 262_144;
const SNAPSHOT_LIMIT: usize = 8;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Kind {
    StatisticsJson,
    StatisticsCsv,
    DiagnosticsJson,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Language {
    Tr,
    En,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Request {
    pub request_id: String,
    pub kind: Kind,
    pub language: Language,
    pub snapshot_request_id: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Error {
    InvalidInput,
    SnapshotExpired,
    Busy,
    Unavailable,
    UnsafePath,
    FileExists,
    WriteFailed,
}

#[derive(Serialize)]
#[serde(untagged)]
pub(crate) enum Reply {
    Done {
        schema_version: u8,
        request_id: String,
        status: &'static str,
        kind: Kind,
        bytes_written: usize,
    },
    Error {
        schema_version: u8,
        request_id: String,
        status: &'static str,
        error: Failure,
    },
}

#[derive(Serialize)]
pub(crate) struct Failure {
    code: Error,
    retryable: bool,
    message_key: String,
    diagnostic_id: Option<String>,
}

pub(crate) fn failure(id: &str, code: Error) -> Reply {
    let name = serde_json::to_value(code)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_else(|| "unavailable".into());
    Reply::Error {
        schema_version: 1,
        request_id: if valid_id(id) {
            id.into()
        } else {
            Uuid::nil().to_string()
        },
        status: "error",
        error: Failure {
            code,
            retryable: matches!(
                code,
                Error::SnapshotExpired | Error::Busy | Error::FileExists | Error::WriteFailed
            ),
            message_key: format!("export.{name}"),
            diagnostic_id: None,
        },
    }
}

fn valid_id(id: &str) -> bool {
    Uuid::parse_str(id).is_ok_and(|v| !v.is_nil() && v.to_string() == id)
}

/// An explicit aggregate allowlist; raw basis/attribution and process metadata never enter the cache.
#[derive(Clone, Serialize)]
struct Summary {
    schema_version: u8,
    source: &'static str,
    scope: &'static str,
    component: &'static str,
    observed_at_utc: String,
    period: Period,
    from_utc: Option<String>,
    to_utc: String,
    retention_days: u16,
    totals: Totals,
    coverage: Coverage,
    collection: Collection,
}

#[derive(Clone, Serialize)]
struct Totals {
    status: AggregateStatus,
    comparable_events: u64,
    excluded_events: u64,
    before_tokens: Option<u64>,
    after_tokens: Option<u64>,
    saved_tokens: Option<i64>,
    saved_percent: Option<f64>,
}

#[derive(Clone, Serialize)]
struct Coverage {
    events: u64,
    distinct_origins: u64,
    unplaced_events: u64,
    rolling_window: bool,
    aggregate_only: bool,
    unidentified_rows_observed: bool,
    complete_traffic: bool,
}

#[derive(Clone, Serialize)]
struct Collection {
    status: crate::metrics::collector::CollectionStatus,
    interval_seconds: u64,
    last_capture_utc: Option<String>,
    last_error: Option<crate::metrics::collector::CollectionError>,
    sampling_interrupted_since_start: bool,
}

impl Summary {
    fn capture(v: &OverviewSnapshot) -> Self {
        let d = &v.data;
        let t = &d.totals;
        let c = &d.coverage;
        let CollectionSnapshot {
            status,
            interval_seconds,
            last_capture_utc,
            last_error,
            sampling_interrupted_since_start,
            ..
        } = &v.collection;
        Self {
            schema_version: 1,
            source: "headroom_local_metrics",
            scope: "observed_requests",
            component: "input",
            observed_at_utc: v.observed_at_utc.clone(),
            period: d.period,
            from_utc: d.from_utc.clone(),
            to_utc: d.to_utc.clone(),
            retention_days: d.retention_days,
            totals: Totals {
                status: t.status,
                comparable_events: t.comparable_events,
                excluded_events: t.excluded_events,
                before_tokens: t.before_tokens,
                after_tokens: t.after_tokens,
                saved_tokens: t.saved_tokens,
                saved_percent: t.saved_percent,
            },
            coverage: Coverage {
                events: c.events,
                distinct_origins: c.distinct_origins,
                unplaced_events: c.unplaced_events,
                rolling_window: c.rolling_window,
                aggregate_only: c.aggregate_only,
                unidentified_rows_observed: c.unidentified_rows_observed,
                complete_traffic: c.complete_traffic,
            },
            collection: Collection {
                status: *status,
                interval_seconds: *interval_seconds,
                last_capture_utc: last_capture_utc.clone(),
                last_error: *last_error,
                sampling_interrupted_since_start: *sampling_interrupted_since_start,
            },
        }
    }
}

#[derive(Clone, Default)]
pub(crate) struct ReportManager(Arc<Mutex<State>>);

#[derive(Default)]
struct State {
    snapshots: VecDeque<(String, Summary)>,
    exporting: bool,
    exiting: bool,
}

pub(crate) struct Job {
    manager: ReportManager,
    request: Request,
    summary: Option<Summary>,
}

impl Drop for Job {
    fn drop(&mut self) {
        self.manager
            .0
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .exporting = false;
    }
}

impl ReportManager {
    pub(crate) fn remember(&self, snapshot: &OverviewSnapshot) {
        let mut state = self.0.lock().unwrap_or_else(|p| p.into_inner());
        state.snapshots.retain(|(id, _)| id != &snapshot.request_id);
        state
            .snapshots
            .push_back((snapshot.request_id.clone(), Summary::capture(snapshot)));
        while state.snapshots.len() > SNAPSHOT_LIMIT {
            state.snapshots.pop_front();
        }
    }

    pub(crate) fn begin(&self, request: Request) -> Result<Job, Error> {
        if !valid_id(&request.request_id)
            || request
                .snapshot_request_id
                .as_deref()
                .is_some_and(|id| !valid_id(id))
            || request.kind != Kind::DiagnosticsJson && request.snapshot_request_id.is_none()
        {
            return Err(Error::InvalidInput);
        }
        let mut state = self.0.lock().unwrap_or_else(|p| p.into_inner());
        if state.exporting || state.exiting {
            return Err(Error::Busy);
        }
        let summary = match &request.snapshot_request_id {
            Some(id) => Some(
                state
                    .snapshots
                    .iter()
                    .find(|(known, _)| known == id)
                    .ok_or(Error::SnapshotExpired)?
                    .1
                    .clone(),
            ),
            None => None,
        };
        state.exporting = true;
        Ok(Job {
            manager: self.clone(),
            request,
            summary,
        })
    }

    /// Hold this lock until the existing config exit barrier is committed; failed exits leave exports enabled.
    pub(crate) fn hold_for_exit(&self, hold_config: impl FnOnce() -> bool) -> bool {
        let mut state = self.0.lock().unwrap_or_else(|p| p.into_inner());
        if state.exporting || state.exiting || !hold_config() {
            return false;
        }
        state.exiting = true;
        true
    }
}

pub(crate) struct Artifact {
    pub suggested_name: String,
    pub extension: &'static str,
    pub language: Language,
    pub bytes: Vec<u8>,
}

impl Job {
    pub(crate) fn save(
        self,
        runtime: &RuntimeReply,
        picker: impl FnOnce(&Artifact) -> Result<Option<PathBuf>, ()>,
    ) -> Reply {
        let id = &self.request.request_id;
        let result = (|| {
            let artifact = self.render(runtime)?;
            let Some(path) = picker(&artifact).map_err(|_| Error::Unavailable)? else {
                return Ok(None);
            };
            super::writer::save_new(&path, artifact.extension, &artifact.bytes)?;
            Ok(Some(artifact.bytes.len()))
        })();
        match result {
            Ok(bytes) => Reply::Done {
                schema_version: 1,
                request_id: id.clone(),
                kind: self.request.kind,
                status: if bytes.is_some() {
                    "saved"
                } else {
                    "cancelled"
                },
                bytes_written: bytes.unwrap_or(0),
            },
            Err(code) => failure(id, code),
        }
    }

    fn render(&self, runtime: &RuntimeReply) -> Result<Artifact, Error> {
        let (extension, prefix, bytes) = match self.request.kind {
            Kind::StatisticsJson => (
                "json",
                self.summary_name(),
                serde_json::to_vec_pretty(self.summary.as_ref().ok_or(Error::SnapshotExpired)?)
                    .map_err(|_| Error::Unavailable)?,
            ),
            Kind::StatisticsCsv => (
                "csv",
                self.summary_name(),
                csv(
                    self.summary.as_ref().ok_or(Error::SnapshotExpired)?,
                    self.request.language,
                ),
            ),
            Kind::DiagnosticsJson => {
                // Select individual fixed public fields, never serialize the whole runtime DTO to the report.
                let value = serde_json::to_value(runtime).map_err(|_| Error::Unavailable)?;
                let diagnostic = serde_json::json!({
                    "schema_version": 1, "kind": "diagnostics", "created_at_utc": Utc::now().to_rfc3339(),
                    "application": {"version": env!("CARGO_PKG_VERSION"), "os": std::env::consts::OS, "architecture": std::env::consts::ARCH},
                    "runtime": {
                        "engine_status": fixed(&value["engine"]["status"], &["unavailable","unknown","stopped","starting","ready","stopping","failed"]),
                        "engine_error": fixed(&value["engine"]["error_code"], &["validation_failed","unsupported_protocol","environment_blocked","busy","port_conflict","engine_not_ready","ownership_lost","timeout","cleanup_failed","config_conflict"]),
                        "compression_status": fixed(&value["compression"]["status"], &["unavailable","unknown","enabled","paused"]),
                        "active_stream_count": value["compression"]["active_stream_count"].as_u64().filter(|n| *n<=9_007_199_254_740_991),
                    },
                    "statistics": self.summary,
                });
                (
                    "json",
                    if self.request.language == Language::Tr {
                        "tani"
                    } else {
                        "diagnostics"
                    }
                    .into(),
                    serde_json::to_vec_pretty(&diagnostic).map_err(|_| Error::Unavailable)?,
                )
            }
        };
        if bytes.is_empty() || bytes.len() > MAX_REPORT_BYTES {
            return Err(Error::Unavailable);
        }
        Ok(Artifact {
            suggested_name: format!(
                "headroom-{prefix}-{}.{}",
                Utc::now().format("%Y-%m-%dT%H-%M-%SZ"),
                extension
            ),
            extension,
            language: self.request.language,
            bytes,
        })
    }

    fn summary_name(&self) -> String {
        let period = self
            .summary
            .as_ref()
            .map(|s| s.period)
            .unwrap_or(Period::Session);
        let value = match (self.request.language, period) {
            (Language::Tr, Period::Session) => "istatistik-oturum",
            (Language::Tr, Period::Today) => "istatistik-bugun",
            (Language::Tr, Period::SevenDays) => "istatistik-7-gun",
            (Language::En, Period::Session) => "statistics-session",
            (Language::En, Period::Today) => "statistics-today",
            (Language::En, Period::SevenDays) => "statistics-7-days",
        };
        value.into()
    }
}

fn fixed<'a>(value: &serde_json::Value, allowed: &[&'a str]) -> Option<&'a str> {
    allowed.iter().copied().find(|v| value.as_str() == Some(v))
}

fn csv_text(value: &str) -> String {
    let dangerous = value
        .trim_start_matches(|c: char| c.is_whitespace() || c == '\u{feff}')
        .starts_with(['=', '+', '-', '@']);
    format!(
        "\"{}{}\"",
        if dangerous { "'" } else { "" },
        value.replace('"', "\"\"")
    )
}

fn csv(summary: &Summary, language: Language) -> Vec<u8> {
    let headers = if language == Language::Tr {
        [
            "Kaynak",
            "Kapsam",
            "Bileşen",
            "Ölçüm zamanı (UTC)",
            "Dönem",
            "Başlangıç (UTC)",
            "Bitiş (UTC)",
            "Ölçüm durumu",
            "Önce (token)",
            "Sonra (token)",
            "Tasarruf (token)",
            "Tasarruf (%)",
            "Karşılaştırılabilir",
            "Hariç tutulan",
            "Ölçüm kaydı",
            "Gözlenen istek",
            "Zamanı bilinmeyen",
            "Yuvarlanan pencere",
            "Yalnız toplu veri",
            "Kimliksiz kayıt var",
            "Tüm trafik",
            "Saklama (gün)",
        ]
    } else {
        [
            "Source",
            "Scope",
            "Component",
            "Observed at (UTC)",
            "Period",
            "From (UTC)",
            "To (UTC)",
            "Measurement status",
            "Before (tokens)",
            "After (tokens)",
            "Saved (tokens)",
            "Saved (%)",
            "Comparable",
            "Excluded",
            "Recorded events",
            "Observed requests",
            "Unplaced events",
            "Rolling window",
            "Aggregate only",
            "Unidentified rows observed",
            "Complete traffic",
            "Retention (days)",
        ]
    };
    let separator = if language == Language::Tr { ";" } else { "," };
    let t = &summary.totals;
    let c = &summary.coverage;
    let status = serde_json::to_value(t.status).unwrap_or_default();
    let period = match (language, summary.period) {
        (Language::Tr, Period::Session) => "Oturum",
        (Language::Tr, Period::Today) => "Bugün",
        (Language::Tr, Period::SevenDays) => "7 gün",
        (Language::En, Period::Session) => "Session",
        (Language::En, Period::Today) => "Today",
        (Language::En, Period::SevenDays) => "7 days",
    };
    let mut cells = [
        summary.source,
        summary.scope,
        summary.component,
        &summary.observed_at_utc,
        period,
        summary.from_utc.as_deref().unwrap_or(""),
        &summary.to_utc,
        status.as_str().unwrap_or("unknown"),
    ]
    .map(csv_text)
    .to_vec();
    cells.extend(
        [
            t.before_tokens.map(|n| n.to_string()),
            t.after_tokens.map(|n| n.to_string()),
            t.saved_tokens.map(|n| n.to_string()),
            t.saved_percent.map(|n| {
                if language == Language::Tr {
                    n.to_string().replace('.', ",")
                } else {
                    n.to_string()
                }
            }),
        ]
        .map(|v| v.unwrap_or_default()),
    );
    cells.extend(
        [
            t.comparable_events,
            t.excluded_events,
            c.events,
            c.distinct_origins,
            c.unplaced_events,
        ]
        .map(|n| n.to_string()),
    );
    cells.extend(
        [
            c.rolling_window,
            c.aggregate_only,
            c.unidentified_rows_observed,
            c.complete_traffic,
        ]
        .map(|v| v.to_string()),
    );
    cells.push(summary.retention_days.to_string());
    let text = format!(
        "\u{feff}{}\r\n{}\r\n",
        headers.map(csv_text).join(separator),
        cells.join(separator)
    );
    text.into_bytes()
}

#[cfg(test)]
#[path = "report_tests.rs"]
mod tests;
