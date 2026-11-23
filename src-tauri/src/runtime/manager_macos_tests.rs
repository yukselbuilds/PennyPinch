use super::*;
use serde_json::Value;
use std::fs;
use std::io::{Read, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::os::unix::fs::{DirBuilderExt, MetadataExt};
use std::path::PathBuf;
use std::thread;

struct ShutdownOnDrop(RuntimeManager);

impl Drop for ShutdownOnDrop {
    fn drop(&mut self) {
        let _ = self.0.shutdown();
    }
}

fn isolated_paths() -> (PathBuf, PathBuf) {
    let runtime_env = if cfg!(target_os = "linux") {
        "HEADROOM_LINUX_RUNTIME_ROOT"
    } else {
        "HEADROOM_MACOS_RUNTIME_ROOT"
    };
    let runtime = PathBuf::from(std::env::var_os(runtime_env).expect("pinned runtime path"));
    assert!(runtime.is_absolute());
    assert_eq!(
        runtime.file_name().and_then(|name| name.to_str()),
        Some("runtime")
    );
    let runtime_info = fs::symlink_metadata(&runtime).expect("pinned runtime exists");
    assert!(runtime_info.is_dir() && !runtime_info.file_type().is_symlink());
    assert_eq!(fs::canonicalize(&runtime).unwrap(), runtime);
    let resources = runtime
        .parent()
        .expect("runtime resource parent")
        .to_path_buf();

    let proof_root = PathBuf::from(
        std::env::var_os(crate::runtime::ENGINE_PROOF_ENV).expect("private engine proof path"),
    );
    assert!(proof_root.is_absolute());
    let proof_info = fs::symlink_metadata(&proof_root).expect("private proof root exists");
    assert!(proof_info.is_dir() && !proof_info.file_type().is_symlink());
    assert_eq!(proof_info.uid(), unsafe { libc::geteuid() });
    assert_eq!(proof_info.mode() & 0o7777, 0o700);
    assert_eq!(fs::canonicalize(&proof_root).unwrap(), proof_root);

    let suffix = Uuid::new_v4().simple().to_string();
    let state = proof_root.join(format!("app-data-with-a-long-user-directory-{}", suffix));
    fs::DirBuilder::new()
        .mode(0o700)
        .create(&state)
        .expect("create private runtime state root");
    let state_info = fs::symlink_metadata(&state).unwrap();
    assert_eq!(state_info.uid(), unsafe { libc::geteuid() });
    assert_eq!(state_info.mode() & 0o7777, 0o700);
    assert_eq!(fs::canonicalize(&state).unwrap(), state);
    (resources, state)
}

fn value(reply: impl Serialize) -> Value {
    serde_json::to_value(reply).expect("serialize bounded runtime reply")
}

fn wait_for_ingress(manager: &RuntimeManager) -> Value {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        let snapshot = value(manager.get_ingress_snapshot(Uuid::new_v4().to_string()));
        if snapshot["endpoint"]["status"] == "ready"
            && snapshot["endpoint"]["address"]["host"] == "127.0.0.1"
        {
            return snapshot;
        }
        assert!(
            Instant::now() < deadline,
            "stable ingress did not become ready"
        );
        thread::sleep(Duration::from_millis(50));
    }
}

fn assert_runtime_status(manager: &RuntimeManager, expected: &str) {
    let snapshot = value(manager.get_runtime_snapshot(Uuid::new_v4().to_string()));
    assert_eq!(snapshot["engine"]["status"], expected);
}

fn assert_error(reply: impl Serialize, expected: &str) {
    let reply = value(reply);
    assert_eq!(reply["status"], "error");
    assert_eq!(reply["error"]["code"], expected);
}

fn start_ready(manager: &RuntimeManager) -> Value {
    let reply = value(manager.start_engine(Uuid::new_v4().to_string()));
    assert_eq!(
        reply["engine"]["status"], "ready",
        "start_error={}",
        reply["error"]["code"]
    );
    reply
}

