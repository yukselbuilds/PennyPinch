use crate::runtime::ingress::StableIngress;
use crate::runtime::ingress_relay::{BackendTarget, IngressRelay, RelayError, RelayLimits};
use std::fs;
use std::io::{Read, Write};
use std::net::{Shutdown, TcpStream};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::sync::{mpsc, Arc};
use std::thread;
use std::time::{Duration, Instant};
use uuid::Uuid;

struct FixtureRoot {
    path: PathBuf,
    identity: (u64, u64),
}

impl FixtureRoot {
    fn new() -> Self {
        let path = PathBuf::from(
            std::env::var_os("HEADROOM_MACOS_PROOF_ROOT")
                .expect("CI must provide HEADROOM_MACOS_PROOF_ROOT"),
        );
        assert!(path.is_absolute());
        let info = fs::symlink_metadata(&path).expect("proof root exists");
        assert!(info.is_dir() && !info.file_type().is_symlink());
        assert_eq!(info.uid(), unsafe { libc::geteuid() });
        assert_eq!(info.mode() & 0o7777, 0o700);
        assert_eq!(fs::canonicalize(&path).unwrap(), path);

        let suffix = Uuid::new_v4().simple().to_string();
        let directory = path.join(format!("rl-{}", &suffix[..8]));
        fs::create_dir(&directory).expect("create isolated relay fixture root");
        fs::set_permissions(&directory, fs::Permissions::from_mode(0o700))
            .expect("restrict relay fixture root");
        assert_eq!(fs::canonicalize(&directory).unwrap(), directory);
        let info = fs::symlink_metadata(&directory).unwrap();
        assert_eq!(info.mode() & 0o7777, 0o700);
        Self {
            identity: (info.dev(), info.ino()),
            path: directory,
        }
    }

    fn unix_socket(&self) -> (PathBuf, UnixListener) {
        let path = self.path.join("s");
        assert!(path.as_os_str().as_bytes().len() < 104);
        (
            path.clone(),
            UnixListener::bind(path).expect("bind private backend socket"),
        )
    }
}

impl Drop for FixtureRoot {
    fn drop(&mut self) {
        let Ok(current) = fs::symlink_metadata(&self.path) else {
            return;
        };
        if !current.is_dir()
            || current.file_type().is_symlink()
            || (current.dev(), current.ino()) != self.identity
            || fs::canonicalize(&self.path).ok().as_deref() != Some(self.path.as_path())
        {
            return;
        }
        if let Ok(entries) = fs::read_dir(&self.path) {
            for entry in entries.flatten() {
                if let Ok(kind) = entry.file_type() {
                    if kind.is_file() || kind.is_symlink() || kind.is_socket() {
                        let _ = fs::remove_file(entry.path());
                    }
                }
            }
        }
        let _ = fs::remove_dir(&self.path);
    }
}

fn limits() -> RelayLimits {
    RelayLimits {
        max_connections: 4,
        buffer_bytes: 1024,
        connect_timeout: Duration::from_secs(1),
        idle_timeout: Duration::from_secs(4),
        lifetime: Duration::from_secs(6),
        cancellation_grace: Duration::from_millis(500),
    }
}

fn target(
    socket_path: PathBuf,
    pid: u32,
    alive: Arc<dyn Fn() -> bool + Send + Sync>,
) -> BackendTarget {
    BackendTarget::new_unix(socket_path, Uuid::new_v4(), pid, alive)
        .expect("valid pinned Unix target")
}

fn accept_bounded(listener: UnixListener) -> Option<UnixStream> {
    listener.set_nonblocking(true).ok()?;
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        match listener.accept() {
            Ok((stream, _)) => {
                stream.set_nonblocking(false).ok()?;
                return Some(stream);
            }
            Err(error)
                if error.kind() == std::io::ErrorKind::WouldBlock && Instant::now() < deadline =>
            {
                thread::sleep(Duration::from_millis(5));
            }
            _ => return None,
        }
    }
}

