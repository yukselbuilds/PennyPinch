use super::attribution::*;
use super::normalize::*;
use super::query::AggregateStatus;
use super::store::{CaptureFlags, MetricsStore, Period};
use chrono::{DateTime, Utc};
const GEN: &str = "00000000-0000-4000-8000-000000000001";
fn observation(id: &str) -> Observation {
    let read = |tokens| Reading {
        tokens: Some(tokens),
        basis: Basis {
            kind: CounterKind::LocalEstimate,
            tokenizer: Some("fixture-v1".into()),
        },
        component: Component::MessageInput,
    };
    Observation {
        attribution: Attribution::default(),
        runtime_generation: GEN.into(),
        request_id: id.into(),
        source: Source::Proxy,
        proxy_request_id: None,
        timestamp_utc: Some("2026-11-05T08:00:00Z".into()),
        optimizer: Optimizer::Headroom,
        before: read(100),
        after: read(50),
        provider_usage: ProviderUsage::default(),
    }
}
fn attributed(id: &str, agent: &str, role: AgentRole, upstream: &str) -> Observation {
    let mut value = observation(id);
    value.attribution = Attribution::observed(Some("any-model-v1"), Some("provider"));
    value.attribution.route =
        Some(native_route(GEN, id, upstream, Client::Codex, Some((agent, role, None))).unwrap());
    value
}
fn events(values: &[Observation]) -> Vec<NormalizedEvent> {
    normalize(values).unwrap().events
}

#[test]
fn pinned_observed_model_never_infers_client_or_agent_from_tags_or_names() {
    let payload=br#"{"recent_requests":[{"request_id":"one","timestamp":"2026-11-05T08:00:00Z","model":"gpt-codex-claude","provider":"openai","input_tokens_original":100,"input_tokens_optimized":50,"agent_id":"PRIVATE_CANARY","client":"codex","role":"subagent","tags":{"client":"codex","agent_id":"PRIVATE_CANARY"}}],"agents":[{"role":"main","verified":true}]}"#;
    let mut wire: serde_json::Value = serde_json::from_slice(payload).unwrap();
    wire["recent_requests"][0]["attribution"] = serde_json::to_value(
        attributed("one", "claimed-agent", AgentRole::Main, &"a".repeat(64)).attribution,
    )
    .unwrap();
    let projection = project_headroom_041(
        &serde_json::to_vec(&wire).unwrap(),
        GEN,
        "0.41.0",
        Source::Proxy,
    )
    .unwrap();
    let result = aggregate(&projection.batch.events);
    assert_eq!(result.coverage.requests, 1);
    assert_eq!(result.coverage.model_observed, 1);
    assert_eq!(result.groups[0].model.as_deref(), Some("gpt-codex-claude"));
    assert!(result.groups[0].client.is_none() && result.groups[0].agent.is_none());
    assert_eq!(result.agent_status, CoverageStatus::Unknown);
    assert_eq!(result.client_status, CoverageStatus::Unknown);
    assert_eq!(
        result.groups[0].totals.status,
        AggregateStatus::NoComparableEvents
    );
    assert!(!serde_json::to_string(&projection.batch)
        .unwrap()
        .contains("CANARY"));
}

