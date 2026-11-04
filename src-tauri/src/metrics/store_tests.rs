use super::normalize::*;
use super::query::AggregateStatus;
use super::store::*;
use chrono::{DateTime, FixedOffset, Utc};
use rusqlite::Connection;
use std::fs;
use std::path::PathBuf;
use uuid::Uuid;

fn fixture_root() -> PathBuf {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("work/proofs/metrics-store/tests")
        .join(Uuid::new_v4().to_string());
    fs::create_dir_all(&root).unwrap();
    root.join("store")
}

fn now() -> DateTime<FixedOffset> {
    DateTime::parse_from_rfc3339("2026-11-04T12:00:00+03:00").unwrap()
}

fn observation(id: &str, stamp: &str) -> Observation {
    let reading = |tokens| Reading {
        tokens: Some(tokens),
        basis: Basis {
            kind: CounterKind::LocalEstimate,
            tokenizer: Some("fixture-v1".into()),
        },
        component: Component::MessageInput,
    };
    Observation {
        runtime_generation: "00000000-0000-4000-8000-000000000001".into(),
        request_id: id.into(),
        source: Source::Proxy,
        proxy_request_id: None,
        timestamp_utc: Some(stamp.into()),
        optimizer: Optimizer::Headroom,
        before: reading(100),
        after: reading(50),
        provider_usage: ProviderUsage::default(),
    }
}

fn insert(store: &MetricsStore, values: &[Observation]) -> Result<IngestResult, MetricsError> {
    store.ingest_batch(
        &normalize(values).unwrap(),
        CaptureFlags::default(),
        now().with_timezone(&Utc),
    )
}

#[test]
fn metrics_store_missing_read_is_passive_and_unknown_not_zero() {
    let root = fixture_root();
    let result = MetricsStore::in_directory(&root)
        .query_at(Period::Today, now())
        .unwrap();
    assert!(!root.exists());
    assert_eq!(result.retention_days, 30);
    assert_eq!(result.totals.status, AggregateStatus::NoComparableEvents);
    assert_eq!(result.totals.saved_percent, None);
    assert!(!result.coverage.complete_traffic);
}

#[test]
fn metrics_store_replay_reopen_and_new_session_do_not_recount() {
    let root = fixture_root();
    let store = MetricsStore::in_directory(&root);
    let proxy = observation("one", "2026-11-04T08:00:00Z");
    assert_eq!(insert(&store, &[proxy.clone()]).unwrap().inserted, 1);
    let mut mcp = proxy.clone();
    mcp.source = Source::Mcp;
    mcp.proxy_request_id = Some("one".into());
    mcp.request_id = "mcp-one".into();
    assert_eq!(insert(&store, &[mcp, proxy.clone()]).unwrap().replayed, 1);
    let first = store.query_at(Period::Session, now()).unwrap();
    assert_eq!(first.coverage.events, 1);
    assert_eq!(first.totals.before_tokens, Some(100));
    let reopened = MetricsStore::in_directory(&root);
    assert_eq!(insert(&reopened, &[proxy]).unwrap().inserted, 0);
    let today = reopened.query_at(Period::Today, now()).unwrap();
    assert_ne!(first.monitoring_session_id, today.monitoring_session_id);
    assert_eq!(today.totals.saved_tokens, Some(50));
    assert_eq!(today.coverage.events, 1);
    assert_eq!(
        reopened
            .query_at(Period::Session, now())
            .unwrap()
            .coverage
            .events,
        0
    );
    let conn = Connection::open(root.join("metrics.sqlite3")).unwrap();
    let encoded: String = conn
        .query_row("SELECT payload FROM events", [], |row| row.get(0))
        .unwrap();
    let stored: serde_json::Value = serde_json::from_str(&encoded).unwrap();
    assert_eq!(stored["observed_via"], serde_json::json!(["proxy", "mcp"]));
}

