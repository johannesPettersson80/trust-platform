#![allow(clippy::float_cmp)] // Integrator predictions, the dead time and the operating range are exact, compared exactly on purpose.

use super::*;
use crate::identification::data::tests::{fopdt_samples, good_sample};
use crate::identification::data::DataSource;
use crate::identification::plant::{identify_plant, IdentifyPlantOptions};

/// A stable model of `kind` with `gain`, without time constants, dead time, residuals or uncertainty, and with no error.
pub(crate) fn fitted_model(kind: LowOrderModelKind, gain: f64) -> IdentifiedModel {
    IdentifiedModel {
        kind,
        gain,
        time_constant_seconds: None,
        second_time_constant_seconds: None,
        dead_time_seconds: None,
        normalized_error: Some(0.0),
        normalized_estimation_error: None,
        normalized_validation_error: None,
        estimation_residuals: None,
        validation_residuals: None,
        uncertainty: None,
        stable: true,
        on_search_boundary: false,
    }
}

/// The options of an open-loop fit without a declared process: the `TypeScript` default `{}`.
pub(crate) fn open_loop() -> IdentifyPlantOptions {
    IdentifyPlantOptions {
        declared_process: None,
        closed_loop: false,
        pulse: false,
    }
}

/// Samples one second apart whose input steps to one at 2 s and is restored at 4 s, with the given outputs.
fn stepped(outputs: &[f64]) -> Vec<IdentificationSample> {
    outputs
        .iter()
        .zip(0_u32..)
        .map(|(&output, index)| {
            let input = if (2..4).contains(&index) { 1.0 } else { 0.0 };
            good_sample(f64::from(index), input, output)
        })
        .collect()
}

// control-studio-identify-tune.test.ts: "reconstructs step and restoration superposition for first-order and
// integrating fits".
#[test]
fn reconstructs_step_and_restoration_superposition_for_first_order_and_integrating_fits() {
    let first_order = model_fit_evidence(
        &IdentifiedModel {
            time_constant_seconds: Some(1.0),
            ..fitted_model(LowOrderModelKind::FirstOrder, 1.0)
        },
        &stepped(&[0.0, 0.0, 0.0, 0.6321, 0.8647, 0.3181]),
    )
    .expect("the record has a measured step");
    assert!(
        first_order.points[4].predicted > 0.8,
        "restoring the input must not erase the held first-order response"
    );
    let expected = 1.0 - (-3.0_f64).exp() - (1.0 - (-1.0_f64).exp());
    assert!((first_order.points[5].predicted - expected).abs() < 0.01);
    let integrating = model_fit_evidence(
        &fitted_model(LowOrderModelKind::Integrating, 1.0),
        &stepped(&[0.0, 0.0, 0.0, 1.0, 2.0, 2.0]),
    )
    .expect("the record has a measured step");
    assert_eq!(integrating.points[4].predicted, 2.0);
    assert_eq!(integrating.points[5].predicted, 2.0);
}

// control-studio-identify-tune.test.ts: "retains measured, predicted, residual, range, delay, units, and warnings as
// model evidence".
#[test]
fn retains_measured_predicted_residual_range_delay_units_and_warnings_as_model_evidence() {
    let samples: Vec<IdentificationSample> = fopdt_samples(2.0)
        .into_iter()
        .map(|sample| IdentificationSample {
            input_unit: Some("%".to_string()),
            output_unit: Some("degC".to_string()),
            ..sample
        })
        .collect();
    let identified = identify_plant(
        &samples,
        &[],
        DataSource::Simulation,
        None,
        0.35,
        &open_loop(),
    )
    .expect("the step identifies a model");
    let evidence = model_fit_evidence(&identified.recommended, &samples)
        .expect("the record has a measured step");
    assert_eq!(evidence.points.len(), samples.len());
    assert!(evidence
        .points
        .iter()
        .all(|point| point.predicted.is_finite() && point.residual.is_finite()));
    assert!(evidence.root_mean_square_error >= 0.0);
    assert_eq!(evidence.output_unit, "degC");
    assert!(evidence.stable);
    assert!(evidence.delay_seconds >= 0.0);
    let last = samples.last().map_or(f64::NAN, |sample| sample.output);
    assert_eq!(
        evidence.operating_range,
        OperatingRange {
            input_minimum: 0.0,
            input_maximum: 2.0,
            output_minimum: 0.0,
            output_maximum: last,
        }
    );
}

#[test]
fn warns_of_instability_error_fraction_reached_and_an_unresolved_dead_time_in_that_order() {
    let model = IdentifiedModel {
        time_constant_seconds: Some(1.0),
        dead_time_seconds: Some(0.5),
        normalized_error: Some(0.3),
        stable: false,
        ..fitted_model(LowOrderModelKind::FirstOrderDeadTime, 1.0)
    };
    let evidence = model_fit_evidence(&model, &stepped(&[0.0, 0.0, 0.0, 0.6321, 0.8647, 0.3181]))
        .expect("the record has a measured step");
    // The held part ends at the restoration, so its outputs from the step are 0 and 0.6321. The tail mean of N2.3
    // takes at least three samples, here both of them.
    let fraction = evidence
        .final_change_fraction
        .expect("a held step has a fraction reached");
    assert!((fraction - 0.31605).abs() < 1e-12, "{fraction}");
    assert_eq!(
        evidence.warnings,
        [
            "The fitted model is not stable.",
            "Prediction error is high for controller design.",
            "The response reached 32 % of its fitted final change.",
            "Dead time is shorter than one sample interval and is weakly resolved.",
        ]
    );
    assert!(!evidence.stable);
    assert_eq!(evidence.delay_seconds, 0.5);
    let integrating = model_fit_evidence(
        &fitted_model(LowOrderModelKind::Integrating, 1.0),
        &stepped(&[0.0, 0.0, 0.0, 1.0, 2.0, 2.0]),
    )
    .expect("the record has a measured step");
    assert_eq!(
        integrating.final_change_fraction, None,
        "an integrating form has no final change"
    );
    assert_eq!(integrating.warnings, [] as [String; 0]);
}

// Specification 35, B3.3: the page shows the fit's largest absolute residual as the engine states it and computes none.
#[test]
fn the_largest_absolute_residual_is_the_largest_magnitude_of_the_residuals_and_absent_without_samples(
) {
    // The integrating prediction is 0, 0, 0, 1, 2, 2, so the measured 1.5 lies 0.5 below it: a residual of -0.5, the only one.
    let evidence = model_fit_evidence(
        &fitted_model(LowOrderModelKind::Integrating, 1.0),
        &stepped(&[0.0, 0.0, 0.0, 1.0, 1.5, 2.0]),
    )
    .expect("the record has a measured step");
    assert_eq!(evidence.points[4].residual, -0.5);
    assert_eq!(evidence.maximum_absolute_residual(), Some(0.5));
    let without_samples = ModelFitEvidence {
        points: Vec::new(),
        ..evidence
    };
    assert_eq!(without_samples.maximum_absolute_residual(), None);
}
