use super::{BackendTarget, IngressRelay, RelayLimits};
use crate::runtime::ingress::{verify_backend_peer, StableIngress};
use std::fs;
use std::io::{BufRead, Read, Write};
use std::net::{Ipv4Addr, Shutdown, SocketAddr, SocketAddrV4, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

#[test]
fn opaque_relay_preserves_responses_and_anthropic_bytes_with_half_close() {
    let responses_request = b"POST /v1/responses HTTP/1.1\r\nHost: fixture\r\nAuthorization: Bearer SENTINEL\r\nX-Opaque: \xff\r\n\r\n{\"model\":\"gpt-5-codex\",\"reasoning\":{\"effort\":\"high\"},\"input\":[{\"type\":\"function_call_output\",\"output\":\"opaque\"}]}".to_vec();
    let responses_reply = b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\n\r\ndata: {\"type\":\"response.output_text.delta\",\"delta\":\"fixture\"}\n\n".to_vec();
    assert_opaque_exchange("responses", responses_request, responses_reply);

    let anthropic_request = b"POST /v1/messages HTTP/1.1\r\nHost: fixture\r\nx-api-key: SENTINEL\r\nanthropic-version: 2023-06-01\r\nContent-Type: application/json\r\n\r\n{\"model\":\"claude-opus-4-6\",\"thinking\":{\"type\":\"enabled\",\"budget_tokens\":8192},\"stream\":true}".to_vec();
    let anthropic_reply = b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\n\r\nevent: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"delta\":{\"type\":\"text_delta\",\"text\":\"fixture\"}}\n\nevent: message_stop\ndata: {\"type\":\"message_stop\"}\n\n".to_vec();
    assert_opaque_exchange("anthropic", anthropic_request, anthropic_reply);
}

fn assert_opaque_exchange(label: &str, request: Vec<u8>, response: Vec<u8>) {
    let fixture = FixtureRoot::new();
    let backend = TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0)).unwrap();
    let backend_port = backend.local_addr().unwrap().port();
    backend.set_nonblocking(true).unwrap();
    let (captured_tx, captured_rx) = std::sync::mpsc::channel();
    let (accepted_tx, accepted_rx) = std::sync::mpsc::channel();
    let expected_response = response.clone();
    let backend_thread = thread::spawn(move || {
        let Some((mut stream, _)) = accept_bounded(&backend, Duration::from_secs(2)) else {
            let _ = captured_tx.send(Vec::new());
            return;
        };
        let _ = accepted_tx.send(());
        let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
        let mut received = Vec::new();
        let _ = stream.read_to_end(&mut received);
        let _ = stream.write_all(&expected_response);
        let _ = stream.shutdown(std::net::Shutdown::Write);
        let _ = captured_tx.send(received);
    });

    let stable = StableIngress::open_or_create(&fixture.path).unwrap();
    let mut relay = IngressRelay::new(stable, RelayLimits::default()).unwrap();
    relay
        .set_backend(test_target_waiting_for_accept(backend_port, accepted_rx))
        .unwrap();

    let mut client = TcpStream::connect(relay.local_addr()).unwrap();
    client
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    let _ = client.write_all(&request);
    let _ = client.shutdown(std::net::Shutdown::Write);
    let mut received_response = Vec::new();
    let _ = client.read_to_end(&mut received_response);

    let received_request = captured_rx
        .recv_timeout(Duration::from_secs(2))
        .unwrap_or_default();
    let backend_joined = join_bounded(backend_thread, Duration::from_secs(3)).is_some();
    let shutdown = relay.shutdown(Duration::from_secs(1));
    let reopen_result = relay.set_backend(test_target(1));
    let relay_failure = relay.last_fixed_failure();
    let rebind_available = TcpListener::bind(relay.local_addr()).is_ok();
    fixture.cleanup();
    assert!(backend_joined);
    assert!(shutdown.complete);
    assert!(shutdown.graceful);
    assert!(shutdown.listener_released);
    assert_eq!(reopen_result, Err(super::RelayError::Draining));
    assert!(rebind_available);
    assert!(
        received_request == request,
        "{label} request byte stream changed or was absent; relay_failure={relay_failure:?}"
    );
    assert!(
        received_response == response,
        "{label} response byte stream changed or was absent; relay_failure={relay_failure:?}"
    );
}

struct FixtureRoot {
    path: PathBuf,
    proof_root: PathBuf,
    cleaned: bool,
}

impl FixtureRoot {
    fn new() -> Self {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let project_root = manifest_dir.parent().unwrap().canonicalize().unwrap();
        let proof_root = project_root.join("work/proofs/connection-ingress");
        assert_clean_ancestors(&proof_root);
        fs::create_dir_all(&proof_root).unwrap();
        let proof_root = proof_root.canonicalize().unwrap();
        assert!(proof_root.starts_with(&project_root));
        assert_clean_ancestors(&proof_root);
        let path = proof_root.join(uuid::Uuid::new_v4().to_string());
        fs::create_dir(&path).unwrap();
        let path = path.canonicalize().unwrap();
        assert!(path.starts_with(&proof_root));
        assert_clean_ancestors(&path);
        Self {
            path,
            proof_root,
            cleaned: false,
        }
    }

