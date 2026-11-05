use super::activity::*;
use super::attribution::{self, AgentRole, Attribution, Client};
use super::normalize::*;
use super::request_metadata::{Outcome, RequestMetadata};
use super::store::{CaptureFlags, MetricsError, MetricsStore, Period};
use chrono::{DateTime, Duration, FixedOffset, Utc};
use serde_json::json;
use std::path::PathBuf;

fn now() -> DateTime<FixedOffset> {
    DateTime::parse_from_rfc3339("2026-11-05T15:00:00+03:00").unwrap()
}
fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("work/proofs/activity/tests")
        .join(uuid::Uuid::new_v4().to_string())
        .join("store")
}
fn observation(id: &str) -> Observation {
    serde_json::from_value(json!({"runtime_generation":"0fce5171-68c1-4bb6-9846-461933fd51c1","request_id":id,"source":"proxy","proxy_request_id":null,"timestamp_utc":"2026-11-05T11:00:00Z","optimizer":"headroom",
        "before":{"tokens":1000,"basis":{"kind":"local_estimate","tokenizer":"PRIVATE_BASIS_CANARY"},"component":"message_input"},
        "after":{"tokens":500,"basis":{"kind":"local_estimate","tokenizer":"PRIVATE_BASIS_CANARY"},"component":"message_input"},"provider_usage":{"input":null,"output":null,"cached_input":null}})).unwrap()
}
fn insert(store: &MetricsStore, obs: &[Observation]) {
    store
        .ingest_batch(
            &normalize(obs).unwrap(),
            CaptureFlags::default(),
            now().with_timezone(&Utc),
        )
        .unwrap();
}

#[test]
fn activity_metadata_enrichment_and_conflict_do_not_change_token_or_attribution() {
    let path = root();
    let store = MetricsStore::in_directory(&path);
    let mut obs = observation("one");
    obs.attribution = Attribution::observed(Some("model-a"), None);
    insert(&store, &[obs.clone()]);
    obs.metadata = RequestMetadata {
        total_ms: Some(0.0),
        outcome: Outcome::NoErrorReported,
        ..Default::default()
    };
    insert(&store, &[obs.clone()]);
    let first = store
        .activity_at(Period::Today, &ActivityFilter::default(), now())
        .unwrap();
    assert_eq!(first.rows[0].metadata.total_ms, Some(0.0));
    assert_eq!(first.rows[0].totals.saved_tokens, Some(500));
    obs.metadata.total_ms = Some(1.0);
    insert(&store, &[obs.clone()]);
    obs.metadata.total_ms = Some(0.0);
    insert(&store, &[obs]);
    let data = store
        .activity_at(Period::Today, &ActivityFilter::default(), now())
        .unwrap();
    assert_eq!(data.rows.len(), 1);
    assert!(data.rows[0].metadata.conflicted);
    assert_eq!(data.rows[0].metadata.total_ms, None);
    assert_eq!(data.rows[0].totals.saved_tokens, Some(500));
    assert_eq!(data.rows[0].measurement, MeasurementFilter::Measured);
    assert_eq!(data.rows[0].identity.model.as_deref(), Some("model-a"));
    let mut invalid = observation("invalid");
    invalid.metadata.total_ms = Some(f64::NAN);
    assert_eq!(
        normalize(&[invalid]).unwrap_err(),
        NormalizeError::InvalidPayload
    );
    let mut invalid = observation("invalid");
    invalid.metadata.transforms_count = Some(129);
    assert!(normalize(&[invalid]).is_err());
}

#[test]
fn activity_filter_is_applied_before_cap_and_unknown_time_is_last() {
    let path = root();
    let store = MetricsStore::in_directory(&path);
    let mut values = Vec::new();
    for i in 0..140 {
        let mut obs = observation(&format!("r-{i}"));
        obs.timestamp_utc = Some((now().with_timezone(&Utc) - Duration::seconds(i)).to_rfc3339());
        obs.attribution =
            Attribution::observed(Some(if i == 139 { "rare" } else { "common" }), None);
        if i == 139 {
            obs.attribution.route = Some(
                attribution::native_route(
                    &obs.runtime_generation,
                    &obs.request_id,
                    &"a".repeat(64),
                    Client::Codex,
                    Some(("agent", AgentRole::Subagent, Some("parent"))),
                )
                .unwrap(),
            );
            obs.metadata.outcome = Outcome::ErrorReported;
        }
        values.push(obs);
    }
    let mut unknown = observation("unplaced");
    unknown.timestamp_utc = None;
    values.push(unknown);
    insert(&store, &values);
    let data = store
        .activity_at(Period::Today, &ActivityFilter::default(), now())
        .unwrap();
    assert_eq!(
        (
            data.matched_requests,
            data.shown_requests,
            data.limit,
            data.has_more
        ),
        (141, 100, 100, true)
    );
    assert_eq!(data.unplaced_requests, 1);
    assert_eq!(
        data.rows[0].timestamp_utc.as_deref(),
        Some("2026-11-05T12:00:00Z")
    );
    let filter = ActivityFilter {
        model: ModelFilter::Exact {
            value: "rare".into(),
        },
        client: ClientFilter::Codex,
        outcome: OutcomeFilter::ErrorReported,
        measurement: MeasurementFilter::Measured,
    };
    let rare = store.activity_at(Period::Today, &filter, now()).unwrap();
    assert_eq!(rare.matched_requests, 1);
    assert!(!rare.has_more);
    assert_eq!(
        rare.rows[0].identity.agent.as_ref().unwrap().role,
        AgentRole::Subagent
    );
    let unknown = store
        .activity_at(
            Period::Today,
            &ActivityFilter {
                model: ModelFilter::Unknown {},
                ..Default::default()
            },
            now(),
        )
        .unwrap();
    assert_eq!(unknown.rows.len(), 1);
    assert!(unknown.rows[0].timestamp_utc.is_none());
    assert_eq!(unknown.rows[0].measurement, MeasurementFilter::Unmeasurable);
    let filter = ActivityFilter {
        model: ModelFilter::Exact {
            value: "C:/PRIVATE_CANARY".into(),
        },
        ..Default::default()
    };
    assert_eq!(
        store
            .activity_at(Period::Today, &filter, now())
            .unwrap_err(),
        MetricsError::InvalidInput
    );
}