#[test]
fn same_model_two_agents_and_separate_upstreams_keep_request_routes_separate() {
    let upstream = "a".repeat(64);
    let other = "b".repeat(64);
    let result = aggregate(&events(&[
        attributed("one", "parent", AgentRole::Main, &upstream),
        attributed("two", "child", AgentRole::Subagent, &upstream),
        attributed("three", "other", AgentRole::Main, &other),
    ]));
    assert_eq!(result.coverage.requests, 3);
    assert_eq!(result.groups.len(), 3);
    assert_eq!(result.agent_status, CoverageStatus::Verified);
    assert_eq!(result.client_status, CoverageStatus::Verified);
    let roles: Vec<_> = result
        .groups
        .iter()
        .map(|row| row.agent.as_ref().unwrap().role)
        .collect();
    assert!(roles.contains(&AgentRole::Main) && roles.contains(&AgentRole::Subagent));
    let routes: std::collections::BTreeSet<_> = result
        .groups
        .iter()
        .map(|row| row.upstream_id.clone())
        .collect();
    assert_eq!(routes.len(), 2);
    assert!(result
        .groups
        .iter()
        .all(|row| row.model.as_deref() == Some("any-model-v1")
            && row.totals.saved_tokens == Some(50)));
    let safe = serde_json::to_string(&result).unwrap();
    assert!(
        !safe.contains("\"parent\"") && !safe.contains("\"child\"") && !safe.contains(&upstream)
    );
}

#[test]
fn unknown_metadata_and_missing_role_make_observed_scope_partial_not_global() {
    let result = aggregate(&events(&[
        attributed("one", "known", AgentRole::Main, &"a".repeat(64)),
        observation("two"),
        attributed("three", "role-unknown", AgentRole::Unknown, &"a".repeat(64)),
    ]));
    assert_eq!(result.coverage.requests, 3);
    assert_eq!(result.coverage.client_verified, 2);
    assert_eq!(result.coverage.agent_verified, 1);
    assert_eq!(result.client_status, CoverageStatus::Partial);
    assert_eq!(result.agent_status, CoverageStatus::Partial);
    assert_eq!(result.scope, "observed_requests");
}

#[test]
fn route_must_bind_exact_origin_and_bounded_nonsecret_identity() {
    assert_eq!(
        serde_json::to_string(&Client::OpenCode).unwrap(),
        "\"opencode\""
    );
    let mut wrong = attributed("one", "agent", AgentRole::Main, &"a".repeat(64));
    wrong.request_id = "other".into();
    assert!(normalize(&[wrong]).is_err());
    assert!(native_route(GEN, "one", "https://PRIVATE_CANARY", Client::Codex, None).is_err());
    assert!(native_route(
        GEN,
        "one",
        &"a".repeat(64),
        Client::Codex,
        Some(("self", AgentRole::Main, Some("parent")))
    )
    .is_err());
    assert!(native_route(
        GEN,
        "one",
        &"a".repeat(64),
        Client::Codex,
        Some(("self", AgentRole::Subagent, Some("self")))
    )
    .is_err());
    let label = Attribution::observed(Some("https://secret.example/path"), Some("bad\nheader"));
    assert!(label.is_unknown());
    assert!(Attribution::observed(Some(&"x".repeat(129)), None).is_unknown());
    assert!(Attribution::observed(Some("sk-PRIVATE_CANARY"), None).is_unknown());
}

#[test]
fn attribution_enrichment_and_absorbing_conflict_do_not_change_token_accounting() {
    let original = observation("one");
    let mut added = original.clone();
    added.attribution = Attribution::observed(Some("model-a"), Some("provider"));
    let batch = normalize(&[original.clone(), added.clone()]).unwrap();
    let merged = &batch.events[0];
    assert_eq!(merged.status, MeasurementStatus::Measured);
    assert_eq!(merged.saved_tokens, Some(50));
    assert_eq!(merged.attribution.model.as_deref(), Some("model-a"));
    let mut conflict = added.clone();
    conflict.attribution = Attribution::observed(Some("model-b"), Some("provider"));
    let batch = normalize(&[original, added.clone(), conflict, added]).unwrap();
    let merged = &batch.events[0];
    assert_eq!(merged.status, MeasurementStatus::Measured);
    assert!(merged.attribution.conflicted);
    assert!(merged.attribution.model.is_none());
    assert_eq!(aggregate(&batch.events).coverage.conflicts, 1);
    let first = attributed("same-origin", "agent-a", AgentRole::Main, &"a".repeat(64));
    let other = attributed(
        "same-origin",
        "agent-b",
        AgentRole::Subagent,
        &"b".repeat(64),
    );
    let batch = normalize(&[first.clone(), other, first]).unwrap();
    assert_eq!(batch.events[0].status, MeasurementStatus::Measured);
    assert!(batch.events[0].attribution.conflicted);
    assert_eq!(aggregate(&batch.events).coverage.requests, 1);
}