    fn cleanup(mut self) {
        assert!(self.try_remove_owned_tree());
        self.cleaned = true;
    }

    fn try_remove_owned_tree(&self) -> bool {
        let Ok(resolved) = self.path.canonicalize() else {
            return !self.path.exists();
        };
        if resolved != self.path
            || !resolved.starts_with(&self.proof_root)
            || !clean_tree(&resolved, &resolved)
        {
            return false;
        }
        fs::remove_dir_all(resolved).is_ok()
    }
}

impl Drop for FixtureRoot {
    fn drop(&mut self) {
        if !self.cleaned {
            self.cleaned = self.try_remove_owned_tree();
        }
    }
}

fn assert_clean_ancestors(path: &Path) {
    use std::os::windows::fs::MetadataExt;
    use windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT;
    for ancestor in path.ancestors() {
        if let Ok(metadata) = fs::symlink_metadata(ancestor) {
            assert!(!metadata.file_type().is_symlink());
            assert_eq!(metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT, 0);
        }
    }
}

fn clean_tree(root: &Path, current: &Path) -> bool {
    use std::os::windows::fs::MetadataExt;
    use windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT;
    let Ok(metadata) = fs::symlink_metadata(current) else {
        return false;
    };
    if metadata.file_type().is_symlink()
        || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
        || !current
            .canonicalize()
            .map(|resolved| resolved.starts_with(root))
            .unwrap_or(false)
    {
        return false;
    }
    if metadata.is_dir() {
        let Ok(entries) = fs::read_dir(current) else {
            return false;
        };
        for entry in entries {
            let Ok(entry) = entry else {
                return false;
            };
            if !clean_tree(root, &entry.path()) {
                return false;
            }
        }
    }
    true
}

fn relay_with_target(
    fixture: &FixtureRoot,
    backend_port: u16,
    limits: RelayLimits,
) -> IngressRelay {
    let stable = StableIngress::open_or_create(&fixture.path).unwrap();
    let relay = IngressRelay::new(stable, limits).unwrap();
    relay.set_backend(test_target(backend_port)).unwrap();
    relay
}

fn test_target(port: u16) -> BackendTarget {
    test_target_with_generation(port, uuid::Uuid::new_v4())
}

fn test_target_with_generation(port: u16, generation: uuid::Uuid) -> BackendTarget {
    BackendTarget::new(port, generation, std::process::id(), Arc::new(|| true)).unwrap()
}

fn test_target_waiting_for_accept(
    port: u16,
    accepted: std::sync::mpsc::Receiver<()>,
) -> BackendTarget {
    let accepted = Arc::new(Mutex::new(accepted));
    let checks = Arc::new(AtomicUsize::new(0));
    BackendTarget::new(
        port,
        uuid::Uuid::new_v4(),
        std::process::id(),
        Arc::new(move || match checks.fetch_add(1, Ordering::SeqCst) {
            0 => true,
            1 => accepted
                .lock()
                .map(|receiver| receiver.recv_timeout(Duration::from_millis(500)).is_ok())
                .unwrap_or(false),
            _ => true,
        }),
    )
    .unwrap()
}

#[test]
fn connect_timeout_socket_peer_owner_is_visible_after_accept() {
    let listener = TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0)).unwrap();
    let address = listener.local_addr().unwrap();
    listener.set_nonblocking(true).unwrap();
    let accept_listener = listener.try_clone().unwrap();
    let accept_thread =
        thread::spawn(move || accept_bounded(&accept_listener, Duration::from_secs(1)));
    let client = TcpStream::connect_timeout(&address, Duration::from_secs(1)).unwrap();
    let accepted = join_bounded(accept_thread, Duration::from_secs(2)).flatten();
    assert!(accepted.is_some());
    assert_eq!(
        verify_backend_peer(&client, std::process::id()),
        Ok(()),
        "connect_timeout peer should have an exact owner row"
    );
    drop(accepted);
}

fn read_to_end_timeout(stream: &mut TcpStream, timeout: Duration) -> Vec<u8> {
    let _ = stream.set_read_timeout(Some(timeout));
    let mut bytes = Vec::new();
    let _ = stream.read_to_end(&mut bytes);
    bytes
}

fn accept_bounded(
    listener: &TcpListener,
    timeout: Duration,
) -> Option<(TcpStream, std::net::SocketAddr)> {
    let deadline = Instant::now() + timeout;
    loop {
        match listener.accept() {
            Ok((stream, address)) => {
                stream.set_nonblocking(false).ok()?;
                return Some((stream, address));
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                if Instant::now() >= deadline {
                    return None;
                }
                thread::sleep(Duration::from_millis(5));
            }
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => return None,
        }
    }
}

