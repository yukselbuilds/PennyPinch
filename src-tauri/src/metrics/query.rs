use super::normalize::{Basis, Component, NormalizedEvent, MAX_TOKEN_COUNT};
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AggregateStatus {
    Measured,
    NoComparableEvents,
    MixedBases,
    CounterOverflow,
}

#[derive(Clone, Debug, Serialize)]
pub struct BasisTotal {
    pub basis: Basis,
    pub component: Component,
    pub status: AggregateStatus,
    pub events: u64,
    pub before_tokens: Option<u64>,
    pub after_tokens: Option<u64>,
    pub saved_tokens: Option<i64>,
    pub saved_percent: Option<f64>,
}

#[derive(Clone, Debug, Serialize)]
pub struct PeriodTotals {
    pub status: AggregateStatus,
    pub comparable_events: u64,
    pub excluded_events: u64,
    pub before_tokens: Option<u64>,
    pub after_tokens: Option<u64>,
    pub saved_tokens: Option<i64>,
    pub saved_percent: Option<f64>,
    pub groups: Vec<BasisTotal>,
    pub omitted_basis_groups: u64,
}

/// Bounded summary for per-request projections; raw basis labels never leave this DTO.
#[derive(Debug, Serialize)]
pub struct MetricSummary {
    pub status: AggregateStatus,
    pub comparable_events: u64,
    pub excluded_events: u64,
    pub before_tokens: Option<u64>,
    pub after_tokens: Option<u64>,
    pub saved_tokens: Option<i64>,
    pub saved_percent: Option<f64>,
}
impl From<PeriodTotals> for MetricSummary {
    fn from(v: PeriodTotals) -> Self {
        Self {
            status: v.status,
            comparable_events: v.comparable_events,
            excluded_events: v.excluded_events,
            before_tokens: v.before_tokens,
            after_tokens: v.after_tokens,
            saved_tokens: v.saved_tokens,
            saved_percent: v.saved_percent,
        }
    }
}

/// Input is the unique, validated event set selected by the metrics store.
pub fn aggregate_unique_events(events: &[NormalizedEvent]) -> PeriodTotals {
    aggregate_event_refs(events.iter())
}

pub(crate) fn aggregate_event_refs<'a>(
    events: impl IntoIterator<Item = &'a NormalizedEvent>,
) -> PeriodTotals {
    let mut sums: BTreeMap<(Basis, Component), (u128, u128, u64)> = BTreeMap::new();
    let mut comparable_events = 0;
    let mut event_count = 0;
    for event in events {
        event_count += 1;
        if !event.main_metric_eligible {
            continue;
        }
        let before = event.before.tokens.expect("validated measured input");
        let after = event.after.tokens.expect("validated measured input");
        comparable_events += 1;
        let group = sums
            .entry((event.before.basis.clone(), event.before.component))
            .or_default();
        group.0 += before as u128;
        group.1 += after as u128;
        group.2 += 1;
    }
    let groups: Vec<_> = sums
        .into_iter()
        .map(|((basis, component), (before, after, events))| {
            if before > MAX_TOKEN_COUNT as u128 || after > MAX_TOKEN_COUNT as u128 {
                BasisTotal {
                    basis,
                    component,
                    events,
                    status: AggregateStatus::CounterOverflow,
                    before_tokens: None,
                    after_tokens: None,
                    saved_tokens: None,
                    saved_percent: None,
                }
            } else {
                let saved = before as i64 - after as i64;
                BasisTotal {
                    basis,
                    component,
                    events,
                    status: AggregateStatus::Measured,
                    before_tokens: Some(before as u64),
                    after_tokens: Some(after as u64),
                    saved_tokens: Some(saved),
                    saved_percent: (before > 0).then(|| 100.0 * saved as f64 / before as f64),
                }
            }
        })
        .collect();
    let mut result = PeriodTotals {
        status: AggregateStatus::NoComparableEvents,
        comparable_events,
        excluded_events: event_count - comparable_events,
        before_tokens: None,
        after_tokens: None,
        saved_tokens: None,
        saved_percent: None,
        groups,
        omitted_basis_groups: 0,
    };
    if let [group] = result.groups.as_slice() {
        result.status = group.status;
        result.before_tokens = group.before_tokens;
        result.after_tokens = group.after_tokens;
        result.saved_tokens = group.saved_tokens;
        result.saved_percent = group.saved_percent;
    } else if result.groups.len() > 1 {
        result.status = AggregateStatus::MixedBases;
    }
    result.omitted_basis_groups = result.groups.len().saturating_sub(64) as u64;
    result.groups.truncate(64);
    result
}
