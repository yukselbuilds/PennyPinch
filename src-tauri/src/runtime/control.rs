//! Private optimization control: only an owned backend, one POST, no retry/fallback.
use serde::Deserialize;
use serde_json::json;

pub(crate) const MAX_CONTROL_COUNTER: u64 = 9_007_199_254_740_991;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ControlError {
    InvalidInput,
    InvalidResponse,
    Unavailable,
    OwnershipLost,
    Transport,
    ConfigConflict,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct OptimizationState {
    pub enabled: bool,
    pub revision: u64,
    pub can_pause: bool,
    pub active_http_requests: u64,
}
#[derive(Clone, Copy)]
pub(crate) enum ControlOperation {
    Read,
    Set {
        expected_revision: u64,
        enabled: bool,
    },
}
pub(crate) struct ControlTarget<'a> {
    pub port: u16,
    pub owner_pid: u32,
    pub generation: &'a str,
    pub secret: &'a str,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireReply {
    schema_version: u8,
    request_id: String,
    runtime_generation: String,
    status: String,
    code: Option<String>,
    enabled: bool,
    revision: u64,
    can_pause: bool,
    active_http_requests: u64,
}

fn valid_id(id: &str) -> bool {
    uuid::Uuid::parse_str(id).is_ok_and(|v| !v.is_nil() && v.to_string() == id)
}
pub(super) fn valid_secret(s: &str) -> bool {
    s.len() == 32
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        && s.bytes().any(|b| b != b'0')
}
fn parse_reply(
    status: u16,
    body: &[u8],
    id: &str,
    generation: &str,
    operation: ControlOperation,
) -> Result<OptimizationState, ControlError> {
    if body.len() > 2048 {
        return Err(ControlError::InvalidResponse);
    }
    let reply: WireReply =
        serde_json::from_slice(body).map_err(|_| ControlError::InvalidResponse)?;
    if reply.schema_version != 1
        || reply.request_id != id
        || reply.runtime_generation != generation
        || reply.revision > MAX_CONTROL_COUNTER
        || reply.active_http_requests > MAX_CONTROL_COUNTER
    {
        return Err(ControlError::InvalidResponse);
    }
    if status == 409 && reply.status == "error" && reply.code.as_deref() == Some("config_conflict")
    {
        return Err(ControlError::ConfigConflict);
    }
    if status != 200 || reply.status != "ok" || reply.code.is_some() {
        return Err(ControlError::InvalidResponse);
    }
    if let ControlOperation::Set {
        expected_revision,
        enabled,
    } = operation
    {
        if reply.enabled != enabled
            || reply.revision < expected_revision
            || reply.revision > expected_revision.saturating_add(1)
        {
            return Err(ControlError::InvalidResponse);
        }
    }
    Ok(OptimizationState {
        enabled: reply.enabled,
        revision: reply.revision,
        can_pause: reply.can_pause,
        active_http_requests: reply.active_http_requests,
    })
}

pub(crate) fn request(
    target: ControlTarget<'_>,
    id: &str,
    operation: ControlOperation,
    is_alive: impl Fn() -> bool,
) -> Result<OptimizationState, ControlError> {
    if target.port == 0 || target.owner_pid == 0 {
        return Err(ControlError::InvalidInput);
    }
    let frame = encode_request(id, target.generation, target.secret, operation)?;
    #[cfg(not(windows))]
    {
        let _ = (target, id, operation, is_alive, frame);
        Err(ControlError::Unavailable)
    }
    #[cfg(windows)]
    {
        use super::ingress::{verify_backend_peer, IngressError};
        use std::io::Write;
        use std::net::{SocketAddr, TcpStream};
        use std::time::{Duration, Instant};
        if !is_alive() {
            return Err(ControlError::OwnershipLost);
        }
        let deadline = Instant::now() + Duration::from_millis(1500);
        let address = SocketAddr::from(([127, 0, 0, 1], target.port));
        let mut stream = TcpStream::connect_timeout(&address, Duration::from_millis(300))
            .map_err(|_| ControlError::Transport)?;
        let peer_deadline = Instant::now() + Duration::from_millis(50);
        loop {
            match verify_backend_peer(&stream, target.owner_pid) {
                Ok(()) => break,
                Err(IngressError::PeerOwnerMissing) if Instant::now() < peer_deadline => {
                    std::thread::sleep(Duration::from_millis(2))
                }
                _ => return Err(ControlError::OwnershipLost),
            }
        }
        if !is_alive() {
            return Err(ControlError::OwnershipLost);
        }
        stream
            .set_write_timeout(Some(deadline.saturating_duration_since(Instant::now())))
            .map_err(|_| ControlError::Transport)?;
        stream
            .write_all(&frame)
            .map_err(|_| ControlError::Transport)?;
        let (status, body) = super::manager::read_http_body_with_limit(&mut stream, deadline, 2048)
            .ok_or(ControlError::Transport)?;
        if !is_alive() {
            return Err(ControlError::OwnershipLost);
        }
        parse_reply(status, &body, id, target.generation, operation)
    }
}