#[test]
fn metrics_store_local_calendar_boundaries_exclude_future_and_survive_zone_change() {
    let store = MetricsStore::in_directory(&fixture_root());
    let stamps = [
        "2026-10-28T20:59:59Z",
        "2026-10-28T21:00:00Z",
        "2026-11-03T20:59:59Z",
        "2026-11-03T21:00:00Z",
        "2026-11-04T09:00:00Z",
        "2026-11-04T09:00:00.001Z",
    ];
    let values: Vec<_> = stamps
        .iter()
        .enumerate()
        .map(|(i, stamp)| observation(&format!("r{i}"), stamp))
        .collect();
    insert(&store, &values).unwrap();
    let today = store.query_at(Period::Today, now()).unwrap();
    assert_eq!(today.coverage.events, 2);
    assert_eq!(today.from_utc.as_deref(), Some("2026-11-03T21:00:00Z"));
    let week = store.query_at(Period::SevenDays, now()).unwrap();
    assert_eq!(week.coverage.events, 4);
    assert_eq!(week.from_utc.as_deref(), Some("2026-10-28T21:00:00Z"));
    let utc_day = store
        .query_at(Period::Today, now().with_timezone(&Utc))
        .unwrap();
    assert_eq!(utc_day.coverage.events, 1);
}

#[test]
fn metrics_store_conflict_is_absorbing_and_unplaced_coverage_is_visible() {
    let store = MetricsStore::in_directory(&fixture_root());
    let first = observation("conflict", "2026-11-04T08:00:00Z");
    insert(&store, &[first.clone()]).unwrap();
    let mut changed = first.clone();
    changed.after.tokens = Some(70);
    changed.timestamp_utc = Some("2026-11-03T08:00:00Z".into());
    assert_eq!(insert(&store, &[changed]).unwrap().conflicted, 1);
    insert(&store, &[first]).unwrap();
    let result = store.query_at(Period::Today, now()).unwrap();
    assert_eq!(result.totals.saved_tokens, None);
    assert_eq!(result.coverage.unplaced_events, 1);
    assert!(!result.coverage.complete_traffic);
}

