use super::normalize::*;

const GENERATION: &str = "00000000-0000-4000-8000-000000000001";
const NEXT_GENERATION: &str = "00000000-0000-4000-8000-000000000002";

fn reading(tokens: Option<u64>) -> Reading {
    Reading {
        tokens,
        basis: Basis {
            kind: CounterKind::LocalEstimate,
            tokenizer: Some("fixture-tokenizer-v1".into()),
        },
        component: Component::MessageInput,
    }
}

fn event(id: &str, before: Option<u64>, after: Option<u64>) -> Observation {
    Observation {
        metadata: Default::default(),
        attribution: Default::default(),
        runtime_generation: GENERATION.into(),
        request_id: id.into(),
        source: Source::Proxy,
        proxy_request_id: None,
        timestamp_utc: Some("2026-11-03T12:00:00Z".into()),
        optimizer: Optimizer::Headroom,
        before: reading(before),
        after: reading(after),
        provider_usage: ProviderUsage::default(),
    }
}

#[test]
fn metrics_empty_is_not_zero_savings() {
    let batch = normalize(&[]).unwrap();
    assert!(batch.events.is_empty());
    assert_eq!(batch.observations, 0);
}

#[test]
fn current_and_previous_engine_stats_preserve_the_same_measurement_basis() {
    let raw = br#"{"recent_requests":[{"request_id":"fixture","timestamp":"2026-11-16T12:00:00Z","input_tokens_original":100,"input_tokens_optimized":80,"cost_usd":12.5}]}"#;
    let previous = project_headroom_041(raw, GENERATION, "0.41.0", Source::Proxy).unwrap();
    let current = project_headroom_041(raw, GENERATION, "0.44.0", Source::Proxy).unwrap();
    assert_eq!(
        serde_json::to_value(&previous.batch).unwrap(),
        serde_json::to_value(&current.batch).unwrap()
    );
    assert!(current.batch.events[0].saved_tokens.is_none());
}

#[test]
fn owned_websocket_pair_upgrades_only_the_matching_legacy_row() {
    use serde_json::json;
    let stats = json!({"recent_requests":[
        {"request_id":"hr_1_000001","timestamp":"2026-11-22T12:00:00Z","model":"same-model",
         "input_tokens_original":11010,"input_tokens_optimized":10799},
        {"request_id":"hr_1_000002","timestamp":"2026-11-22T12:00:00Z","model":"same-model",
         "input_tokens_original":11010,"input_tokens_optimized":10799}
    ]});
    let mut owned = json!({
        "schema_version":2,"runtime_generation":GENERATION,"surface":"provider_usage",
        "records":[{"request_id":"hr_1_000001","surface":"openai_responses_ws","client_status":null,"failed":false,
         "truncated":false,"attempts":[{"completed":true,"usage":{"input_tokens":10799,"output_tokens":15}}],
         "local_input":{"before":1000,"after":789,"counter":"headroom_estimator"}}]
    });
    let raw = stats.to_string();
    let projected = |owned: &serde_json::Value| {
        project_owned_headroom_041(
            raw.as_bytes(),
            Some(owned.to_string().as_bytes()),
            GENERATION,
        )
        .unwrap()
        .batch
        .events
    };
    let measured = projected(&owned);
    let pair = measured
        .iter()
        .find(|event| event.saved_tokens == Some(211))
        .unwrap();
    assert_eq!(pair.before.tokens, Some(1000));
    assert_eq!(pair.after.tokens, Some(789));
    assert_eq!(pair.saved_percent, Some(21.1));
    assert_eq!(pair.before.basis.kind, CounterKind::LocalEstimate);
    assert_eq!(pair.before.component, Component::WholeInput);
    assert_eq!(pair.provider_usage.input, Some(10799));
    assert_eq!(
        measured
            .iter()
            .filter(|event| event.main_metric_eligible)
            .count(),
        1
    );
    let plain = project_owned_headroom_041(raw.as_bytes(), None, GENERATION).unwrap();
    let raw_row = plain
        .batch
        .events
        .iter()
        .find(|event| event.event_id == pair.event_id)
        .unwrap();
    let mut stored = raw_row.clone();
    assert!(!merge_event(&mut stored, pair));
    assert_eq!(&stored, pair);
    assert!(!merge_event(&mut stored, raw_row));
    assert_eq!(&stored, pair);

    owned["records"][0]["local_input"]["after"] = json!(1001);
    let changed = projected(&owned);
    assert!(merge_event(
        &mut stored,
        changed
            .iter()
            .find(|event| event.event_id == pair.event_id)
            .unwrap()
    ));
    assert_eq!(stored.status, MeasurementStatus::Conflict);
    for (field, bad) in [
        ("truncated", json!(true)),
        ("failed", json!(true)),
        ("surface", json!("openai_responses")),
        ("request_id", json!("mismatched")),
    ] {
        let mut unsafe_record = owned.clone();
        unsafe_record["records"][0][field] = bad;
        assert!(projected(&unsafe_record)
            .iter()
            .all(|event| event.saved_tokens.is_none()));
    }
    let duplicate = owned["records"][0].clone();
    owned["records"].as_array_mut().unwrap().push(duplicate);
    assert!(projected(&owned)
        .iter()
        .all(|event| event.saved_tokens.is_none()));
}