fn request_readyz(port: u16) {
    let address = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port);
    let mut stream = TcpStream::connect_timeout(&address, Duration::from_secs(2))
        .expect("owned loopback ingress accepts readiness probe");
    stream
        .set_write_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    stream
        .write_all(b"GET /readyz HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")
        .expect("write bounded readiness probe");
    let (status, body) = read_http_body(&mut stream, Instant::now() + Duration::from_secs(3))
        .expect("bounded readiness response through the owned relay");
    assert_eq!(status, 200);
    assert!(health_payload_matches(&body, EXPECTED_ENGINE_VERSION));
}

fn assert_no_route(port: u16) {
    let address = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port);
    let mut stream = TcpStream::connect_timeout(&address, Duration::from_secs(2))
        .expect("stable ingress remains bound after engine stop");
    stream.set_nonblocking(true).unwrap();
    let _ = stream.write_all(b"GET /readyz HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n");
    let _ = stream.shutdown(std::net::Shutdown::Write);
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        match stream.read(&mut [0u8; 1]) {
            Ok(0) => return,
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::ConnectionReset | std::io::ErrorKind::ConnectionAborted
                ) =>
            {
                return
            }
            Err(error)
                if error.kind() == std::io::ErrorKind::WouldBlock && Instant::now() < deadline =>
            {
                thread::sleep(Duration::from_millis(10));
            }
            Ok(_) => panic!("stopped backend returned a response"),
            Err(_) => panic!("bounded no-route probe did not close"),
        }
    }
}