fn wait_for_no_active(relay: &IngressRelay) -> bool {
    let deadline = Instant::now() + Duration::from_secs(2);
    while relay.active_connection_count() != 0 && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(5));
    }
    relay.active_connection_count() == 0
}

fn wait_for_failure(relay: &IngressRelay, expected: RelayError) -> bool {
    let deadline = Instant::now() + Duration::from_secs(2);
    while relay.last_fixed_failure() != Some(expected) && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(5));
    }
    relay.last_fixed_failure() == Some(expected)
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum SocketEnd {
    Eof,
    Reset,
}

fn read_socket_to_close(stream: &mut UnixStream) -> Option<(Vec<u8>, SocketEnd)> {
    stream.set_nonblocking(true).ok()?;
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut bytes = Vec::new();
    let mut chunk = [0u8; 512];
    while Instant::now() < deadline {
        match stream.read(&mut chunk) {
            Ok(0) => return Some((bytes, SocketEnd::Eof)),
            Ok(count) if bytes.len() + count <= 4096 => {
                bytes.extend_from_slice(&chunk[..count]);
            }
            Ok(_) => return None,
            Err(error)
                if error.kind() == std::io::ErrorKind::WouldBlock
                    || error.kind() == std::io::ErrorKind::Interrupted =>
            {
                thread::sleep(Duration::from_millis(5));
            }
            Err(error) if error.kind() == std::io::ErrorKind::ConnectionReset => {
                return Some((bytes, SocketEnd::Reset));
            }
            _ => return None,
        }
    }
    None
}

#[test]
fn unix_route_preserves_delta_before_terminal_and_graceful_half_close_drain() {
    const REQUEST: &[u8] = b"POST /v1/responses HTTP/1.1\r\nContent-Length: 0\r\n\r\n";
    const DELTA: &[u8] = b"event: response.output_text.delta\ndata: {\"delta\":\"fixture\"}\n\n";
    const TERMINAL: &[u8] = b"event: response.completed\ndata: {}\n\n";

    let root = FixtureRoot::new();
    let (socket_path, listener) = root.unix_socket();
    let (delta_tx, delta_rx) = mpsc::sync_channel(1);
    let (terminal_tx, terminal_rx) = mpsc::sync_channel(1);
    let server = thread::spawn(move || {
        let Some(mut backend) = accept_bounded(listener) else {
            let _ = delta_tx.send(false);
            return false;
        };
        let Some((request, request_end)) = read_socket_to_close(&mut backend) else {
            let _ = delta_tx.send(false);
            return false;
        };
        if request_end != SocketEnd::Eof || request != REQUEST || backend.write_all(DELTA).is_err()
        {
            let _ = delta_tx.send(false);
            return false;
        }
        if delta_tx.send(true).is_err()
            || !terminal_rx
                .recv_timeout(Duration::from_secs(2))
                .unwrap_or(false)
        {
            return false;
        }
        backend.write_all(TERMINAL).is_ok()
    });

    let stable = StableIngress::open_or_create(&root.path).expect("open stable ingress");
    let mut relay = IngressRelay::new(stable, limits()).expect("start relay");
    relay
        .set_backend(target(socket_path, std::process::id(), Arc::new(|| true)))
        .expect("route to owned Unix engine");

    let mut client = TcpStream::connect_timeout(&relay.local_addr(), Duration::from_secs(1))
        .expect("connect local ingress");
    client
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    client
        .set_write_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    client.write_all(REQUEST).expect("send bounded request");
    client
        .shutdown(Shutdown::Write)
        .expect("half-close request body");
    let delta_arrived = delta_rx
        .recv_timeout(Duration::from_secs(2))
        .unwrap_or(false);
    let mut delta = vec![0u8; DELTA.len()];
    let delta_read = client.read_exact(&mut delta).is_ok();
    assert!(delta_arrived && delta_read && delta == DELTA);
    assert_eq!(relay.active_connection_count(), 1);
    relay
        .begin_drain()
        .expect("remove route while stream remains active");
    assert_eq!(relay.route_generation(), None);
    terminal_tx
        .send(true)
        .expect("allow existing stream terminal");
    let mut terminal = vec![0u8; TERMINAL.len()];
    assert!(client.read_exact(&mut terminal).is_ok());
    assert_eq!(terminal, TERMINAL);
    let mut eof = [0u8; 1];
    assert_eq!(client.read(&mut eof).unwrap(), 0);
    assert!(server.join().unwrap_or(false));
    assert!(wait_for_no_active(&relay));
    let finished = relay.finish_drain(Duration::from_secs(3));
    assert!(finished.complete && finished.graceful);
    assert_eq!(finished.active_connection_count, 0);
    let drained = relay.shutdown(Duration::from_secs(3));
    assert!(drained.complete && drained.graceful && drained.listener_released);
    assert_eq!(drained.active_connection_count, 0);
}