#[test]
fn owned_provider_usage_requires_exact_identity_and_sums_every_successful_attempt() {
    use serde_json::json;
    let stats = json!({"recent_requests":[
        {"request_id":"hr_1_000001","timestamp":"2026-11-15T12:00:00Z","model":"shared",
         "input_tokens_original":100,"input_tokens_optimized":80},
        {"request_id":"hr_1_000002","timestamp":"2026-11-15T12:00:00Z","model":"shared",
         "input_tokens_original":300,"input_tokens_optimized":200}
    ]});
    let first = json!({"status":200,"usage":{
        "input_tokens":80,"cache_read_input_tokens":900,"cache_creation_input_tokens":100,"output_tokens":6
    }});
    let follow = json!({"status":200,"usage":{
        "input_tokens":220,"cache_read_input_tokens":1100,"cache_creation_input_tokens":0,"output_tokens":23
    }});
    let mut sidecar = json!({
        "schema_version":1,"runtime_generation":GENERATION,"surface":"anthropic_messages",
        "records":[{"request_id":"hr_1_000001","truncated":false,"attempts":[first,follow],
                    "usage":{"total_input_tokens":900},"private_body":"PRIVATE_CANARY"}]
    });
    let project = |payload: &serde_json::Value| {
        project_owned_headroom_041(
            &serde_json::to_vec(&stats).unwrap(),
            Some(&serde_json::to_vec(payload).unwrap()),
            GENERATION,
        )
        .unwrap()
        .batch
    };
    let batch = project(&sidecar);
    assert_eq!(batch.events.len(), 2);
    let for_after = |batch: &NormalizedBatch, n| {
        batch
            .events
            .iter()
            .find(|e| e.after.tokens == Some(n))
            .unwrap()
            .provider_usage
            .clone()
    };
    let captured = for_after(&batch, 80);
    assert_eq!(captured.input, Some(2400));
    assert_eq!(captured.output, Some(29));
    assert_eq!(captured.cached_input, Some(2000));
    assert_eq!(captured.cache_write_input, Some(100));
    assert_eq!(captured.upstream_attempts, Some(2));
    assert_eq!(for_after(&batch, 200), ProviderUsage::default());
    assert!(batch.events.iter().all(|e| e.saved_tokens.is_none()));
    assert!(!serde_json::to_string(&batch)
        .unwrap()
        .contains("PRIVATE_CANARY"));

    sidecar["runtime_generation"] = json!(NEXT_GENERATION);
    assert_eq!(for_after(&project(&sidecar), 80), ProviderUsage::default());
    sidecar["runtime_generation"] = json!(GENERATION);
    sidecar["records"][0]["attempts"][0]["usage"]["cache_creation_input_tokens"] = json!(null);
    let partial = for_after(&project(&sidecar), 80);
    assert_eq!(partial.input, None);
    assert_eq!(partial.output, Some(29));
    assert_eq!(partial.cache_write_input, None);
    sidecar["records"][0]["attempts"][0]["status"] = json!(429);
    sidecar["records"][0]["client_status"] = json!(502);
    assert_eq!(
        project(&sidecar)
            .events
            .iter()
            .find(|event| event.after.tokens == Some(80))
            .unwrap()
            .metadata
            .outcome,
        super::request_metadata::Outcome::ErrorReported
    );
    let failed = for_after(&project(&sidecar), 80);
    assert_eq!(failed.input, None);
    assert_eq!(failed.output, None);
    assert_eq!(failed.upstream_attempts, Some(2));
    sidecar["records"][0]["attempts"][0]["status"] = json!(200);
    sidecar["records"][0]["attempts"][0]["usage"]["cache_creation_input_tokens"] = json!(0);
    assert_eq!(for_after(&project(&sidecar), 80).cache_write_input, Some(0));
    let duplicate = sidecar["records"][0].clone();
    sidecar["records"].as_array_mut().unwrap().push(duplicate);
    let ambiguous = for_after(&project(&sidecar), 80);
    assert!(ambiguous.conflicted);
    assert_eq!(ambiguous.upstream_attempts, None);
}