#[test]
fn linked_layer_events_count_one_origin_and_unknown_never_turns_into_an_agent() {
    let mut first = observation("one");
    first.attribution = Attribution::observed(Some("shared"), Some("provider"));
    let mut layer = first.clone();
    layer.optimizer = Optimizer::Rtk;
    layer.attribution = Attribution::default();
    let result = aggregate(&events(&[first, layer]));
    assert_eq!(result.coverage.events, 2);
    assert_eq!(result.coverage.requests, 1);
    assert_eq!(result.groups.len(), 1);
    assert_eq!(result.groups[0].model.as_deref(), Some("shared"));
    assert_eq!(result.groups[0].totals.saved_tokens, Some(50));
    assert_eq!(result.agent_status, CoverageStatus::Unknown);
}

#[test]
fn group_limit_keeps_all_requests_and_totals_in_explicit_other_row() {
    let values: Vec<_> = (0..80)
        .map(|n| {
            let mut value = observation(&format!("id-{n}"));
            value.attribution = Attribution::observed(Some(&format!("model-{n:02}")), None);
            value
        })
        .collect();
    let result = aggregate(&events(&values));
    assert_eq!(result.groups.len(), 64);
    assert_eq!(result.omitted_groups, 17);
    let other = result.groups.iter().find(|row| row.is_other).unwrap();
    assert_eq!(other.coverage.requests, 17);
    assert!(other.model.is_none());
    assert_eq!(
        result
            .groups
            .iter()
            .map(|row| row.coverage.requests)
            .sum::<u64>(),
        80
    );
    assert_eq!(
        result
            .groups
            .iter()
            .map(|row| row.totals.saved_tokens.unwrap())
            .sum::<i64>(),
        4000
    );
}

#[test]
fn old_payload_replay_reopen_and_period_filters_preserve_attribution_without_recount() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("work/proofs/attribution/store-tests")
        .join(uuid::Uuid::new_v4().to_string());
    std::fs::create_dir_all(root.parent().unwrap()).unwrap();
    let store = MetricsStore::in_directory(&root);
    let now = DateTime::parse_from_rfc3339("2026-11-05T12:00:00Z")
        .unwrap()
        .with_timezone(&Utc);
    let value = observation("persisted");
    let batch = normalize(&[value.clone()]).unwrap();
    let encoded = serde_json::to_string(&batch.events[0]).unwrap();
    assert!(!encoded.contains("attribution"));
    let old: NormalizedEvent = serde_json::from_str(&encoded).unwrap();
    assert!(old.attribution.is_unknown());
    store
        .ingest_batch(&batch, CaptureFlags::default(), now)
        .unwrap();
    let mut added = value;
    added.attribution = Attribution::observed(Some("persisted-model"), Some("provider"));
    store
        .ingest_batch(&normalize(&[added]).unwrap(), CaptureFlags::default(), now)
        .unwrap();
    let reopened = MetricsStore::in_directory(&root);
    let result = reopened.query_at(Period::Today, now).unwrap();
    assert_eq!(result.coverage.events, 1);
    assert_eq!(result.totals.saved_tokens, Some(50));
    assert_eq!(
        result.attribution.groups[0].model.as_deref(),
        Some("persisted-model")
    );
    assert_eq!(
        reopened
            .query_at(Period::Session, now)
            .unwrap()
            .attribution
            .coverage
            .requests,
        0
    );
    let previous = now - chrono::Duration::days(1);
    assert_eq!(
        reopened
            .query_at(Period::Today, previous)
            .unwrap()
            .attribution
            .coverage
            .requests,
        0
    );
}
