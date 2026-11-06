//! H029 regression and opt-in pinned-engine acceptance, using isolated roots only.
use super::*;
use crate::runtime::manager::RuntimeManager;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

fn config_fixture(kind: &str) -> (&'static str, Vec<u8>) {
    match kind {
        "codex" => (
            "codex/config.toml",
            b"model = \"fixture-before\"\n".to_vec(),
        ),
        "claude_code" => (
            "claude/settings.json",
            br#"{"env":{"KEEP":"fixture-before"},"model":"fixture-model"}
"#
            .to_vec(),
        ),
        "opencode_v1" => (
            "xdg/opencode/opencode.jsonc",
            String::from_utf8(opencode_three_route_fixture().to_vec())
                .unwrap()
                .replace("wire-a", "fixture-before")
                .into_bytes(),
        ),
        _ => unreachable!(),
    }
}

#[test]
fn disconnect_all_adapters_preserve_later_edits_and_other_clients_after_reopen() {
    for kind in ["codex", "claude_code", "opencode_v1"] {
        let fixture = Fixture::new();
        let (relative, original) = config_fixture(kind);
        let config = fixture.path(relative);
        write_file(&config, &original);
        let other = fixture.path("other-client/settings.json");
        write_file(&other, b"{\"endpoint\":\"https://untouched.invalid\"}\n");
        let other_before = fs::read(&other).unwrap();
        let (plans, installation, scan) = manager(&fixture, kind, config.clone());
        let owned = apply(&plans, &preview(&plans, installation, scan))
            .ownership
            .unwrap();
        let edited = String::from_utf8(fs::read(&config).unwrap())
            .unwrap()
            .replace("fixture-before", "fixture-after");
        assert!(edited.contains("fixture-after"));
        fs::write(&config, edited).unwrap();
        drop(plans);
        let (reopened, _, _) = manager(&fixture, kind, config.clone());
        let released = success(undo(&reopened, kind, &owned));
        assert_eq!(released.status, "released");
        assert_eq!(
            fs::read(&config).unwrap(),
            String::from_utf8(original)
                .unwrap()
                .replace("fixture-before", "fixture-after")
                .as_bytes()
        );
        assert_eq!(fs::read(&other).unwrap(), other_before);
        assert_eq!(released.ownership.unwrap().revision, owned.revision + 1);
        assert_eq!(error_code(undo(&reopened, kind, &owned)), "config_conflict");
    }
}

#[test]
fn disconnect_all_adapters_refuse_changed_owned_endpoint_without_writing() {
    for kind in ["codex", "claude_code", "opencode_v1"] {
        let fixture = Fixture::new();
        let (relative, original) = config_fixture(kind);
        let config = fixture.path(relative);
        write_file(&config, &original);
        let (plans, installation, scan) = manager(&fixture, kind, config.clone());
        let owned = apply(&plans, &preview(&plans, installation, scan))
            .ownership
            .unwrap();
        let edited = String::from_utf8(fs::read(&config).unwrap())
            .unwrap()
            .replace("http://127.0.0.1:41731", "https://user.example.invalid");
        assert!(edited.contains("https://user.example.invalid"));
        fs::write(&config, edited.as_bytes()).unwrap();
        let state = fixture.path("state/ownership").join(format!("{kind}.json"));
        let state_before = fs::read(&state).unwrap();
        assert_eq!(error_code(undo(&plans, kind, &owned)), "config_conflict");
        assert_eq!(fs::read(&config).unwrap(), edited.as_bytes());
        assert_eq!(fs::read(&state).unwrap(), state_before);
    }
}

struct Gate {
    case: String,
    release: Sender<bool>,
}

struct Upstream {
    port: u16,
    next: Receiver<Gate>,
    calls: Arc<AtomicUsize>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Upstream {
    fn new() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let port = listener.local_addr().unwrap().port();
        let (entered, next) = mpsc::channel();
        let calls = Arc::new(AtomicUsize::new(0));
        let stop = Arc::new(AtomicBool::new(false));
        let count = Arc::clone(&calls);
        let stopped = Arc::clone(&stop);
        let thread = thread::spawn(move || {
            let mut workers = Vec::new();
            while !stopped.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        let entered = entered.clone();
                        let count = Arc::clone(&count);
                        workers.push(thread::spawn(move || serve_stream(stream, entered, count)));
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(5))
                    }
                    Err(_) => panic!("isolated upstream accept failed"),
                }
            }
            for worker in workers {
                worker.join().unwrap();
            }
        });
        Self {
            port,
            next,
            calls,
            stop,
            thread: Some(thread),
        }
    }
}

