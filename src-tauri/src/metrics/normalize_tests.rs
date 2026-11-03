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
