#![allow(clippy::float_cmp, clippy::cast_precision_loss)] // The statuses and the final-window starts are compared exactly.

use super::*;

/// Twenty points on a 0-to-1 measured ramp, each with the same residual (the `TypeScript` `points` helper).
fn points(residuals: &[f64]) -> Vec<ResponsePoint> {
    residuals
        .iter()
        .enumerate()
        .map(|(index, &residual)| ResponsePoint {
            measured: index as f64 / (residuals.len() - 1) as f64,
            residual,
        })
        .collect()
}

#[test]
fn a_response_that_agrees_everywhere_passes() {
    let score = score_independent_response(&points(&[0.05; 20]), 0.15);
    assert_eq!(score.status, ScoreStatus::Pass);
    assert!((score.normalized_error - 0.05).abs() <= 1e-12);
}

#[test]
fn a_response_that_misses_the_whole_response_fails() {
    let score = score_independent_response(&points(&[0.3; 20]), 0.15);
    assert_eq!(score.status, ScoreStatus::Fail);
    assert!(!score.whole_within_limit);
}

#[test]
fn a_response_that_fails_only_its_final_tenth_fails_the_check() {
    // Eighteen agreeing points, then two that miss: the whole response is within the limit, the final 10 % is not.
    let mut residuals = vec![0.0; 18];
    residuals.extend([0.3, 0.3]);
    let score = score_independent_response(&points(&residuals), 0.15);
    assert!(
        score.whole_within_limit,
        "the whole response passes: {}",
        score.normalized_error
    );
    assert!(
        !score.final_within_limit,
        "the final 10 % fails: {}",
        score.final_normalized_error
    );
    assert_eq!(score.status, ScoreStatus::Fail);
}

#[test]
fn the_final_set_holds_the_last_tenth_and_at_least_three_points() {
    // Specification N3.2: the final set holds max(3, ⌈0.1·N⌉) samples; the TypeScript used max(1, …). Twenty points
    // give the last three (⌈2⌉ < 3), five give three, thirty-one give the last four (⌈3.1⌉ = 4), and one gives all.
    assert_eq!(
        [
            final_tenth_start(20),
            final_tenth_start(5),
            final_tenth_start(31),
            final_tenth_start(1)
        ],
        [17, 2, 27, 0]
    );
    assert_eq!(final_tenth_start(0), 0, "no points give an empty final set");
}

#[test]
fn a_flat_measurement_has_no_span_so_its_check_fails() {
    // Specification N1.1 and N3.2: the error is divided by the plain measured span, with no floor. All measured values
    // are equal, so the span is zero, the normalised error is not finite, and the check fails.
    let flat: Vec<ResponsePoint> = (0..4)
        .map(|_| ResponsePoint {
            measured: 5.0,
            residual: 1e-9,
        })
        .collect();
    let score = score_independent_response(&flat, 2.0);
    assert!(!score.normalized_error.is_finite());
    assert_eq!(score.status, ScoreStatus::Fail);
}
