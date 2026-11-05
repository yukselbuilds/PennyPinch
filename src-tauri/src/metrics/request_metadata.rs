//! Only bounded, content-free fields cross the collector and persistence boundary.
use super::normalize::NormalizeError;
use serde::{de::IgnoredAny, Deserialize, Deserializer, Serialize};

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    #[default]
    Unknown,
    NoErrorReported,
    ErrorReported,
}
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RequestMetadata {
    pub total_ms: Option<f64>,
    pub optimization_ms: Option<f64>,
    pub first_token_ms: Option<f64>,
    pub outcome: Outcome,
    pub cache_reported_hit: Option<bool>,
    pub transforms_count: Option<u16>,
    pub conflicted: bool,
}
pub(crate) fn duration(value: Option<f64>) -> Option<f64> {
    value.filter(|n| n.is_finite() && (0.0..=86_400_000.0).contains(n))
}
impl RequestMetadata {
    pub fn is_unknown(&self) -> bool {
        self == &Self::default()
    }
    pub(crate) fn validate(&self) -> Result<(), NormalizeError> {
        if [self.total_ms, self.optimization_ms, self.first_token_ms]
            .into_iter()
            .any(|v| v != duration(v))
            || self.transforms_count.is_some_and(|n| n > 128)
            || self.conflicted
                && self
                    != &(Self {
                        conflicted: true,
                        ..Self::default()
                    })
        {
            return Err(NormalizeError::InvalidPayload);
        }
        Ok(())
    }
    pub(crate) fn merge(&mut self, incoming: &Self) {
        if self.conflicted {
            return;
        }
        fn enrich<T: Copy + PartialEq>(a: &mut Option<T>, b: Option<T>) -> bool {
            match (*a, b) {
                (Some(x), Some(y)) if x != y => false,
                (None, Some(_)) => {
                    *a = b;
                    true
                }
                _ => true,
            }
        }
        let mut compatible = !incoming.conflicted
            && enrich(&mut self.total_ms, incoming.total_ms)
            && enrich(&mut self.optimization_ms, incoming.optimization_ms)
            && enrich(&mut self.first_token_ms, incoming.first_token_ms)
            && enrich(&mut self.cache_reported_hit, incoming.cache_reported_hit)
            && enrich(&mut self.transforms_count, incoming.transforms_count);
        if self.outcome == Outcome::Unknown {
            self.outcome = incoming.outcome;
        } else if incoming.outcome != Outcome::Unknown && self.outcome != incoming.outcome {
            compatible = false;
        }
        if !compatible {
            *self = Self {
                conflicted: true,
                ..Self::default()
            };
        }
    }
}

/// Missing, null and non-null are distinct. The error value itself is never allocated.
impl<'de> Deserialize<'de> for ErrorPresence {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Ok(Self(if Option::<IgnoredAny>::deserialize(d)?.is_some() {
            Outcome::ErrorReported
        } else {
            Outcome::NoErrorReported
        }))
    }
}
#[derive(Default)]
pub(crate) struct ErrorPresence(pub Outcome);
#[derive(Default)]
pub(crate) struct TransformCount(pub Option<u16>);
impl<'de> Deserialize<'de> for TransformCount {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct Count;
        impl<'de> serde::de::Visitor<'de> for Count {
            type Value = TransformCount;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a transform list or null")
            }
            fn visit_none<E: serde::de::Error>(self) -> Result<Self::Value, E> {
                Ok(TransformCount(None))
            }
            fn visit_unit<E: serde::de::Error>(self) -> Result<Self::Value, E> {
                self.visit_none()
            }
            fn visit_some<D: Deserializer<'de>>(self, d: D) -> Result<Self::Value, D::Error> {
                d.deserialize_seq(self)
            }
            fn visit_seq<A: serde::de::SeqAccess<'de>>(
                self,
                mut seq: A,
            ) -> Result<Self::Value, A::Error> {
                let mut count = 0usize;
                while seq.next_element::<IgnoredAny>()?.is_some() {
                    count += 1;
                }
                Ok(TransformCount((count <= 128).then_some(count as u16)))
            }
        }
        d.deserialize_option(Count)
    }
}