#[test]
fn owned_responses_usage_keeps_inclusive_input_and_protocols_separate() {
    use serde_json::json;
    let stats = json!({"recent_requests":[
        {"request_id":"hr_1_000001","timestamp":"2026-11-16T12:00:00Z","model":"shared",
         "input_tokens_original":100,"input_tokens_optimized":80},
        {"request_id":"hr_1_000002","timestamp":"2026-11-16T12:00:00Z","model":"shared",
         "input_tokens_original":300,"input_tokens_optimized":200}
    ]});
    let mut sidecar = json!({"schema_version":2,"runtime_generation":GENERATION,"surface":"provider_usage",
    "records":[
        {"request_id":"hr_1_000001","surface":"openai_responses","truncated":false,"attempts":[
            {"status":200,"usage":{"input_tokens":1000,"cache_read_input_tokens":900,"cache_creation_input_tokens":100,"output_tokens":6}},
            {"status":200,"usage":{"input_tokens":1400,"cache_read_input_tokens":1100,"output_tokens":23}}
        ]},
        {"request_id":"hr_1_000002","surface":"anthropic_messages","truncated":false,"attempts":[
            {"status":200,"usage":{"input_tokens":80,"cache_read_input_tokens":900,"cache_creation_input_tokens":100,"output_tokens":6}}
        ]}
    ]});
    let project = |payload: &serde_json::Value| {
        project_owned_headroom_041(
            &serde_json::to_vec(&stats).unwrap(),
            Some(&serde_json::to_vec(payload).unwrap()),
            GENERATION,
        )
        .unwrap()
        .batch
    };
    let batch = project(&sidecar);
    let usage = |batch: &NormalizedBatch, after| {
        batch
            .events
            .iter()
            .find(|event| event.after.tokens == Some(after))
            .unwrap()
            .provider_usage
            .clone()
    };
    let responses = usage(&batch, 80);
    assert_eq!(responses.input, Some(2400));
    assert_eq!(responses.output, Some(29));
    assert_eq!(responses.cached_input, Some(2000));
    assert_eq!(responses.cache_write_input, None);
    assert_eq!(responses.upstream_attempts, Some(2));
    assert_eq!(usage(&batch, 200).input, Some(1080));
    assert!(batch
        .events
        .iter()
        .all(|event| event.saved_tokens.is_none()));
    sidecar["records"][0]["attempts"][0]["status"] = json!(429);
    sidecar["records"][0]["client_status"] = json!(502);
    let failure_batch = project(&sidecar);
    assert_eq!(
        failure_batch
            .events
            .iter()
            .find(|event| event.after.tokens == Some(80))
            .unwrap()
            .metadata
            .outcome,
        super::request_metadata::Outcome::ErrorReported
    );
    let failed = usage(&failure_batch, 80);
    assert_eq!(failed.input, None);
    assert_eq!(failed.output, None);
    sidecar["records"][0]["surface"] = json!("responses_ws_unverified");
    assert_eq!(usage(&project(&sidecar), 80), ProviderUsage::default());
    sidecar["runtime_generation"] = json!(NEXT_GENERATION);
    assert!(project(&sidecar)
        .events
        .iter()
        .all(|event| event.provider_usage == ProviderUsage::default()));
}

