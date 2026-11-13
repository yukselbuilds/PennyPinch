use super::protocol::*;
use serde_json::{json, Value};
use std::collections::BTreeSet;

const ONE: &str = "7132bced-3ce0-4c1b-8f48-d93d7b96f7c0";
const TWO: &str = "7132bced-3ce0-4c1b-8f48-d93d7b96f7c1";
const CHILD: &str = "7132bced-3ce0-4c1b-8f48-d93d7b96f7c2";

#[test]
fn ipc_validation_and_one_use_fresh_consent_precede_any_endpoint_or_process_access() {
    use super::{Operation, Request, SessionManager};
    let mut raw = json!({"schema_version":1,"request_id":ONE,"operation":"read","snapshot_id":null,"confirm_open":false});
    let mut request: Request = serde_json::from_value(raw.clone()).unwrap();
    assert!(super::valid_request(&request));
    for field in ["executable", "thread_id", "model", "auth", "endpoint"] {
        raw[field] = "PRIVATE_CANARY".into();
        assert!(serde_json::from_value::<Request>(raw.clone()).is_err());
        raw.as_object_mut().unwrap().remove(field);
    }
    request.operation = Operation::OpenCodexCenter;
    request.snapshot_id = Some(TWO.into());
    let manager = SessionManager::default();
    assert_eq!(manager.execute(request).error, Some("validation_failed"));
    let make = || Request {
        schema_version: 1,
        request_id: ONE.into(),
        operation: Operation::OpenCodexCenter,
        snapshot_id: Some(TWO.into()),
        confirm_open: true,
    };
    manager.0.lock().unwrap().ticket = Some((
        TWO.into(),
        std::time::Instant::now() - std::time::Duration::from_secs(46),
    ));
    assert_eq!(manager.execute(make()).error, Some("stale_snapshot"));
    manager.0.lock().unwrap().ticket = Some((TWO.into(), std::time::Instant::now()));
    let result = manager.execute(make());
    assert!(result.snapshot.is_none());
    assert_eq!(
        result.error,
        Some(if cfg!(windows) {
            "stale_snapshot"
        } else {
            "unavailable"
        })
    );
    assert_eq!(manager.execute(make()).error, Some("stale_snapshot"));
    let lock = manager.0.lock().unwrap();
    assert_eq!(manager.execute(make()).error, Some("busy"));
    drop(lock);
    let encoded = serde_json::to_string(&super::Scope::unknown(
        "claude_code",
        "unsupported_metadata",
    ))
    .unwrap();
    assert!(encoded.contains("\"loaded_count\":null"));
    assert!(!encoded.contains("PRIVATE_CANARY"));
}
fn thread(id: &str, state: Value) -> Value {
    json!({"id":id,"cwd":"D:\\private\\project-alpha","createdAt":1700000000,"updatedAt":1700000001,
        "model":"any-model-v17","modelProvider":"openai","reasoningEffort":"xhigh","source":"cli","status":state,
        "preview":"PRIVATE_CANARY","name":"PRIVATE_CANARY","turns":[{"content":"SECRET_CANARY"}],"apiKey":"SECRET_CANARY"})
}
fn snapshot(threads: Vec<Value>, loaded: Vec<&str>, closed: Vec<&str>) -> Snapshot {
    normalize(
        &"a".repeat(64),
        serde_json::from_value(json!({"data":threads,"nextCursor":null})).unwrap(),
        serde_json::from_value(json!({"data":loaded,"nextCursor":null})).unwrap(),
        &closed.into_iter().map(String::from).collect(),
    )
    .unwrap()
}

#[test]
fn same_model_does_not_merge_chats_or_fabricate_traffic_assignment() {
    let mut child = thread(CHILD, json!({"type":"active","activeFlags":[]}));
    child["parentThreadId"] = ONE.into();
    child["source"] = json!({"subAgent":{"thread_spawn":{"parent_thread_id":ONE}}});
    child["model"] = "different-model-v3".into();
    let value = snapshot(
        vec![
            thread(ONE, json!({"type":"idle"})),
            thread(TWO, json!({"type":"idle"})),
            child,
        ],
        vec![ONE, TWO, CHILD],
        vec![],
    );
    assert_eq!(value.rows.len(), 3);
    assert_eq!(value.loaded_count, 3);
    let child = value.rows.iter().find(|r| r.role == "subagent").unwrap();
    let parent = value
        .rows
        .iter()
        .find(|r| Some(&r.key) == child.parent_key.as_ref())
        .unwrap();
    assert_eq!(
        value
            .rows
            .iter()
            .map(|r| &r.id_hint)
            .collect::<BTreeSet<_>>()
            .len(),
        3
    );
    assert!(value
        .rows
        .iter()
        .any(|r| r.key == child.parent_key.clone().unwrap()));
    assert_ne!(parent.key, child.key);
    assert!(value
        .rows
        .iter()
        .all(|r| r.traffic_assignment == "unverified"));
    assert_eq!(
        value
            .rows
            .iter()
            .filter(|r| r.model.as_deref() == Some("any-model-v17"))
            .count(),
        2
    );
    let encoded = serde_json::to_string(&value.rows).unwrap();
    for forbidden in [
        "PRIVATE_CANARY",
        "SECRET_CANARY",
        "private",
        "content",
        "saved_tokens",
        "auth",
    ] {
        assert!(!encoded.contains(forbidden));
    }
    assert!(value
        .rows
        .iter()
        .all(|r| r.project.as_deref() == Some("project-alpha")));
}