fn join_bounded<T>(thread: thread::JoinHandle<T>, timeout: Duration) -> Option<T> {
    let deadline = Instant::now() + timeout;
    while !thread.is_finished() && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(5));
    }
    if thread.is_finished() {
        thread.join().ok()
    } else {
        drop(thread);
        None
    }
}

fn wait_child_bounded(child: &mut std::process::Child, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return status.success(),
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(5)),
            _ => break,
        }
    }
    let _ = child.kill();
    let cleanup_deadline = Instant::now() + Duration::from_secs(1);
    while Instant::now() < cleanup_deadline {
        if child.try_wait().ok().flatten().is_some() {
            return false;
        }
        thread::sleep(Duration::from_millis(5));
    }
    false
}

fn wait_for_active(relay: &IngressRelay, expected: usize) -> bool {
    let deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() < deadline {
        if relay.active_connection_count() == expected {
            return true;
        }
        thread::sleep(Duration::from_millis(5));
    }
    relay.active_connection_count() == expected
}

#[test]
fn route_switch_keeps_existing_connection_on_its_original_generation() {
    let fixture = FixtureRoot::new();
    let backend_a = TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0)).unwrap();
    let port_a = backend_a.local_addr().unwrap().port();
    backend_a.set_nonblocking(true).unwrap();
    let backend_b = TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0)).unwrap();
    let port_b = backend_b.local_addr().unwrap().port();
    backend_b.set_nonblocking(true).unwrap();
    let (release_a_tx, release_a_rx) = std::sync::mpsc::channel();
    let (request_a_tx, request_a_rx) = std::sync::mpsc::channel();
    let thread_a = thread::spawn(move || {
        let Some((mut stream, _)) = accept_bounded(&backend_a, Duration::from_secs(2)) else {
            let _ = request_a_tx.send(Vec::new());
            return;
        };
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let mut request = [0; 9];
        let _ = stream.read_exact(&mut request);
        let _ = stream.write_all(b"A-start\n");
        let _ = request_a_tx.send(request.to_vec());
        let _ = release_a_rx.recv_timeout(Duration::from_secs(2));
        let _ = stream.write_all(b"A-end\n");
        let _ = stream.shutdown(Shutdown::Write);
    });
    let (request_b_tx, request_b_rx) = std::sync::mpsc::channel();
    let (release_b_tx, release_b_rx) = std::sync::mpsc::channel();
    let thread_b = thread::spawn(move || {
        let Some((mut stream, _)) = accept_bounded(&backend_b, Duration::from_secs(2)) else {
            let _ = request_b_tx.send(Vec::new());
            return;
        };
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let mut request = Vec::new();
        let _ = stream.read_to_end(&mut request);
        let _ = request_b_tx.send(request);
        let _ = release_b_rx.recv_timeout(Duration::from_secs(2));
        let _ = stream.write_all(b"B-response\n");
        let _ = stream.shutdown(Shutdown::Write);
    });

    let stable = StableIngress::open_or_create(&fixture.path).unwrap();
    let mut relay = IngressRelay::new(stable, RelayLimits::default()).unwrap();
    let stable_address = relay.local_addr();
    let generation_a = uuid::Uuid::new_v4();
    let generation_b = uuid::Uuid::new_v4();
    relay
        .set_backend(test_target_with_generation(port_a, generation_a))
        .unwrap();
    let mut old_client = TcpStream::connect(stable_address).unwrap();
    old_client
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    old_client.write_all(b"request-A").unwrap();
    let mut old_start = [0; 8];
    let old_start_read = old_client.read_exact(&mut old_start).is_ok();
    let request_a = request_a_rx
        .recv_timeout(Duration::from_secs(2))
        .unwrap_or_default();
    assert!(wait_for_generation(&relay, generation_a, 1));

    relay
        .set_backend(test_target_with_generation(port_b, generation_b))
        .unwrap();
    assert_eq!(relay.active_generation_count(generation_a), 1);
    assert_eq!(relay.active_generation_count(generation_b), 0);
    assert_eq!(relay.local_addr(), stable_address);
    let mut new_client = TcpStream::connect(stable_address).unwrap();
    new_client
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    new_client.write_all(b"request-B").unwrap();
    new_client.shutdown(Shutdown::Write).unwrap();
    let request_b = request_b_rx
        .recv_timeout(Duration::from_secs(2))
        .unwrap_or_default();
    assert_eq!(relay.active_generation_count(generation_b), 1);
    let _ = release_b_tx.send(());
    let new_response = read_to_end_timeout(&mut new_client, Duration::from_secs(2));
    let thread_b_joined = join_bounded(thread_b, Duration::from_secs(3)).is_some();
    assert!(wait_for_generation(&relay, generation_b, 0));

    let _ = release_a_tx.send(());
    let old_tail = read_to_end_timeout(&mut old_client, Duration::from_secs(2));
    let thread_a_joined = join_bounded(thread_a, Duration::from_secs(3)).is_some();
    let shutdown = relay.shutdown(Duration::from_secs(1));
    fixture.cleanup();

    assert!(old_start_read);
    assert_eq!(&old_start, b"A-start\n");
    assert_eq!(request_a, b"request-A");
    assert_eq!(request_b, b"request-B");
    assert_eq!(new_response, b"B-response\n");
    assert_eq!(old_tail, b"A-end\n");
    assert_eq!(relay.active_generation_count(generation_a), 0);
    assert!(thread_a_joined && thread_b_joined && shutdown.complete);
}

