//! The pass rule of an independent model check (B5.2, N3.5): the frozen model's free-running prediction must agree with
//! the recorded measured output within the error limit over the whole response, and over its final set (N3.2). Each error
//! is the root-mean-square residual divided by the span of the measured output (`independentModelScore.ts`). The
//! reason sentences of a score belong to the product, not to this library.

// `IndependentResponseScore` and `ScoreStatus` name the module's subject.
#![allow(clippy::module_name_repetitions)]

use super::count_f64;
use crate::sampling::{largest, smallest};

/// The fewest points of the final set, whatever the response's length (N3.2).
const MINIMUM_FINAL_POINTS: usize = 3;

/// One point of an independent response: the measured output and the residual of the frozen prediction there.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ResponsePoint {
    /// The measured output at the point.
    pub measured: f64,
    /// The residual `yₖ − ŷₖ` of the frozen model at the point.
    pub residual: f64,
}

/// The verdict of an independent check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScoreStatus {
    /// Both the whole response and its final set are within the limit.
    Pass,
    /// The whole response or its final set is outside the limit, or an error is not finite.
    Fail,
}

/// The errors of an independent check and whether each is within the limit.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct IndependentResponseScore {
    /// The normalised error over the whole response, `√(Σeₖ²/n)/span`.
    pub normalized_error: f64,
    /// The normalised error over the final set of the points (N3.2).
    pub final_normalized_error: f64,
    /// Whether the whole-response error is finite and not above the limit.
    pub whole_within_limit: bool,
    /// Whether the final-window error is finite and not above the limit.
    pub final_within_limit: bool,
    /// Pass when both errors are within the limit.
    pub status: ScoreStatus,
}

/// The first index of the final set of `count` points (N3.2): the final set holds the last `max(3, ⌈0.1 · count⌉)`
/// points, or all of them when there are fewer. The index is zero for no points and for fewer than three.
#[must_use]
pub fn final_tenth_start(count: usize) -> usize {
    count.saturating_sub(count.div_ceil(10).max(MINIMUM_FINAL_POINTS))
}

/// Scores an independent response against an error limit (B5.2): the normalised error over the whole response and
/// over its final set (N3.2). Each error is divided by the plain span of the measured outputs (N1.1, no floor): a flat
/// response has no span, so its errors are not finite and the check fails. The caller refuses a flat output before it
/// scores (N3.2).
#[must_use]
pub fn score_independent_response(
    points: &[ResponsePoint],
    limit: f64,
) -> IndependentResponseScore {
    let measured: Vec<f64> = points.iter().map(|point| point.measured).collect();
    let span = largest(&measured, f64::NEG_INFINITY) - smallest(&measured);
    let normalized = |part: &[ResponsePoint]| -> f64 {
        let squares: f64 = part
            .iter()
            .map(|point| point.residual * point.residual)
            .sum();
        (squares / count_f64(part.len())).sqrt() / span
    };
    let normalized_error = normalized(points);
    let final_normalized_error = normalized(&points[final_tenth_start(points.len())..]);
    let whole_within_limit = normalized_error.is_finite() && normalized_error <= limit;
    let final_within_limit = final_normalized_error.is_finite() && final_normalized_error <= limit;
    IndependentResponseScore {
        normalized_error,
        final_normalized_error,
        whole_within_limit,
        final_within_limit,
        status: if whole_within_limit && final_within_limit {
            ScoreStatus::Pass
        } else {
            ScoreStatus::Fail
        },
    }
}

#[cfg(test)]
mod tests;
