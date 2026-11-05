use super::normalize::{project_headroom_041, Source};
use serde_json::{json, Value};
const GENERATION: &str = "0fce5171-68c1-4bb6-9846-461933fd51c1";

fn project(rows: Value) -> Value {
    serde_json::to_value(
        project_headroom_041(
            &serde_json::to_vec(&rows).unwrap(),
            GENERATION,
            "0.41.0",
            Source::Proxy,
        )
        .unwrap(),
    )
    .unwrap()
}
#[test]
fn activity_metadata_secondary_enriches_without_double_count_or_content() {
    let id = json!({"request_id":"request","timestamp":"2026-11-05T12:00:00Z","model":"model-a","input_tokens_original":100,"input_tokens_optimized":50,"total_latency_ms":12.5,"optimization_latency_ms":0,"transforms_applied":["PRIVATE_CANARY"]});
    let mut log = id.clone();
    log["error"] = json!({"body":"PRIVATE_CANARY"});
    log["cache_hit"] = json!(false);
    log["request_messages"] = json!(["PRIVATE_CANARY"]);
    log["first_token_ms"] = json!(99);
    let value = project(json!({"recent_requests":[id],"request_logs":[log]}));
    assert_eq!(value["batch"]["events"].as_array().unwrap().len(), 1);
    let m = &value["batch"]["events"][0]["metadata"];
    assert_eq!(m["total_ms"], 12.5);
    assert_eq!(m["optimization_ms"], 0.0);
    assert_eq!(m["outcome"], "error_reported");
    assert_eq!(m["cache_reported_hit"], false);
    assert_eq!(m["transforms_count"], 1);
    assert!(m["first_token_ms"].is_null());
    assert!(!value.to_string().contains("PRIVATE_CANARY"));
}
#[test]
fn activity_metadata_mismatched_secondary_is_ignored_and_null_is_not_success() {
    let value = project(
        json!({"recent_requests":[{"request_id":"r","timestamp":"2026-11-05T12:00:00Z","model":"a","total_latency_ms":-1}],"request_logs":[{"request_id":"r","timestamp":"2026-11-05T12:00:00Z","model":"b","error":"PRIVATE_CANARY","cache_hit":true}]}),
    );
    let m = &value["batch"]["events"][0]["metadata"];
    assert!(m.is_null() || m["outcome"] == "unknown");
    assert!(m["total_ms"].is_null());
    assert!(m["cache_reported_hit"].is_null());
    let value = project(
        json!({"request_logs":[{"request_id":"r","error":null,"total_latency_ms":86400001}]}),
    );
    assert_eq!(
        value["batch"]["events"][0]["metadata"]["outcome"],
        "no_error_reported"
    );
    assert!(value["batch"]["events"][0]["metadata"]["total_ms"].is_null());
}

#[test]
fn activity_metadata_requires_matching_time_and_primary_presence_does_not_fall_back() {
    let recent =
        json!({"request_id":"r","timestamp":"2026-11-05T12:00:00Z","optimization_latency_ms":0});
    let mut different = recent.clone();
    different["timestamp"] = json!("2026-11-05T12:00:01Z");
    different["error"] = json!("PRIVATE_CANARY");
    let value = project(json!({"recent_requests":[recent],"request_logs":[different]}));
    assert_eq!(
        value["batch"]["events"][0]["metadata"]["outcome"],
        "unknown"
    );
    let value =
        project(json!({"recent_requests":[],"request_logs":[{"request_id":"r","error":null}]}));
    assert!(value["batch"]["events"].as_array().unwrap().is_empty());
    let value = project(
        json!({"request_logs":[{"request_id":"r","error":null,"transforms_applied":vec!["PRIVATE_CANARY";129]}]}),
    );
    assert!(value["batch"]["events"][0]["metadata"]["transforms_count"].is_null());
    assert!(!value.to_string().contains("CANARY"));
    let value = project(
        json!({"recent_requests":[{"request_id":"r","timestamp":"2026-11-05T12:00:00Z"}],"request_logs":[
        {"request_id":"r","timestamp":"2026-11-05T12:00:00Z","error":null},
        {"request_id":"r","timestamp":"2026-11-05T12:00:00Z","error":"PRIVATE_CANARY"}]}),
    );
    assert_eq!(value["batch"]["events"][0]["metadata"]["conflicted"], true);
    assert_eq!(
        value["batch"]["events"][0]["metadata"]["outcome"],
        "unknown"
    );
}

#[test]
fn activity_source_discards_personal_paths_in_identity() {
    let value = project(
        json!({"request_logs":[{"request_id":"r","error":null,"model":"/Users/PRIVATE_CANARY","provider":"C:/PRIVATE_CANARY"}]}),
    );
    assert!(!value.to_string().contains("CANARY"));
}