#[test]
#[ignore = "requires a pinned native runtime and an isolated state directory"]
fn managed_app_engine_lifecycle() {
    use crate::state::store::{OperatingPreferences, PreferenceStore, PreferencesReply};
    let (resources, state) = isolated_paths();
    let preferences = PreferenceStore::new(Some(state.clone()));
    let saved = preferences.save_with_operating(
        Uuid::new_v4().to_string(),
        "en".into(),
        "dark".into(),
        Some(OperatingPreferences {
            protected_tools: vec!["exec_command".to_owned()],
            ..Default::default()
        }),
        None,
        None,
        0,
        None,
    );
    assert!(
        matches!(&saved, PreferencesReply::Snapshot(snapshot) if snapshot.revision == 1),
        "save_error={}",
        value(saved)["error"]["code"]
    );
    let manager =
        RuntimeManager::with_preferences(Some(resources.clone()), Some(state.clone()), preferences);
    let _cleanup = ShutdownOnDrop(manager.clone());

    let initial_ingress = wait_for_ingress(&manager);
    assert_eq!(initial_ingress["backend"]["status"], "stopped");
    assert_runtime_status(&manager, "stopped");

    let started = start_ready(&manager);
    assert_eq!(started["engine_version"], EXPECTED_ENGINE_VERSION);
    assert_eq!(
        manager.settings_observation()["protected_tools"],
        json!(["exec_command"])
    );
    let first_generation = started["runtime_generation"]
        .as_str()
        .expect("ready generation")
        .to_owned();
    assert!(valid_generation(&first_generation));
    assert_eq!(started["compression"]["status"], "enabled");
    assert_eq!(started["compression"]["revision"], 0);

    let ready_ingress = wait_for_ingress(&manager);
    assert_eq!(ready_ingress["backend"]["status"], "ready");
    assert_eq!(ready_ingress["backend"]["generation"], first_generation);
    let endpoint_id = ready_ingress["endpoint"]["endpoint_id"]
        .as_str()
        .expect("stable endpoint identity")
        .to_owned();
    let port = ready_ingress["endpoint"]["address"]["port"]
        .as_u64()
        .and_then(|port| u16::try_from(port).ok())
        .filter(|port| *port != 0)
        .expect("stable loopback port");
    let configured = manager
        .ready_stable_endpoint()
        .expect("verified client endpoint");
    assert_eq!(configured.endpoint_id, endpoint_id);
    assert_eq!(configured.host, "127.0.0.1");
    assert_eq!(configured.port, port);
    request_readyz(port);

    let metrics = manager.capture_metrics().expect("owned metrics snapshot");
    assert_eq!(metrics.generation, first_generation);
    assert!(!metrics.projection.aggregate_only);

    let paused = value(manager.set_optimization(
        Uuid::new_v4().to_string(),
        first_generation.clone(),
        0,
        false,
    ));
    assert_eq!(paused["compression"]["status"], "paused");
    assert_eq!(paused["compression"]["revision"], 1);
    assert_error(
        manager.set_optimization(
            Uuid::new_v4().to_string(),
            first_generation.clone(),
            0,
            true,
        ),
        "config_conflict",
    );
    let resumed = value(manager.set_optimization(
        Uuid::new_v4().to_string(),
        first_generation.clone(),
        1,
        true,
    ));
    assert_eq!(resumed["compression"]["status"], "enabled");
    assert_eq!(resumed["compression"]["revision"], 2);
    assert_eq!(resumed["runtime_generation"], first_generation);

    let wrong_generation = format!(
        "{}{}",
        if first_generation.starts_with('0') {
            '1'
        } else {
            '0'
        },
        &first_generation[1..]
    );
    assert_error(
        manager.stop_engine(Uuid::new_v4().to_string(), wrong_generation),
        "ownership_lost",
    );
    assert_runtime_status(&manager, "ready");

    let stopped = value(manager.stop_engine(Uuid::new_v4().to_string(), first_generation.clone()));
    assert_eq!(stopped["engine"]["status"], "stopped");
    let stopped_ingress = wait_for_ingress(&manager);
    assert_eq!(stopped_ingress["backend"]["status"], "stopped");
    assert_eq!(stopped_ingress["active_connection_count"], 0);
    assert_no_route(port);

    let restarted = start_ready(&manager);
    let second_generation = restarted["runtime_generation"]
        .as_str()
        .expect("restarted generation")
        .to_owned();
    assert!(valid_generation(&second_generation));
    assert_ne!(second_generation, first_generation);
    let restarted_ingress = wait_for_ingress(&manager);
    assert_eq!(restarted_ingress["endpoint"]["endpoint_id"], endpoint_id);
    assert_eq!(restarted_ingress["endpoint"]["address"]["port"], port);
    assert_eq!(
        restarted_ingress["backend"]["generation"],
        second_generation
    );
    request_readyz(port);

    assert!(
        manager.shutdown(),
        "manager shutdown must release owned processes and listener"
    );
    assert!(TcpStream::connect_timeout(
        &SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port),
        Duration::from_millis(500)
    )
    .is_err());

    let socket = socket2::Socket::new(socket2::Domain::IPV4, socket2::Type::STREAM, None).unwrap();
    socket.set_reuse_address(true).unwrap();
    socket
        .bind(&SocketAddr::from((Ipv4Addr::LOCALHOST, port)).into())
        .expect("foreign saved-port listener");
    socket.listen(1).unwrap();
    let foreign = TcpListener::from(socket);
    let conflicted = RuntimeManager::new(Some(resources.clone()), Some(state.clone()));
    let _conflict_cleanup = ShutdownOnDrop(conflicted.clone());
    assert_error(
        conflicted.start_engine(Uuid::new_v4().to_string()),
        "port_conflict",
    );
    assert!(conflicted.shutdown());
    assert!(
        TcpStream::connect_timeout(&foreign.local_addr().unwrap(), Duration::from_secs(1)).is_ok()
    );
    drop(foreign);

    let reopened = RuntimeManager::new(Some(resources), Some(state));
    let _reopened_cleanup = ShutdownOnDrop(reopened.clone());
    let reopened_ingress = wait_for_ingress(&reopened);
    assert_eq!(reopened_ingress["endpoint"]["endpoint_id"], endpoint_id);
    assert_eq!(reopened_ingress["endpoint"]["address"]["port"], port);
    assert_eq!(reopened_ingress["backend"]["status"], "stopped");
    assert_runtime_status(&reopened, "stopped");
}
