use super::*;
use crate::metrics::{
    attribution::{Attribution, Route},
    normalize::{Basis, MeasurementStatus, ProviderUsage, Reading},
    query::aggregate_unique_events,
    request_metadata::RequestMetadata,
    store::Coverage,
};

const GEN: &str = "7132bced-3ce0-4c1b-8f48-d93d7b96f7ce";

fn now() -> DateTime<Utc> {
    "2026-11-10T12:00:00Z".parse().unwrap()
}

fn sample(id: usize, age_seconds: i64, after: u64) -> NormalizedEvent {
    let reading = |tokens| Reading {
        tokens: Some(tokens),
        basis: Basis {
            kind: CounterKind::LocalEstimate,
            tokenizer: Some("o200k_base".into()),
        },
        component: Component::MessageInput,
    };
    NormalizedEvent {
        schema_version: 1,
        runtime_generation: GEN.into(),
        origin_id: format!("fixture-origin-{id}"),
        event_id: format!("fixture-event-{id}"),
        observed_via: vec![Source::Proxy],
        timestamp_utc: Some((now() - Duration::seconds(age_seconds)).to_rfc3339()),
        optimizer: Optimizer::Headroom,
        before: reading(100),
        after: reading(after),
        provider_usage: ProviderUsage::default(),
        status: MeasurementStatus::Measured,
        saved_tokens: Some(100 - after as i64),
        saved_percent: Some(100.0 - after as f64),
        main_metric_eligible: true,
        metadata: RequestMetadata {
            outcome: Outcome::NoErrorReported,
            ..Default::default()
        },
        attribution: Attribution::observed(Some("model-fixture"), Some("provider-fixture")),
    }
}

fn evaluate(events: Vec<NormalizedEvent>, generation: Option<&str>) -> Vec<Recommendation> {
    let totals = aggregate_unique_events(&events);
    let selection = PeriodSelection {
        retention_days: 30,
        monitoring_session_id: String::new(),
        from: None,
        now: now(),
        last_event: events
            .iter()
            .filter_map(|event| event.timestamp_utc.as_deref()?.parse().ok())
            .max(),
        coverage: Coverage {
            unplaced_events: events
                .iter()
                .filter(|event| event.timestamp_utc.is_none())
                .count() as u64,
            ..Coverage::default()
        },
        events,
    };
    for_period(&selection, &totals, generation)
}

#[test]
fn input_growth_uses_only_one_placed_comparable_basis_and_clears_after_better_measurement() {
    assert!(evaluate(vec![], None).is_empty());
    let negative = sample(1, 5, 120);
    assert_eq!(
        evaluate(vec![negative.clone()], None),
        [Recommendation::InputGrowth {
            basis: CounterKind::LocalEstimate,
            component: Component::MessageInput,
            before_tokens: 100,
            after_tokens: 120,
            events: 1,
        }]
    );
    assert!(evaluate(vec![negative.clone(), sample(2, 0, 70)], None).is_empty());
    let mut partial = negative.clone();
    partial.main_metric_eligible = false;
    assert!(evaluate(vec![partial], None).is_empty());
    let mut mixed = sample(2, 1, 120);
    mixed.before.basis.kind = CounterKind::ProviderTokenizer;
    mixed.after.basis.kind = CounterKind::ProviderTokenizer;
    assert!(evaluate(vec![negative.clone(), mixed], None).is_empty());
    let mut unplaced = negative;
    unplaced.timestamp_utc = None;
    assert!(evaluate(vec![unplaced], None).is_empty());
}

#[test]
fn local_duration_needs_twenty_distinct_recent_records_of_current_generation() {
    let mut samples: Vec<_> = (0..20)
        .map(|id| {
            let mut event = sample(id, id as i64, 50);
            event.metadata.total_ms = Some(60_000.0);
            event.metadata.optimization_ms = Some(if id >= 18 { 1000.0 } else { 20.0 });
            event
        })
        .collect();
    assert!(evaluate(samples.clone(), None).is_empty());
    assert!(evaluate(
        samples.clone(),
        Some("a49e61d9-c802-420f-b95f-755357e33443")
    )
    .is_empty());
    assert_eq!(
        evaluate(samples.clone(), Some(GEN)),
        [Recommendation::SlowOptimization {
            p95_ms: 1000.0,
            requests: 20
        }]
    );
    let mut ambiguous = samples[0].clone();
    ambiguous.event_id.push_str("-different-layer");
    let mut duplicate = samples.clone();
    duplicate.push(ambiguous);
    assert!(evaluate(duplicate, Some(GEN)).is_empty());
    samples[19].metadata.optimization_ms = Some(999.0);
    assert!(evaluate(samples.clone(), Some(GEN)).is_empty());
    samples[19].metadata.optimization_ms = Some(1000.0);
    samples[19].timestamp_utc = Some((now() - Duration::minutes(15)).to_rfc3339());
    assert!(evaluate(samples, Some(GEN)).is_empty());
}

#[test]
fn cache_drop_uses_two_recorded_windows_and_preserves_unknown_and_route_scope() {
    let mut samples: Vec<_> = (0..40)
        .map(|id| {
            let mut event = sample(id, if id < 20 { 1200 } else { 60 } + id as i64, 50);
            event.metadata.cache_reported_hit = Some(id < 12 || (20..28).contains(&id));
            event
        })
        .collect();
    assert_eq!(
        evaluate(samples.clone(), Some(GEN)),
        [Recommendation::CacheShareDrop {
            model: "model-fixture".into(),
            provider: "provider-fixture".into(),
            previous_hits: 12,
            previous_requests: 20,
            recent_hits: 8,
            recent_requests: 20,
        }]
    );
    let mut unreported = samples.clone();
    unreported[39].metadata.cache_reported_hit = None;
    assert!(evaluate(unreported, Some(GEN)).is_empty());
    let mut changed_model = samples.clone();
    changed_model[20].attribution.model = Some("another-model".into());
    assert!(evaluate(changed_model, Some(GEN)).is_empty());
    let mut changed_route = samples.clone();
    changed_route[20].attribution.route = Some(Route {
        origin_id: changed_route[20].origin_id.clone(),
        upstream_id: "a".repeat(64),
        client: Client::Codex,
        agent: None,
    });
    assert!(evaluate(changed_route, Some(GEN)).is_empty());
    let mut conflicting = samples.clone();
    conflicting[0].metadata = RequestMetadata {
        conflicted: true,
        ..Default::default()
    };
    assert!(evaluate(conflicting, Some(GEN)).is_empty());
    samples[29].metadata.cache_reported_hit = Some(true);
    assert!(evaluate(samples, Some(GEN)).is_empty());
}

#[test]
fn only_explicit_stale_installation_evidence_is_advised_once_per_client() {
    let advice: Vec<_> = installation_checks([
        ("codex", "stale"),
        ("claude_code", "unknown"),
        ("codex", "stale"),
        ("opencode", "current"),
        ("other", "stale"),
    ])
    .collect();
    assert_eq!(
        advice,
        [Recommendation::InstallationStale {
            client: Client::Codex
        }]
    );
    assert_eq!(installation_checks([("codex", "current")]).count(), 0);
}
