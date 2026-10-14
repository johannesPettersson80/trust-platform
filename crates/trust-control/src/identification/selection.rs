//! The choice of the recommended low-order form (specification 35, B3.5 and A10.3; numerical annex N2.1, N2.3, N3.2 and
//! N3.3): one rule for every path, whether the candidates come from the single-run preview, the validated fit or the
//! excitation fit. The library fits the forms; this module decides which fitted form is recommended.

use trust_control_math::identify::LowOrderModelKind;

use crate::identification::plant::IdentifiedModel;
use crate::identification::texts::MINIMUM_REACHED_FRACTION;

/// A delay is resolved when it is longer than this many sample intervals of the fit (N3.3: `L > 2·Ts`).
const RESOLVED_DELAY_INTERVALS: f64 = 2.0;

/// The share of the first-order error that the delayed form's error must stay below to be preferred (N3.3: 98 %).
const RESOLVED_DELAY_ERROR_SHARE: f64 = 0.98;

/// One fitted form with the checks that the choice reads (N3.2, N2.3, N3.3).
#[derive(Debug, Clone, PartialEq)]
pub struct CheckedCandidate {
    /// The form: its gain, its dynamics, its decisive normalised error and its dead time.
    pub model: IdentifiedModel,
    /// The normalised error over the final `max(3, ⌈0.1 · N⌉)` samples of the record (N3.2).
    pub final_normalized_error: f64,
    /// The fraction of its fitted final change that the held step reached (N2.3); `None` where it is not defined.
    pub reached_fraction: Option<f64>,
    /// The sample interval of the fitted data in seconds: the resolution against which a delay is judged (N3.3).
    pub sample_interval_seconds: f64,
}

/// Whether a form's dynamics are stable (N2.1): a static gain and an integrator always are; a first-order form needs a
/// positive time constant, and a second-order form (item 5) needs positive time constants, which give `a1 > 0` and
/// `a2 > 0`.
#[must_use]
pub fn is_stable_form(
    kind: LowOrderModelKind,
    time_constant_seconds: Option<f64>,
    second_time_constant_seconds: Option<f64>,
) -> bool {
    match kind {
        LowOrderModelKind::StaticGain | LowOrderModelKind::Integrating => true,
        LowOrderModelKind::FirstOrder | LowOrderModelKind::FirstOrderDeadTime => {
            time_constant_seconds.is_some_and(|tau| tau > 0.0)
        }
        LowOrderModelKind::SecondOrder | LowOrderModelKind::SecondOrderDeadTime => {
            time_constant_seconds.is_some_and(|tau| tau > 0.0)
                && second_time_constant_seconds.is_some_and(|tau| tau > 0.0)
        }
    }
}

/// The recommended form among the fitted candidates (N3.3; `selectLowOrderModel` for every path).
///
/// A form is within the limit when its normalised error over the whole response and over its final set are each finite
/// and at most `maximum_normalized_error` (N3.2), and it must be stable (N2.1). It is acceptable when it is within the
/// limit and, where its fraction is defined, it reached 63 % of its fitted final change (N2.3, A10.3). The simplest
/// acceptable form is selected, by [`LowOrderModelKind::order`]; the one exception is the resolved delay: when the first
/// order and the first order with delay are both acceptable, the delayed form is selected if its delay is longer than two
/// sample intervals and its whole-response error is below 98 % of the first order's. A more accurate form is never ranked
/// ahead of a simpler acceptable one.
///
/// When no form is acceptable, the simplest form within the limit is returned, so that the caller's refusal names its
/// fraction and the hold it suggests (A10.3). `None` when no form is within the limit.
#[must_use]
pub fn select(
    candidates: &[CheckedCandidate],
    maximum_normalized_error: f64,
) -> Option<IdentifiedModel> {
    let within: Vec<&CheckedCandidate> = candidates
        .iter()
        .filter(|candidate| {
            candidate.model.stable
                && is_within_limit(candidate.model.decisive_error(), maximum_normalized_error)
                && is_within_limit(candidate.final_normalized_error, maximum_normalized_error)
        })
        .collect();
    if within.is_empty() {
        return None;
    }
    let acceptable: Vec<&CheckedCandidate> = within
        .iter()
        .copied()
        .filter(|candidate| {
            candidate
                .reached_fraction
                .is_none_or(|fraction| fraction >= MINIMUM_REACHED_FRACTION)
        })
        .collect();
    if acceptable.is_empty() {
        return simplest(&within).map(|candidate| candidate.model.clone());
    }
    let first_order = acceptable
        .iter()
        .copied()
        .find(|candidate| candidate.model.kind == LowOrderModelKind::FirstOrder);
    let delayed = acceptable
        .iter()
        .copied()
        .find(|candidate| candidate.model.kind == LowOrderModelKind::FirstOrderDeadTime);
    if let (Some(first_order), Some(delayed)) = (first_order, delayed) {
        if resolves_delay(delayed, first_order) {
            return Some(delayed.model.clone());
        }
    }
    simplest(&acceptable).map(|candidate| candidate.model.clone())
}

/// Whether a normalised error is finite and at most the limit (N3.2).
fn is_within_limit(error: f64, limit: f64) -> bool {
    error.is_finite() && error <= limit
}

/// The simplest of the candidates by the rank of N2.1. Kinds are unique among the candidates, so no two share a rank.
fn simplest<'a>(candidates: &[&'a CheckedCandidate]) -> Option<&'a CheckedCandidate> {
    candidates
        .iter()
        .copied()
        .min_by_key(|candidate| candidate.model.kind.order())
}

/// The resolved-delay rule (N3.3): the first order with delay is preferred to the first order when its fitted delay is
/// longer than two sample intervals and its whole-response error is below 98 % of the first order's error.
fn resolves_delay(delayed: &CheckedCandidate, first_order: &CheckedCandidate) -> bool {
    let delay = delayed.model.dead_time_seconds.unwrap_or(0.0);
    delay > RESOLVED_DELAY_INTERVALS * delayed.sample_interval_seconds
        && delayed.model.decisive_error()
            < RESOLVED_DELAY_ERROR_SHARE * first_order.model.decisive_error()
}

#[cfg(test)]
#[path = "selection_tests.rs"]
mod tests;