impl Drop for Upstream {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(worker) = self.thread.take() {
            let _ = worker.join();
        }
    }
}

fn serve_stream(mut stream: TcpStream, entered: Sender<Gate>, calls: Arc<AtomicUsize>) {
    // The accept loop is nonblocking; set the accepted socket mode explicitly
    // before using blocking HTTP reads (including on Windows).
    stream.set_nonblocking(false).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    stream
        .set_write_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut line = String::new();
    match reader.read_line(&mut line) {
        Ok(0) | Err(_) => return,
        Ok(_) => {}
    }
    let prewarm = line.trim() == "HEAD / HTTP/1.1";
    assert!(prewarm || line.trim() == "POST /v1/chat/completions HTTP/1.1");
    let mut length = None;
    let mut auth = false;
    loop {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) | Err(_) => return,
            Ok(_) => {}
        }
        assert!(line.len() < 8192);
        if line == "\r\n" {
            break;
        }
        if let Some((key, value)) = line.trim().split_once(':') {
            if key.eq_ignore_ascii_case("content-length") {
                length = Some(value.trim().parse::<usize>().unwrap());
            }
            if key.eq_ignore_ascii_case("authorization") {
                auth = value.trim() == "Bearer headroom-local-nonfunctional-test";
            }
        }
    }
    if prewarm {
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
            .unwrap();
        return;
    }
    let length = length.expect("bounded request length");
    assert!(length < 65536 && auth);
    let mut body = vec![0; length];
    reader.read_exact(&mut body).unwrap();
    let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(body["model"], "o4-mini");
    assert_eq!(body["stream"], true);
    let case = body["messages"][0]["content"].as_str().unwrap().to_owned();
    calls.fetch_add(1, Ordering::SeqCst);
    stream
        .write_all(
            b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n",
        )
        .unwrap();
    stream.write_all(b"data: {\"id\":\"local\",\"object\":\"chat.completion.chunk\",\"model\":\"o4-mini\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\",\"content\":\"begin\"},\"finish_reason\":null}]}\n\n").unwrap();
    stream.flush().unwrap();
    let (release, gate) = mpsc::channel();
    if entered.send(Gate { case, release }).is_ok()
        && gate.recv_timeout(Duration::from_secs(60)).unwrap_or(false)
    {
        stream.write_all(b"data: {\"id\":\"local\",\"object\":\"chat.completion.chunk\",\"model\":\"o4-mini\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"end\"},\"finish_reason\":\"stop\"}],\"usage\":{\"prompt_tokens\":4,\"completion_tokens\":2,\"total_tokens\":6}}\n\ndata: [DONE]\n\n").unwrap();
    }
}

fn active_request(port: u16, case: &str) -> (Receiver<()>, JoinHandle<Vec<u8>>) {
    let session = case.to_owned();
    let body = serde_json::to_vec(&serde_json::json!({"model":"o4-mini","stream":true,"messages":[{"role":"user","content":case}]})).unwrap();
    let (first, seen) = mpsc::channel();
    let handle = thread::spawn(move || {
        let mut socket = TcpStream::connect(("127.0.0.1", port)).unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(65)))
            .unwrap();
        write!(socket, "POST /v1/chat/completions HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nAuthorization: Bearer headroom-local-nonfunctional-test\r\nx-headroom-session-id: h029-{session}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len()).unwrap();
        socket.write_all(&body).unwrap();
        let mut raw = Vec::new();
        let mut buffer = [0; 4096];
        let mut first = Some(first);
        loop {
            match socket.read(&mut buffer) {
                Ok(0) => break,
                Ok(size) => {
                    raw.extend_from_slice(&buffer[..size]);
                    assert!(raw.len() < 2 * 1024 * 1024);
                    if raw.windows(5).any(|v| v == b"begin") {
                        if let Some(first) = first.take() {
                            let _ = first.send(());
                        }
                    }
                }
                Err(error)
                    if matches!(
                        error.kind(),
                        std::io::ErrorKind::ConnectionReset | std::io::ErrorKind::ConnectionAborted
                    ) =>
                {
                    break
                }
                Err(_) => panic!("bounded stream did not close"),
            }
        }
        raw
    });
    (seen, handle)
}

