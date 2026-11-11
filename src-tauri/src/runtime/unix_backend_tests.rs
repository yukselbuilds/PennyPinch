use super::{UnixBackendError, UnixTarget};
use crate::metrics::normalize::MeasurementStatus;
use crate::runtime::control::{ControlError, ControlOperation, OptimizationState};
use std::fs;
use std::io::{Read, Write};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

const GENERATION: &str = "00000000000040008000000000000001";
const SECRET: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const REQUEST_ID: &str = "7132bced-3ce0-4c1b-8f48-d93d7b96f7ce";

struct TestSocket {
    directory: PathBuf,
    path: PathBuf,
}

impl TestSocket {
    fn new() -> Self {
        let manifest = fs::canonicalize(env!("CARGO_MANIFEST_DIR")).expect("canonical manifest");
        let work = manifest.parent().expect("manifest parent").join("work");
        fs::create_dir_all(&work).expect("create fixture work root");
        let work = fs::canonicalize(work).expect("canonical fixture work root");
        let suffix = uuid::Uuid::new_v4().simple().to_string();
        let directory = work.join(format!("ux-{}", &suffix[..8]));
        fs::create_dir(&directory).expect("create owned fixture directory");
        fs::set_permissions(&directory, fs::Permissions::from_mode(0o700))
            .expect("restrict fixture directory");
        let path = directory.join("s");
        assert!(path.as_os_str().as_bytes().len() < 104);
        Self { directory, path }
    }

    fn listen(&self) -> UnixListener {
        UnixListener::bind(&self.path).expect("bind fixture socket")
    }

    fn target(&self, expected_pid: u32) -> UnixTarget<'_> {
        UnixTarget {
            socket_path: &self.path,
            expected_pid,
        }
    }
}

impl Drop for TestSocket {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
        let _ = fs::remove_dir(&self.directory);
    }
}

fn read_request(stream: &mut UnixStream, request_prefix: &[u8]) -> usize {
    let mut bytes = Vec::with_capacity(512);
    let mut byte = [0u8; 1];
    while bytes.len() < 8192 && !bytes.ends_with(b"\r\n\r\n") {
        match stream.read(&mut byte) {
            Ok(1) => bytes.push(byte[0]),
            Ok(0) | Err(_) => return bytes.len(),
            _ => unreachable!(),
        }
    }
    if bytes.is_empty() || !bytes.ends_with(b"\r\n\r\n") {
        return bytes.len();
    }
    assert!(
        bytes.starts_with(request_prefix),
        "unexpected fixed request route"
    );
    let header_end = bytes.len();
    let headers = std::str::from_utf8(&bytes).expect("fixture request headers are ASCII");
    let content_length = headers
        .lines()
        .find_map(|line| line.strip_prefix("Content-Length: "))
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(0)
        .min(8192usize.saturating_sub(header_end));
    let mut remaining = content_length;
    let mut chunk = [0u8; 512];
    while remaining > 0 {
        let take = remaining.min(chunk.len());
        match stream.read(&mut chunk[..take]) {
            Ok(0) | Err(_) => break,
            Ok(count) => remaining -= count,
        }
    }
    header_end + content_length - remaining
}

fn serve_once(
    listener: UnixListener,
    response: Option<Vec<u8>>,
    request_prefix: &'static [u8],
) -> JoinHandle<usize> {
    serve_with_liveness(listener, response, request_prefix, None)
}

fn serve_with_liveness(
    listener: UnixListener,
    response: Option<Vec<u8>>,
    request_prefix: &'static [u8],
    revoke: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
) -> JoinHandle<usize> {
    thread::spawn(move || {
        listener.set_nonblocking(true).expect("nonblocking fixture");
        let deadline = Instant::now() + Duration::from_secs(3);
        let mut stream = loop {
            match listener.accept() {
                Ok((stream, _)) => break stream,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    assert!(Instant::now() < deadline, "fixture accept deadline");
                    thread::sleep(Duration::from_millis(5));
                }
                Err(_) => panic!("fixture accept failed"),
            }
        };
        stream
            .set_nonblocking(false)
            .expect("blocking fixture stream");
        if let Err(error) = stream.set_read_timeout(Some(Duration::from_secs(2))) {
            assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
            stream
                .set_nonblocking(true)
                .expect("bounded closed-peer check");
            let mut byte = [0u8; 1];
            match stream.read(&mut byte) {
                Ok(0) => return 0,
                Err(error)
                    if matches!(
                        error.kind(),
                        std::io::ErrorKind::ConnectionReset
                            | std::io::ErrorKind::ConnectionAborted
                            | std::io::ErrorKind::NotConnected
                    ) =>
                {
                    return 0;
                }
                _ => panic!("unconfigured fixture peer is active or received data"),
            }
        }
        let received = read_request(&mut stream, request_prefix);
        if received > 0 {
            if let Some(alive) = revoke {
                alive.store(false, std::sync::atomic::Ordering::SeqCst);
            }
            if let Some(body) = response {
                let header = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                stream
                    .write_all(header.as_bytes())
                    .expect("write fixture header");
                stream.write_all(&body).expect("write fixture body");
            }
        }
        received
    })
}

