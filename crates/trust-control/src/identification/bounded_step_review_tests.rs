#![allow(clippy::float_cmp)] // The `TypeScript` compares grid values, means and frozen parameters exactly on purpose.

use trust_control_math::identify::SampleQuality;

use crate::identification::step_response::{qualify_step_response, SearchGrid};

use super::*;

// The review assertions of `control-studio-step-response.test.ts`. The `TypeScript` reaches the review through
// `IdentificationWorkflow`; these tests pass the review the input the workflow builds: the first fit names the
// estimation recording "estimation" and the workflow's limit of 0.15, and the validation fit names both recordings.

/// The settings of the bounded step tests (`settings`).
fn settings() -> StepResponseSettings {
    StepResponseSettings {
        baseline_duration_seconds: 5.0,
        final_duration_seconds: 5.0,
        maximum_window_range: 0.1,
        maximum_window_slope_per_second: 0.01,
        measurement_bound: 0.05,
        onset_threshold: 0.15,
        onset_samples: 3,
        delay: SearchGrid {
            minimum: 0.0,
            maximum: 2.0,
            step: 0.1,
        },
        time_constant: SearchGrid {
            minimum: 0.1,
            maximum: 20.0,
            step: 0.1,
        },
        baseline_step: 0.01,
        gain_step: 0.001,
        timestamp_shift_seconds: 0.0,
    }
}

/// A held step of `change` input units from 0 s to 40 s, sampled every 0.1 s from −5 s, with a first-order response of
/// gain 0.2 and time constant 5 s after a 1 s delay (`record`).
fn record(change: f64) -> Vec<IdentificationSample> {
    (0..452_u32)
        .map(|index| {
            let time = f64::from(index) / 10.0 - 5.0;
            IdentificationSample {
                time_seconds: time,
                input: if (0.0..40.0).contains(&time) {
                    75.0 + change
                } else {
                    75.0
                },
                output: if time <= 1.0 {
                    35.0
                } else {
                    35.0 + 0.2 * change * (1.0 - (-(time - 1.0) / 5.0).exp())
                },
                quality: SampleQuality::Good,
                input_unit: Some("%".to_string()),
                output_unit: Some("degC".to_string()),
                clipped: None,
                saturated: None,
            }
        })
        .collect()
}

/// The input of the first fit: the estimation recording alone.
fn estimation_input(estimation: Vec<IdentificationSample>) -> IdentificationReviewInput {
    IdentificationReviewInput {
        recording: None,
        step_response_settings: Some(settings()),
        first_order_step_response_settings: None,
        maximum_normalized_error: Some(0.15),
        estimation,
        validation: None,
        estimation_recording_id: Some("estimation".to_string()),
        validation_recording_id: None,
        excluded: Vec::new(),
        source: DataSource::Simulation,
        requested_kind: IdentificationModelSelection::Fopdt,
        declared_process: None,
        closed_loop: None,
    }
}

/// The input of the validation fit: the frozen estimation recording and an independent validation recording.
fn validation_input(
    estimation: Vec<IdentificationSample>,
    validation: Vec<IdentificationSample>,
) -> IdentificationReviewInput {
    IdentificationReviewInput {
        maximum_normalized_error: None,
        validation: Some(validation),
        validation_recording_id: Some("validation".to_string()),
        ..estimation_input(estimation)
    }
}

fn estimation_review(estimation: Vec<IdentificationSample>) -> ValidationReview {
    let review = prepare_bounded_step_review(&estimation_input(estimation))
        .expect("the estimation fit is reviewed");
    let IdentificationReview::ValidationRequired(review) = review else {
        panic!("the estimation fit still needs validation");
    };
    review
}

fn validation_review(input: &IdentificationReviewInput) -> AcceptanceReview {
    let review = prepare_bounded_step_review(input).expect("the validation fit is reviewed");
    let IdentificationReview::AcceptanceRequired(review) = review else {
        panic!("the validation fit awaits acceptance");
    };
    review
}

