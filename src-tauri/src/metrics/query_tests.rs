use super::normalize::{
    normalize, Basis, Component, CounterKind, Observation, Optimizer, ProviderUsage, Reading,
    Source, MAX_TOKEN_COUNT,
};
use super::query::{aggregate_unique_events, AggregateStatus};

const GENERATION: &str = "00000000-0000-4000-8000-000000000001";

fn basis(kind: CounterKind, tokenizer: Option<&str>) -> Basis {
    Basis {
        kind,
        tokenizer: tokenizer.map(str::to_owned),
    }
}

fn reading(tokens: Option<u64>, basis: &Basis, component: Component) -> Reading {
    Reading {
        tokens,
        basis: basis.clone(),
        component,
    }
}

fn observation(
    id: &str,
    before: Option<u64>,
    after: Option<u64>,
    basis: &Basis,
    component: Component,
    optimizer: Optimizer,
) -> Observation {
    Observation {
        metadata: Default::default(),
        attribution: Default::default(),
        runtime_generation: GENERATION.into(),
        request_id: id.into(),
        source: Source::Proxy,
        proxy_request_id: None,
        timestamp_utc: Some("2026-11-04T12:00:00Z".into()),
        optimizer,
        before: reading(before, basis, component),
        after: reading(after, basis, component),
        provider_usage: ProviderUsage::default(),
    }
}

fn normalized(observations: &[Observation]) -> Vec<super::normalize::NormalizedEvent> {
    normalize(observations).unwrap().events
}

fn local_basis() -> Basis {
    basis(CounterKind::LocalEstimate, Some("fixture-tokenizer-v1"))
}

#[test]
fn period_savings_use_weighted_totals_not_mean_request_percentages() {
    let basis = local_basis();
    let events = normalized(&[
        observation(
            "small",
            Some(1_000),
            Some(500),
            &basis,
            Component::MessageInput,
            Optimizer::Headroom,
        ),
        observation(
            "large",
            Some(9_000),
            Some(8_100),
            &basis,
            Component::MessageInput,
            Optimizer::Headroom,
        ),
    ]);

    let totals = aggregate_unique_events(&events);
    assert_eq!(totals.status, AggregateStatus::Measured);
    assert_eq!(totals.comparable_events, 2);
    assert_eq!(totals.excluded_events, 0);
    assert_eq!(totals.before_tokens, Some(10_000));
    assert_eq!(totals.after_tokens, Some(8_600));
    assert_eq!(totals.saved_tokens, Some(1_400));
    assert_eq!(totals.saved_percent, Some(14.0));
    assert_eq!(totals.groups.len(), 1);
}

#[test]
fn empty_or_only_ineligible_events_are_not_reported_as_zero_savings() {
    let empty = aggregate_unique_events(&[]);
    assert_eq!(empty.status, AggregateStatus::NoComparableEvents);
    assert_eq!(empty.comparable_events, 0);
    assert_eq!(empty.excluded_events, 0);
    assert_eq!(empty.before_tokens, None);
    assert_eq!(empty.after_tokens, None);
    assert_eq!(empty.saved_tokens, None);
    assert_eq!(empty.saved_percent, None);

    let basis = local_basis();
    let missing = observation(
        "missing",
        None,
        Some(10),
        &basis,
        Component::MessageInput,
        Optimizer::Headroom,
    );
    let events = normalized(&[
        observation(
            "rtk",
            Some(100),
            Some(50),
            &basis,
            Component::MessageInput,
            Optimizer::Rtk,
        ),
        observation(
            "output",
            Some(100),
            Some(50),
            &basis,
            Component::Output,
            Optimizer::Headroom,
        ),
        missing,
    ]);
    let totals = aggregate_unique_events(&events);
    assert_eq!(totals.status, AggregateStatus::NoComparableEvents);
    assert_eq!(totals.comparable_events, 0);
    assert_eq!(totals.excluded_events, 3);
    assert!(totals.groups.is_empty());
    assert_eq!(totals.saved_tokens, None);
    assert_eq!(totals.saved_percent, None);
}

#[test]
fn zero_savings_increases_and_zero_baselines_keep_distinct_meaning() {
    let basis = local_basis();
    for (id, before, after, saved, percent) in [
        ("unchanged", 100, 100, 0, Some(0.0)),
        ("increase", 100, 150, -50, Some(-50.0)),
        ("zero-baseline", 0, 10, -10, None),
    ] {
        let events = normalized(&[observation(
            id,
            Some(before),
            Some(after),
            &basis,
            Component::MessageInput,
            Optimizer::Headroom,
        )]);
        let totals = aggregate_unique_events(&events);
        assert_eq!(totals.status, AggregateStatus::Measured, "{id}");
        assert_eq!(totals.saved_tokens, Some(saved), "{id}");
        assert_eq!(totals.saved_percent, percent, "{id}");
    }
}

