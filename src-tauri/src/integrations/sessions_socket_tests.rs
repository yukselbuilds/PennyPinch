use super::*;
use std::fs;
use std::thread;

fn fixture() -> (PathBuf, Socket) {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("work/proofs/s51")
        .join(&uuid::Uuid::new_v4().to_string()[..8]);
    fs::create_dir_all(&root).unwrap();
    let path = root.join("socket-ç");
    let listener = Socket::new(Domain::UNIX, Type::STREAM, None).unwrap();
    listener.bind(&SockAddr::unix(&path).unwrap()).unwrap();
    listener.listen(2).unwrap();
    (path, listener)
}
fn clean(path: &Path) {
    fs::remove_file(path).unwrap();
    fs::remove_dir(path.parent().unwrap()).unwrap();
}

#[test]
fn native_unix_websocket_reads_only_metadata_and_correlates_rpc_replies() {
    let (path, listener) = fixture();
    let task = thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        let mut ws = tungstenite::accept(stream).unwrap();
        let request = |ws: &mut WebSocket<Socket>| {
            serde_json::from_str::<serde_json::Value>(ws.read().unwrap().to_text().unwrap())
                .unwrap()
        };
        assert_eq!(request(&mut ws), protocol::requests()[0]);
        ws.send(Message::Text(
            r#"{"id":1,"result":{"userAgent":"fixture"}}"#.into(),
        ))
        .unwrap();
        assert_eq!(request(&mut ws), protocol::requests()[1]);
        assert_eq!(request(&mut ws), protocol::requests()[2]);
        ws.send(Message::Text(
            r#"{"method":"item/agentMessage/delta","params":{"delta":"SECRET_CANARY"}}"#.into(),
        ))
        .unwrap();
        ws.send(Message::Text(
            r#"{"id":2,"result":{"data":[],"nextCursor":null}}"#.into(),
        ))
        .unwrap();
        assert_eq!(request(&mut ws), protocol::requests()[3]);
        ws.send(Message::Text(
            r#"{"id":3,"result":{"data":[],"nextCursor":null}}"#.into(),
        ))
        .unwrap();
    });
    let io = DeadlineIo::connect(&path).unwrap();
    assert_eq!(peer_pid(&io.socket).unwrap(), std::process::id());
    let value = read_metadata(io, &"a".repeat(64)).unwrap();
    assert!(value.rows.is_empty());
    task.join().unwrap();
    clean(&path);
}

#[test]
fn peer_authentication_rejects_an_unsigned_listener_before_sending_application_bytes() {
    let (path, listener) = fixture();
    let task = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        let mut bytes = [0u8; 16];
        assert_eq!(stream.read(&mut bytes).unwrap(), 0);
    });
    let io = DeadlineIo::connect(&path).unwrap();
    let pid = peer_pid(&io.socket).unwrap();
    assert_eq!(verify_peer(pid).err(), Some("untrusted_endpoint"));
    drop(io);
    task.join().unwrap();
    clean(&path);
}

#[test]
fn stale_wrong_rpc_reply_is_not_accepted_as_a_session_snapshot() {
    let (path, listener) = fixture();
    let task = thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        let mut ws = tungstenite::accept(stream).unwrap();
        let _ = ws.read().unwrap();
        ws.send(Message::Text(
            r#"{"id":999,"result":{"userAgent":"SECRET_CANARY"}}"#.into(),
        ))
        .unwrap();
    });
    let result = read_metadata(DeadlineIo::connect(&path).unwrap(), &"a".repeat(64));
    assert_eq!(result.err(), Some("invalid_response"));
    task.join().unwrap();
    clean(&path);
}

