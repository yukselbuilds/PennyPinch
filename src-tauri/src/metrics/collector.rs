use super::activity::{
    ActivityDetailReply, ActivityDetailRequest, ActivityListReply, ActivityQueryRequest,
    ActivityReply, ActivitySnapshot,
};
use super::store::{
    MetricsError, MetricsFailure, MetricsQueryRequest, MetricsStore, PeriodMetrics,
};
use crate::insights::rules::Recommendation;
use crate::runtime::manager::RuntimeManager;
use crate::runtime::metrics_read::{OwnedMetricsSnapshot, RuntimeMetricsError};
use chrono::{Local, SecondsFormat, Utc};
use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CollectionStatus {
    Waiting,
    Recording,
    Stopped,
    Interrupted,
    Unavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CollectionError {
    Busy,
    Unavailable,
    OwnershipLost,
    Transport,
    InvalidSnapshot,
    Storage,
}

#[derive(Clone, Debug, Serialize)]
pub struct CollectionSnapshot {
    pub status: CollectionStatus,
    pub interval_seconds: u64,
    pub last_capture_utc: Option<String>,
    pub runtime_generation: Option<String>,
    pub last_error: Option<CollectionError>,
    pub sampling_interrupted_since_start: bool,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum OverviewReply {
    Snapshot(OverviewSnapshot),
    Error(MetricsFailure),
}

#[derive(Debug, Serialize)]
pub struct OverviewSnapshot {
    pub schema_version: u8,
    pub request_id: String,
    pub status: &'static str,
    pub observed_at_utc: String,
    pub data: PeriodMetrics,
    pub collection: CollectionSnapshot,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub recommendations: Vec<Recommendation>,
}

#[derive(Clone)]
pub struct OverviewManager {
    inner: Arc<CollectorInner>,
}
struct CollectorInner {
    store: MetricsStore,
    state: Arc<Mutex<CollectorState>>,
    stop: Arc<AtomicBool>,
    worker: Mutex<Option<JoinHandle<()>>>,
}

struct CollectorState {
    snapshot: CollectionSnapshot,
    last_success: Option<Instant>,
}

fn collection_snapshot(state: &CollectorState) -> CollectionSnapshot {
    let mut collection = state.snapshot.clone();
    if collection.status == CollectionStatus::Recording
        && state.last_success.is_some_and(|at| {
            at.elapsed() > Duration::from_secs(collection.interval_seconds * 2 + 2)
        })
    {
        collection.status = CollectionStatus::Interrupted;
        collection.last_error = Some(CollectionError::Unavailable);
        collection.sampling_interrupted_since_start = true;
    }
    collection
}

impl OverviewManager {
    pub fn new(runtime: RuntimeManager, store: MetricsStore) -> Self {
        Self::with_source(
            store,
            move || runtime.capture_metrics(),
            Duration::from_secs(5),
        )
    }

    fn with_source(
        store: MetricsStore,
        mut source: impl FnMut() -> Result<OwnedMetricsSnapshot, RuntimeMetricsError> + Send + 'static,
        interval: Duration,
    ) -> Self {
        let state = Arc::new(Mutex::new(CollectorState {
            snapshot: CollectionSnapshot {
                status: CollectionStatus::Waiting,
                interval_seconds: interval.as_secs().max(1),
                last_capture_utc: None,
                runtime_generation: None,
                last_error: None,
                sampling_interrupted_since_start: false,
            },
            last_success: None,
        }));
        let stop = Arc::new(AtomicBool::new(false));
        let (poll_state, poll_stop, poll_store) = (state.clone(), stop.clone(), store.clone());
        let worker = thread::Builder::new()
            .name("headroom-metrics-collector".into())
            .spawn(move || {
                let mut last_attempt = None;
                while !poll_stop.load(Ordering::SeqCst) {
                    let started = Instant::now();
                    let next = started + interval;
                    let missed = last_attempt.is_some_and(|last| {
                        started.saturating_duration_since(last)
                            > interval * 2 + Duration::from_secs(1)
                    });
                    last_attempt = Some(started);
                    let result = source();
                    if poll_stop.load(Ordering::SeqCst) {
                        break;
                    }
                    let mut state = poll_state.lock().unwrap_or_else(|p| p.into_inner());
                    if missed {
                        state.snapshot.sampling_interrupted_since_start = true;
                    }
                    match result {
                        Ok(captured) => {
                            // Runtime wire uses UUID hex; H023 event identities use canonical UUID text.
                            let valid =
                                crate::runtime::manager::valid_generation(&captured.generation)
                                    && uuid::Uuid::parse_str(&captured.generation).is_ok_and(
                                        |id| {
                                            !id.is_nil()
                                                && captured.projection.batch.events.iter().all(
                                                    |event| {
                                                        event.runtime_generation == id.to_string()
                                                    },
                                                )
                                        },
                                    );
                            let error = if !valid {
                                Some(CollectionError::InvalidSnapshot)
                            } else if poll_store
                                .ingest_projection(&captured.projection, captured.captured_at)
                                .is_err()
                            {
                                Some(CollectionError::Storage)
                            } else {
                                None
                            };
                            if let Some(error) = error {
                                interrupted(&mut state, error);
                            } else {
                                state.snapshot.status = CollectionStatus::Recording;
                                state.snapshot.last_capture_utc = Some(
                                    captured
                                        .captured_at
                                        .to_rfc3339_opts(SecondsFormat::AutoSi, true),
                                );
                                state.snapshot.runtime_generation = Some(captured.generation);
                                state.snapshot.last_error = None;
                                state.last_success = Some(Instant::now());
                            }
                        }
                        Err(RuntimeMetricsError::Stopped | RuntimeMetricsError::NotReady) => {
                            state.snapshot.status =
                                if matches!(result, Err(RuntimeMetricsError::Stopped)) {
                                    CollectionStatus::Stopped
                                } else {
                                    CollectionStatus::Waiting
                                };
                            state.snapshot.last_error = None;
                            state.snapshot.runtime_generation = None;
                        }
                        Err(error) => interrupted(
                            &mut state,
                            match error {
                                RuntimeMetricsError::Busy => CollectionError::Busy,
                                RuntimeMetricsError::OwnershipLost => {
                                    CollectionError::OwnershipLost
                                }
                                RuntimeMetricsError::Transport => CollectionError::Transport,
                                RuntimeMetricsError::InvalidSnapshot => {
                                    CollectionError::InvalidSnapshot
                                }
                                _ => CollectionError::Unavailable,
                            },
                        ),
                    }
                    drop(state);
                    thread::park_timeout(next.saturating_duration_since(Instant::now()));
                }
            });
        let worker = match worker {
            Ok(worker) => Some(worker),
            Err(_) => {
                interrupted(
                    &mut state.lock().unwrap_or_else(|p| p.into_inner()),
                    CollectionError::Unavailable,
                );
                None
            }
        };
        Self {
            inner: Arc::new(CollectorInner {
                store,
                state,
                stop,
                worker: Mutex::new(worker),
            }),
        }
    }

    pub fn read(
        &self,
        request: MetricsQueryRequest,
        include_recommendations: bool,
    ) -> OverviewReply {
        if !request.is_valid() {
            return OverviewReply::Error(MetricsStore::error_detail(
                request.request_id,
                MetricsError::InvalidInput,
            ));
        }
        let state = self.inner.state.lock().unwrap_or_else(|p| p.into_inner());
        let now = Local::now();
        let collection = collection_snapshot(&state);
        let recording_generation = if collection.status == CollectionStatus::Recording {
            collection.runtime_generation.as_deref()
        } else {
            None
        };
        match self.inner.store.query_with_recommendations_at(
            request.period,
            now,
            include_recommendations,
            recording_generation,
        ) {
            Ok((data, recommendations)) => OverviewReply::Snapshot(OverviewSnapshot {
                schema_version: 1,
                request_id: request.request_id,
                status: "ok",
                observed_at_utc: now
                    .with_timezone(&Utc)
                    .to_rfc3339_opts(SecondsFormat::AutoSi, true),
                data,
                collection,
                recommendations,
            }),
            Err(error) => {
                OverviewReply::Error(MetricsStore::error_detail(request.request_id, error))
            }
        }
    }

    pub fn read_activity(&self, request: ActivityQueryRequest) -> ActivityListReply {
        if !request.is_valid() {
            return ActivityReply::error(request.request_id, MetricsError::InvalidInput);
        }
        let state = self.inner.state.lock().unwrap_or_else(|p| p.into_inner());
        let now = Local::now();
        match self
            .inner
            .store
            .activity_at(request.period, &request.filter, now)
        {
            Ok(data) => ActivityReply::Snapshot(ActivitySnapshot {
                schema_version: 1,
                request_id: request.request_id,
                status: "ok",
                observed_at_utc: now
                    .with_timezone(&Utc)
                    .to_rfc3339_opts(SecondsFormat::AutoSi, true),
                data,
                collection: Some(collection_snapshot(&state)),
            }),
            Err(code) => ActivityReply::error(request.request_id, code),
        }
    }
    pub fn read_activity_detail(&self, request: ActivityDetailRequest) -> ActivityDetailReply {
        if !request.is_valid() {
            return ActivityReply::error(request.request_id, MetricsError::InvalidInput);
        }
        let _state = self.inner.state.lock().unwrap_or_else(|p| p.into_inner());
        let now = Local::now();
        match self
            .inner
            .store
            .activity_detail_at(request.period, &request.origin_id, now)
        {
            Ok(data) => ActivityReply::Snapshot(ActivitySnapshot {
                schema_version: 1,
                request_id: request.request_id,
                status: "ok",
                observed_at_utc: now
                    .with_timezone(&Utc)
                    .to_rfc3339_opts(SecondsFormat::AutoSi, true),
                data,
                collection: None,
            }),
            Err(code) => ActivityReply::error(request.request_id, code),
        }
    }
    pub fn shutdown(&self) -> bool {
        self.inner.shutdown()
    }
}

impl CollectorInner {
    fn shutdown(&self) -> bool {
        self.stop.store(true, Ordering::SeqCst);
        let worker = self.worker.lock().unwrap_or_else(|p| p.into_inner()).take();
        if let Some(worker) = worker {
            worker.thread().unpark();
            worker.join().is_ok()
        } else {
            true
        }
    }
}

impl Drop for CollectorInner {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}

fn interrupted(state: &mut CollectorState, error: CollectionError) {
    state.snapshot.status = if error == CollectionError::Unavailable {
        CollectionStatus::Unavailable
    } else {
        CollectionStatus::Interrupted
    };
    state.snapshot.last_error = Some(error);
    state.snapshot.sampling_interrupted_since_start = true;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics::normalize::{project_headroom_041, Source};
    use crate::metrics::store::Period;
    use chrono::Utc;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::thread;
    use std::time::Instant;
    const FIRST: &str = "00000000000040008000000000000001";
    const NEXT: &str = "00000000000040008000000000000002";
    fn store() -> MetricsStore {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("work/proofs/overview/collector-tests")
            .join(uuid::Uuid::new_v4().to_string());
        std::fs::create_dir_all(root.parent().unwrap()).unwrap();
        MetricsStore::in_directory(&root)
    }
    fn capture(generation: &str) -> OwnedMetricsSnapshot {
        let now = Utc::now();
        let stamp = now.to_rfc3339_opts(chrono::SecondsFormat::AutoSi, true);
        let payload = serde_json::json!({"recent_requests":[{"request_id":"same","timestamp":stamp,"input_tokens_original":100,"input_tokens_optimized":50}]});
        OwnedMetricsSnapshot {
            generation: generation.into(),
            captured_at: now,
            projection: project_headroom_041(
                payload.to_string().as_bytes(),
                generation,
                "0.41.0",
                Source::Proxy,
            )
            .unwrap(),
        }
    }
    fn query(manager: &OverviewManager) -> OverviewSnapshot {
        match manager.read(
            MetricsQueryRequest {
                schema_version: 1,
                request_id: uuid::Uuid::new_v4().to_string(),
                period: Period::Session,
            },
            false,
        ) {
            OverviewReply::Snapshot(value) => value,
            _ => panic!("expected local overview snapshot"),
        }
    }
    fn wait_until(mut condition: impl FnMut() -> bool) {
        let deadline = Instant::now() + Duration::from_secs(2);
        while !condition() {
            assert!(Instant::now() < deadline, "collector deadline");
            thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn polling_replay_and_generation_change_keep_request_identity_separate() {
        let calls = Arc::new(AtomicUsize::new(0));
        let counter = calls.clone();
        let first = capture(FIRST);
        let first_time = first.projection.batch.events[0].timestamp_utc.clone();
        let manager = OverviewManager::with_source(
            store(),
            move || {
                let n = counter.fetch_add(1, Ordering::SeqCst);
                let mut snapshot = capture(if n < 2 { FIRST } else { NEXT });
                snapshot.projection.batch.events[0].timestamp_utc = first_time.clone();
                Ok(snapshot)
            },
            Duration::from_millis(20),
        );
        wait_until(|| calls.load(Ordering::SeqCst) >= 4);
        let value = query(&manager);
        assert_eq!(value.data.coverage.events, 2);
        assert_eq!(value.collection.runtime_generation.as_deref(), Some(NEXT));
        assert!(!value.data.coverage.complete_traffic);
        assert!(value.data.totals.saved_percent.is_none());
        assert!(manager.shutdown());
        let stopped = calls.load(Ordering::SeqCst);
        thread::sleep(Duration::from_millis(50));
        assert_eq!(calls.load(Ordering::SeqCst), stopped);
    }

    #[test]
    fn source_failure_marks_interruption_without_removing_recorded_history() {
        let calls = Arc::new(AtomicUsize::new(0));
        let count = calls.clone();
        let manager = OverviewManager::with_source(
            store(),
            move || {
                if count.fetch_add(1, Ordering::SeqCst) == 0 {
                    Ok(capture(FIRST))
                } else {
                    Err(RuntimeMetricsError::OwnershipLost)
                }
            },
            Duration::from_millis(20),
        );
        wait_until(|| query(&manager).collection.status == CollectionStatus::Interrupted);
        let value = query(&manager);
        assert_eq!(value.data.coverage.events, 1);
        assert_eq!(
            value.collection.last_error,
            Some(CollectionError::OwnershipLost)
        );
        assert!(value.collection.sampling_interrupted_since_start);
        assert!(manager.shutdown());
    }

    #[test]
    #[cfg(windows)]
    fn owned_http_metadata_reaches_sqlite_without_payload_or_replay_inflation() {
        use std::io::{Read, Write};
        use std::net::TcpListener;
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let stamp = Utc::now().to_rfc3339();
        let payload = serde_json::json!({"recent_requests":[{"request_id":"http-fixture", "timestamp":stamp,"model":"observed-model","provider":"observed-provider","client":"codex","role":"subagent","agent_id":"PRIVATE_CANARY",
            "total_latency_ms":12.5,"optimization_latency_ms":0,"transforms_applied":["PRIVATE_CANARY"],
            "input_tokens_original":120000,"input_tokens_optimized":75000,"request_messages":["PRIVATE_CANARY"],"tags":{"auth":"PRIVATE_CANARY"}}],
            "request_logs":[{"request_id":"http-fixture","timestamp":stamp,"model":"observed-model","error":{"body":"PRIVATE_CANARY"},"cache_hit":true,"response_content":"PRIVATE_CANARY"}]}).to_string();
        let server = thread::spawn(move || {
            for _ in 0..2 {
                let (mut socket, _) = listener.accept().unwrap();
                socket
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                let mut bytes = Vec::new();
                let mut byte = [0u8; 1];
                while bytes.len() < 2048 && !bytes.ends_with(b"\r\n\r\n") {
                    if socket.read(&mut byte).unwrap_or(0) != 1 {
                        break;
                    }
                    bytes.push(byte[0]);
                }
                assert!(bytes.starts_with(b"GET /stats?cached=1 HTTP/1.1\r\n"));
                write!(
                    socket,
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    payload.len(),
                    payload
                )
                .unwrap();
            }
        });
        let calls = Arc::new(AtomicUsize::new(0));
        let counter = calls.clone();
        let manager = OverviewManager::with_source(
            store(),
            move || {
                if counter.fetch_add(1, Ordering::SeqCst) < 2 {
                    crate::runtime::metrics_read::read_owned_projection(
                        port,
                        std::process::id(),
                        FIRST,
                        || true,
                    )
                } else {
                    Err(RuntimeMetricsError::Stopped)
                }
            },
            Duration::from_millis(20),
        );
        wait_until(|| query(&manager).collection.status == CollectionStatus::Stopped);
        let snapshot = query(&manager);
        assert_eq!(snapshot.data.coverage.events, 1);
        assert_eq!(snapshot.data.totals.comparable_events, 0);
        assert!(snapshot.data.totals.saved_tokens.is_none());
        assert!(snapshot.data.totals.saved_percent.is_none());
        assert_eq!(snapshot.data.attribution.coverage.model_observed, 1);
        assert_eq!(
            snapshot.data.attribution.groups[0].model.as_deref(),
            Some("observed-model")
        );
        assert!(snapshot.data.attribution.groups[0].client.is_none());
        assert!(snapshot.data.attribution.groups[0].agent.is_none());
        assert!(!serde_json::to_string(&snapshot).unwrap().contains("CANARY"));
        let request = super::ActivityQueryRequest {
            schema_version: 1,
            request_id: uuid::Uuid::new_v4().to_string(),
            period: super::super::store::Period::Session,
            filter: Default::default(),
        };
        let super::ActivityReply::Snapshot(list) = manager.read_activity(request) else {
            panic!("activity read failed")
        };
        assert_eq!(list.data.rows.len(), 1);
        assert_eq!(list.data.rows[0].metadata.total_ms, Some(12.5));
        assert_eq!(
            list.data.rows[0].metadata.outcome,
            super::super::request_metadata::Outcome::ErrorReported
        );
        assert_eq!(list.data.rows[0].metadata.cache_reported_hit, Some(true));
        assert!(list.data.rows[0].metadata.first_token_ms.is_none());
        assert!(list.data.rows[0].identity.client.is_none());
        let origin = list.data.rows[0].origin_id.clone();
        assert!(!serde_json::to_string(&list).unwrap().contains("CANARY"));
        let super::ActivityReply::Snapshot(detail) =
            manager.read_activity_detail(super::ActivityDetailRequest {
                schema_version: 1,
                request_id: uuid::Uuid::new_v4().to_string(),
                period: super::super::store::Period::Session,
                origin_id: origin,
            })
        else {
            panic!("activity detail failed")
        };
        assert_eq!(detail.data.measurements.len(), 1);
        assert!(!serde_json::to_string(&detail).unwrap().contains("CANARY"));
        assert!(manager.shutdown());
        server.join().unwrap();
    }

    #[test]
    fn stopped_source_is_passive_and_mismatched_generation_is_not_stored() {
        let store = store();
        let untouched = store.clone();
        let manager = OverviewManager::with_source(
            store,
            || Err(RuntimeMetricsError::Stopped),
            Duration::from_millis(20),
        );
        wait_until(|| query(&manager).collection.status == CollectionStatus::Stopped);
        assert_eq!(query(&manager).data.coverage.events, 0);
        assert!(manager.shutdown());
        let manager = OverviewManager::with_source(
            untouched,
            || {
                let mut value = capture(FIRST);
                value.generation = NEXT.into();
                Ok(value)
            },
            Duration::from_millis(20),
        );
        wait_until(|| {
            query(&manager).collection.last_error == Some(CollectionError::InvalidSnapshot)
        });
        assert_eq!(query(&manager).data.coverage.events, 0);
        assert!(manager.shutdown());
    }
}