fn snapshot(runtime: &RuntimeManager) -> serde_json::Value {
    serde_json::to_value(runtime.get_runtime_snapshot(Uuid::new_v4().to_string())).unwrap()
}

fn receive_gate(
    upstream: &Upstream,
    runtime: &RuntimeManager,
    request: JoinHandle<Vec<u8>>,
    case: &str,
) -> (Gate, JoinHandle<Vec<u8>>) {
    let deadline = Instant::now() + Duration::from_secs(30);
    let gate = loop {
        if let Ok(gate) = upstream.next.try_recv() {
            break Ok(gate);
        }
        if request.is_finished() {
            let bytes = request.join().unwrap();
            let text = String::from_utf8_lossy(&bytes);
            assert_eq!(
                "upstream_received",
                text.lines().next().unwrap_or("closed"),
                "case={case}; ownership={}; engine={}",
                runtime.ownership_diagnostic_for_test(),
                snapshot(&runtime)["engine"]
            );
            unreachable!();
        }
        if Instant::now() >= deadline {
            break Err(());
        }
        thread::sleep(Duration::from_millis(10));
    };
    assert_eq!(
        gate.is_ok(),
        true,
        "case={case}; client_finished={}",
        request.is_finished()
    );
    let gate = gate.unwrap();
    return (gate, request);
}

struct RuntimeCleanup(RuntimeManager);
impl Drop for RuntimeCleanup {
    fn drop(&mut self) {
        let _ = self.0.shutdown();
    }
}

