//! Read only the current owned backend; no caller-supplied URL or credentials.
use crate::metrics::normalize::EngineProjection;
use chrono::{DateTime, Utc};
#[cfg(any(windows, target_os = "macos", target_os = "linux"))]
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RuntimeMetricsError {
    Stopped,
    NotReady,
    Busy,
    Unavailable,
    OwnershipLost,
    Transport,
    InvalidSnapshot,
}

#[derive(Debug)]
pub(crate) struct OwnedMetricsSnapshot {
    pub generation: String,
    pub captured_at: DateTime<Utc>,
    pub projection: EngineProjection,
}

#[cfg(any(windows, target_os = "macos", target_os = "linux"))]
fn private_get(path: &str, secret: &str) -> String {
    format!("GET {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nX-Headroom-Desktop-Control: {secret}\r\nConnection: close\r\n\r\n")
}

#[cfg(any(windows, target_os = "macos", target_os = "linux"))]
fn project_owned_snapshot(
    body: &[u8],
    generation: &str,
    secret: Option<&str>,
    deadline: Instant,
    get: impl FnOnce(&[u8]) -> Result<(u16, Vec<u8>), RuntimeMetricsError>,
) -> Result<OwnedMetricsSnapshot, RuntimeMetricsError> {
    let usage = match secret.filter(|_| Instant::now() < deadline) {
        Some(secret) => {
            let request = private_get("/__headroom_desktop/provider-usage", secret);
            match get(request.as_bytes()) {
                Ok((200, body)) => Some(body),
                Err(RuntimeMetricsError::OwnershipLost) => {
                    return Err(RuntimeMetricsError::OwnershipLost);
                }
                _ => None,
            }
        }
        None => None,
    };
    let projection =
        crate::metrics::normalize::project_owned_headroom_041(body, usage.as_deref(), generation)
            .map_err(|_| RuntimeMetricsError::InvalidSnapshot)?;
    Ok(OwnedMetricsSnapshot {
        generation: generation.to_owned(),
        captured_at: Utc::now(),
        projection,
    })
}