fn wait_for_generation(relay: &IngressRelay, generation: uuid::Uuid, expected: usize) -> bool {
    let deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() < deadline {
        if relay.active_generation_count(generation) == expected {
            return true;
        }
        thread::sleep(Duration::from_millis(5));
    }
    relay.active_generation_count(generation) == expected
}

#[test]
fn gated_sse_delta_reaches_client_before_backend_terminal_frame() {
    let fixture = FixtureRoot::new();
    let backend = TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0)).unwrap();
    let port = backend.local_addr().unwrap().port();
    backend.set_nonblocking(true).unwrap();
    let delta = b"data: {\"type\":\"response.output_text.delta\",\"delta\":\"early\"}\n\n".to_vec();
    let terminal = b"data: [DONE]\n\n".to_vec();
    let delta_for_server = delta.clone();
    let terminal_for_server = terminal.clone();
    let (sent_tx, sent_rx) = std::sync::mpsc::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let request =
        b"POST /v1/responses HTTP/1.1\r\nAuthorization: Bearer SENTINEL\r\n\r\n{\"stream\":true}"
            .to_vec();
    let expected_request = request.clone();
    let (request_tx, request_rx) = std::sync::mpsc::channel();
    let backend_thread = thread::spawn(move || {
        let Some((mut stream, _)) = accept_bounded(&backend, Duration::from_secs(2)) else {
            let _ = request_tx.send(Vec::new());
            let _ = sent_tx.send(());
            return;
        };
        let mut captured = Vec::new();
        let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
        let _ = stream.read_to_end(&mut captured);
        let _ = request_tx.send(captured);
        let _ = stream.write_all(&delta_for_server);
        let _ = sent_tx.send(());
        let _ = release_rx.recv_timeout(Duration::from_secs(2));
        let _ = stream.write_all(&terminal_for_server);
        let _ = stream.shutdown(Shutdown::Write);
    });
    let mut relay = relay_with_target(&fixture, port, RelayLimits::default());
    let mut client = TcpStream::connect(relay.local_addr()).unwrap();
    client
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    client.write_all(&request).unwrap();
    client.shutdown(Shutdown::Write).unwrap();
    sent_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    let first_frame = read_exact_prefix(&mut client, delta.len());
    let _ = release_tx.send(());
    let trailing = read_to_end_timeout(&mut client, Duration::from_secs(2));
    let captured = request_rx
        .recv_timeout(Duration::from_secs(2))
        .unwrap_or_default();
    let backend_joined = join_bounded(backend_thread, Duration::from_secs(3)).is_some();
    let shutdown = relay.shutdown(Duration::from_secs(1));
    fixture.cleanup();
    assert_eq!(first_frame, delta);
    assert_eq!(trailing, terminal);
    assert_eq!(captured, expected_request);
    assert!(backend_joined && shutdown.complete);
}

#[test]
fn partial_sse_disconnect_is_not_replayed_to_another_backend_connection() {
    let fixture = FixtureRoot::new();
    let backend = TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0)).unwrap();
    let port = backend.local_addr().unwrap().port();
    backend.set_nonblocking(true).unwrap();
    let request =
        b"POST /v1/messages HTTP/1.1\r\nx-api-key: SENTINEL\r\n\r\n{\"stream\":true}".to_vec();
    let partial = b"event: content_block_delta\ndata: {\"type\":\"content_block_delta\",".to_vec();
    let partial_for_server = partial.clone();
    let (captured_tx, captured_rx) = std::sync::mpsc::channel();
    let (reconnect_tx, reconnect_rx) = std::sync::mpsc::channel();
    let (accepted_tx, accepted_rx) = std::sync::mpsc::channel();
    let backend_thread = thread::spawn(move || {
        let Some((mut stream, _)) = accept_bounded(&backend, Duration::from_secs(2)) else {
            let _ = captured_tx.send(Vec::new());
            let _ = reconnect_tx.send(false);
            return;
        };
        let _ = accepted_tx.send(());
        let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
        let mut captured = Vec::new();
        let _ = stream.read_to_end(&mut captured);
        let _ = captured_tx.send(captured);
        let _ = stream.write_all(&partial_for_server);
        drop(stream);
        let _ = backend.set_nonblocking(true);
        thread::sleep(Duration::from_millis(120));
        let accepted_again = backend.accept().is_ok();
        let _ = reconnect_tx.send(accepted_again);
    });
    let stable = StableIngress::open_or_create(&fixture.path).unwrap();
    let mut relay = IngressRelay::new(stable, RelayLimits::default()).unwrap();
    relay
        .set_backend(test_target_waiting_for_accept(port, accepted_rx))
        .unwrap();
    let mut client = TcpStream::connect(relay.local_addr()).unwrap();
    client
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    client.write_all(&request).unwrap();
    client.shutdown(Shutdown::Write).unwrap();
    let received = read_to_end_timeout(&mut client, Duration::from_secs(2));
    let captured = captured_rx
        .recv_timeout(Duration::from_secs(2))
        .unwrap_or_default();
    let backend_reconnected = reconnect_rx
        .recv_timeout(Duration::from_secs(2))
        .unwrap_or(true);
    let relay_failure = relay.last_fixed_failure();
    let backend_joined = join_bounded(backend_thread, Duration::from_secs(3)).is_some();
    let shutdown = relay.shutdown(Duration::from_secs(1));
    fixture.cleanup();
    assert!(
        received == partial,
        "partial response changed or was absent; relay_failure={relay_failure:?}"
    );
    assert_eq!(captured, request);
    assert!(
        !backend_reconnected,
        "relay opened a second backend connection"
    );
    assert!(backend_joined && shutdown.complete);
}