#[test]
#[ignore = "opt-in real pinned-engine gated SSE + config undo + graceful/timeout stop; loopback only"]
fn disconnect_live_pinned_sse_completion_timeout_no_replay_and_cleanup() {
    let fixture = Fixture::new();
    let upstream = Upstream::new();
    let state = fixture.path("runtime");
    let settings = serde_json::json!({"openai_base_url":format!("http://127.0.0.1:{}/v1",upstream.port),"anthropic_base_url":format!("http://127.0.0.1:{}",upstream.port)});
    write_file(
        &state.join("workspace/settings.json"),
        &serde_json::to_vec(&settings).unwrap(),
    );
    let runtime = RuntimeManager::isolated_for_test(state);
    let cleanup = RuntimeCleanup(runtime.clone());
    let foreign = TcpListener::bind("127.0.0.1:0").unwrap();
    let foreign_port = foreign.local_addr().unwrap().port();
    let started = serde_json::to_value(runtime.start_engine(Uuid::new_v4().to_string())).unwrap();
    assert_eq!(
        started["engine"]["status"], "ready",
        "pinned engine did not start; fixed_error={}",
        started["error"]["code"]
    );
    let generation = started["runtime_generation"].as_str().unwrap().to_owned();
    let endpoint = runtime.ready_stable_endpoint().unwrap();
    let port = endpoint.port;
    let mut owned = Vec::new();
    for kind in ["codex", "claude_code", "opencode_v1"] {
        let (relative, original) = config_fixture(kind);
        let config = fixture.path(relative);
        write_file(&config, &original);
        let roots = root_inputs(kind, &config);
        let rt = runtime.clone();
        let (plans, installation, scan) = manager_with_providers(
            &fixture,
            kind,
            move || roots.clone(),
            move || rt.ready_stable_endpoint(),
        );
        let applied = apply(&plans, &preview(&plans, installation, scan));
        owned.push((kind, config, original, plans, applied.ownership.unwrap()));
    }
    let other_before = owned
        .iter()
        .skip(1)
        .map(|v| fs::read(&v.1).unwrap())
        .collect::<Vec<_>>();
    let (first, request) = active_request(port, "disconnect-only");
    let (gate, request) = receive_gate(&upstream, &runtime, request, "disconnect-only");
    assert_eq!(gate.case, "disconnect-only");
    first.recv_timeout(Duration::from_secs(10)).unwrap();
    let codex = &owned[0];
    assert_eq!(
        success(undo(&codex.3, codex.0, &codex.4)).status,
        "released"
    );
    assert_eq!(fs::read(&codex.1).unwrap(), codex.2);
    assert_eq!(
        owned
            .iter()
            .skip(1)
            .map(|v| fs::read(&v.1).unwrap())
            .collect::<Vec<_>>(),
        other_before
    );
    assert_eq!(snapshot(&runtime)["runtime_generation"], generation);
    assert_eq!(runtime.ready_stable_endpoint().unwrap().port, port);
    gate.release.send(true).unwrap();
    let raw = request.join().unwrap();
    assert!(raw.windows(12).any(|v| v == b"data: [DONE]"));
    assert_eq!(upstream.calls.load(Ordering::SeqCst), 1);
    for (case, expected) in [
        ("graceful-stop", "stopped"),
        ("timeout-stop", "cleanup_failed"),
    ] {
        if case == "timeout-stop" {
            let value =
                serde_json::to_value(runtime.start_engine(Uuid::new_v4().to_string())).unwrap();
            assert_eq!(value["engine"]["status"], "ready");
            assert_ne!(value["runtime_generation"], generation);
        }
        let current = snapshot(&runtime)["runtime_generation"]
            .as_str()
            .unwrap()
            .to_owned();
        let backend_port = runtime.listener_port_for_test().unwrap();
        let (first, request) = active_request(port, case);
        let (gate, request) = receive_gate(&upstream, &runtime, request, case);
        assert_eq!(gate.case, case);
        first.recv_timeout(Duration::from_secs(10)).unwrap();
        let rt = runtime.clone();
        let began = Instant::now();
        let stop = thread::spawn(move || {
            serde_json::to_value(rt.stop_engine(Uuid::new_v4().to_string(), current)).unwrap()
        });
        while snapshot(&runtime)["engine"]["status"] != "stopping"
            && began.elapsed() < Duration::from_secs(2)
        {
            thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(snapshot(&runtime)["engine"]["status"], "stopping");
        // The public drain boundary is the stable ingress, not the private
        // engine listener (which stays owned until graceful shutdown finishes).
        let mut rejected = TcpStream::connect(("127.0.0.1", port)).unwrap();
        rejected
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let _ = rejected
            .write_all(b"GET /readyz HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n");
        let mut byte = [0];
        match rejected.read(&mut byte) {
            Ok(0) => {}
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::ConnectionReset
                        | std::io::ErrorKind::ConnectionAborted
                        | std::io::ErrorKind::BrokenPipe
                ) => {}
            _ => panic!("stable ingress accepted a new request during stop"),
        }
        assert!(began.elapsed() < Duration::from_secs(2));
        assert!(
            !request.is_finished(),
            "active SSE closed before the gate was released"
        );
        if case == "graceful-stop" {
            gate.release.send(true).unwrap();
        }
        let stopped = stop.join().unwrap();
        assert!(began.elapsed() < Duration::from_secs(60));
        let raw = request.join().unwrap();
        if case == "graceful-stop" {
            assert_eq!(stopped["engine"]["status"], expected);
            assert!(raw.windows(12).any(|v| v == b"data: [DONE]"));
        } else {
            assert_eq!(stopped["error"]["code"], expected);
            assert!(!raw.windows(12).any(|v| v == b"data: [DONE]"));
            gate.release.send(false).unwrap();
        }
        assert!(TcpStream::connect(("127.0.0.1", backend_port)).is_err());
        let ingress =
            serde_json::to_value(runtime.get_ingress_snapshot(Uuid::new_v4().to_string())).unwrap();
        assert_eq!(ingress["active_connection_count"], 0);
        assert!(TcpStream::connect(("127.0.0.1", foreign_port)).is_ok());
    }
    assert_eq!(
        upstream.calls.load(Ordering::SeqCst),
        3,
        "each POST must reach upstream exactly once"
    );
    assert!(runtime.shutdown());
    assert!(TcpStream::connect(("127.0.0.1", port)).is_err());
    assert!(TcpStream::connect(("127.0.0.1", foreign_port)).is_ok());
    drop(cleanup);
}
