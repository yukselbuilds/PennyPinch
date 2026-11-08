use super::*;
use crate::metrics::normalize::{normalize, Observation};
use crate::metrics::store::{CaptureFlags, MetricsStore};
use serde_json::json;

fn store() -> MetricsStore {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("work/proofs/recovery/metrics-tests")
        .join(Uuid::new_v4().to_string());
    MetricsStore::in_directory(&root)
}
fn data(store: &MetricsStore) -> ActivityData {
    store
        .activity_at(Period::SevenDays, &ActivityFilter::default(), Utc::now())
        .unwrap()
}
fn observe(id: &str, optimizer: &str, at: Option<String>, outcome: &str) -> Observation {
    serde_json::from_value(json!({
        "runtime_generation":"0fce5171-68c1-4bb6-9846-461933fd51c1", "request_id":id,
        "source":"proxy", "proxy_request_id":null, "timestamp_utc":at, "optimizer":optimizer,
        "before":{"tokens":100,"basis":{"kind":"local_estimate","tokenizer":"PRIVATE_BASIS"},"component":"message_input"},
        "after":{"tokens":50,"basis":{"kind":"local_estimate","tokenizer":"PRIVATE_BASIS"},"component":"message_input"},
        "provider_usage":{"input":null,"output":null,"cached_input":null},
        "metadata":{"outcome":outcome,"total_ms":null,"optimization_ms":null,"first_token_ms":null,"cache_reported_hit":null,"transforms_count":null,"conflicted":false}
    })).unwrap()
}
fn insert(store: &MetricsStore, values: &[Observation]) {
    store
        .ingest_batch(
            &normalize(values).unwrap(),
            CaptureFlags::default(),
            Utc::now(),
        )
        .unwrap();
}

#[test]
fn recovery_traffic_distinguishes_no_signal_unknown_and_reported_error_without_raw_fields() {
    let store = store();
    let empty = traffic_from_data(data(&store));
    assert_eq!(empty.status, "none");
    assert!(empty.last_observed_utc.is_none());
    let at = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);
    insert(
        &store,
        &[observe(
            "one",
            "headroom",
            Some(at.clone()),
            "error_reported",
        )],
    );
    let wire = serde_json::to_value(traffic_from_data(data(&store))).unwrap();
    assert_eq!(
        wire,
        json!({"status":"observed","period":"seven_days","last_observed_utc":at,
        "outcome":"error_reported","error_layer":"unknown"})
    );
    assert!(!wire.to_string().contains("PRIVATE"));
    let mut truncated = data(&store);
    truncated.rows.clear();
    truncated.has_more = true;
    assert_eq!(traffic_from_data(truncated).status, "unavailable");
    let mut unplaced = data(&store);
    unplaced.rows.clear();
    unplaced.unplaced_requests = 1;
    assert_eq!(traffic_from_data(unplaced).status, "unavailable");
}

#[test]
fn recovery_external_layer_is_not_headroom_traffic_and_no_error_is_not_relabelled_success() {
    let store = store();
    insert(
        &store,
        &[observe(
            "external",
            "rtk",
            Some(Utc::now().to_rfc3339()),
            "unknown",
        )],
    );
    assert_eq!(traffic_from_data(data(&store)).status, "none");
    insert(
        &store,
        &[observe(
            "headroom",
            "headroom",
            Some(Utc::now().to_rfc3339()),
            "no_error_reported",
        )],
    );
    let result = traffic_from_data(data(&store));
    assert_eq!(result.status, "observed");
    assert_eq!(result.outcome, Outcome::NoErrorReported);
    assert_eq!(result.error_layer, "unknown");
    insert(
        &store,
        &[observe("headroom", "headroom", None, "error_reported")],
    );
    assert_eq!(traffic_from_data(data(&store)).outcome, Outcome::Unknown);
}

#[test]
fn recovery_request_rejects_extra_data_and_error_envelope_is_allowlisted() {
    assert!(serde_json::from_value::<Request>(json!({"schema_version":1,
        "request_id":Uuid::new_v4().to_string(),"path":"PRIVATE"}))
    .is_err());
    let id = Uuid::new_v4().to_string();
    for code in ["invalid_input", "busy", "unavailable"] {
        let wire = serde_json::to_value(failure(id.clone(), code)).unwrap();
        assert_eq!(
            wire,
            json!({"schema_version":1,"request_id":id,"status":"error",
            "error":{"code":code,"retryable":code=="busy","message_key":format!("recovery.{code}"),"diagnostic_id":null}})
        );
    }
    assert_eq!(
        serde_json::to_value(failure("PRIVATE".into(), "invalid_input")).unwrap()["request_id"],
        Uuid::nil().to_string()
    );
    let manager = RecoveryManager::default();
    assert!(manager
        .0
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_ok());
    assert!(manager
        .0
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err());
    drop(Guard(&manager.0));
    assert!(!manager.0.load(Ordering::SeqCst));
}

#[test]
fn recovery_rescan_during_read_cannot_emit_stale_with_zero_installations() {
    let mut client = Client {
        client_kind: "codex",
        detected_count: 0,
        installation_status: "not_found",
        compatibility: "not_verified",
        config_state: "not_owned",
        config_scope: "unknown",
        endpoint_binding: "not_owned",
    };
    revalidate_client(&mut client, false, true);
    assert_eq!(client.installation_status, "unknown");
    assert_eq!(client.detected_count, 0);
    client.detected_count = 1;
    client.installation_status = "current";
    client.compatibility = "config_supported";
    client.config_state = "matches_owned_endpoint";
    client.config_scope = "user_config";
    revalidate_client(&mut client, true, true);
    assert_eq!(client.installation_status, "stale");
    assert_eq!(client.config_state, "not_rechecked");
    assert_eq!(client.compatibility, "not_verified");
}