#[test]
fn delayed_client_reader_receives_large_response_without_truncation() {
    let fixture = FixtureRoot::new();
    let backend = TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0)).unwrap();
    let port = backend.local_addr().unwrap().port();
    backend.set_nonblocking(true).unwrap();
    let payload: Vec<u8> = (0..4 * 1024 * 1024)
        .map(|index| ((index * 31 + 7) % 251) as u8)
        .collect();
    let request = b"GET /fixture HTTP/1.1\r\n\r\n".to_vec();
    let request_for_backend = request.clone();
    let backend_payload = payload.clone();
    let (accepted_tx, accepted_rx) = std::sync::mpsc::channel();
    let backend_thread = thread::spawn(move || {
        let Some((mut stream, _)) = accept_bounded(&backend, Duration::from_secs(2)) else {
            return false;
        };
        let _ = accepted_tx.send(());
        let _ = stream.set_write_timeout(Some(Duration::from_secs(5)));
        let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
        let mut received_request = vec![0; request_for_backend.len()];
        if stream.read_exact(&mut received_request).is_err()
            || received_request != request_for_backend
        {
            return false;
        }
        let sent = stream.write_all(&backend_payload).is_ok();
        let _ = stream.shutdown(Shutdown::Write);
        sent
    });
    let limits = RelayLimits {
        max_connections: 2,
        buffer_bytes: 1024,
        connect_timeout: Duration::from_secs(1),
        idle_timeout: Duration::from_secs(10),
        lifetime: Duration::from_secs(20),
        cancellation_grace: Duration::from_secs(1),
    };
    let stable = StableIngress::open_or_create(&fixture.path).unwrap();
    let mut relay = IngressRelay::new(stable, limits).unwrap();
    relay
        .set_backend(test_target_waiting_for_accept(port, accepted_rx))
        .unwrap();
    let (mut client, effective_receive_buffer) =
        connect_with_receive_buffer(relay.local_addr(), 2048);
    client
        .set_read_timeout(Some(Duration::from_secs(15)))
        .unwrap();
    assert!(effective_receive_buffer > 0);
    assert!(effective_receive_buffer < payload.len());
    client.write_all(&request).unwrap();
    client.shutdown(Shutdown::Write).unwrap();
    thread::sleep(Duration::from_millis(180));
    let mut received = Vec::new();
    let read_ok = client.read_to_end(&mut received).is_ok();
    let backend_sent = join_bounded(backend_thread, Duration::from_secs(7)).unwrap_or(false);
    let relay_failure = relay.last_fixed_failure();
    let shutdown = relay.shutdown(Duration::from_secs(1));
    fixture.cleanup();
    assert!(
        backend_sent,
        "backend fixture did not complete the response"
    );
    assert!(
        read_ok && received == payload,
        "delayed-reader response incomplete: received_bytes={}, read_ok={read_ok}, backend_sent={backend_sent}, relay_failure={relay_failure:?}",
        received.len()
    );
    assert!(shutdown.complete);
}