#[test]
fn control_discards_reply_after_runtime_liveness_is_lost() {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    let fixture = TestSocket::new();
    let alive = Arc::new(AtomicBool::new(true));
    let join = serve_with_liveness(
        fixture.listen(),
        Some(control_reply()),
        b"POST /__headroom_desktop/control HTTP/1.1\r\n",
        Some(Arc::clone(&alive)),
    );
    assert_eq!(
        crate::runtime::control::request_unix(
            fixture.target(std::process::id()),
            GENERATION,
            SECRET,
            REQUEST_ID,
            ControlOperation::Read,
            || alive.load(Ordering::SeqCst),
        )
        .unwrap_err(),
        ControlError::OwnershipLost
    );
    assert!(join.join().expect("revoked control fixture completes") > 0);
}

fn control_reply() -> Vec<u8> {
    serde_json::json!({
        "schema_version": 1,
        "request_id": REQUEST_ID,
        "runtime_generation": GENERATION,
        "status": "ok",
        "code": null,
        "enabled": true,
        "revision": 5,
        "can_pause": true,
        "active_http_requests": 2
    })
    .to_string()
    .into_bytes()
}

#[test]
fn control_accepts_owned_peer_and_correlates_revision() {
    let fixture = TestSocket::new();
    let join = serve_once(
        fixture.listen(),
        Some(control_reply()),
        b"POST /__headroom_desktop/control HTTP/1.1\r\n",
    );
    let state: OptimizationState = crate::runtime::control::request_unix(
        fixture.target(std::process::id()),
        GENERATION,
        SECRET,
        REQUEST_ID,
        ControlOperation::Set {
            expected_revision: 4,
            enabled: true,
        },
        || true,
    )
    .expect("accept same-process owned control peer");
    assert_eq!(state.revision, 5);
    assert!(state.enabled);
    assert!(join.join().expect("fixture completes") > 0);
}

#[test]
fn wrong_peer_and_revoked_liveness_never_send_http_bytes() {
    let fixture = TestSocket::new();
    let join = serve_once(
        fixture.listen(),
        None,
        b"POST /__headroom_desktop/control HTTP/1.1\r\n",
    );
    let pid = std::process::id();
    let other_pid = if pid < i32::MAX as u32 {
        pid + 1
    } else {
        pid - 1
    };
    assert_eq!(
        crate::runtime::control::request_unix(
            fixture.target(other_pid),
            GENERATION,
            SECRET,
            REQUEST_ID,
            ControlOperation::Read,
            || true,
        )
        .unwrap_err(),
        ControlError::OwnershipLost
    );
    assert_eq!(join.join().expect("foreign peer closes"), 0);

    let fixture = TestSocket::new();
    let listener = fixture.listen();
    listener
        .set_nonblocking(true)
        .expect("make pre-connect check bounded");
    assert_eq!(
        fixture
            .target(std::process::id())
            .connect(|| false)
            .unwrap_err(),
        UnixBackendError::OwnershipLost
    );
    assert!(
        matches!(listener.accept(), Err(error) if error.kind() == std::io::ErrorKind::WouldBlock)
    );
    drop(listener);

    let fixture = TestSocket::new();
    let join = serve_once(
        fixture.listen(),
        None,
        b"POST /__headroom_desktop/control HTTP/1.1\r\n",
    );
    let checks = std::sync::atomic::AtomicUsize::new(0);
    assert_eq!(
        fixture
            .target(std::process::id())
            .connect(|| checks.fetch_add(1, std::sync::atomic::Ordering::SeqCst) == 0)
            .unwrap_err(),
        UnixBackendError::OwnershipLost
    );
    assert_eq!(join.join().expect("post-connect lease closes"), 0);
}

