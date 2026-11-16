//! Explicit local fixture acceptance; never contacts a model provider.
use super::*;
use crate::state::store::{OperatingPreferences, PreferenceStore, PreferencesReply};
use std::io::{BufRead, BufReader};
use std::net::TcpListener;
use std::sync::atomic::AtomicUsize;

struct OwnedRuntime(RuntimeManager);
impl Drop for OwnedRuntime {
    fn drop(&mut self) {
        let _ = self.0.shutdown();
    }
}

struct Upstream {
    port: u16,
    bodies: mpsc::Receiver<Value>,
    calls: Arc<AtomicUsize>,
    stop: Arc<AtomicBool>,
    worker: Option<thread::JoinHandle<()>>,
}
impl Upstream {
    fn new() -> Self {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        listener.set_nonblocking(true).unwrap();
        let port = listener.local_addr().unwrap().port();
        let (tx, bodies) = mpsc::channel();
        let calls = Arc::new(AtomicUsize::new(0));
        let count = calls.clone();
        let stop = Arc::new(AtomicBool::new(false));
        let halted = stop.clone();
        let worker = thread::spawn(move || {
            let mut workers = Vec::new();
            while !halted.load(Ordering::Acquire) {
                match listener.accept() {
                    Ok((socket, _)) => {
                        let tx = tx.clone();
                        let count = count.clone();
                        workers.push(thread::spawn(move || serve(socket, tx, count)));
                    }
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(5))
                    }
                    Err(_) => panic!("local settings upstream accept"),
                }
            }
            for worker in workers {
                worker.join().unwrap();
            }
        });
        Self {
            port,
            bodies,
            calls,
            stop,
            worker: Some(worker),
        }
    }
}
impl Drop for Upstream {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(w) = self.worker.take() {
            let _ = w.join();
        }
    }
}
fn serve(mut socket: TcpStream, tx: mpsc::Sender<Value>, calls: Arc<AtomicUsize>) {
    socket.set_nonblocking(false).unwrap();
    socket
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    let mut reader = BufReader::new(socket.try_clone().unwrap());
    let mut first = String::new();
    if !matches!(reader.read_line(&mut first),Ok(n) if n>0) {
        return;
    }
    let mut length = 0;
    let mut auth = false;
    loop {
        let mut line = String::new();
        if !matches!(reader.read_line(&mut line),Ok(n) if n>0) {
            return;
        }
        if line == "\r\n" {
            break;
        }
        if let Some((key, value)) = line.trim().split_once(':') {
            if key.eq_ignore_ascii_case("content-length") {
                length = value.trim().parse::<usize>().unwrap();
            }
            if key.eq_ignore_ascii_case("authorization") {
                auth = value.trim() == "Bearer headroom-local-nonfunctional-test";
            }
        }
    }
    if first.starts_with("HEAD ") {
        socket
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
            .unwrap();
        return;
    }
    assert_eq!(first.trim(), "POST /v1/responses HTTP/1.1");
    assert!(auth && length < 2 * 1024 * 1024);
    let mut raw = vec![0; length];
    reader.read_exact(&mut raw).unwrap();
    let body: Value = serde_json::from_slice(&raw).unwrap();
    calls.fetch_add(1, Ordering::SeqCst);
    tx.send(body).unwrap();
    let answer=serde_json::to_vec(&json!({"id":"local-h031","object":"response","status":"completed","model":"o4-mini","output":[{"type":"message","role":"assistant","content":[{"type":"output_text","text":"local fixture"}]}],"usage":{"input_tokens":100,"output_tokens":2,"total_tokens":102}})).unwrap();
    write!(socket,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",answer.len()).unwrap();
    socket.write_all(&answer).unwrap();
}
fn request(port: u16, tool: &str, content: &str) {
    let body=serde_json::to_vec(&json!({"model":"o4-mini","reasoning":{"effort":"high"},"stream":false,"input":[
        {"type":"function_call","call_id":"h031-read","name":tool,"arguments":"{\"format\":\"exact\"}"},
        {"type":"function_call_output","call_id":"h031-read","output":content},
        {"type":"message","role":"user","content":[{"type":"input_text","text":"Use the exact numbers."}]}
    ]})).unwrap();
    let mut socket = TcpStream::connect(("127.0.0.1", port)).unwrap();
    socket
        .set_read_timeout(Some(Duration::from_secs(40)))
        .unwrap();
    write!(socket,"POST /v1/responses HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nAuthorization: Bearer headroom-local-nonfunctional-test\r\nx-headroom-session-id: h031-{tool}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",body.len()).unwrap();
    socket.write_all(&body).unwrap();
    let mut result = Vec::new();
    socket.read_to_end(&mut result).unwrap();
    assert!(
        result.starts_with(b"HTTP/1.1 200"),
        "local protection request must reach fixture"
    );
}
fn save(prefs: &PreferenceStore, tools: Vec<String>) {
    let PreferencesReply::Snapshot(current) = prefs.get_preferences(Uuid::new_v4().to_string())
    else {
        panic!("settings fixture prefs unavailable")
    };
    assert!(matches!(
        prefs.save_with_operating(
            Uuid::new_v4().to_string(),
            "tr".into(),
            "system".into(),
            Some(OperatingPreferences {
                protected_tools: tools,
                ..Default::default()
            }),
            None,
            None,
            current.revision,
            current.state_fingerprint
        ),
        PreferencesReply::Snapshot(_)
    ));
}

#[test]
#[ignore = "opt-in pinned motor + protected tool results; local upstream only, no provider request"]
fn operating_live_tools_apply_at_manual_restart_and_preserve_exact_numeric_results() {
    let project =
        fs::canonicalize(PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap()).unwrap();
    let relative = PathBuf::from("work/proofs/settings/live-runs").join(Uuid::new_v4().to_string());
    let root = project.join(&relative);
    fs::create_dir_all(root.join("runtime/workspace")).unwrap();
    let upstream = Upstream::new();
    fs::write(root.join("runtime/workspace/settings.json"),serde_json::to_vec(&json!({"openai_base_url":format!("http://127.0.0.1:{}/v1",upstream.port),"anthropic_base_url":format!("http://127.0.0.1:{}",upstream.port)})).unwrap()).unwrap();
    let prefs = PreferenceStore::for_test(project.clone(), relative.join("preferences")).unwrap();
    let first_tool = "headroom_h031_exact_a";
    let second_tool = "headroom_h031_exact_b";
    save(&prefs, vec![first_tool.into()]);
    let mut layout = RuntimeLayout::from_roots(
        project.clone(),
        project.join("runtime"),
        root.join("runtime"),
    )
    .unwrap();
    layout.preferences = Some(prefs.clone());
    let guard = OwnedRuntime(RuntimeManager::from_layout(Ok(layout)));
    let rt = &guard.0;
    let foreign = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let foreign_port = foreign.local_addr().unwrap().port();
    let mut checks = Vec::new();
    let mut generations = Vec::new();
    let mut endpoints = Vec::new();
    for tool in [first_tool, second_tool] {
        let start = serde_json::to_value(rt.start_engine(Uuid::new_v4().to_string())).unwrap();
        assert_eq!(
            start["engine"]["status"], "ready",
            "fixed_error={}",
            start["error"]["code"]
        );
        let generation = start["runtime_generation"].as_str().unwrap().to_owned();
        generations.push(generation.clone());
        let effective = rt.settings_observation();
        assert_eq!(effective["status"], "verified");
        assert_eq!(effective["protected_tools"], json!([tool]));
        assert_eq!(effective["local_stats_enabled"], true);
        assert_eq!(effective["beacon_enabled"], false);
        assert_eq!(effective["ccr_ttl_seconds"], 7200);
        let endpoint = rt.ready_stable_endpoint().unwrap();
        endpoints.push(endpoint.port);
        let content=(0..600).map(|i|format!("row={i:06} account=ACCT-000042 ratio=0.000100 delta=-0.00 precise=9007199254740993123456789012345\n")).collect::<String>();
        request(endpoint.port, tool, &content);
        let captured = upstream
            .bodies
            .recv_timeout(Duration::from_secs(5))
            .unwrap();
        assert_eq!(captured["model"], "o4-mini");
        assert_eq!(captured["reasoning"]["effort"], "high");
        let outputs = captured["input"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|v| v["type"] == "function_call_output")
            .collect::<Vec<_>>();
        assert_eq!(outputs.len(), 1);
        assert_eq!(outputs[0]["call_id"], "h031-read");
        assert_eq!(outputs[0]["output"], content);
        checks.push(json!({"tool":tool,"input_bytes":content.len(),"exact_output":true,"stats_enabled":true,"beacon_enabled":false,"generation":generation}));
        if tool == first_tool {
            save(&prefs, vec![second_tool.into()]);
            assert_eq!(
                rt.settings_observation()["protected_tools"],
                json!([first_tool])
            );
        }
        let stopped =
            serde_json::to_value(rt.stop_engine(Uuid::new_v4().to_string(), generation)).unwrap();
        assert_eq!(stopped["engine"]["status"], "stopped");
        assert_eq!(rt.settings_observation()["status"], "inactive");
    }
    assert_ne!(generations[0], generations[1]);
    assert_eq!(endpoints[0], endpoints[1]);
    assert_eq!(upstream.calls.load(Ordering::SeqCst), 2);
    assert!(rt.shutdown());
    assert!(TcpStream::connect(("127.0.0.1", endpoints[0])).is_err());
    assert!(TcpStream::connect(("127.0.0.1", foreign_port)).is_ok());
    fs::write(root.join("result.json"),serde_json::to_vec_pretty(&json!({"passed":true,"upstream_posts":2,"checks":checks,"same_endpoint":true,"new_generation":true,"user_settings_changed":false,"foreign_listener_alive":true,"provider_model_calls":0})).unwrap()).unwrap();
}