fn connect_with_receive_buffer(address: SocketAddr, requested: i32) -> (TcpStream, usize) {
    use std::os::windows::io::{FromRawSocket, RawSocket};
    use windows_sys::Win32::Networking::WinSock::{
        connect, getsockopt, setsockopt, WSASocketW, AF_INET, INVALID_SOCKET, IPPROTO_TCP,
        SOCKADDR, SOCKADDR_IN, SOCKET, SOCK_STREAM, SOL_SOCKET, SO_RCVBUF,
        WSA_FLAG_NO_HANDLE_INHERIT, WSA_FLAG_OVERLAPPED,
    };
    struct TestSocket(SOCKET);
    impl Drop for TestSocket {
        fn drop(&mut self) {
            use windows_sys::Win32::Networking::WinSock::closesocket;
            // SAFETY: this helper uniquely owns its socket until conversion.
            let _ = unsafe { closesocket(self.0) };
        }
    }
    let raw = unsafe {
        WSASocketW(
            AF_INET as i32,
            SOCK_STREAM,
            IPPROTO_TCP,
            std::ptr::null(),
            0,
            WSA_FLAG_NO_HANDLE_INHERIT | WSA_FLAG_OVERLAPPED,
        )
    };
    assert_ne!(raw, INVALID_SOCKET, "fixture socket creation failed");
    let socket = TestSocket(raw);
    // Configure before connect so Windows applies this receive window to the
    // established loopback connection, before the backend starts its response.
    let set = unsafe {
        setsockopt(
            socket.0,
            SOL_SOCKET,
            SO_RCVBUF,
            (&requested as *const i32).cast(),
            std::mem::size_of::<i32>() as i32,
        )
    };
    assert_eq!(set, 0, "could not bound the client receive window");
    let mut effective = 0i32;
    let mut length = std::mem::size_of::<i32>() as i32;
    let read = unsafe {
        getsockopt(
            socket.0,
            SOL_SOCKET,
            SO_RCVBUF,
            (&mut effective as *mut i32).cast(),
            &mut length,
        )
    };
    assert_eq!(read, 0, "could not inspect the bounded receive window");
    assert_eq!(length as usize, std::mem::size_of::<i32>());
    let effective_receive_buffer = usize::try_from(effective).unwrap_or(0);
    let SocketAddr::V4(address) = address else {
        panic!("fixture only supports IPv4 loopback");
    };
    let mut native_address = SOCKADDR_IN::default();
    native_address.sin_family = AF_INET;
    native_address.sin_port = address.port().to_be();
    native_address.sin_addr.S_un.S_addr = u32::from_ne_bytes(address.ip().octets());
    let connected = unsafe {
        connect(
            socket.0,
            (&native_address as *const SOCKADDR_IN).cast::<SOCKADDR>(),
            std::mem::size_of::<SOCKADDR_IN>() as i32,
        )
    };
    assert_eq!(connected, 0, "fixture loopback connect failed");
    let raw = socket.0;
    std::mem::forget(socket);
    // SAFETY: raw is uniquely owned and converted to TcpStream exactly once.
    let stream = unsafe { TcpStream::from_raw_socket(raw as RawSocket) };
    (stream, effective_receive_buffer)
}

fn read_exact_prefix(stream: &mut TcpStream, length: usize) -> Vec<u8> {
    let mut bytes = vec![0; length];
    if stream.read_exact(&mut bytes).is_err() {
        return Vec::new();
    }
    bytes
}

#[test]
fn no_route_closes_connection_without_forwarding_payload() {
    let fixture = FixtureRoot::new();
    let stable = StableIngress::open_or_create(&fixture.path).unwrap();
    let mut relay = IngressRelay::new(stable, RelayLimits::default()).unwrap();
    let mut client = TcpStream::connect(relay.local_addr()).unwrap();
    client.write_all(b"SENTINEL-NO-ROUTE").unwrap();
    client.shutdown(Shutdown::Write).unwrap();
    let response = read_to_end_timeout(&mut client, Duration::from_secs(2));
    let shutdown = relay.shutdown(Duration::from_secs(1));
    fixture.cleanup();
    assert!(response.is_empty());
    assert_eq!(shutdown.active_connection_count, 0);
    assert!(shutdown.complete);
}

#[test]
fn connection_limit_and_drain_cancel_are_bounded_and_do_not_reopen_route() {
    let fixture = FixtureRoot::new();
    let backend = TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0)).unwrap();
    let port = backend.local_addr().unwrap().port();
    backend.set_nonblocking(true).unwrap();
    let (accepted_tx, accepted_rx) = std::sync::mpsc::channel::<()>();
    let backend_thread = thread::spawn(move || {
        let Some((mut stream, _)) = accept_bounded(&backend, Duration::from_secs(2)) else {
            let _ = accepted_tx.send(());
            return;
        };
        let _ = accepted_tx.send(());
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let mut request = Vec::new();
        let _ = stream.read_to_end(&mut request);
    });
    let limits = RelayLimits {
        max_connections: 1,
        buffer_bytes: 1024,
        connect_timeout: Duration::from_secs(1),
        idle_timeout: Duration::from_secs(5),
        lifetime: Duration::from_secs(5),
        cancellation_grace: Duration::from_secs(1),
    };
    let mut relay = relay_with_target(&fixture, port, limits);
    let first = TcpStream::connect(relay.local_addr()).unwrap();
    assert!(wait_for_active(&relay, 1));
    accepted_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    let mut capped = TcpStream::connect(relay.local_addr()).unwrap();
    let capped_response = read_to_end_timeout(&mut capped, Duration::from_secs(2));
    relay.begin_drain().unwrap();
    let mut during_drain = TcpStream::connect(relay.local_addr()).unwrap();
    let drain_response = read_to_end_timeout(&mut during_drain, Duration::from_secs(2));
    let report = relay.finish_drain(Duration::from_millis(40));
    let backend_joined = join_bounded(backend_thread, Duration::from_secs(3)).is_some();
    let shutdown = relay.shutdown(Duration::from_secs(1));
    fixture.cleanup();
    assert!(capped_response.is_empty() && drain_response.is_empty());
    assert_eq!(report.active_connection_count, 0);
    assert!(report.complete);
    assert_eq!(report.cancellations_requested, 1);
    assert!(!report.graceful, "forced cancellation is not graceful");
    assert!(backend_joined && shutdown.complete);
    let _ = first.shutdown(Shutdown::Both);
}