#[test]
fn stats_rejects_foreign_peer_before_sending_http() {
    let fixture = TestSocket::new();
    let join = serve_once(fixture.listen(), None, b"GET /stats?cached=1 HTTP/1.1\r\n");
    let other_pid = if std::process::id() == 1 { 2 } else { 1 };
    let result = crate::runtime::metrics_read::read_owned_projection_unix(
        fixture.target(other_pid),
        GENERATION,
        SECRET,
        || true,
    );
    assert!(matches!(
        result,
        Err(crate::runtime::metrics_read::RuntimeMetricsError::OwnershipLost)
    ));
    assert_eq!(join.join().expect("foreign stats peer closes"), 0);
}

#[test]
fn control_rejects_uncorrelated_reply() {
    let fixture = TestSocket::new();
    let mut reply: serde_json::Value = serde_json::from_slice(&control_reply()).unwrap();
    reply["request_id"] = serde_json::json!("7132bced-3ce0-4c1b-8f48-d93d7b96f7cf");
    let join = serve_once(
        fixture.listen(),
        Some(reply.to_string().into_bytes()),
        b"POST /__headroom_desktop/control HTTP/1.1\r\n",
    );
    assert_eq!(
        crate::runtime::control::request_unix(
            fixture.target(std::process::id()),
            GENERATION,
            SECRET,
            REQUEST_ID,
            ControlOperation::Read,
            || true,
        )
        .unwrap_err(),
        ControlError::InvalidResponse
    );
    assert!(join.join().expect("uncorrelated fixture completes") > 0);
}

#[test]
fn symlink_socket_is_not_followed() {
    let fixture = TestSocket::new();
    let listener = fixture.listen();
    let alias = fixture.directory.join("alias");
    std::os::unix::fs::symlink("s", &alias).unwrap();
    let result = UnixTarget {
        socket_path: &alias,
        expected_pid: std::process::id(),
    }
    .connect(|| true);
    fs::remove_file(alias).unwrap();
    assert_eq!(result.unwrap_err(), UnixBackendError::InvalidEndpoint);
    drop(listener);
}

#[test]
fn unsafe_parent_permissions_are_rejected() {
    let fixture = TestSocket::new();
    let listener = fixture.listen();
    fs::set_permissions(&fixture.directory, fs::Permissions::from_mode(0o755))
        .expect("make fixture parent deliberately unsafe");
    assert_eq!(
        fixture
            .target(std::process::id())
            .connect(|| true)
            .unwrap_err(),
        UnixBackendError::InvalidEndpoint
    );
    drop(listener);
}

#[test]
fn legacy_stats_keep_unknown_measurement_basis() {
    let fixture = TestSocket::new();
    let body = serde_json::json!({
        "recent_requests": [{
            "request_id": "fixture-request",
            "timestamp": "2026-11-04T08:00:00Z",
            "input_tokens_original": 100,
            "input_tokens_optimized": 50,
            "request_messages": ["CONTENT_CANARY"],
            "tags": {"auth": "SECRET_CANARY"}
        }]
    })
    .to_string()
    .into_bytes();
    let join = serve_once(
        fixture.listen(),
        Some(body),
        b"GET /stats?cached=1 HTTP/1.1\r\n",
    );
    let snapshot = crate::runtime::metrics_read::read_owned_projection_unix(
        fixture.target(std::process::id()),
        GENERATION,
        SECRET,
        || true,
    )
    .expect("read owned Unix stats projection");
    assert!(join.join().expect("stats fixture completes") > 0);
    assert_eq!(snapshot.generation, GENERATION);
    assert_eq!(snapshot.projection.batch.events.len(), 1);
    assert_eq!(
        snapshot.projection.batch.events[0].status,
        MeasurementStatus::UnknownBasis
    );
    assert!(!snapshot.projection.complete_traffic);
    let safe = serde_json::to_string(&snapshot.projection.batch).expect("serialize projection");
    assert!(!safe.contains("CANARY"));
    assert!(!safe.contains("request_messages"));
}