#[test]
fn owned_websocket_usage_requires_completion_and_preserves_unknown_scope() {
    use serde_json::json;
    let stats = json!({"recent_requests":[{"request_id":"hr_1_000001","timestamp":"2026-11-16T12:00:00Z",
        "input_tokens_original":100,"input_tokens_optimized":80}]});
    let mut sidecar = json!({"schema_version":2,"runtime_generation":GENERATION,"surface":"provider_usage",
        "records":[{"request_id":"hr_1_000001","surface":"openai_responses_ws","client_status":null,
            "failed":false,"truncated":false,"attempts":[{"completed":true,"usage":{
                "input_tokens":2400,"cache_read_input_tokens":2000,"cache_creation_input_tokens":100,"output_tokens":29}}]}]});
    let project = |value: &serde_json::Value| {
        project_owned_headroom_041(
            &serde_json::to_vec(&stats).unwrap(),
            Some(&serde_json::to_vec(value).unwrap()),
            GENERATION,
        )
        .unwrap()
        .batch
    };
    let row = project(&sidecar).events.remove(0);
    assert_eq!(row.provider_usage.input, Some(2400));
    assert_eq!(row.provider_usage.cached_input, Some(2000));
    assert_eq!(row.provider_usage.cache_write_input, Some(100));
    assert_eq!(row.provider_usage.upstream_attempts, Some(1));
    assert!(row.saved_tokens.is_none());
    sidecar["records"][0]["attempts"][0]["completed"] = json!(false);
    sidecar["records"][0]["attempts"][0]["status"] = json!(200);
    let row = project(&sidecar).events.remove(0);
    assert_eq!(row.provider_usage.input, None);
    assert_eq!(row.provider_usage.output, None);
    sidecar["records"][0]["failed"] = json!(true);
    sidecar["records"][0]["truncated"] = json!(true);
    let row = project(&sidecar).events.remove(0);
    assert_eq!(row.provider_usage, ProviderUsage::default());
    assert_eq!(
        row.metadata.outcome,
        super::request_metadata::Outcome::ErrorReported
    );
}

#[test]
fn provider_conflict_and_missing_snapshots_preserve_the_compression_measurement() {
    let unknown = event("request", Some(100), Some(70));
    let mut known = unknown.clone();
    known.provider_usage.input = Some(50);
    known.provider_usage.upstream_attempts = Some(1);
    let batch = normalize(&[unknown.clone(), known.clone(), unknown.clone()]).unwrap();
    assert_eq!(batch.events[0].provider_usage.input, Some(50));
    let mut other = known.clone();
    other.provider_usage.input = Some(51);
    let batch = normalize(&[unknown, known.clone(), other, known]).unwrap();
    let value = &batch.events[0];
    assert!(value.provider_usage.conflicted);
    assert_eq!(value.provider_usage.input, None);
    assert_eq!(value.status, MeasurementStatus::Measured);
    assert_eq!(value.saved_tokens, Some(30));
    assert_eq!(batch.conflicting_events, 0);
}

#[test]
fn metrics_comparable_reduction_zero_baseline_and_increase() {
    let inputs = [
        (Some(120_000), Some(75_000), Some(45_000), Some(37.5)),
        (Some(100), Some(100), Some(0), Some(0.0)),
        (Some(0), Some(0), Some(0), None),
        (Some(0), Some(10), Some(-10), None),
        (Some(100), Some(150), Some(-50), Some(-50.0)),
        (Some(100), None, None, None),
    ];
    for (before, after, saved, percent) in inputs {
        let batch = normalize(&[event("request", before, after)]).unwrap();
        let value = &batch.events[0];
        assert_eq!(value.saved_tokens, saved);
        assert_eq!(value.saved_percent, percent);
        assert_eq!(value.main_metric_eligible, saved.is_some());
    }
}

#[test]
fn metrics_basis_and_component_must_match() {
    let mut value = event("basis", Some(100), Some(50));
    value.after.basis.kind = CounterKind::ProviderTokenizer;
    let result = normalize(&[value.clone()]).unwrap();
    assert_eq!(result.events[0].status, MeasurementStatus::Incomparable);
    assert_eq!(result.events[0].saved_tokens, None);
    value.after = value.before.clone();
    value.after.component = Component::WholeInput;
    assert_eq!(normalize(&[value]).unwrap().events[0].saved_percent, None);
}

#[test]
fn metrics_proxy_mcp_replay_deduplicates_only_proven_origin() {
    let proxy = event("origin", Some(100), Some(50));
    let mut mcp = proxy.clone();
    mcp.source = Source::Mcp;
    mcp.request_id = "mcp-observation".into();
    mcp.proxy_request_id = Some("origin".into());
    let result = normalize(&[proxy.clone(), mcp.clone(), proxy]).unwrap();
    assert_eq!(result.events.len(), 1);
    assert_eq!(result.duplicate_observations, 2);
    assert_eq!(
        result.events[0].observed_via,
        vec![Source::Proxy, Source::Mcp]
    );
    mcp.proxy_request_id = None;
    let result = normalize(&[mcp]).unwrap();
    assert_eq!(result.events[0].status, MeasurementStatus::Uncorrelated);
    assert!(!result.events[0].main_metric_eligible);
}