#[test]
fn retains_measured_windows_and_the_explicit_consistent_candidate_grid() {
    let data = record(10.0);
    assert!(qualify_step_response(&data, &settings()).is_ok());
    let review = estimation_review(data);
    let step = review
        .step_response
        .expect("the requested measured-step analysis must be retained");
    assert_eq!(step.baseline.mean, 35.0);
    assert!(step.final_window.mean > 36.99);
    assert_eq!(step.input_change, 10.0);
    assert!(step.consistency.count > 0);
    let gain = step
        .consistency
        .gain
        .expect("a consistent set has a gain range");
    assert!((gain.minimum..=gain.maximum).contains(&0.2));
    assert_eq!(step.best.delay_seconds, 1.0);
    assert_eq!(step.best.time_constant_seconds, 5.0);
}

#[test]
fn keeps_a_drifting_final_window_as_a_reported_partial_response_and_refuses_invalid_hold_samples() {
    // Only the baseline must be steady (N2.4); the final window need only lie inside the held step (N2.6). A final
    // window that is still moving is a partial response (A10.3): it is reported as unsteady, and its bounds do not
    // apply (N2.4).
    let mut drifting = record(10.0);
    for sample in &mut drifting {
        if sample.time_seconds >= 35.0 {
            sample.output += 0.03 * (sample.time_seconds - 35.0);
        }
    }
    assert!(
        qualify_step_response(&drifting, &settings()).is_ok(),
        "a drifting final window must not refuse the bounded step"
    );
    let measured =
        measure_step_response(&drifting, &settings()).expect("the drifting step is measured");
    assert!(
        measured.final_window.range > settings().maximum_window_range,
        "the drifting final window stays reported as unsteady"
    );
    assert!(
        prepare_bounded_step_review(&estimation_input(drifting)).is_ok(),
        "a partial response with a moving final window is still fitted (A10.3)"
    );

    let mut defective = record(10.0);
    defective[100].quality = SampleQuality::Bad;
    assert!(
        qualify_step_response(&defective, &settings()).is_err(),
        "a bad sample must prevent bounded step qualification"
    );
    assert!(prepare_bounded_step_review(&estimation_input(defective)).is_err());
}

#[test]
fn retains_rejected_independent_predictions_without_accepting_the_model() {
    let frozen = estimation_review(record(10.0)).selected;
    let holdout: Vec<IdentificationSample> = record(12.0)
        .into_iter()
        .map(|sample| IdentificationSample {
            output: 35.0 + (sample.output - 35.0) * 1.2,
            ..sample
        })
        .collect();
    assert!(qualify_step_response(&holdout, &settings()).is_ok());
    let review = validation_review(&validation_input(record(10.0), holdout));
    assert!(
        !review.fit.points.is_empty(),
        "a rejected independent prediction must remain inspectable"
    );
    assert!(!review.validation_qualification.suitable);
    let reason = review
        .validation_qualification
        .reason
        .as_deref()
        .unwrap_or_default();
    assert!(reason.contains("independent measurement bound"), "{reason}");
    assert_eq!(review.selected.gain, frozen.gain);
    assert_eq!(
        review.selected.time_constant_seconds,
        frozen.time_constant_seconds
    );
    assert_eq!(review.selected.dead_time_seconds, frozen.dead_time_seconds);
    assert!(review
        .fit
        .points
        .iter()
        .any(|point| point.residual.abs() > settings().measurement_bound));
}

#[test]
fn scores_a_changed_validation_step_with_frozen_estimation_parameters_before_explicit_acceptance() {
    let estimated = estimation_review(record(10.0));
    let validated = validation_review(&validation_input(record(10.0), record(12.0)));
    assert_eq!(
        validated.selected.gain, estimated.selected.gain,
        "independent validation must not refit the estimation gain"
    );
    let estimated_step = estimated
        .step_response
        .expect("the estimation analysis is retained");
    let validated_step = validated
        .step_response
        .expect("the estimation analysis is retained");
    assert_eq!(validated_step.best, estimated_step.best);
    assert_eq!(
        validated.selected.dead_time_seconds,
        estimated.selected.dead_time_seconds
    );
    assert!(
        validated.validation_qualification.suitable,
        "the frozen model meets the changed step, so it may be accepted"
    );
}

#[test]
fn keeps_the_frozen_estimation_grid_beside_the_measurement_of_the_plotted_validation_record() {
    let validated = validation_review(&validation_input(record(10.0), record(12.0)));
    assert_eq!(
        validated.fit.step_response.map(|step| step.input_change),
        Some(10.0),
        "the frozen estimation grid keeps its original source"
    );
    assert_eq!(
        validated
            .fit
            .step_response_measurement
            .map(|step| step.input_change),
        Some(12.0),
        "the plotted validation trace has its own measurement"
    );
}

