use super::*;
use crate::metrics::attribution::{AttributionSummary, Counts, CoverageStatus};
use crate::metrics::collector::{CollectionSnapshot, CollectionStatus};
use crate::metrics::normalize::{Basis, Component, CounterKind};
use crate::metrics::query::{BasisTotal, PeriodTotals};
use crate::metrics::store::{Coverage as MetricCoverage, PeriodMetrics};

const SNAPSHOT: &str = "7132bced-3ce0-4c1b-8f48-d93d7b96f7ce";

fn snapshot(after: u64) -> OverviewSnapshot {
    let saved = 800i64 - after as i64;
    OverviewSnapshot {
        schema_version: 1,
        request_id: SNAPSHOT.into(),
        status: "ok",
        observed_at_utc: "2026-11-07T12:00:00Z".into(),
        data: PeriodMetrics {
            monitoring_session_id: "PRIVATE_SESSION_CANARY".into(),
            period: Period::Today,
            from_utc: Some("2026-11-06T21:00:00Z".into()),
            to_utc: "2026-11-07T12:00:00Z".into(),
            retention_days: 30,
            last_event_utc: None,
            series: vec![],
            totals: PeriodTotals {
                status: AggregateStatus::Measured,
                comparable_events: 1,
                excluded_events: 0,
                before_tokens: Some(800),
                after_tokens: Some(after),
                saved_tokens: Some(saved),
                saved_percent: Some(saved as f64 / 8.0),
                groups: vec![BasisTotal {
                    basis: Basis {
                        kind: CounterKind::LocalEstimate,
                        tokenizer: Some(r"C:\Users\PRIVATE_BASIS_CANARY\sk-test".into()),
                    },
                    component: Component::MessageInput,
                    status: AggregateStatus::Measured,
                    events: 1,
                    before_tokens: Some(800),
                    after_tokens: Some(after),
                    saved_tokens: Some(saved),
                    saved_percent: Some(saved as f64 / 8.0),
                }],
                omitted_basis_groups: 0,
            },
            coverage: MetricCoverage {
                events: 1,
                distinct_origins: 1,
                ..Default::default()
            },
            attribution: AttributionSummary {
                scope: "observed_requests",
                coverage: Counts::default(),
                client_status: CoverageStatus::Unknown,
                agent_status: CoverageStatus::Unknown,
                groups: vec![],
                omitted_groups: 0,
            },
        },
        collection: CollectionSnapshot {
            status: CollectionStatus::Stopped,
            interval_seconds: 5,
            last_capture_utc: None,
            runtime_generation: Some("PRIVATE_GENERATION_CANARY".into()),
            last_error: None,
            sampling_interrupted_since_start: false,
        },
    }
}

fn request(kind: Kind, language: Language, id: Option<&str>) -> Request {
    Request {
        request_id: Uuid::new_v4().to_string(),
        kind,
        language,
        snapshot_request_id: id.map(str::to_owned),
    }
}
fn runtime() -> RuntimeReply {
    crate::runtime::manager::RuntimeManager::failure_reply(Uuid::new_v4().to_string())
}
fn job(value: &OverviewSnapshot, kind: Kind, language: Language) -> Job {
    let manager = ReportManager::default();
    manager.remember(value);
    manager
        .begin(request(kind, language, Some(&value.request_id)))
        .unwrap()
}

#[test]
fn json_preserves_signed_zero_and_unknown_without_private_basis_or_identity() {
    for after in [700, 800, 900] {
        let value = snapshot(after);
        let artifact = job(&value, Kind::StatisticsJson, Language::Tr)
            .render(&runtime())
            .unwrap();
        let data: serde_json::Value = serde_json::from_slice(&artifact.bytes).unwrap();
        assert_eq!(data["totals"]["saved_tokens"], 800i64 - after as i64);
        assert_eq!(
            data["totals"]["saved_percent"],
            (800i64 - after as i64) as f64 / 8.0
        );
        assert_eq!(data["period"], "today");
        assert_eq!(data["source"], "headroom_local_metrics");
        assert_eq!(data["coverage"]["complete_traffic"], false);
        let output = String::from_utf8(artifact.bytes).unwrap();
        assert!(!output.contains("PRIVATE_"));
        assert!(!output.contains("tokenizer"));
        assert!(!output.contains(SNAPSHOT));
        assert!(serde_json::to_string(&value)
            .unwrap()
            .contains("PRIVATE_BASIS_CANARY"));
    }
    let mut value = snapshot(800);
    let t = &mut value.data.totals;
    t.status = AggregateStatus::NoComparableEvents;
    t.comparable_events = 0;
    t.excluded_events = 1;
    t.before_tokens = None;
    t.after_tokens = None;
    t.saved_tokens = None;
    t.saved_percent = None;
    let artifact = job(&value, Kind::StatisticsJson, Language::En)
        .render(&runtime())
        .unwrap();
    let data: serde_json::Value = serde_json::from_slice(&artifact.bytes).unwrap();
    assert!(data["totals"]["saved_percent"].is_null());
    assert!(data["totals"]["saved_tokens"].is_null());
    let bytes = job(&value, Kind::StatisticsCsv, Language::En)
        .render(&runtime())
        .unwrap()
        .bytes;
    let csv = String::from_utf8(bytes).unwrap();
    let row = csv.lines().nth(1).unwrap().split(',').collect::<Vec<_>>();
    assert!(row[8..12].iter().all(|v| v.is_empty()));
}