pub(crate) fn read_owned_projection(
    port: u16,
    expected_pid: u32,
    generation: &str,
    secret: Option<&str>,
    is_alive: impl Fn() -> bool,
) -> Result<OwnedMetricsSnapshot, RuntimeMetricsError> {
    #[cfg(not(windows))]
    {
        let _ = (port, expected_pid, generation, secret, is_alive);
        return Err(RuntimeMetricsError::Unavailable);
    }
    #[cfg(windows)]
    {
        use super::ingress::{verify_backend_peer, IngressError};
        use crate::metrics::normalize::MAX_SNAPSHOT_BYTES;
        use std::io::Write;
        use std::net::{SocketAddr, TcpStream};
        let parsed =
            uuid::Uuid::parse_str(generation).map_err(|_| RuntimeMetricsError::InvalidSnapshot)?;
        if parsed.is_nil()
            || !super::manager::valid_generation(generation)
            || port == 0
            || expected_pid == 0
            || secret.is_some_and(|value| !super::control::valid_secret(value))
        {
            return Err(RuntimeMetricsError::InvalidSnapshot);
        }
        let deadline = Instant::now() + Duration::from_millis(1500);
        let address = SocketAddr::from(([127, 0, 0, 1], port));
        let get = |request: &[u8]| {
            if !is_alive() {
                return Err(RuntimeMetricsError::OwnershipLost);
            }
            let timeout = deadline
                .saturating_duration_since(Instant::now())
                .min(Duration::from_millis(300));
            let mut connection = TcpStream::connect_timeout(&address, timeout)
                .map_err(|_| RuntimeMetricsError::Transport)?;
            let peer_deadline = deadline.min(Instant::now() + Duration::from_millis(50));
            loop {
                match verify_backend_peer(&connection, expected_pid) {
                    Ok(()) => break,
                    Err(IngressError::PeerOwnerMissing) if Instant::now() < peer_deadline => {
                        std::thread::sleep(Duration::from_millis(2))
                    }
                    _ => return Err(RuntimeMetricsError::OwnershipLost),
                }
            }
            if !is_alive() {
                return Err(RuntimeMetricsError::OwnershipLost);
            }
            connection
                .set_write_timeout(Some(deadline.saturating_duration_since(Instant::now())))
                .and_then(|_| connection.write_all(request))
                .map_err(|_| RuntimeMetricsError::Transport)?;
            let reply = super::manager::read_http_body_with_limit(
                &mut connection,
                deadline,
                MAX_SNAPSHOT_BYTES,
            );
            if !is_alive() {
                return Err(RuntimeMetricsError::OwnershipLost);
            }
            reply.ok_or(RuntimeMetricsError::Transport)
        };
        let (status, body) =
            get(b"GET /stats?cached=1 HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")?;
        if status != 200 {
            return Err(RuntimeMetricsError::Transport);
        }
        let snapshot = project_owned_snapshot(&body, generation, secret, deadline, get)?;
        if !is_alive() {
            return Err(RuntimeMetricsError::OwnershipLost);
        }
        Ok(snapshot)
    }
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
pub(crate) fn read_owned_projection_unix(
    target: super::unix_backend::UnixTarget<'_>,
    generation: &str,
    secret: &str,
    is_alive: impl Fn() -> bool,
) -> Result<OwnedMetricsSnapshot, RuntimeMetricsError> {
    use super::unix_backend::UnixBackendError;
    use crate::metrics::normalize::MAX_SNAPSHOT_BYTES;
    use std::io::Write;

    let parsed =
        uuid::Uuid::parse_str(generation).map_err(|_| RuntimeMetricsError::InvalidSnapshot)?;
    if parsed.is_nil()
        || !super::manager::valid_generation(generation)
        || !super::control::valid_secret(secret)
    {
        return Err(RuntimeMetricsError::InvalidSnapshot);
    }
    let deadline = Instant::now() + Duration::from_millis(1500);
    let get = |request: &[u8]| {
        let mut connection = target
            .connect_with_timeout(
                deadline.saturating_duration_since(Instant::now()),
                &is_alive,
            )
            .map_err(|error| match error {
                UnixBackendError::Transport => RuntimeMetricsError::Transport,
                _ => RuntimeMetricsError::OwnershipLost,
            })?;
        connection
            .set_write_timeout(Some(deadline.saturating_duration_since(Instant::now())))
            .and_then(|_| connection.write_all(request))
            .map_err(|_| RuntimeMetricsError::Transport)?;
        let reply = super::manager::read_http_body_with_limit(
            &mut connection,
            deadline,
            MAX_SNAPSHOT_BYTES,
        );
        if !is_alive() {
            return Err(RuntimeMetricsError::OwnershipLost);
        }
        reply.ok_or(RuntimeMetricsError::Transport)
    };
    let (status, body) = get(private_get("/stats?cached=1", secret).as_bytes())?;
    if status != 200 {
        return Err(RuntimeMetricsError::Transport);
    }
    let shape: serde_json::Value =
        serde_json::from_slice(&body).map_err(|_| RuntimeMetricsError::InvalidSnapshot)?;
    if shape.get("recent_requests").is_none() && shape.get("request_logs").is_none() {
        return Err(RuntimeMetricsError::InvalidSnapshot);
    }
    let snapshot = project_owned_snapshot(&body, generation, Some(secret), deadline, get)?;
    if !is_alive() {
        return Err(RuntimeMetricsError::OwnershipLost);
    }
    Ok(snapshot)
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use crate::metrics::normalize::MeasurementStatus;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::thread;
    use std::time::Duration;
    const GENERATION: &str = "00000000000040008000000000000001";

    fn serve(
        body: Vec<u8>,
        revoke: Option<Arc<AtomicBool>>,
    ) -> (u16, Arc<AtomicUsize>, thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let port = listener.local_addr().unwrap().port();
        let read = Arc::new(AtomicUsize::new(0));
        let count = read.clone();
        let join = thread::spawn(move || {
            let deadline = std::time::Instant::now() + Duration::from_secs(1);
            let (mut connection, _) = loop {
                match listener.accept() {
                    Ok(connection) => break connection,
                    Err(error)
                        if error.kind() == std::io::ErrorKind::WouldBlock
                            && std::time::Instant::now() < deadline =>
                    {
                        thread::sleep(Duration::from_millis(5))
                    }
                    _ => panic!("bounded fixture accept"),
                }
            };
            connection.set_nonblocking(false).unwrap();
            connection
                .set_read_timeout(Some(Duration::from_millis(700)))
                .unwrap();
            let mut request = Vec::new();
            let mut byte = [0u8; 1];
            while request.len() < 2048 && !request.ends_with(b"\r\n\r\n") {
                match connection.read(&mut byte) {
                    Ok(1) => request.push(byte[0]),
                    _ => break,
                }
            }
            count.store(request.len(), Ordering::SeqCst);
            if request.is_empty() {
                return;
            }
            assert!(request.starts_with(b"GET /stats?cached=1 HTTP/1.1\r\n"));
            if let Some(alive) = revoke {
                alive.store(false, Ordering::SeqCst);
            }
            let header = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            let _ = connection.write_all(header.as_bytes());
            let _ = connection.write_all(&body);
        });
        (port, read, join)
    }

    #[test]
    fn owned_stats_projects_only_metadata_and_never_upgrades_legacy_basis() {
        let body = serde_json::json!({"recent_requests":[{"request_id":"request", "timestamp":"2026-11-04T08:00:00Z", "input_tokens_original":100,"input_tokens_optimized":50,"request_messages":["CONTENT_CANARY"],"tags":{"auth":"SECRET_CANARY"}}]});
        let (port, _, join) = serve(body.to_string().into_bytes(), None);
        let snapshot =
            read_owned_projection(port, std::process::id(), GENERATION, None, || true).unwrap();
        join.join().unwrap();
        assert_eq!(snapshot.generation, GENERATION);
        assert_eq!(snapshot.projection.batch.events.len(), 1);
        assert_eq!(
            snapshot.projection.batch.events[0].status,
            MeasurementStatus::UnknownBasis
        );
        assert!(!snapshot.projection.complete_traffic);
        let safe = serde_json::to_string(&snapshot.projection.batch).unwrap();
        assert!(!safe.contains("CANARY"));
        assert!(!safe.contains("request_messages"));
    }

    #[test]
    fn wrong_peer_receives_no_http_request() {
        let (port, count, join) = serve(br#"{"recent_requests":[]}"#.to_vec(), None);
        let result = read_owned_projection(port, std::process::id() + 1, GENERATION, None, || true);
        assert_eq!(result.unwrap_err(), RuntimeMetricsError::OwnershipLost);
        join.join().unwrap();
        assert_eq!(count.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn revoked_lease_never_connects_or_accepts_a_completed_response() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        assert_eq!(
            read_owned_projection(
                listener.local_addr().unwrap().port(),
                std::process::id(),
                GENERATION,
                None,
                || false
            )
            .unwrap_err(),
            RuntimeMetricsError::OwnershipLost
        );
        assert!(listener.accept().is_err());
        drop(listener);
        let alive = Arc::new(AtomicBool::new(true));
        let check = alive.clone();
        let (port, count, join) = serve(br#"{"recent_requests":[]}"#.to_vec(), Some(alive));
        let result = read_owned_projection(port, std::process::id(), GENERATION, None, || {
            check.load(Ordering::SeqCst)
        });
        assert_eq!(result.unwrap_err(), RuntimeMetricsError::OwnershipLost);
        join.join().unwrap();
        assert!(count.load(Ordering::SeqCst) > 0);
    }

    #[test]
    fn stats_limit_is_larger_than_health_but_still_bounded() {
        let body =
            serde_json::json!({"recent_requests":[],"noise":"x".repeat(80*1024)}).to_string();
        let (port, _, join) = serve(body.into_bytes(), None);
        assert!(read_owned_projection(port, std::process::id(), GENERATION, None, || true).is_ok());
        join.join().unwrap();
        let (port, _, join) = serve(vec![b'x'; 2 * 1024 * 1024 + 1], None);
        assert_eq!(
            read_owned_projection(port, std::process::id(), GENERATION, None, || true).unwrap_err(),
            RuntimeMetricsError::Transport
        );
        join.join().unwrap();
    }

    #[test]
    fn malformed_metadata_is_fixed_error_without_raw_body() {
        let (port, _, join) = serve(br#"{"secret":"SECRET_CANARY"}"#.to_vec(), None);
        assert_eq!(
            read_owned_projection(port, std::process::id(), GENERATION, None, || true).unwrap_err(),
            RuntimeMetricsError::InvalidSnapshot
        );
        join.join().unwrap();
    }

    #[test]
    #[ignore = "run by the isolated Anthropic engine proof"]
    fn owned_anthropic_usage_reaches_activity_detail() {
        assert_owned_usage_detail("desktop-anthropic-usage", 2);
    }

    #[test]
    #[ignore = "run by the isolated Responses engine proof"]
    fn owned_responses_usage_reaches_activity_detail() {
        assert_owned_usage_detail("desktop-responses-usage", 2);
    }

    #[test]
    #[ignore = "run by the isolated WebSocket engine proof"]
    fn owned_websocket_usage_reaches_activity_detail() {
        assert_owned_usage_detail("desktop-ws-usage", 1);
    }

    #[test]
    #[ignore = "run by the isolated Chat Completions engine proof"]
    fn owned_chat_usage_reaches_activity_detail() {
        assert_owned_usage_detail("desktop-chat-usage", 2);
    }

    fn assert_owned_usage_detail(proof_directory: &str, expected_attempts: u64) {
        use crate::metrics::store::{MetricsStore, Period};
        use serde::Deserialize;
        use std::path::{Path, PathBuf};

        #[derive(Deserialize)]
        struct ProofInput {
            port: u16,
            pid: u32,
            generation: String,
            secret: String,
            directory: PathBuf,
            local_input: Option<ProofPair>,
        }
        #[derive(Deserialize)]
        struct ProofPair {
            before: u64,
            after: u64,
        }
        let mut frame = Vec::new();
        std::io::stdin().take(4097).read_to_end(&mut frame).unwrap();
        assert!(frame.len() <= 4096);
        let input: ProofInput = serde_json::from_slice(&frame).expect("owned fixture input");
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../work/proofs/optimization")
            .join(proof_directory)
            .canonicalize()
            .unwrap();
        let directory = input.directory.canonicalize().unwrap();
        assert!(directory != root && directory.starts_with(&root));
        let snapshot = read_owned_projection(
            input.port,
            input.pid,
            &input.generation,
            Some(&input.secret),
            || true,
        )
        .expect("owned fixture metrics");
        let matched: Vec<_> = snapshot
            .projection
            .batch
            .events
            .iter()
            .filter(|event| event.provider_usage.input == Some(2400))
            .collect();
        assert_eq!(matched.len(), 1);
        let store = MetricsStore::new(Some(directory));
        store
            .ingest_projection(&snapshot.projection, snapshot.captured_at)
            .unwrap();
        let detail = store
            .activity_detail_at(Period::Today, &matched[0].origin_id, snapshot.captured_at)
            .unwrap();
        assert_eq!(detail.measurements.len(), 1);
        let row = &detail.measurements[0];
        assert_eq!(row.provider_usage.input, Some(2400));
        assert_eq!(row.provider_usage.output, Some(29));
        assert_eq!(row.provider_usage.cached_input, Some(2000));
        assert_eq!(row.provider_usage.cache_write_input, Some(100));
        assert_eq!(
            row.provider_usage.upstream_attempts,
            Some(expected_attempts)
        );
        if let Some(pair) = input.local_input {
            assert!(row.main_metric_eligible);
            assert_eq!(row.before.tokens, Some(pair.before));
            assert_eq!(row.after.tokens, Some(pair.after));
            assert_eq!(
                row.saved_tokens,
                Some(pair.before as i64 - pair.after as i64)
            );
        } else {
            assert!(!row.main_metric_eligible);
            assert!(row.saved_tokens.is_none());
        }
        assert!(!serde_json::to_string(&detail)
            .unwrap()
            .contains(&input.secret));
    }
}