#[test]
fn refuses_other_models_shared_recordings_and_unchanged_amplitudes() {
    let automatic = IdentificationReviewInput {
        requested_kind: IdentificationModelSelection::Automatic,
        ..estimation_input(record(10.0))
    };
    let refusal = prepare_bounded_step_review(&automatic).expect_err("only FOPDT is bounded");
    assert_eq!(
        refusal.0,
        "Bounded step fitting needs FOPDT and the complete recording."
    );
    let shared = IdentificationReviewInput {
        validation_recording_id: Some("estimation".to_string()),
        ..validation_input(record(10.0), record(12.0))
    };
    let refusal =
        prepare_bounded_step_review(&shared).expect_err("one recording cannot validate itself");
    assert_eq!(
        refusal.0,
        "Bounded step validation requires two distinct immutable recording identities."
    );
    let unchanged = validation_input(record(10.0), record(10.0));
    let refusal =
        prepare_bounded_step_review(&unchanged).expect_err("the same amplitude is refused");
    assert_eq!(
        refusal.0,
        "Use a distinct validation step with a different applied amplitude."
    );
}

/// The validation step of `record(change)` sampled every 0.4 s from −4.8 s to 43.2 s: the same ideal first-order response,
/// the step at 0 s, the restoration at 40 s and a dead time of 1 s, which is not a whole number of these samples.
fn sampled_validation(change: f64) -> Vec<IdentificationSample> {
    (0_u32..121)
        .map(|index| {
            let time = (f64::from(index) - 12.0) * 0.4;
            IdentificationSample {
                time_seconds: time,
                input: if (0.0..40.0).contains(&time) {
                    75.0 + change
                } else {
                    75.0
                },
                output: if time <= 1.0 {
                    35.0
                } else {
                    35.0 + 0.2 * change * (1.0 - (-(time - 1.0) / 5.0).exp())
                },
                quality: SampleQuality::Good,
                input_unit: Some("%".to_string()),
                output_unit: Some("degC".to_string()),
                clipped: None,
                saturated: None,
            }
        })
        .collect()
}

#[test]
fn a_validation_step_is_scored_under_the_recorded_input_on_its_own_sample_grid() {
    // N3.1: the frozen model is simulated from the validation's steady baseline under the recorded plant input, on the
    // recorded sample times (N1.2), not against an ideal step. The validation step is an ideal first-order response, which
    // the ideal prediction would match exactly. Sampled every 0.4 s, its 1 s dead time is not a whole number of samples, so
    // the simulated input is the latest recorded input at or before t − 1 s. The residuals are then not zero, the 0.05 bound
    // is exceeded, and the validation recording is unsuitable.
    let validated = validation_review(&validation_input(record(10.0), sampled_validation(12.0)));
    let rms = validated.fit.root_mean_square_error;
    assert!((rms - 0.024_473_409_108).abs() <= 1e-9, "rms {rms}");
    // N3.2: the normalized error divides the rms by the output span of the validation's fit record, from the start of its
    // baseline window to the end of its hold (2.398 934 73). The response it used to divide by is 2.398 282 16.
    let normalized = validated
        .selected
        .normalized_validation_error
        .expect("a validation error");
    assert!(
        (normalized - 0.010_201_781_964).abs() <= 1e-9,
        "normalized {normalized}"
    );
    assert!(!validated.validation_qualification.suitable);
    let reason = validated
        .validation_qualification
        .reason
        .as_deref()
        .unwrap_or_default();
    assert!(reason.contains("independent measurement bound"), "{reason}");
}

#[test]
fn a_best_fit_on_the_time_constant_grid_edge_is_kept_on_the_estimated_model() {
    // N2.4: the bounded grid's largest time constant is 4 s, below the true 5 s, so the best grid fit lies on the edge of its
    // grid, and the first-order-with-delay model keeps that search-boundary finding.
    let mut input = estimation_input(record(10.0));
    let mut narrow = settings();
    narrow.time_constant.maximum = 4.0;
    input.step_response_settings = Some(narrow);
    let review = prepare_bounded_step_review(&input).expect("the estimation fit is reviewed");
    let IdentificationReview::ValidationRequired(review) = review else {
        panic!("the estimation fit still needs validation");
    };
    assert!(review.selected.on_search_boundary, "{:?}", review.selected);
}

