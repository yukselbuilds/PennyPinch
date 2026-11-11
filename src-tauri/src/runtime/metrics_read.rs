//! Read only the current owned backend; no caller-supplied URL or credentials.
use crate::metrics::normalize::EngineProjection;
use chrono::{DateTime, Utc};

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

pub(crate) fn read_owned_projection(
    port: u16,
    expected_pid: u32,
    generation: &str,
    is_alive: impl Fn() -> bool,
) -> Result<OwnedMetricsSnapshot, RuntimeMetricsError> {
    #[cfg(not(windows))]
    {
        let _ = (port, expected_pid, generation, is_alive);
        return Err(RuntimeMetricsError::Unavailable);
    }
    #[cfg(windows)]
    {
        use super::ingress::{verify_backend_peer, IngressError};
        use crate::metrics::normalize::{project_headroom_041, Source, MAX_SNAPSHOT_BYTES};
        use std::io::Write;
        use std::net::{SocketAddr, TcpStream};
        use std::time::{Duration, Instant};
        let parsed =
            uuid::Uuid::parse_str(generation).map_err(|_| RuntimeMetricsError::InvalidSnapshot)?;
        if parsed.is_nil()
            || !super::manager::valid_generation(generation)
            || port == 0
            || expected_pid == 0
        {
            return Err(RuntimeMetricsError::InvalidSnapshot);
        }
        if !is_alive() {
            return Err(RuntimeMetricsError::OwnershipLost);
        }
        let budget = Duration::from_millis(1500);
        let deadline = Instant::now() + budget;
        let address = SocketAddr::from(([127, 0, 0, 1], port));
        let mut connection = TcpStream::connect_timeout(&address, Duration::from_millis(300))
            .map_err(|_| RuntimeMetricsError::Transport)?;
        let peer_deadline = Instant::now() + Duration::from_millis(50);
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
            .map_err(|_| RuntimeMetricsError::Transport)?;
        connection
            .write_all(
                b"GET /stats?cached=1 HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n",
            )
            .map_err(|_| RuntimeMetricsError::Transport)?;
        let (status, body) = super::manager::read_http_body_with_limit(
            &mut connection,
            deadline,
            MAX_SNAPSHOT_BYTES,
        )
        .ok_or(RuntimeMetricsError::Transport)?;
        if !is_alive() {
            return Err(RuntimeMetricsError::OwnershipLost);
        }
        if status != 200 {
            return Err(RuntimeMetricsError::Transport);
        }
        let projection = project_headroom_041(&body, generation, "0.41.0", Source::Proxy)
            .map_err(|_| RuntimeMetricsError::InvalidSnapshot)?;
        Ok(OwnedMetricsSnapshot {
            generation: generation.to_owned(),
            captured_at: Utc::now(),
            projection,
        })
    }
}

#[cfg(target_os = "macos")]
pub(crate) fn read_owned_projection_unix(
    target: super::unix_backend::UnixTarget<'_>,
    generation: &str,
    secret: &str,
    is_alive: impl Fn() -> bool,
) -> Result<OwnedMetricsSnapshot, RuntimeMetricsError> {
    use super::unix_backend::UnixBackendError;
    use crate::metrics::normalize::{project_headroom_041, Source, MAX_SNAPSHOT_BYTES};
    use std::io::Write;
    use std::time::{Duration, Instant};

    let parsed =
        uuid::Uuid::parse_str(generation).map_err(|_| RuntimeMetricsError::InvalidSnapshot)?;
    if parsed.is_nil()
        || !super::manager::valid_generation(generation)
        || !super::control::valid_secret(secret)
    {
        return Err(RuntimeMetricsError::InvalidSnapshot);
    }
    let deadline = Instant::now() + Duration::from_millis(1500);
    let mut connection = target.connect(&is_alive).map_err(|error| match error {
        UnixBackendError::Transport => RuntimeMetricsError::Transport,
        _ => RuntimeMetricsError::OwnershipLost,
    })?;
    connection
        .set_write_timeout(Some(deadline.saturating_duration_since(Instant::now())))
        .map_err(|_| RuntimeMetricsError::Transport)?;
    let request = format!("GET /stats?cached=1 HTTP/1.1\r\nHost: 127.0.0.1\r\nX-Headroom-Desktop-Control: {secret}\r\nConnection: close\r\n\r\n");
    connection
        .write_all(request.as_bytes())
        .map_err(|_| RuntimeMetricsError::Transport)?;
    let (status, body) =
        super::manager::read_http_body_with_limit(&mut connection, deadline, MAX_SNAPSHOT_BYTES)
            .ok_or(RuntimeMetricsError::Transport)?;
    if !is_alive() {
        return Err(RuntimeMetricsError::OwnershipLost);
    }
    if status != 200 {
        return Err(RuntimeMetricsError::Transport);
    }
    let shape: serde_json::Value =
        serde_json::from_slice(&body).map_err(|_| RuntimeMetricsError::InvalidSnapshot)?;
    if shape.get("recent_requests").is_none() && shape.get("request_logs").is_none() {
        return Err(RuntimeMetricsError::InvalidSnapshot);
    }
    let projection = project_headroom_041(&body, generation, "0.41.0", Source::Proxy)
        .map_err(|_| RuntimeMetricsError::InvalidSnapshot)?;
    Ok(OwnedMetricsSnapshot {
        generation: generation.to_owned(),
        captured_at: Utc::now(),
        projection,
    })
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
            read_owned_projection(port, std::process::id(), GENERATION, || true).unwrap();
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
        let result = read_owned_projection(port, std::process::id() + 1, GENERATION, || true);
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
        let result = read_owned_projection(port, std::process::id(), GENERATION, || {
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
        assert!(read_owned_projection(port, std::process::id(), GENERATION, || true).is_ok());
        join.join().unwrap();
        let (port, _, join) = serve(vec![b'x'; 2 * 1024 * 1024 + 1], None);
        assert_eq!(
            read_owned_projection(port, std::process::id(), GENERATION, || true).unwrap_err(),
            RuntimeMetricsError::Transport
        );
        join.join().unwrap();
    }

    #[test]
    fn malformed_metadata_is_fixed_error_without_raw_body() {
        let (port, _, join) = serve(br#"{"secret":"SECRET_CANARY"}"#.to_vec(), None);
        assert_eq!(
            read_owned_projection(port, std::process::id(), GENERATION, || true).unwrap_err(),
            RuntimeMetricsError::InvalidSnapshot
        );
        join.join().unwrap();
    }
}