#[test]
fn locale_csv_is_utf8_bom_crlf_and_signed_numbers_stay_numeric() {
    let value = snapshot(900);
    for language in [Language::Tr, Language::En] {
        let artifact = job(&value, Kind::StatisticsCsv, language)
            .render(&runtime())
            .unwrap();
        assert_eq!(&artifact.bytes[..3], &[0xef, 0xbb, 0xbf]);
        let text = String::from_utf8(artifact.bytes).unwrap();
        assert_eq!(text.matches("\r\n").count(), 2);
        let separator = if language == Language::Tr { ';' } else { ',' };
        let cells = text
            .lines()
            .nth(1)
            .unwrap()
            .split(separator)
            .collect::<Vec<_>>();
        assert_eq!(cells.len(), 22);
        assert_eq!(cells[10], "-100");
        assert_eq!(
            cells[11],
            if language == Language::Tr {
                "-12,5"
            } else {
                "-12.5"
            }
        );
        assert!(artifact.suggested_name.ends_with(".csv"));
        assert!(!artifact.suggested_name.contains(':'));
    }
    for text in [
        "=SUM(A1)",
        " +command",
        "\t-2+4",
        "\u{feff}@SUM(A1)",
        "\r\n=1+1",
    ] {
        assert!(csv_text(text).starts_with("\"'"));
    }
    assert_eq!(csv_text("a\"b"), "\"a\"\"b\"");
}

#[test]
fn frozen_job_survives_new_reads_and_bounded_cache_evicts_old_ids() {
    let manager = ReportManager::default();
    let mut value = snapshot(700);
    manager.remember(&value);
    let old = manager
        .begin(request(Kind::StatisticsJson, Language::En, Some(SNAPSHOT)))
        .unwrap();
    assert_eq!(
        manager
            .begin(request(Kind::StatisticsJson, Language::En, Some(SNAPSHOT)))
            .err(),
        Some(Error::Busy)
    );
    for _ in 0..10 {
        value.request_id = Uuid::new_v4().to_string();
        value.data.totals.saved_tokens = Some(0);
        manager.remember(&value);
    }
    let bytes = old.render(&runtime()).unwrap().bytes;
    let exported: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(exported["totals"]["saved_tokens"], 100);
    drop(old);
    assert_eq!(manager.0.lock().unwrap().snapshots.len(), SNAPSHOT_LIMIT);
    assert_eq!(
        manager
            .begin(request(Kind::StatisticsJson, Language::En, Some(SNAPSHOT)))
            .err(),
        Some(Error::SnapshotExpired)
    );
}

#[test]
fn cancellation_failure_and_exit_barrier_release_without_latching_failed_exit() {
    let manager = ReportManager::default();
    assert!(!manager.hold_for_exit(|| false));
    let current = manager
        .begin(request(Kind::DiagnosticsJson, Language::En, None))
        .unwrap();
    assert!(!manager.hold_for_exit(|| panic!("active export must not hold config")));
    let reply = serde_json::to_value(current.save(&runtime(), |_| Ok(None))).unwrap();
    assert_eq!(reply["status"], "cancelled");
    assert_eq!(reply["bytes_written"], 0);
    let retry = manager
        .begin(request(Kind::DiagnosticsJson, Language::Tr, None))
        .unwrap();
    let reply = serde_json::to_value(retry.save(&runtime(), |_| Err(()))).unwrap();
    assert_eq!(reply["error"]["code"], "unavailable");
    assert!(manager.hold_for_exit(|| true));
    assert_eq!(
        manager
            .begin(request(Kind::DiagnosticsJson, Language::Tr, None))
            .err(),
        Some(Error::Busy)
    );
}

#[test]
fn invalid_or_expired_statistics_never_fall_back_and_diagnostics_need_no_metrics() {
    let manager = ReportManager::default();
    assert_eq!(
        manager
            .begin(request(Kind::StatisticsJson, Language::Tr, None))
            .err(),
        Some(Error::InvalidInput)
    );
    assert_eq!(
        manager
            .begin(request(
                Kind::StatisticsCsv,
                Language::Tr,
                Some("D:\\private")
            ))
            .err(),
        Some(Error::InvalidInput)
    );
    assert_eq!(
        manager
            .begin(request(Kind::StatisticsJson, Language::Tr, Some(SNAPSHOT)))
            .err(),
        Some(Error::SnapshotExpired)
    );
    let value = manager
        .begin(request(Kind::DiagnosticsJson, Language::Tr, None))
        .unwrap()
        .render(&runtime())
        .unwrap();
    let diagnostic: serde_json::Value = serde_json::from_slice(&value.bytes).unwrap();
    assert!(diagnostic["statistics"].is_null());
    assert!(diagnostic["runtime"]["engine_status"].is_null());
    assert!(diagnostic.get("path").is_none());
    assert!(value.suggested_name.starts_with("headroom-tani-"));
    assert!(serde_json::from_value::<Request>(serde_json::json!({"request_id":SNAPSHOT,"kind":"statistics_json","language":"tr","snapshot_request_id":SNAPSHOT,"path":"C:/unsafe"})).is_err());
}