fn encode_request(
    id: &str,
    generation: &str,
    secret: &str,
    operation: ControlOperation,
) -> Result<Vec<u8>, ControlError> {
    if !valid_id(id)
        || !valid_secret(secret)
        || !valid_secret(generation)
        || matches!(operation, ControlOperation::Set { expected_revision, .. } if expected_revision > MAX_CONTROL_COUNTER)
    {
        return Err(ControlError::InvalidInput);
    }
    let mut body =
        json!({"schema_version":1,"request_id":id,"runtime_generation":generation,"action":"read"});
    if let ControlOperation::Set {
        expected_revision,
        enabled,
    } = operation
    {
        body["action"] = json!("set");
        body["expected_revision"] = json!(expected_revision);
        body["enabled"] = json!(enabled);
    }
    let body = serde_json::to_vec(&body).map_err(|_| ControlError::InvalidInput)?;
    let mut frame = format!("POST /__headroom_desktop/control HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nX-Headroom-Desktop-Control: {secret}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len()).into_bytes();
    frame.extend_from_slice(&body);
    Ok(frame)
}

#[cfg(target_os = "macos")]
pub(crate) fn request_unix(
    target: super::unix_backend::UnixTarget<'_>,
    generation: &str,
    secret: &str,
    id: &str,
    operation: ControlOperation,
    is_alive: impl Fn() -> bool,
) -> Result<OptimizationState, ControlError> {
    use super::unix_backend::UnixBackendError;
    use std::io::Write;
    use std::time::{Duration, Instant};

    let frame = encode_request(id, generation, secret, operation)?;
    let deadline = Instant::now() + Duration::from_millis(1500);
    let mut stream = target.connect(&is_alive).map_err(|error| match error {
        UnixBackendError::Transport => ControlError::Transport,
        _ => ControlError::OwnershipLost,
    })?;
    stream
        .set_write_timeout(Some(deadline.saturating_duration_since(Instant::now())))
        .map_err(|_| ControlError::Transport)?;
    stream
        .write_all(&frame)
        .map_err(|_| ControlError::Transport)?;
    let (status, body) = super::manager::read_http_body_with_limit(&mut stream, deadline, 2048)
        .ok_or(ControlError::Transport)?;
    if !is_alive() {
        return Err(ControlError::OwnershipLost);
    }
    parse_reply(status, &body, id, generation, operation)
}

#[cfg(test)]
mod tests {
    use super::*;
    const ID: &str = "7132bced-3ce0-4c1b-8f48-d93d7b96f7ce";
    const GENERATION: &str = "7132bced3ce04c1b8f48d93d7b96f7d0";
    fn reply() -> serde_json::Value {
        json!({"schema_version":1,"request_id":ID,"runtime_generation":GENERATION,"status":"ok","code":null,"enabled":false,"revision":1,"can_pause":true,"active_http_requests":2})
    }
    #[test]
    fn ack_is_correlated_bounded_and_allowlisted() {
        let operation = ControlOperation::Set {
            expected_revision: 0,
            enabled: false,
        };
        let parse = |v: serde_json::Value| {
            parse_reply(
                200,
                &serde_json::to_vec(&v).unwrap(),
                ID,
                GENERATION,
                operation,
            )
        };
        assert_eq!(parse(reply()).unwrap().revision, 1);
        for (key, value) in [
            ("request_id", json!("PRIVATE_CANARY")),
            ("runtime_generation", json!("a".repeat(32))),
            ("enabled", json!(true)),
            ("revision", json!(3)),
            ("secret", json!("PRIVATE_CANARY")),
        ] {
            let mut v = reply();
            v[key] = value;
            assert_eq!(parse(v).unwrap_err(), ControlError::InvalidResponse);
        }
        let mut v = reply();
        v["status"] = json!("error");
        v["code"] = json!("config_conflict");
        assert_eq!(
            parse_reply(
                409,
                &serde_json::to_vec(&v).unwrap(),
                ID,
                GENERATION,
                operation
            )
            .unwrap_err(),
            ControlError::ConfigConflict
        );
    }

    #[cfg(windows)]
    fn server(
        body: Option<Vec<u8>>,
        revoke: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
    ) -> (
        u16,
        std::sync::Arc<std::sync::atomic::AtomicUsize>,
        std::thread::JoinHandle<()>,
    ) {
        use std::io::{Read, Write};
        use std::net::TcpListener;
        use std::sync::{
            atomic::{AtomicUsize, Ordering},
            Arc,
        };
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let count = Arc::new(AtomicUsize::new(0));
        let observed = count.clone();
        let join = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(2)))
                .unwrap();
            let mut bytes = Vec::new();
            let mut byte = [0];
            while bytes.len() < 4096 && !bytes.ends_with(b"\r\n\r\n") {
                match stream.read(&mut byte) {
                    Ok(1) => bytes.push(byte[0]),
                    _ => break,
                }
            }
            if bytes.is_empty() {
                return;
            }
            assert!(bytes.starts_with(b"POST /__headroom_desktop/control HTTP/1.1\r\n"));
            let header = std::str::from_utf8(&bytes).unwrap();
            let length: usize = header
                .lines()
                .find_map(|l| l.strip_prefix("Content-Length: "))
                .unwrap()
                .parse()
                .unwrap();
            let mut payload = vec![0; length];
            stream.read_exact(&mut payload).unwrap();
            observed.store(1, Ordering::SeqCst);
            let parsed: serde_json::Value = serde_json::from_slice(&payload).unwrap();
            assert_eq!(parsed["request_id"], ID);
            assert_eq!(parsed["runtime_generation"], GENERATION);
            if let Some(flag) = revoke {
                flag.store(false, Ordering::SeqCst);
            }
            if let Some(body) = body {
                let header = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                let _ = stream.write_all(header.as_bytes());
                let _ = stream.write_all(&body);
            }
        });
        (port, count, join)
    }
    #[test]
    #[cfg(windows)]
    fn owned_control_is_one_post_and_rejects_foreign_peer_before_secret_write() {
        use std::sync::atomic::Ordering;
        let (port, count, join) = server(Some(serde_json::to_vec(&reply()).unwrap()), None);
        let target = ControlTarget {
            port,
            owner_pid: std::process::id(),
            generation: GENERATION,
            secret: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        };
        assert_eq!(
            request(
                target,
                ID,
                ControlOperation::Set {
                    expected_revision: 0,
                    enabled: false
                },
                || true
            )
            .unwrap()
            .enabled,
            false
        );
        join.join().unwrap();
        assert_eq!(count.load(Ordering::SeqCst), 1);
        let (port, count, join) = server(Some(serde_json::to_vec(&reply()).unwrap()), None);
        let target = ControlTarget {
            port,
            owner_pid: std::process::id() + 1,
            generation: GENERATION,
            secret: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        };
        assert_eq!(
            request(target, ID, ControlOperation::Read, || true).unwrap_err(),
            ControlError::OwnershipLost
        );
        join.join().unwrap();
        assert_eq!(count.load(Ordering::SeqCst), 0);
    }
    #[test]
    #[cfg(windows)]
    fn failed_ack_and_revoked_liveness_never_retry_or_report_enabled() {
        use std::sync::{
            atomic::{AtomicBool, Ordering},
            Arc,
        };
        let (port, count, join) = server(None, None);
        let target = ControlTarget {
            port,
            owner_pid: std::process::id(),
            generation: GENERATION,
            secret: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        };
        assert_eq!(
            request(
                target,
                ID,
                ControlOperation::Set {
                    expected_revision: 0,
                    enabled: false
                },
                || true
            )
            .unwrap_err(),
            ControlError::Transport
        );
        join.join().unwrap();
        assert_eq!(count.load(Ordering::SeqCst), 1);
        let alive = Arc::new(AtomicBool::new(true));
        let (port, count, join) = server(
            Some(serde_json::to_vec(&reply()).unwrap()),
            Some(alive.clone()),
        );
        let target = ControlTarget {
            port,
            owner_pid: std::process::id(),
            generation: GENERATION,
            secret: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        };
        assert_eq!(
            request(target, ID, ControlOperation::Read, || alive
                .load(Ordering::SeqCst))
            .unwrap_err(),
            ControlError::OwnershipLost
        );
        join.join().unwrap();
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }
}