#[test]
fn unknown_incompatible_or_uncorrelated_events_are_excluded() {
    let local = local_basis();
    let provider = basis(CounterKind::ProviderTokenizer, Some("provider-v2"));
    let unknown = basis(CounterKind::Unknown, None);

    let mut different_basis = observation(
        "basis-mismatch",
        Some(100),
        Some(50),
        &local,
        Component::MessageInput,
        Optimizer::Headroom,
    );
    different_basis.after.basis = provider;
    let mut different_component = observation(
        "component-mismatch",
        Some(100),
        Some(50),
        &local,
        Component::MessageInput,
        Optimizer::Headroom,
    );
    different_component.after.component = Component::WholeInput;
    let mut uncorrelated = observation(
        "uncorrelated",
        Some(100),
        Some(50),
        &local,
        Component::MessageInput,
        Optimizer::Headroom,
    );
    uncorrelated.source = Source::Mcp;
    let mut missing_time = observation(
        "missing-time",
        Some(100),
        Some(50),
        &local,
        Component::MessageInput,
        Optimizer::Headroom,
    );
    missing_time.timestamp_utc = None;

    let events = normalized(&[
        observation(
            "unknown",
            Some(100),
            Some(50),
            &unknown,
            Component::MessageInput,
            Optimizer::Headroom,
        ),
        different_basis,
        different_component,
        uncorrelated,
        missing_time,
    ]);
    assert!(events.iter().all(|event| !event.main_metric_eligible));

    let totals = aggregate_unique_events(&events);
    assert_eq!(totals.status, AggregateStatus::NoComparableEvents);
    assert_eq!(totals.comparable_events, 0);
    assert_eq!(totals.excluded_events, 5);
    assert_eq!(totals.before_tokens, None);
    assert_eq!(totals.after_tokens, None);
    assert_eq!(totals.saved_tokens, None);
    assert_eq!(totals.saved_percent, None);
}

#[test]
fn optimizer_layers_and_provider_usage_do_not_inflate_headroom_input() {
    let basis = local_basis();
    let mut headroom = observation(
        "headroom",
        Some(100),
        Some(60),
        &basis,
        Component::MessageInput,
        Optimizer::Headroom,
    );
    headroom.provider_usage = ProviderUsage {
        input: Some(MAX_TOKEN_COUNT),
        output: Some(MAX_TOKEN_COUNT),
        cached_input: Some(MAX_TOKEN_COUNT),
        ..ProviderUsage::default()
    };
    let mut tool_schema = observation(
        "tool-schema",
        Some(500),
        Some(100),
        &basis,
        Component::ToolSchema,
        Optimizer::Headroom,
    );
    tool_schema.provider_usage.input = Some(500);
    let cache = observation(
        "cache",
        Some(400),
        Some(100),
        &basis,
        Component::CacheInput,
        Optimizer::Cache,
    );
    let rtk = observation(
        "rtk",
        Some(300),
        Some(100),
        &basis,
        Component::MessageInput,
        Optimizer::Rtk,
    );
    let output = observation(
        "output",
        Some(200),
        Some(100),
        &basis,
        Component::Output,
        Optimizer::Output,
    );
    let events = normalized(&[headroom, tool_schema, cache, rtk, output]);

    let totals = aggregate_unique_events(&events);
    assert_eq!(totals.status, AggregateStatus::Measured);
    assert_eq!(totals.comparable_events, 1);
    assert_eq!(totals.excluded_events, 4);
    assert_eq!(totals.before_tokens, Some(100));
    assert_eq!(totals.after_tokens, Some(60));
    assert_eq!(totals.saved_tokens, Some(40));
    assert_eq!(totals.saved_percent, Some(40.0));
}