#[test]
fn metrics_store_retention_expires_old_events_without_importing_them_again() {
    let root = fixture_root();
    let store = MetricsStore::in_directory(&root);
    let old = observation("old", "2026-10-05T08:59:59Z");
    let boundary = observation("boundary", "2026-10-05T09:00:00Z");
    let recent = observation("recent", "2026-11-04T08:00:00Z");
    let result = insert(&store, &[old.clone(), boundary, recent]).unwrap();
    assert_eq!(result.inserted, 2);
    assert_eq!(result.expired, 1);
    assert_eq!(insert(&store, &[old]).unwrap().inserted, 0);
    let later = now() + chrono::Duration::days(31);
    assert_eq!(
        store
            .query_at(Period::Today, later)
            .unwrap()
            .coverage
            .events,
        0
    );
    let conn = Connection::open(root.join("metrics.sqlite3")).unwrap();
    assert_eq!(
        conn.query_row("SELECT count(*) FROM events", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        0
    );
}

#[test]
fn metrics_store_legacy_projection_is_partial_and_contains_no_raw_payload() {
    let root = fixture_root();
    let store = MetricsStore::in_directory(&root);
    let snapshot = serde_json::json!({"recent_requests":[{"request_id":"legacy", "timestamp":"2026-11-04T08:00:00Z", "input_tokens_original":2306, "input_tokens_optimized":12, "prompt":"HEADROOM_CONTENT_CANARY", "tags":{"authorization":"HEADROOM_SECRET_CANARY"}}]});
    let projected = project_headroom_041(
        snapshot.to_string().as_bytes(),
        "00000000-0000-4000-8000-000000000001",
        "0.41.0",
        Source::Proxy,
    )
    .unwrap();
    assert_eq!(projected.batch.events[0].before.tokens, Some(2306));
    store
        .ingest_projection(&projected, now().with_timezone(&Utc))
        .unwrap();
    let result = store.query_at(Period::Today, now()).unwrap();
    assert_eq!(result.totals.saved_percent, None);
    assert!(result.coverage.rolling_window);
    assert!(!result.coverage.complete_traffic);
    let bytes = fs::read(root.join("metrics.sqlite3")).unwrap();
    assert!(!bytes
        .windows(b"HEADROOM_CONTENT_CANARY".len())
        .any(|w| w == b"HEADROOM_CONTENT_CANARY"));
    assert!(!bytes
        .windows(b"HEADROOM_SECRET_CANARY".len())
        .any(|w| w == b"HEADROOM_SECRET_CANARY"));
}

#[test]
fn metrics_store_wrong_schema_and_invalid_batch_preserve_existing_data() {
    let root = fixture_root();
    let store = MetricsStore::in_directory(&root);
    let value = observation("one", "2026-11-04T08:00:00Z");
    insert(&store, &[value.clone()]).unwrap();
    let mut batch = normalize(&[value]).unwrap();
    batch.events[0].schema_version = 2;
    assert_eq!(
        store
            .ingest_batch(&batch, CaptureFlags::default(), now().with_timezone(&Utc))
            .unwrap_err(),
        MetricsError::InvalidInput
    );
    batch.events[0].schema_version = 1;
    let replacement = if batch.events[0].event_id.starts_with('a') {
        "b"
    } else {
        "a"
    };
    batch.events[0].event_id.replace_range(..1, replacement);
    assert_eq!(
        store
            .ingest_batch(&batch, CaptureFlags::default(), now().with_timezone(&Utc))
            .unwrap_err(),
        MetricsError::InvalidInput
    );
    assert_eq!(
        store
            .query_at(Period::Today, now())
            .unwrap()
            .coverage
            .events,
        1
    );
    let conn = Connection::open(root.join("metrics.sqlite3")).unwrap();
    conn.pragma_update(None, "user_version", 99).unwrap();
    drop(conn);
    let before = fs::read(root.join("metrics.sqlite3")).unwrap();
    assert_eq!(
        store.query_at(Period::Today, now()).unwrap_err(),
        MetricsError::UnsupportedSchema
    );
    assert_eq!(fs::read(root.join("metrics.sqlite3")).unwrap(), before);
}

#[test]
fn metrics_store_busy_and_failed_batch_are_atomic() {
    let root = fixture_root();
    let store = MetricsStore::in_directory(&root);
    insert(&store, &[observation("existing", "2026-11-04T08:00:00Z")]).unwrap();
    let conn = Connection::open(root.join("metrics.sqlite3")).unwrap();
    conn.execute_batch("BEGIN IMMEDIATE").unwrap();
    assert_eq!(
        insert(&store, &[observation("locked", "2026-11-04T08:00:00Z")]).unwrap_err(),
        MetricsError::Busy
    );
    conn.execute_batch("ROLLBACK").unwrap();
    let blocked = normalize(&[observation("blocked", "2026-11-04T08:00:00Z")])
        .unwrap()
        .events[0]
        .event_id
        .clone();
    conn.execute_batch(&format!("CREATE TRIGGER reject_second BEFORE INSERT ON events WHEN NEW.event_id = '{blocked}' BEGIN SELECT RAISE(ABORT, 'fixture'); END;")).unwrap();
    let mut batch = normalize(&[
        observation("new", "2026-11-04T08:00:00Z"),
        observation("blocked", "2026-11-04T08:00:00Z"),
    ])
    .unwrap();
    batch.events.sort_by_key(|e| e.event_id == blocked);
    assert!(store
        .ingest_batch(&batch, CaptureFlags::default(), now().with_timezone(&Utc))
        .is_err());
    assert_eq!(
        store
            .query_at(Period::Today, now())
            .unwrap()
            .coverage
            .events,
        1
    );
}

#[test]
fn metrics_store_rejects_hardlinked_and_unrelated_files_without_writing() {
    let root = fixture_root();
    fs::create_dir(&root).unwrap();
    let outside = root.parent().unwrap().join("external.dat");
    fs::write(&outside, b"DO_NOT_CHANGE").unwrap();
    fs::hard_link(&outside, root.join("metrics.sqlite3")).unwrap();
    let store = MetricsStore::in_directory(&root);
    assert_eq!(
        insert(&store, &[observation("one", "2026-11-04T08:00:00Z")]).unwrap_err(),
        MetricsError::UnsafePath
    );
    assert_eq!(fs::read(&outside).unwrap(), b"DO_NOT_CHANGE");
    let root = fixture_root();
    fs::create_dir(&root).unwrap();
    fs::write(root.join("metrics.sqlite3"), b"UNRELATED").unwrap();
    assert!(MetricsStore::in_directory(&root)
        .query_at(Period::Today, now())
        .is_err());
    assert_eq!(
        fs::read(root.join("metrics.sqlite3")).unwrap(),
        b"UNRELATED"
    );
}

#[test]
fn metrics_store_protocol_rejects_noncanonical_nil_and_redacts_invalid_ids() {
    let root = fixture_root();
    let store = MetricsStore::in_directory(&root);
    for id in [
        "urn:uuid:00000000-0000-0000-0000-000000000000",
        "HEADROOM_SECRET_CANARY",
    ] {
        let reply = store.query(MetricsQueryRequest {
            schema_version: 1,
            request_id: id.into(),
            period: Period::Today,
        });
        assert!(matches!(reply, MetricsReply::Error(_)));
        assert!(!serde_json::to_string(&reply)
            .unwrap()
            .contains("HEADROOM_SECRET_CANARY"));
        assert!(!root.exists());
    }
    assert!(serde_json::from_value::<MetricsQueryRequest>(serde_json::json!({"schema_version":1,"request_id":Uuid::new_v4().to_string(),"period":"today","payload":"forbidden"})).is_err());
}

#[test]
fn metrics_store_aggregate_and_unidentified_sources_never_make_zero_or_full_traffic() {
    let store = MetricsStore::in_directory(&fixture_root());
    for snapshot in [
        serde_json::json!({"requests":{"total":51}}),
        serde_json::json!({"recent_requests":[{"request_id":null}]}),
    ] {
        let projection = project_headroom_041(
            snapshot.to_string().as_bytes(),
            "00000000-0000-4000-8000-000000000001",
            "0.41.0",
            Source::Proxy,
        )
        .unwrap();
        store
            .ingest_projection(&projection, now().with_timezone(&Utc))
            .unwrap();
    }
    let result = store.query_at(Period::Today, now()).unwrap();
    assert_eq!(result.coverage.events, 0);
    assert_eq!(result.totals.saved_tokens, None);
    assert!(
        result.coverage.aggregate_only
            && result.coverage.rolling_window
            && result.coverage.unidentified_rows_observed
    );
    assert!(!result.coverage.complete_traffic);
}

#[test]
fn metrics_store_partial_subtotals_and_runtime_restart_stay_distinct() {
    let store = MetricsStore::in_directory(&fixture_root());
    let first = observation("same", "2026-11-04T08:00:00Z");
    let mut restarted = first.clone();
    restarted.runtime_generation = "00000000-0000-4000-8000-000000000002".into();
    let mut unknown = observation("unknown", "2026-11-04T08:00:00Z");
    unknown.after.tokens = None;
    insert(&store, &[first, restarted, unknown]).unwrap();
    let result = store.query_at(Period::Today, now()).unwrap();
    assert_eq!(result.coverage.events, 3);
    assert_eq!(result.coverage.distinct_origins, 3);
    assert_eq!(result.totals.comparable_events, 2);
    assert_eq!(result.totals.excluded_events, 1);
    assert_eq!(result.totals.before_tokens, Some(200));
    assert_eq!(result.totals.saved_percent, Some(50.0));
    assert!(!result.coverage.complete_traffic);
}

#[test]
fn metrics_store_oversize_database_is_rejected_before_sqlite_changes_it() {
    let root = fixture_root();
    fs::create_dir(&root).unwrap();
    let path = root.join("metrics.sqlite3");
    let file = fs::File::create(&path).unwrap();
    file.set_len(128 * 1024 * 1024 + 1).unwrap();
    drop(file);
    assert_eq!(
        MetricsStore::in_directory(&root)
            .query_at(Period::Today, now())
            .unwrap_err(),
        MetricsError::Capacity
    );
    assert_eq!(fs::metadata(path).unwrap().len(), 128 * 1024 * 1024 + 1);
}

#[test]
fn metrics_store_child_process_fixture() {
    let Some(stage) = std::env::var_os("HEADROOM_METRICS_TEST_STAGE") else {
        return;
    };
    let root = PathBuf::from(std::env::var_os("HEADROOM_METRICS_TEST_ROOT").unwrap());
    let boundary = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("work/proofs/metrics-store/tests");
    assert!(
        root.starts_with(boundary)
            && !root
                .components()
                .any(|c| matches!(c, std::path::Component::ParentDir))
    );
    let store = MetricsStore::in_directory(&root);
    if stage == "write" {
        let mut first = observation("small", "2026-11-04T08:00:00Z");
        first.before.tokens = Some(1000);
        first.after.tokens = Some(500);
        let mut second = observation("large", "2026-11-04T08:00:00Z");
        second.before.tokens = Some(9000);
        second.after.tokens = Some(8100);
        insert(&store, &[first, second]).unwrap();
    } else {
        assert_eq!(stage, "read");
    }
    let today = store.query_at(Period::Today, now()).unwrap();
    let session = store.query_at(Period::Session, now()).unwrap();
    assert_eq!(today.totals.saved_tokens, Some(1400));
    assert_eq!(today.totals.saved_percent, Some(14.0));
    let report = serde_json::json!({"fixture_only":true,"pid":std::process::id(),"session_id":today.monitoring_session_id,
        "saved_tokens":today.totals.saved_tokens,"saved_percent":today.totals.saved_percent,
        "session_events":session.coverage.events,"retention_days":today.retention_days});
    fs::write(
        root.parent()
            .unwrap()
            .join(format!("{}-process.json", stage.to_string_lossy())),
        serde_json::to_vec(&report).unwrap(),
    )
    .unwrap();
}

#[test]
fn metrics_store_real_process_restart_preserves_history_and_starts_new_session() {
    let root = fixture_root();
    for stage in ["write", "read"] {
        let mut child = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "metrics::store_tests::metrics_store_child_process_fixture",
                "--test-threads=1",
            ])
            .env("HEADROOM_METRICS_TEST_STAGE", stage)
            .env("HEADROOM_METRICS_TEST_ROOT", &root)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        loop {
            if let Some(status) = child.try_wait().unwrap() {
                assert!(status.success(), "fixture process must exit successfully");
                break;
            }
            if std::time::Instant::now() > deadline {
                let _ = child.kill();
                let _ = child.wait();
                panic!("fixture process deadline");
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
    }
    let report = |stage| {
        serde_json::from_slice::<serde_json::Value>(
            &fs::read(root.parent().unwrap().join(format!("{stage}-process.json"))).unwrap(),
        )
        .unwrap()
    };
    let writer = report("write");
    let reader = report("read");
    assert_ne!(writer["pid"], reader["pid"]);
    assert_ne!(writer["session_id"], reader["session_id"]);
    assert_eq!(writer["session_events"], 2);
    assert_eq!(reader["session_events"], 0);
    assert_eq!(reader["saved_percent"], 14.0);
    assert_eq!(reader["retention_days"], 30);
}