#[test]
fn wrong_unix_peer_receives_no_client_bytes() {
    let root = FixtureRoot::new();
    let (socket_path, listener) = root.unix_socket();
    let server = thread::spawn(move || {
        let Some(mut backend) = accept_bounded(listener) else {
            return None;
        };
        let (received, closed_by) = read_socket_to_close(&mut backend)?;
        Some((received.len(), closed_by))
    });
    let stable = StableIngress::open_or_create(&root.path).expect("open stable ingress");
    let mut relay = IngressRelay::new(stable, limits()).expect("start relay");
    let pid = std::process::id();
    let other_pid = if pid == 1 { 2 } else { 1 };
    relay
        .set_backend(target(socket_path, other_pid, Arc::new(|| true)))
        .expect("set deliberately mismatched PID target");

    let mut client = TcpStream::connect_timeout(&relay.local_addr(), Duration::from_secs(1))
        .expect("connect local ingress");
    client
        .set_write_timeout(Some(Duration::from_secs(1)))
        .unwrap();
    let _ = client.write_all(b"fixture client request must not reach peer");
    let _ = client.shutdown(Shutdown::Write);
    let (received_bytes, closed_by) = server.join().unwrap_or(None).unwrap();
    assert!(matches!(closed_by, SocketEnd::Eof | SocketEnd::Reset));
    assert_eq!(received_bytes, 0);
    assert!(wait_for_no_active(&relay));
    assert_eq!(
        relay.last_fixed_failure(),
        Some(RelayError::PeerOwnerMismatch)
    );
    let drained = relay.shutdown(Duration::from_secs(3));
    assert!(drained.complete && drained.listener_released);
}

#[test]
fn dead_target_liveness_prevents_unix_connect_and_forwarding() {
    let root = FixtureRoot::new();
    let (socket_path, listener) = root.unix_socket();
    listener
        .set_nonblocking(true)
        .expect("bound check is nonblocking");
    let stable = StableIngress::open_or_create(&root.path).expect("open stable ingress");
    let mut relay = IngressRelay::new(stable, limits()).expect("start relay");
    relay
        .set_backend(target(socket_path, std::process::id(), Arc::new(|| false)))
        .expect("set dead target");

    let mut client = TcpStream::connect_timeout(&relay.local_addr(), Duration::from_secs(1))
        .expect("connect local ingress");
    let _ = client.write_all(b"fixture bytes must remain unread");
    let _ = client.shutdown(Shutdown::Write);
    assert!(wait_for_failure(&relay, RelayError::TargetNotAlive));
    assert!(wait_for_no_active(&relay));
    assert!(matches!(
        listener.accept(),
        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock
    ));
    assert_eq!(relay.last_fixed_failure(), Some(RelayError::TargetNotAlive));
    drop(client);
    drop(listener);
    let drained = relay.shutdown(Duration::from_secs(3));
    assert!(drained.complete && drained.listener_released);
}