#[test]
fn metrics_conflicting_replay_is_unknown_and_restart_is_separate() {
    let first = event("same-id", Some(100), Some(50));
    let mut conflict = first.clone();
    conflict.after.tokens = Some(70);
    let result = normalize(&[first.clone(), conflict]).unwrap();
    assert_eq!(result.conflicting_events, 1);
    assert_eq!(result.events[0].status, MeasurementStatus::Conflict);
    assert_eq!(result.events[0].saved_tokens, None);
    let mut restarted = first.clone();
    restarted.runtime_generation = NEXT_GENERATION.into();
    assert_eq!(normalize(&[first, restarted]).unwrap().events.len(), 2);
}

#[test]
fn metrics_other_layers_and_provider_usage_do_not_inflate_input_savings() {
    for (optimizer, component) in [
        (Optimizer::Rtk, Component::MessageInput),
        (Optimizer::Headroom, Component::ToolSchema),
        (Optimizer::Cache, Component::CacheInput),
        (Optimizer::Output, Component::Output),
    ] {
        let mut value = event("layer", Some(100), Some(50));
        value.optimizer = optimizer;
        value.before.component = component;
        value.after.component = component;
        value.provider_usage.input = Some(12);
        let result = normalize(&[value]).unwrap();
        assert!(!result.events[0].main_metric_eligible);
        assert_eq!(result.events[0].provider_usage.input, Some(12));
    }
    let mut value = event("input", Some(100), Some(50));
    value.provider_usage.input = Some(12);
    assert_eq!(
        normalize(&[value]).unwrap().events[0].saved_tokens,
        Some(50)
    );
}

#[test]
fn metrics_legacy_engine_regression_never_assumes_comparable_tokenizers() {
    let payload = include_bytes!("../../../tests/fixtures/metrics/headroom-041.json");
    let projection = project_headroom_041(payload, GENERATION, "0.41.0", Source::Proxy).unwrap();
    assert_eq!(projection.batch.events.len(), 3);
    assert!(!projection.complete_traffic);
    for value in &projection.batch.events {
        assert_eq!(value.status, MeasurementStatus::UnknownBasis);
        assert_eq!(value.saved_tokens, None);
        assert_eq!(value.saved_percent, None);
        assert!(!value.main_metric_eligible);
    }
    let encoded = serde_json::to_string(&projection).unwrap();
    for secret in [
        "DO_NOT_COPY",
        "request_messages",
        "compressed_messages",
        "authorization",
        "raw-request-id",
        "proxy_savings",
        "99.99",
    ] {
        assert!(!encoded.contains(secret), "unexpected field/content");
    }
}

#[test]
fn metrics_limits_and_invalid_metadata_fail_closed() {
    let mut value = event("bad", Some(MAX_TOKEN_COUNT + 1), Some(1));
    assert_eq!(
        normalize(&[value.clone()]).unwrap_err(),
        NormalizeError::InvalidCounter
    );
    value.before.tokens = Some(100);
    value.timestamp_utc = Some("not-a-time".into());
    assert_eq!(
        normalize(&[value.clone()]).unwrap_err(),
        NormalizeError::InvalidTimestamp
    );
    value.timestamp_utc = None;
    assert!(!normalize(&[value.clone()]).unwrap().events[0].main_metric_eligible);
    value.runtime_generation = "not-a-generation".into();
    assert_eq!(
        normalize(&[value]).unwrap_err(),
        NormalizeError::InvalidIdentity
    );
    assert_eq!(
        project_headroom_041(b"{}", GENERATION, "new", Source::Proxy).unwrap_err(),
        NormalizeError::UnsupportedEngineVersion
    );
}

#[test]
fn metrics_conflict_cannot_keep_ambiguous_time_or_basis() {
    let first = event("ambiguous", Some(100), Some(50));
    let mut second = first.clone();
    second.after.tokens = Some(70);
    let mut third = first.clone();
    third.timestamp_utc = Some("2026-11-04T12:00:00Z".into());
    let result = normalize(&[first, second, third]).unwrap();
    let value = &result.events[0];
    assert_eq!(result.conflicting_events, 1);
    assert_eq!(value.timestamp_utc, None);
    assert_eq!(value.before.basis.kind, CounterKind::Unknown);
    assert_eq!(value.before.component, Component::Unknown);
}