#[test]
#[ignore = "only with the project-owned H051 Codex fixture controller"]
fn pinned_codex_observer_preserves_existing_identity_and_loses_status_on_exit() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("work/proofs/s51")
        .canonicalize()
        .unwrap();
    let home = PathBuf::from(
        std::env::var_os("HEADROOM_H051_FIXTURE_HOME").expect("fixture home required"),
    );
    let resolved_home = home.canonicalize().unwrap();
    assert!(resolved_home.starts_with(&root) && resolved_home != root);
    let owner: serde_json::Value =
        serde_json::from_slice(&fs::read(home.join("fixture-owner.json")).unwrap()).unwrap();
    assert_eq!(owner["fixture_only"], true);
    let expected_pid = owner["pid"].as_u64().unwrap() as u32;
    let connect = || {
        let io =
            DeadlineIo::connect(&home.join("app-server-control/app-server-control.sock")).unwrap();
        assert_eq!(peer_pid(&io.socket).unwrap(), expected_pid);
        assert!(verify_peer(expected_pid).is_ok());
        client_with_config(
            "ws://localhost/",
            io,
            Some(WebSocketConfig::default().max_message_size(Some(MAX_MESSAGE))),
        )
        .unwrap()
        .0
    };
    let call =
        |ws: &mut WebSocket<DeadlineIo>, id: u64, method: &str, params: serde_json::Value| {
            ws.send(Message::Text(
                serde_json::json!({"id":id,"method":method,"params":params})
                    .to_string()
                    .into(),
            ))
            .unwrap();
            reply::<serde_json::Value>(ws, id, &mut BTreeSet::new()).unwrap()
        };
    let mut owner_socket = connect();
    let origin = call(
        &mut owner_socket,
        1,
        "initialize",
        serde_json::json!({"clientInfo":{
        "name":"headroom_fixture_owner","title":"Owned H051 fixture","version":"0.1.0"},
        "capabilities":{"experimentalApi":false,"explicitGatewayOauth":true}}),
    );
    let agent = origin["userAgent"].as_str().unwrap().to_owned();
    assert!(agent.contains("headroom_fixture_owner"));
    owner_socket
        .send(Message::Text(r#"{"method":"initialized"}"#.into()))
        .unwrap();
    for (n, model) in [
        (10, "h051-fixture-v1"),
        (11, "h051-fixture-v1"),
        (12, "h051-other-v2"),
    ] {
        let response = call(
            &mut owner_socket,
            n,
            "thread/start",
            serde_json::json!({"cwd":"D:\\","model":model,
            "modelProvider":"h051","approvalPolicy":"never","sandbox":"read-only","ephemeral":false}),
        );
        assert!(response["thread"]["id"].is_string());
    }
    let probe_agent = || {
        let mut socket = connect();
        let r = protocol::requests();
        socket.send(Message::Text(r[0].to_string().into())).unwrap();
        reply::<Initialize>(&mut socket, 1, &mut BTreeSet::new())
            .unwrap()
            .user_agent
    };
    assert_eq!(probe_agent(), agent);
    let mut reader = Reader::default();
    let deadline = Instant::now() + Duration::from_secs(8);
    let snapshot = loop {
        let value = reader.read_home(&home).unwrap();
        if value.rows.iter().filter(|r| r.model.is_some()).count() == 3 {
            break value;
        }
        assert!(
            Instant::now() < deadline,
            "fixture persistence did not settle"
        );
        thread::sleep(Duration::from_millis(50));
    };
    assert_eq!(snapshot.loaded_count, 3);
    assert!(snapshot.rows.iter().all(|r| r.state == "open"));
    assert_eq!(
        snapshot
            .rows
            .iter()
            .filter(|r| r.model.as_deref() == Some("h051-fixture-v1"))
            .count(),
        2
    );
    assert_eq!(
        probe_agent(),
        agent,
        "observer must not replace the existing user-agent identity"
    );
    if std::env::var_os("HEADROOM_H051_UI_HOLD").is_some() {
        // Empty threads belong to this connection until the real UI test ends.
        fs::write(home.join("ui-fixture-ready"), b"ready").unwrap();
        let deadline = Instant::now() + Duration::from_secs(900);
        while !home.join("ui-fixture-done").exists() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(50));
        }
        assert!(
            home.join("ui-fixture-done").exists(),
            "owned UI fixture timed out"
        );
        assert_ne!(
            probe_agent(),
            agent,
            "UI-opened client did not initialize against the owned server"
        );
        let after = reader.read_home(&home).unwrap();
        for before in &snapshot.rows {
            let row = after
                .rows
                .iter()
                .find(|r| r.key == before.key)
                .expect("existing thread lost");
            assert_eq!(
                (&row.model, &row.provider, &row.reasoning),
                (&before.model, &before.provider, &before.reasoning)
            );
        }
        println!("HEADROOM_H051_NATIVE_OK chats=3 same_model=2 identity_preserved=1 ui_owner_held=1 owned_server_connected=1 configs_preserved=1");
        return;
    }
    if std::env::var_os("HEADROOM_H051_OPEN_CENTER").is_some() {
        assert!(reader.can_open_center());
        let models: std::collections::BTreeMap<_, _> = snapshot
            .rows
            .iter()
            .map(|r| {
                (
                    r.key.clone(),
                    (r.model.clone(), r.provider.clone(), r.reasoning.clone()),
                )
            })
            .collect();
        let opened = reader.open_center().unwrap();
        let pid = unsafe { windows_sys::Win32::System::Threading::GetProcessId(opened._process.0) };
        assert_ne!(pid, 0);
        fs::write(
            home.join("opened-center.json"),
            serde_json::to_vec(&serde_json::json!({"fixture_only":true,"pid":pid})).unwrap(),
        )
        .unwrap();
        drop(opened); // Must not terminate a normal client when Headroom releases its handle.
        let deadline = Instant::now() + Duration::from_secs(12);
        while !home.join("center-observed").exists() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(50));
        }
        assert!(
            home.join("center-observed").exists(),
            "controller did not observe the owned client"
        );
        assert_ne!(
            probe_agent(),
            agent,
            "official client did not initialize against the owned server"
        );
        let after = reader.read_home(&home).unwrap();
        for (key, config) in models {
            let row = after
                .rows
                .iter()
                .find(|r| r.key == key)
                .expect("existing thread lost");
            assert_eq!(
                (
                    row.model.clone(),
                    row.provider.clone(),
                    row.reasoning.clone()
                ),
                config
            );
        }
        println!("HEADROOM_H051_NATIVE_OK chats=3 same_model=2 identity_preserved=1 official_center_opened=1 owned_server_connected=1 configs_preserved=1");
        return;
    }
    fs::write(home.join("stop-owned-server"), b"stop").unwrap();
    let deadline = Instant::now() + Duration::from_secs(8);
    while reader.peer.as_ref().is_some_and(|p| p.alive(expected_pid)) && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(25));
    }
    assert!(
        reader.read_home(&home).is_err(),
        "lost server must not keep reporting live or ended chats"
    );
    println!(
        "HEADROOM_H051_NATIVE_OK chats=3 same_model=2 identity_preserved=1 lost_server_unknown=1"
    );
}