#[test]
fn distinct_counter_bases_and_input_components_remain_separate_groups() {
    let local_a = local_basis();
    let local_b = basis(CounterKind::LocalEstimate, Some("fixture-tokenizer-v2"));
    let provider = basis(CounterKind::ProviderTokenizer, Some("provider-v1"));
    let whole = observation(
        "whole",
        Some(400),
        Some(300),
        &local_a,
        Component::WholeInput,
        Optimizer::Headroom,
    );
    let events = normalized(&[
        observation(
            "local-a",
            Some(100),
            Some(80),
            &local_a,
            Component::MessageInput,
            Optimizer::Headroom,
        ),
        observation(
            "local-b",
            Some(200),
            Some(100),
            &local_b,
            Component::MessageInput,
            Optimizer::Headroom,
        ),
        observation(
            "provider",
            Some(300),
            Some(250),
            &provider,
            Component::MessageInput,
            Optimizer::Headroom,
        ),
        whole,
    ]);

    let totals = aggregate_unique_events(&events);
    assert_eq!(totals.status, AggregateStatus::MixedBases);
    assert_eq!(totals.comparable_events, 4);
    assert_eq!(totals.excluded_events, 0);
    assert_eq!(totals.before_tokens, None);
    assert_eq!(totals.after_tokens, None);
    assert_eq!(totals.saved_tokens, None);
    assert_eq!(totals.saved_percent, None);
    assert_eq!(totals.groups.len(), 4);

    for (expected_basis, expected_component, before, after, saved) in [
        (&local_a, Component::MessageInput, 100, 80, 20),
        (&local_b, Component::MessageInput, 200, 100, 100),
        (&provider, Component::MessageInput, 300, 250, 50),
        (&local_a, Component::WholeInput, 400, 300, 100),
    ] {
        let group = totals
            .groups
            .iter()
            .find(|group| group.basis == *expected_basis && group.component == expected_component)
            .expect("each basis/component pair has its own group");
        assert_eq!(group.status, AggregateStatus::Measured);
        assert_eq!(group.events, 1);
        assert_eq!(group.before_tokens, Some(before));
        assert_eq!(group.after_tokens, Some(after));
        assert_eq!(group.saved_tokens, Some(saved));
    }
}

#[test]
fn zero_total_baseline_keeps_known_savings_but_has_no_percentage() {
    let basis = local_basis();
    let events = normalized(&[observation(
        "zero-baseline",
        Some(0),
        Some(7),
        &basis,
        Component::MessageInput,
        Optimizer::Headroom,
    )]);
    let totals = aggregate_unique_events(&events);
    assert_eq!(totals.status, AggregateStatus::Measured);
    assert_eq!(totals.before_tokens, Some(0));
    assert_eq!(totals.after_tokens, Some(7));
    assert_eq!(totals.saved_tokens, Some(-7));
    assert_eq!(totals.saved_percent, None);
}

#[test]
fn group_counter_overflow_fails_closed_and_mixed_groups_are_not_summed() {
    let local = local_basis();
    let group_overflow = normalized(&[
        observation(
            "max",
            Some(MAX_TOKEN_COUNT),
            Some(0),
            &local,
            Component::MessageInput,
            Optimizer::Headroom,
        ),
        observation(
            "one",
            Some(1),
            Some(0),
            &local,
            Component::MessageInput,
            Optimizer::Headroom,
        ),
    ]);
    let totals = aggregate_unique_events(&group_overflow);
    assert_eq!(totals.status, AggregateStatus::CounterOverflow);
    assert_eq!(totals.before_tokens, None);
    assert_eq!(totals.after_tokens, None);
    assert_eq!(totals.saved_tokens, None);
    assert_eq!(totals.saved_percent, None);
    assert_eq!(totals.groups.len(), 1);
    assert_eq!(totals.groups[0].status, AggregateStatus::CounterOverflow);
    assert_eq!(totals.groups[0].before_tokens, None);
    assert_eq!(totals.groups[0].after_tokens, None);
    assert_eq!(totals.groups[0].saved_tokens, None);
    assert_eq!(totals.groups[0].saved_percent, None);

    let other = basis(CounterKind::ProviderTokenizer, Some("provider-v1"));
    let period_overflow = normalized(&[
        observation(
            "local-max",
            Some(MAX_TOKEN_COUNT),
            Some(MAX_TOKEN_COUNT),
            &local,
            Component::MessageInput,
            Optimizer::Headroom,
        ),
        observation(
            "provider-one",
            Some(1),
            Some(1),
            &other,
            Component::MessageInput,
            Optimizer::Headroom,
        ),
    ]);
    let totals = aggregate_unique_events(&period_overflow);
    // Different basis groups are never added into a common counter total.
    assert_eq!(totals.status, AggregateStatus::MixedBases);
    assert_eq!(totals.before_tokens, None);
    assert_eq!(totals.after_tokens, None);
    assert_eq!(totals.saved_tokens, None);
    assert_eq!(totals.saved_percent, None);
    assert_eq!(totals.groups.len(), 2);
    assert!(totals
        .groups
        .iter()
        .all(|group| group.status == AggregateStatus::Measured));
}

#[test]
fn basis_group_response_is_bounded_without_inventing_an_overall_percentage() {
    let inputs: Vec<_> = (0..65)
        .map(|n| {
            observation(
                &format!("r{n}"),
                Some(100),
                Some(50),
                &basis(CounterKind::LocalEstimate, Some(&format!("tokenizer-{n}"))),
                Component::MessageInput,
                Optimizer::Headroom,
            )
        })
        .collect();
    let totals = aggregate_unique_events(&normalized(&inputs));
    assert_eq!(totals.status, AggregateStatus::MixedBases);
    assert_eq!(totals.comparable_events, 65);
    assert_eq!(totals.groups.len(), 64);
    assert_eq!(totals.omitted_basis_groups, 1);
    assert_eq!(totals.saved_percent, None);
}