#[test]
fn idle_connection_expires_without_replay() {
    let fixture = FixtureRoot::new();
    let backend = TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0)).unwrap();
    let port = backend.local_addr().unwrap().port();
    backend.set_nonblocking(true).unwrap();
    let (accepted_tx, accepted_rx) = std::sync::mpsc::channel();
    let backend_thread = thread::spawn(move || {
        let Some((mut stream, _)) = accept_bounded(&backend, Duration::from_secs(2)) else {
            let _ = accepted_tx.send(());
            return Vec::new();
        };
        let _ = accepted_tx.send(());
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let mut request = Vec::new();
        let _ = stream.read_to_end(&mut request);
        request
    });
    let limits = RelayLimits {
        max_connections: 2,
        buffer_bytes: 512,
        connect_timeout: Duration::from_secs(1),
        idle_timeout: Duration::from_millis(120),
        lifetime: Duration::from_secs(2),
        cancellation_grace: Duration::from_secs(1),
    };
    let mut relay = relay_with_target(&fixture, port, limits);
    let mut client = TcpStream::connect(relay.local_addr()).unwrap();
    client.write_all(b"once").unwrap();
    accepted_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    let _ = read_to_end_timeout(&mut client, Duration::from_secs(2));
    let captured = join_bounded(backend_thread, Duration::from_secs(3)).unwrap_or_default();
    let shutdown = relay.shutdown(Duration::from_secs(1));
    fixture.cleanup();
    assert_eq!(captured, b"once");
    assert_eq!(shutdown.active_connection_count, 0);
    assert!(shutdown.complete);
}

#[test]
fn lifetime_deadline_closes_connection_despite_recent_activity() {
    let fixture = FixtureRoot::new();
    let backend = TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0)).unwrap();
    let port = backend.local_addr().unwrap().port();
    backend.set_nonblocking(true).unwrap();
    let (captured_tx, captured_rx) = std::sync::mpsc::channel();
    let backend_thread = thread::spawn(move || {
        let Some((mut stream, _)) = accept_bounded(&backend, Duration::from_secs(2)) else {
            let _ = captured_tx.send(Vec::new());
            return;
        };
        let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
        let mut captured = Vec::new();
        loop {
            let mut byte = [0];
            match stream.read(&mut byte) {
                Ok(0) | Err(_) => break,
                Ok(_) => {
                    captured.push(byte[0]);
                    if stream.write_all(&byte).is_err() {
                        break;
                    }
                }
            }
        }
        let _ = captured_tx.send(captured);
    });
    let limits = RelayLimits {
        max_connections: 2,
        buffer_bytes: 512,
        connect_timeout: Duration::from_secs(1),
        idle_timeout: Duration::from_secs(1),
        lifetime: Duration::from_millis(220),
        cancellation_grace: Duration::from_secs(1),
    };
    let mut relay = relay_with_target(&fixture, port, limits);
    let mut client = TcpStream::connect(relay.local_addr()).unwrap();
    client
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    let mut sent = Vec::new();
    let started = Instant::now();
    while started.elapsed() < Duration::from_millis(340) {
        if client.write_all(b"x").is_err() {
            break;
        }
        let mut ack = [0];
        if client.read_exact(&mut ack).is_err() {
            break;
        }
        sent.push(b'x');
        thread::sleep(Duration::from_millis(35));
    }
    let _ = client.shutdown(Shutdown::Write);
    let captured = captured_rx
        .recv_timeout(Duration::from_secs(2))
        .unwrap_or_default();
    let backend_joined = join_bounded(backend_thread, Duration::from_secs(3)).is_some();
    let failure = relay.last_fixed_failure();
    let shutdown = relay.shutdown(Duration::from_secs(1));
    fixture.cleanup();
    assert!(!sent.is_empty());
    assert_eq!(captured, sent);
    assert_eq!(failure, Some(super::RelayError::LifetimeTimeout));
    assert!(backend_joined && shutdown.complete);
}