/// A held step of `change` input units from 0 s to 40 s with no dead time: a first-order response of gain 0.2 and time
/// constant 5 s that starts at the step (`immediate_record`).
fn immediate_record(change: f64) -> Vec<IdentificationSample> {
    record(change)
        .into_iter()
        .map(|sample| {
            let output = if sample.time_seconds <= 0.0 {
                35.0
            } else {
                35.0 + 0.2 * change * (1.0 - (-sample.time_seconds / 5.0).exp())
            };
            IdentificationSample { output, ..sample }
        })
        .collect()
}

/// The settings of a step with no dead time: the delay grid holds only zero, so the best delay is zero, a one-sided delay
/// that is no search boundary (`zero_delay_settings`).
fn zero_delay_settings() -> StepResponseSettings {
    StepResponseSettings {
        delay: SearchGrid {
            minimum: 0.0,
            maximum: 0.0,
            step: 0.1,
        },
        ..settings()
    }
}

#[test]
fn the_estimated_normalized_error_divides_by_the_span_of_the_fit_record_not_by_the_response() {
    // N3.2: the root mean square residual over the response divided by the output span of the fit record, from the start
    // of the steady baseline window to the end of the hold (N1.1). A ±0.05 ripple in the hold makes the residuals nonzero,
    // so the span (about 2.05) and the response (about 1.99) differ by far more than the tolerance.
    let rippled: Vec<IdentificationSample> = record(10.0)
        .into_iter()
        .enumerate()
        .map(|(index, sample)| {
            let held = (0.0..40.0).contains(&sample.time_seconds);
            let ripple = match (held, index % 2) {
                (false, _) => 0.0,
                (true, 0) => 0.05,
                (true, _) => -0.05,
            };
            IdentificationSample {
                output: sample.output + ripple,
                ..sample
            }
        })
        .collect();
    let review = estimation_review(rippled.clone());
    let analysis = review
        .step_response
        .expect("the estimation analysis is retained");
    let hold_count =
        f64::from(u32::try_from(analysis.points.len()).expect("the hold holds samples"));
    let rms = (analysis.best.squared_error / hold_count).sqrt();
    let fit_record: Vec<f64> = rippled
        .iter()
        .filter(|sample| (-5.0 - 1e-9..40.0).contains(&sample.time_seconds))
        .map(|sample| sample.output)
        .collect();
    let high = fit_record.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let low = fit_record.iter().copied().fold(f64::INFINITY, f64::min);
    let expected = rms / (high - low);
    let response = (analysis.final_window.mean - analysis.baseline.mean).abs();
    assert!(
        (review.selected.decisive_error() - expected).abs() <= 1e-12,
        "normalized {} against the span {expected}",
        review.selected.decisive_error()
    );
    assert!(
        (rms / response - expected).abs() > 1e-6,
        "the response must not be the divisor"
    );
}

#[test]
fn a_zero_best_delay_is_no_search_boundary_finding_on_the_estimated_model() {
    // N2.4: the best delay is zero, a one-sided delay, so the estimated model carries no search-boundary finding.
    let mut input = estimation_input(immediate_record(10.0));
    input.step_response_settings = Some(zero_delay_settings());
    let review = prepare_bounded_step_review(&input).expect("the estimation fit is reviewed");
    let IdentificationReview::ValidationRequired(review) = review else {
        panic!("the estimation fit still needs validation");
    };
    assert!(!review.selected.on_search_boundary, "{:?}", review.selected);
}

#[test]
fn a_bounded_estimate_whose_best_delay_is_zero_is_accepted_for_the_validated_review() {
    // N2.4: a best delay of zero is one-sided, so it refuses nothing. The estimation keeps its zero delay, and the frozen
    // model meets a changed validation step with the same zero delay, so the review reaches the acceptance the user gives.
    let mut input = validation_input(immediate_record(10.0), immediate_record(12.0));
    input.step_response_settings = Some(zero_delay_settings());
    let review = validation_review(&input);
    assert_eq!(
        review.selected.dead_time_seconds,
        Some(0.0),
        "{:?}",
        review.selected
    );
    assert!(!review.selected.on_search_boundary, "{:?}", review.selected);
    assert!(
        review.validation_qualification.suitable,
        "the frozen model meets the changed step, so it may be accepted"
    );
}
