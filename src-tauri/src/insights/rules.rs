//! Advice is computed from recorded metadata and never changes the runtime.
use crate::metrics::{
    attribution::Client,
    normalize::{Component, CounterKind, NormalizedEvent, Optimizer, Source},
    query::{AggregateStatus, PeriodTotals},
    request_metadata::Outcome,
    store::PeriodSelection,
};
use chrono::{DateTime, Duration, Utc};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

const WINDOW_MINUTES: i64 = 15;
const MIN_SAMPLES: usize = 20;
const SLOW_P95_MS: f64 = 1000.0;

#[derive(Debug, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Recommendation {
    InputGrowth {
        basis: CounterKind,
        component: Component,
        before_tokens: u64,
        after_tokens: u64,
        events: u64,
    },
    SlowOptimization {
        p95_ms: f64,
        requests: usize,
    },
    CacheShareDrop {
        model: String,
        provider: String,
        previous_hits: usize,
        previous_requests: usize,
        recent_hits: usize,
        recent_requests: usize,
    },
    InstallationStale {
        client: Client,
    },
}

pub(crate) fn installation_checks<'a>(
    observations: impl IntoIterator<Item = (&'a str, &'a str)>,
) -> impl Iterator<Item = Recommendation> {
    observations
        .into_iter()
        .filter_map(|(client, freshness)| match (client, freshness) {
            ("codex", "stale") => Some(Client::Codex),
            ("claude_code", "stale") => Some(Client::ClaudeCode),
            ("opencode", "stale") => Some(Client::OpenCode),
            _ => None,
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .map(|client| Recommendation::InstallationStale { client })
}

pub(crate) fn for_period(
    selection: &PeriodSelection,
    totals: &PeriodTotals,
    recording_generation: Option<&str>,
) -> Vec<Recommendation> {
    let mut advice = Vec::new();
    if let (Some(before), Some(after), [group]) = (
        totals.before_tokens,
        totals.after_tokens,
        totals.groups.as_slice(),
    ) {
        if totals.status == AggregateStatus::Measured
            && before > 0
            && after > before
            && selection.last_event.is_some()
            && selection.coverage.unplaced_events == 0
        {
            advice.push(Recommendation::InputGrowth {
                basis: group.basis.kind,
                component: group.component,
                before_tokens: before,
                after_tokens: after,
                events: totals.comparable_events,
            });
        }
    }

    let Some(generation) = recording_generation.and_then(|value| Uuid::parse_str(value).ok())
    else {
        return advice;
    };
    let generation = generation.to_string();
    let window = Duration::minutes(WINDOW_MINUTES);
    let split = selection.now - window;
    let start = split - window;
    let mut unique = BTreeMap::new();
    for event in &selection.events {
        if event.runtime_generation != generation
            || event.optimizer != Optimizer::Headroom
            || !event.observed_via.contains(&Source::Proxy)
        {
            continue;
        }
        let Some(at) = event
            .timestamp_utc
            .as_deref()
            .and_then(|stamp| stamp.parse::<DateTime<Utc>>().ok())
            .filter(|stamp| *stamp > start && *stamp <= selection.now)
        else {
            continue;
        };
        unique
            .entry(event.origin_id.as_str())
            .and_modify(|sample| *sample = None)
            .or_insert(Some((at, event)));
    }
    let samples: Vec<_> = unique
        .into_values()
        .flatten()
        .filter(|(_, event)| {
            !event.metadata.conflicted && event.metadata.outcome == Outcome::NoErrorReported
        })
        .collect();
    let mut durations: Vec<_> = samples
        .iter()
        .filter(|(at, _)| *at > split)
        .filter_map(|(_, event)| event.metadata.optimization_ms.filter(|ms| *ms > 0.0))
        .collect();
    let requests = durations.len();
    if requests >= MIN_SAMPLES {
        let rank = (requests * 95).div_ceil(100) - 1;
        let (_, p95, _) = durations.select_nth_unstable_by(rank, f64::total_cmp);
        if *p95 >= SLOW_P95_MS {
            advice.push(Recommendation::SlowOptimization {
                p95_ms: *p95,
                requests,
            });
        }
    }
    advice.extend(cache_drop(&samples, split));
    advice
}

fn cache_scope(event: &NormalizedEvent) -> Option<(&str, &str, Option<(Client, &str)>)> {
    let attribution = &event.attribution;
    if attribution.conflicted {
        return None;
    }
    Some((
        attribution.model.as_deref()?,
        attribution.provider.as_deref()?,
        attribution
            .route
            .as_ref()
            .map(|route| (route.client, route.upstream_id.as_str())),
    ))
}

fn cache_drop(
    samples: &[(DateTime<Utc>, &NormalizedEvent)],
    split: DateTime<Utc>,
) -> Option<Recommendation> {
    let scope = samples
        .iter()
        .filter(|(at, event)| *at > split && event.metadata.cache_reported_hit.is_some())
        .filter_map(|(at, event)| Some((*at, cache_scope(event)?)))
        .max()?
        .1;
    let mut previous = (0, 0);
    let mut recent = (0, 0);
    for (at, event) in samples {
        if cache_scope(event) != Some(scope) {
            continue;
        }
        if let Some(hit) = event.metadata.cache_reported_hit {
            let counts = if *at > split {
                &mut recent
            } else {
                &mut previous
            };
            counts.0 += usize::from(hit);
            counts.1 += 1;
        }
    }
    let (previous_hits, previous_requests) = previous;
    let (recent_hits, recent_requests) = recent;
    if previous_requests < MIN_SAMPLES || recent_requests < MIN_SAMPLES {
        return None;
    }
    // Compare shares exactly at a 20 percentage point threshold.
    let previous_scaled = previous_hits as u64 * recent_requests as u64;
    let recent_scaled = recent_hits as u64 * previous_requests as u64;
    if 5 * previous_scaled < 5 * recent_scaled + previous_requests as u64 * recent_requests as u64 {
        return None;
    }
    Some(Recommendation::CacheShareDrop {
        model: scope.0.to_owned(),
        provider: scope.1.to_owned(),
        previous_hits,
        previous_requests,
        recent_hits,
        recent_requests,
    })
}

#[cfg(test)]
#[path = "rules_tests.rs"]
mod tests;