#[test]
fn metrics_same_request_layers_and_equivalent_utc_are_not_double_counted() {
    let first = event("layered", Some(100), Some(50));
    let mut rtk = first.clone();
    rtk.optimizer = Optimizer::Rtk;
    let mut tool_schema = first.clone();
    tool_schema.before.component = Component::ToolSchema;
    tool_schema.after.component = Component::ToolSchema;
    let result = normalize(&[first.clone(), rtk, tool_schema]).unwrap();
    assert_eq!(result.events.len(), 3);
    assert_eq!(
        result
            .events
            .iter()
            .filter(|e| e.main_metric_eligible)
            .count(),
        1
    );
    assert!(result
        .events
        .iter()
        .all(|e| e.origin_id == result.events[0].origin_id));
    let mut same_instant = first.clone();
    same_instant.timestamp_utc = Some("2026-11-03T15:00:00+03:00".into());
    let original_key = normalize(&[first.clone()]).unwrap().events[0]
        .event_id
        .clone();
    let replay = normalize(&[same_instant, first]).unwrap();
    assert_eq!(replay.events.len(), 1);
    assert_eq!(replay.conflicting_events, 0);
    assert_eq!(replay.events[0].event_id, original_key);
}

#[test]
fn metrics_partial_windows_aggregate_counters_and_missing_identity_stay_partial() {
    for payload in [
        br#"{"requests":{"total":0},"tokens":{"saved":999}}"#.as_slice(),
        br#"{"requests":{"total":51},"proxy_savings":{"lifetime_saved":4471}}"#.as_slice(),
    ] {
        let result = project_headroom_041(payload, GENERATION, "0.41.0", Source::Proxy).unwrap();
        assert!(result.aggregate_only);
        assert!(!result.complete_traffic);
        assert!(result.batch.events.is_empty());
    }
    let payload = br#"{"requests":{"total":52},"recent_requests":[{}, {"input_tokens_original":100}],"request_logs":[{"request_id":"old","input_tokens_original":100,"input_tokens_optimized":50}]}"#;
    let result = project_headroom_041(payload, GENERATION, "0.41.0", Source::Proxy).unwrap();
    assert_eq!(result.reported_request_count, Some(52));
    assert_eq!(result.unidentified_rows, 2);
    assert!(result.batch.events.is_empty());
    assert!(result.rolling_window && !result.complete_traffic);
}

#[test]
fn metrics_snapshot_limits_types_and_raw_origin_claims_fail_closed() {
    let huge = vec![b' '; MAX_SNAPSHOT_BYTES + 1];
    assert_eq!(
        project_headroom_041(&huge, GENERATION, "0.41.0", Source::Proxy).unwrap_err(),
        NormalizeError::TooLarge
    );
    let many = vec![event("replay", Some(1), Some(1)); MAX_OBSERVATIONS + 1];
    assert_eq!(normalize(&many).unwrap_err(), NormalizeError::TooLarge);
    for invalid in [
        br#"{"recent_requests":[{"request_id":"x","input_tokens_original":-1}]}"#.as_slice(),
        br#"{"recent_requests":[{"request_id":"x","input_tokens_original":true}]}"#.as_slice(),
        br#"{"recent_requests":[{"request_id":"x","input_tokens_original":1.5}]}"#.as_slice(),
        b"{}".as_slice(),
    ] {
        assert_eq!(
            project_headroom_041(invalid, GENERATION, "0.41.0", Source::Proxy).unwrap_err(),
            NormalizeError::InvalidPayload
        );
    }
    let payload = br#"{"recent_requests":[{"request_id":"x","timestamp":"2026-11-03T12:00:00Z","input_tokens_original":100,"input_tokens_optimized":50,"proxy_request_id":"x","has_exact_tokens":true,"basis":{"kind":"local_estimate","tokenizer":"claimed"}}]}"#;
    let result = project_headroom_041(payload, GENERATION, "0.41.0", Source::Mcp).unwrap();
    assert_eq!(
        result.batch.events[0].status,
        MeasurementStatus::UnknownBasis
    );
    assert!(!result.batch.events[0].main_metric_eligible);
}