#[test]
fn stored_idle_working_waiting_closed_and_lost_live_witness_remain_distinct() {
    for (status, loaded, closed, expected) in [
        (json!({"type":"notLoaded"}), false, false, "stored"),
        (json!({"type":"idle"}), true, false, "open"),
        (
            json!({"type":"active","activeFlags":[]}),
            true,
            false,
            "working",
        ),
        (
            json!({"type":"active","activeFlags":["waitingOnApproval"]}),
            true,
            false,
            "waiting",
        ),
        (
            json!({"type":"active","activeFlags":["futureFlag"]}),
            true,
            false,
            "unknown",
        ),
        (
            json!({"type":"active","activeFlags":[]}),
            false,
            false,
            "unknown",
        ),
        (json!({"type":"notLoaded"}), true, false, "loaded"),
        (json!({"type":"notLoaded"}), false, true, "ended"),
        (json!({"type":"idle"}), true, true, "open"),
        (json!({"type":"futureStatus"}), true, false, "unknown"),
    ] {
        let value = snapshot(
            vec![thread(ONE, status)],
            if loaded { vec![ONE] } else { vec![] },
            if closed { vec![ONE] } else { vec![] },
        );
        assert_eq!(value.rows[0].state, expected);
    }
    let missing = snapshot(vec![], vec![ONE], vec![]);
    assert_eq!(missing.rows[0].state, "loaded");
    assert!(missing.rows[0].model.is_none());
}

#[test]
fn invalid_ids_duplicates_and_oversized_pages_do_not_claim_a_live_chat() {
    for data in [
        vec![thread("unsafe-id", json!({"type":"idle"}))],
        vec![
            thread(ONE, json!({"type":"idle"})),
            thread(ONE, json!({"type":"idle"})),
        ],
        vec![thread(ONE, json!({"type":"idle"})); MAX_ROWS + 1],
    ] {
        let value = normalize(
            &"a".repeat(64),
            serde_json::from_value(json!({"data":data,"nextCursor":null})).unwrap(),
            serde_json::from_value(json!({"data":[ONE],"nextCursor":null})).unwrap(),
            &BTreeSet::new(),
        );
        assert_eq!(value.err(), Some("invalid_response"));
    }
    let mut value = thread(ONE, json!({"type":"idle"}));
    value["parentThreadId"] = ONE.into();
    assert!(normalize(
        &"a".repeat(64),
        serde_json::from_value(json!({"data":[value]})).unwrap(),
        serde_json::from_value(json!({"data":[ONE]})).unwrap(),
        &BTreeSet::new()
    )
    .is_err());
}

#[test]
fn snapshots_are_scoped_and_pagination_is_explicit() {
    let mut one = thread(ONE, json!({"type":"notLoaded"}));
    one["modelProvider"] = "sk-secret-sentinel".into();
    let make = |scope: &str| {
        normalize(
            scope,
            serde_json::from_value(json!({"data":[one],"nextCursor":"PRIVATE_CURSOR"})).unwrap(),
            serde_json::from_value(json!({"data":[]})).unwrap(),
            &BTreeSet::new(),
        )
        .unwrap()
    };
    let a = make(&"a".repeat(64));
    let b = make(&"b".repeat(64));
    assert_ne!(a.rows[0].key, b.rows[0].key);
    assert!(a.partial);
    assert!(a.rows[0].provider.is_none());
    assert!(!serde_json::to_string(&a.rows)
        .unwrap()
        .contains("PRIVATE_CURSOR"));
}

#[test]
fn observer_rpc_allowlist_never_starts_a_thread_or_changes_shared_login_policy() {
    let r = requests();
    assert_eq!(
        r.iter()
            .map(|r| r["method"].as_str().unwrap())
            .collect::<Vec<_>>(),
        vec![
            "initialize",
            "initialized",
            "thread/list",
            "thread/loaded/list"
        ]
    );
    assert_eq!(r[0]["params"]["clientInfo"]["name"], "codex-backend");
    assert_eq!(r[0]["params"]["capabilities"]["experimentalApi"], false);
    assert_eq!(r[0]["params"]["capabilities"]["requestAttestation"], false);
    assert_eq!(r[2]["params"]["useStateDbOnly"], true);
    assert_eq!(r[2]["params"]["modelProviders"], json!([]));
    let metadata = metadata_request(4, ONE).unwrap();
    assert_eq!(metadata["method"], "thread/read");
    assert_eq!(metadata["params"]["includeTurns"], false);
    assert!(metadata_request(4, "not-a-thread-id").is_err());
    assert!(metadata_request(1000, ONE).is_err());
    for r in r {
        for forbidden in [
            "thread/start",
            "thread/resume",
            "turn/start",
            "auth",
            "model\"",
        ] {
            assert!(!r.to_string().contains(forbidden));
        }
    }
}