#[test]
fn activity_detail_is_content_free_and_keeps_layers_out_of_main_totals() {
    let store = MetricsStore::in_directory(&root());
    let obs = observation("layered");
    let mut rtk = obs.clone();
    rtk.optimizer = Optimizer::Rtk;
    rtk.after.tokens = Some(100);
    insert(&store, &[obs, rtk]);
    let data = store
        .activity_at(Period::Today, &ActivityFilter::default(), now())
        .unwrap();
    let detail = store
        .activity_detail_at(Period::Today, &data.rows[0].origin_id, now())
        .unwrap();
    assert_eq!(detail.measurements.len(), 2);
    assert_eq!(detail.row.events, 2);
    assert_eq!(detail.row.totals.saved_tokens, Some(500));
    assert_eq!(detail.row.layers.len(), 2);
    assert_eq!(detail.row.totals.excluded_events, 1);
    let exported = serde_json::to_string(&detail).unwrap();
    assert!(!exported.contains("PRIVATE_BASIS_CANARY"));
    assert!(!exported.contains("layered"));
    assert!(detail
        .measurements
        .iter()
        .all(|m| m.before.basis_id.as_ref().unwrap().len() == 64));
    assert_eq!(
        store
            .activity_detail_at(Period::Today, &"0".repeat(64), now())
            .unwrap_err(),
        MetricsError::NotFound
    );
    assert_eq!(
        store
            .activity_detail_at(Period::Today, "../secret", now())
            .unwrap_err(),
        MetricsError::InvalidInput
    );
}

#[test]
fn activity_old_payload_reopens_and_session_and_period_match_overview() {
    let path = root();
    let store = MetricsStore::in_directory(&path);
    let obs = observation("old");
    insert(&store, &[obs.clone()]);
    let event = normalize(&[obs]).unwrap().events.remove(0);
    let mut old = serde_json::to_value(event).unwrap();
    old.as_object_mut().unwrap().remove("metadata");
    let db = rusqlite::Connection::open(path.join("metrics.sqlite3")).unwrap();
    db.execute("UPDATE events SET payload=?1", [old.to_string()])
        .unwrap();
    drop(db);
    let reopened = MetricsStore::in_directory(&path);
    for period in [Period::Session, Period::Today, Period::SevenDays] {
        let overview = reopened.query_at(period, now()).unwrap();
        let activity = reopened
            .activity_at(period, &ActivityFilter::default(), now())
            .unwrap();
        assert_eq!(overview.coverage.events, activity.coverage.events);
        assert_eq!(
            overview.coverage.distinct_origins,
            activity.matched_requests
        );
        assert_eq!(overview.from_utc, activity.from_utc);
        assert_eq!(overview.to_utc, activity.to_utc);
        if period == Period::Session {
            assert_eq!(activity.matched_requests, 0);
        } else {
            assert!(activity.rows[0].metadata.is_unknown());
        }
    }
    let passive = root();
    assert_eq!(
        MetricsStore::in_directory(&passive)
            .activity_at(Period::Today, &ActivityFilter::default(), now())
            .unwrap()
            .matched_requests,
        0
    );
    assert!(!passive.exists());
}

#[test]
fn activity_request_bounds_and_detail_cap_are_defensive() {
    let valid = json!({"schema_version":1,"request_id":uuid::Uuid::new_v4().to_string(),"period":"today","filter":{"client":"any","model":{"kind":"any"},"outcome":"any","measurement":"any"}});
    assert!(
        serde_json::from_value::<ActivityQueryRequest>(valid.clone())
            .unwrap()
            .is_valid()
    );
    let mut wrong = valid.clone();
    wrong["request_id"] = json!("PRIVATE_CANARY");
    assert!(!serde_json::from_value::<ActivityQueryRequest>(wrong)
        .unwrap()
        .is_valid());
    let mut wrong = valid;
    wrong["filter"]["model"]["extra"] = json!("PRIVATE_CANARY");
    assert!(serde_json::from_value::<ActivityQueryRequest>(wrong).is_err());
    let store = MetricsStore::in_directory(&root());
    let mut event = normalize(&[observation("bounded")])
        .unwrap()
        .events
        .remove(0);
    event.status = MeasurementStatus::Conflict;
    event.main_metric_eligible = false;
    event.saved_tokens = None;
    event.saved_percent = None;
    let events = (0..40)
        .map(|i| {
            let mut e = event.clone();
            e.event_id = opaque_id(&["bounded", &i.to_string()]);
            e
        })
        .collect();
    store
        .ingest_batch(
            &NormalizedBatch {
                schema_version: 1,
                events,
                observations: 40,
                duplicate_observations: 0,
                conflicting_events: 40,
            },
            CaptureFlags::default(),
            now().with_timezone(&Utc),
        )
        .unwrap();
    let detail = store
        .activity_detail_at(Period::Today, &event.origin_id, now())
        .unwrap();
    assert_eq!(detail.measurements.len(), 32);
    assert_eq!(detail.omitted_measurements, 8);
    assert_eq!(detail.row.events, 40);
    assert_eq!(detail.row.totals.excluded_events, 40);
}