#[test]
fn foreign_process_listener_receives_zero_client_bytes_and_is_not_terminated() {
    const CHILD_MODE: &str = "HEADROOM_INGRESS_FOREIGN_FIXTURE";
    let fixture = FixtureRoot::new();
    let executable = std::env::current_exe().unwrap();
    let mut child = Command::new(executable)
        .arg("--exact")
        .arg("runtime::ingress_relay::tests::foreign_listener_child_fixture")
        .arg("--nocapture")
        .env(CHILD_MODE, "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let child_pid = child.id();
    let stdout = child.stdout.take().unwrap();
    let (line_tx, line_rx) = std::sync::mpsc::channel();
    let stdout_reader = thread::spawn(move || {
        for line in std::io::BufReader::new(stdout).lines() {
            let Ok(line) = line else { break };
            if line.starts_with("PORT=") || line.starts_with("BYTES=") || line == "DONE" {
                let _ = line_tx.send(line);
            }
        }
    });
    let child_stdin = child.stdin.take().unwrap();
    let child_state = Arc::new(Mutex::new(child));
    let port_line = line_rx
        .recv_timeout(Duration::from_secs(2))
        .unwrap_or_default();
    let port = port_line
        .strip_prefix("PORT=")
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(0);

    let stable = StableIngress::open_or_create(&fixture.path).unwrap();
    let mut relay = IngressRelay::new(stable, RelayLimits::default()).unwrap();
    let child_liveness = Arc::clone(&child_state);
    if port != 0 {
        relay
            .set_backend(
                BackendTarget::new(
                    port,
                    uuid::Uuid::new_v4(),
                    std::process::id(),
                    Arc::new(move || {
                        child_liveness
                            .lock()
                            .map(|mut child| {
                                child
                                    .try_wait()
                                    .map(|status| status.is_none())
                                    .unwrap_or(false)
                            })
                            .unwrap_or(false)
                    }),
                )
                .unwrap(),
            )
            .unwrap();
    }
    let mut client = TcpStream::connect(relay.local_addr()).unwrap();
    let _ = client.write_all(b"CLIENT-SENTINEL-MUST-NOT-REACH-FOREIGN-PID");
    let _ = client.shutdown(Shutdown::Write);
    let response = read_to_end_timeout(&mut client, Duration::from_secs(2));
    let bytes_line = line_rx
        .recv_timeout(Duration::from_secs(2))
        .unwrap_or_default();
    let child_bytes = bytes_line
        .strip_prefix("BYTES=")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(usize::MAX);
    let child_still_alive = child_state
        .lock()
        .map(|mut child| {
            child
                .try_wait()
                .map(|status| status.is_none())
                .unwrap_or(false)
        })
        .unwrap_or(false);

    let mut child_stdin = child_stdin;
    let _ = child_stdin.write_all(b"stop\n");
    let _ = line_rx.recv_timeout(Duration::from_secs(2));
    let child_exit = child_state
        .lock()
        .map(|mut child| wait_child_bounded(&mut child, Duration::from_secs(2)))
        .unwrap_or(false);
    let reader_joined = join_bounded(stdout_reader, Duration::from_secs(2)).is_some();
    let shutdown = relay.shutdown(Duration::from_secs(1));
    let relay_failure = relay.last_fixed_failure();
    fixture.cleanup();

    assert!(port_line.starts_with("PORT="));
    assert_ne!(child_pid, std::process::id());
    assert!(response.is_empty());
    assert_eq!(child_bytes, 0);
    assert_eq!(relay_failure, Some(super::RelayError::PeerOwnerMismatch));
    assert!(child_still_alive);
    assert!(child_exit && reader_joined && shutdown.complete);
}

#[test]
fn foreign_listener_child_fixture() {
    if std::env::var_os("HEADROOM_INGRESS_FOREIGN_FIXTURE").is_none() {
        return;
    }
    let listener = TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0)).unwrap();
    println!("PORT={}", listener.local_addr().unwrap().port());
    let _ = std::io::stdout().flush();
    let (mut stream, _) = listener.accept().unwrap();
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
    let mut payload = [0_u8; 256];
    let received = stream.read(&mut payload).unwrap_or(0);
    println!("BYTES={received}");
    let _ = std::io::stdout().flush();
    let mut input = String::new();
    let _ = std::io::stdin().read_line(&mut input);
    println!("DONE");
    let _ = std::io::stdout().flush();
}

#[test]
fn shutdown_clamps_extreme_timeout_before_instant_arithmetic() {
    let fixture = FixtureRoot::new();
    let stable = StableIngress::open_or_create(&fixture.path).unwrap();
    let mut relay = IngressRelay::new(stable, RelayLimits::default()).unwrap();
    let report = relay.shutdown(Duration::MAX);
    fixture.cleanup();
    assert!(report.complete);
    assert!(report.listener_released);
    assert!(report.graceful);
}
