#![allow(clippy::float_cmp)] // The window and band edges are compared with tolerances; the enum variants and the counts are exact.

use std::f64::consts::PI;

use trust_control_math::identify::{LowOrderModelKind, ParameterInterval};
use trust_control_math::margins::MarginValue;

use super::*;
use crate::identification::plant::{IdentifiedUncertainty, UncertaintyMethod};
use crate::limits::FIT_POINT_LIMIT;

fn model(
    kind: LowOrderModelKind,
    gain: f64,
    time: Option<f64>,
    dead_time: Option<f64>,
) -> IdentifiedModel {
    IdentifiedModel {
        kind,
        gain,
        time_constant_seconds: time,
        second_time_constant_seconds: None,
        dead_time_seconds: dead_time,
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

fn conditions(period_seconds: f64, duration_seconds: f64) -> PredictionConditions {
    PredictionConditions {
        period_seconds,
        duration_seconds,
        reference_change: 1.0,
        operating_input: 0.0,
        operating_output: 0.0,
        input_disturbance: 0.0,
        output_minimum: -20.0,
        output_maximum: 20.0,
        rate_minimum: -1000.0,
        rate_maximum: 1000.0,
    }
}

fn units() -> SeriesUnits {
    SeriesUnits {
        process_value: "m".to_string(),
        manipulated: "V".to_string(),
    }
}

/// The plots of one model with its current and candidate settings, on the given conditions.
fn plots_of(
    model: &IdentifiedModel,
    current: &PredictedGains,
    candidate: &PredictedGains,
    conditions: &PredictionConditions,
) -> Result<EngineeringPlots, Refusal> {
    let units = units();
    build_engineering_plots(&PlotInputs {
        model,
        current: Some(current),
        candidate,
        conditions,
        units: &units,
        requirement: None,
        sweep: None,
    })
}

fn sorted_poles(poles: &[ComplexValue]) -> Vec<(f64, f64)> {
    let mut pairs: Vec<(f64, f64)> = poles
        .iter()
        .map(|pole| (pole.real, pole.imaginary))
        .collect();
    pairs.sort_by(|left, right| left.0.total_cmp(&right.0).then(left.1.total_cmp(&right.1)));
    pairs
}

#[test]
fn the_root_locus_of_the_integrating_shaft_sweeps_the_proportional_gain() {
    // identify-tune and N7.5: K/(s(s + 1)) with a measured-velocity gain Kd = 7 gives the characteristic polynomial
    // s² + 8s + Kp. Kp = 12 gives −2 and −6, Kp = 16 gives −4 twice, and Kp = 20 gives −4 ± 2j.
    let shaft = model(LowOrderModelKind::Integrating, 1.0, Some(1.0), None);
    let candidate = PredictedGains {
        derivative_gain: 7.0,
        ..PredictedGains::parallel(12.0, 0.0, 0.0)
    };
    let sweep = RootLocusSweep {
        parameter: RootLocusParameter::ProportionalGain,
        values: vec![12.0, 16.0, 20.0],
        selected_value: 16.0,
    };
    let units = units();
    let settings = conditions(0.01, 10.0);
    let plots = build_engineering_plots(&PlotInputs {
        model: &shaft,
        current: None,
        candidate: &candidate,
        conditions: &settings,
        units: &units,
        requirement: None,
        sweep: Some(&sweep),
    })
    .expect("the shaft and the sweep are valid");
    match plots.root_locus {
        RootLocus::Qualified {
            parameter,
            selected_value,
            fixed_parameters,
            branches,
            ..
        } => {
            assert_eq!(parameter, RootLocusParameter::ProportionalGain);
            assert_eq!(selected_value, 16.0);
            assert_eq!(fixed_parameters.derivative_gain, Some(7.0));
            assert_eq!(fixed_parameters.proportional_gain, None);
            let gains: Vec<f64> = branches.iter().map(|branch| branch.gain).collect();
            assert_eq!(gains, vec![12.0, 16.0, 20.0]);
            let expected = [
                vec![(-6.0, 0.0), (-2.0, 0.0)],
                vec![(-4.0, 0.0), (-4.0, 0.0)],
                vec![(-4.0, -2.0), (-4.0, 2.0)],
            ];
            for (branch, expected_poles) in branches.iter().zip(expected) {
                let poles = sorted_poles(&branch.poles);
                assert_eq!(poles.len(), expected_poles.len());
                for ((real, imaginary), (expected_real, expected_imaginary)) in
                    poles.iter().zip(expected_poles)
                {
                    assert!(
                        (real - expected_real).abs() <= 1e-6,
                        "{real} vs {expected_real}"
                    );
                    assert!(
                        (imaginary - expected_imaginary).abs() <= 1e-6,
                        "{imaginary} vs {expected_imaginary}"
                    );
                }
            }
        }
        RootLocus::Unavailable { reason } => panic!("the sweep is qualified, not {reason}"),
    }
}

#[test]
fn the_default_sweep_keeps_the_candidate_gain_among_its_sixty_three_values() {
    let shaft = model(LowOrderModelKind::Integrating, 1.0, Some(1.0), None);
    let candidate = PredictedGains::parallel(12.0, 0.0, 0.0);
    let plots = plots_of(&shaft, &candidate, &candidate, &conditions(0.01, 10.0))
        .expect("the plots are valid");
    match plots.root_locus {
        RootLocus::Qualified { branches, .. } => {
            assert_eq!(branches.len(), 63);
            assert!(branches.iter().any(|branch| branch.gain == 12.0));
        }
        RootLocus::Unavailable { reason } => panic!("the default sweep is qualified, not {reason}"),
    }
}

#[test]
fn the_bode_time_and_uncertainty_evidence_come_from_one_model() {
    // identify-tune: a first-order plant with a dead time, a 0.05 s period and the horizon of 45 s (900 periods).
    let mut plant = model(
        LowOrderModelKind::FirstOrderDeadTime,
        1.5,
        Some(4.0),
        Some(0.5),
    );
    plant.uncertainty = Some(IdentifiedUncertainty {
        method: UncertaintyMethod::GridResolutionAndValidationResidual,
        gain: ParameterInterval {
            minimum: 1.4,
            maximum: 1.6,
        },
        time_constant_seconds: Some(ParameterInterval {
            minimum: 3.5,
            maximum: 4.5,
        }),
        second_time_constant_seconds: None,
        dead_time_seconds: Some(ParameterInterval {
            minimum: 0.4,
            maximum: 0.6,
        }),
    });
    let current = PredictedGains::parallel(0.4, 0.1, 0.0);
    let candidate = PredictedGains::parallel(0.8, 0.2, 0.0);
    let plots = plots_of(&plant, &current, &candidate, &conditions(0.05, 45.0))
        .expect("the plots are valid");
    assert_eq!(plots.evidence, PlotEvidence::ModelPredicted);
    assert!(
        plots.response.len() >= 100 && plots.response.len() <= 2000,
        "{}",
        plots.response.len()
    );
    assert!(plots.response.iter().all(|point| [
        point.time_seconds,
        point.candidate_process_value,
        point.candidate_manipulated,
        point.current_process_value.unwrap_or(0.0),
        point.current_manipulated.unwrap_or(0.0),
    ]
    .iter()
    .all(|value| value.is_finite())));
    assert!(plots.plant_bode.len() >= 40);
    assert_eq!(plots.open_loop_bode.len(), plots.plant_bode.len());
    assert!(plots
        .open_loop_bode
        .iter()
        .all(|point| point.phase_degrees >= -720.0 && point.phase_degrees <= 720.0));
    assert!(plots.nyquist.len() >= plots.open_loop_bode.len() * 2 - 1);
    assert_eq!(plots.sensitivity.len(), plots.open_loop_bode.len());
    assert!(plots.sensitivity.iter().all(|point| [
        point.sensitivity_magnitude_db,
        point.complementary_sensitivity_magnitude_db,
        point.control_sensitivity_magnitude_db,
    ]
    .iter()
    .all(|value| value.is_finite())));
    match &plots.root_locus {
        RootLocus::Qualified { branches, .. } => assert!(branches.len() >= 40),
        RootLocus::Unavailable { reason } => panic!("the default sweep is qualified, not {reason}"),
    }
    assert_eq!(plots.uncertainty.status, UncertaintyStatus::Evaluated);
    assert_eq!(plots.uncertainty.points.len(), plots.response.len());
    assert!(plots
        .uncertainty
        .points
        .iter()
        .all(|point| point.lower <= point.nominal && point.nominal <= point.upper));
    assert!(finite_margin(plots.margins.gain_margin_db).is_finite());
    assert!(finite_margin(plots.margins.phase_margin_degrees).is_finite());
}

fn finite_margin(value: MarginValue) -> f64 {
    match value {
        MarginValue::Finite(number) => number,
        other => panic!("the margin is finite, not {other:?}"),
    }
}

#[test]
fn the_plotted_phase_and_its_crossover_markers_are_continuous_with_the_delay_in_closed_form() {
    // N4.3 and N7.7: K = 1.5, T = 4 s, L = 0.5 s with Kp = 0.8 and Ki = 0.2. The PI zero at Ki/Kp = 0.25 rad/s cancels the
    // plant's pole at 1/T, so L(s) = 1.5·(0.8s + 0.2)/(s·(4s + 1))·e^(−0.5s) = 0.3·e^(−0.5s)/s: magnitude 0.3/ω and phase
    // −90° − 0.5ω rad. The grid runs from 0.01/T = 0.0025 rad/s to 2π/L = 4π rad/s.
    // - At 4π rad/s the plant's phase is −atan(16π) − 2π rad = −88.860287° − 360° = −448.860287°, and the loop's is
    //   −90° − 360° = −450°: one turn of the delay below the principal values −88.86° and −90°.
    // - The gain crossover is 0.3/ω = 1 at ω = 0.3 rad/s, where the phase is −90° − 0.15 rad = −98.594367°: the phase margin
    //   is 81.405633°, and the marker lies on the curve there.
    // - The phase crossover is −90° − 0.5ω rad = −180° at ω = π rad/s, where |L| = 0.3/π: the gain margin is
    //   20·log10(π/0.3) = 20.400572 dB, and the marker lies at −180° on the curve.
    // Catches a wrapped curve (−88.86° and −90° at 4π rad/s), and a marker read off the wrapped phase, which sits at +180° or
    // −180° by the sign of a rounding error in Im L.
    let plant = model(
        LowOrderModelKind::FirstOrderDeadTime,
        1.5,
        Some(4.0),
        Some(0.5),
    );
    let candidate = PredictedGains::parallel(0.8, 0.2, 0.0);
    let plots = plots_of(&plant, &candidate, &candidate, &conditions(0.05, 45.0))
        .expect("the plots are valid");
    let plant_last = plots.plant_bode.last().expect("the plant curve has points");
    assert!((plant_last.frequency_rad_per_second - 4.0 * PI).abs() < 1e-9);
    assert!(
        (plant_last.phase_degrees + 448.860_287_028_922).abs() < 1e-9,
        "{}",
        plant_last.phase_degrees
    );
    let loop_last = plots
        .open_loop_bode
        .last()
        .expect("the loop curve has points");
    assert!(
        (loop_last.phase_degrees + 450.0).abs() < 1e-9,
        "{}",
        loop_last.phase_degrees
    );
    for curve in [&plots.plant_bode, &plots.open_loop_bode] {
        for pair in curve.windows(2) {
            let step = pair[1].phase_degrees - pair[0].phase_degrees;
            assert!(step.abs() < 180.0, "a step of {step}°");
        }
    }
    assert!((finite_margin(plots.margins.phase_margin_degrees) - 81.405_633_073_038).abs() < 1e-6);
    assert!((finite_margin(plots.margins.gain_margin_db) - 20.400_572_359_489).abs() < 1e-6);
    let gain = plots.gain_crossover.expect("the loop crosses 0 dB");
    assert!((gain.frequency_rad_per_second - 0.3).abs() < 1e-9);
    assert!(
        (gain.phase_degrees + 98.594_366_926_962).abs() < 1e-6,
        "{}",
        gain.phase_degrees
    );
    let phase = plots
        .phase_crossover
        .expect("the loop's phase crosses −180°");
    assert!((phase.frequency_rad_per_second - PI).abs() < 1e-9);
    assert!(
        (phase.phase_degrees + 180.0).abs() < 1e-6,
        "{}",
        phase.phase_degrees
    );
}

#[test]
fn the_candidate_evidence_exposes_the_gain_crossover_at_zero_decibels() {
    // V-05: K = 1.4, τ = 4 s, L = 0.6 s with P = 0.4 and Ki = 0.1: the loop crosses 0 dB, and the crossover marker
    // is read at that frequency.
    let plant = model(
        LowOrderModelKind::FirstOrderDeadTime,
        1.4,
        Some(4.0),
        Some(0.6),
    );
    let base = PredictedGains::parallel(0.4, 0.1, 0.0);
    let plots =
        plots_of(&plant, &base, &base, &conditions(0.05, 46.0)).expect("the plots are valid");
    let crossover = plots.gain_crossover.expect("this loop crosses 0 dB");
    assert!(crossover.frequency_rad_per_second.is_finite());
    assert!(
        crossover.magnitude_db.abs() < 1e-6,
        "{}",
        crossover.magnitude_db
    );
    assert!(plots
        .phase_crossover
        .is_none_or(|point| point.frequency_rad_per_second.is_finite()));
}

#[test]
fn the_uncertainty_method_reaches_the_plot_as_words_never_as_an_identifier() {
    // V-17: the legend shows the words of the method, not the dash-case identifier.
    assert_eq!(
        method_label(Some(UncertaintyMethod::SampleResolutionEnvelope)),
        "sample-resolution envelope from the estimation record"
    );
    assert_eq!(method_label(None), "declared model set");
    let mut plant = model(LowOrderModelKind::FirstOrder, 1.4, Some(4.0), None);
    plant.uncertainty = Some(IdentifiedUncertainty {
        method: UncertaintyMethod::SampleResolutionEnvelope,
        gain: ParameterInterval {
            minimum: 1.2,
            maximum: 1.6,
        },
        time_constant_seconds: None,
        second_time_constant_seconds: None,
        dead_time_seconds: None,
    });
    let base = PredictedGains::parallel(0.4, 0.1, 0.0);
    let plots =
        plots_of(&plant, &base, &base, &conditions(0.05, 46.0)).expect("the plots are valid");
    assert_eq!(
        plots.uncertainty.method,
        "sample-resolution envelope from the estimation record"
    );
    assert_ne!(plots.uncertainty.method, "sample-resolution-envelope");
}

#[test]
fn an_unbounded_gain_margin_is_reported_as_unbounded_in_the_plots() {
    // identify-tune: a stable loop with no delay whose PI phase never reaches −180° has an unbounded gain margin, while
    // its phase margin is finite and its curves are complete.
    let plant = model(LowOrderModelKind::FirstOrder, 1.45928, Some(4.0), None);
    let current = PredictedGains::parallel(0.4, 0.1, 0.0);
    let candidate = PredictedGains::parallel(0.57285, 0.38139, 0.0);
    let plots = plots_of(&plant, &current, &candidate, &conditions(0.05, 40.0))
        .expect("the plots are valid");
    assert_eq!(plots.margins.gain_margin_db, MarginValue::Unbounded);
    assert!(finite_margin(plots.margins.phase_margin_degrees).is_finite());
    assert!(plots.open_loop_bode.len() >= 40);
}

#[test]
fn windows_and_bands_follow_the_settling_deadline_on_the_sample_grid() {
    // N8.3: a step of 1 from an operating output of 2 has the target 3. The settling time of 4 s with a 0.5 s period
    // is on the grid, so the deadline is 4 s; the settling band is ±0.1 and the allowed error is ±0.2 around 3.
    let plant = model(LowOrderModelKind::FirstOrder, 1.0, Some(1.0), None);
    let base = PredictedGains::parallel(1.0, 0.0, 0.0);
    let requirement = StepRequirement {
        maximum_error: 0.2,
        settling_band: 0.1,
        settling_time_seconds: 4.0,
    };
    let settings = PredictionConditions {
        operating_output: 2.0,
        ..conditions(0.5, 10.0)
    };
    let units = units();
    let plots = build_engineering_plots(&PlotInputs {
        model: &plant,
        current: None,
        candidate: &base,
        conditions: &settings,
        units: &units,
        requirement: Some(&requirement),
        sweep: None,
    })
    .expect("the plots are valid");
    let marks = plots
        .evaluation
        .expect("the requirement gives its windows and bands");
    assert!(
        (marks.deadline_seconds - 4.0).abs() <= 1e-12,
        "{}",
        marks.deadline_seconds
    );
    assert_eq!(marks.window_start_seconds, 0.0);
    assert_eq!(marks.window_end_seconds, 10.0);
    assert!((marks.target - 3.0).abs() <= 1e-12);
    assert!(
        (marks.settling_band.lower - 2.9).abs() <= 1e-12
            && (marks.settling_band.upper - 3.1).abs() <= 1e-12
    );
    assert!(
        (marks.error_band.lower - 2.8).abs() <= 1e-12
            && (marks.error_band.upper - 3.2).abs() <= 1e-12
    );
}

#[test]
fn the_response_keeps_at_most_the_fit_point_limit_with_its_first_and_last_samples() {
    // A 10 s run at 1 ms has 10 000 periods; the plot keeps every second one, the first and the last included.
    let plant = model(LowOrderModelKind::FirstOrder, 1.0, Some(1.0), None);
    let base = PredictedGains::parallel(1.0, 0.0, 0.0);
    let plots =
        plots_of(&plant, &base, &base, &conditions(0.001, 10.0)).expect("the plots are valid");
    assert_eq!(
        plots.response.len(),
        usize::try_from(FIT_POINT_LIMIT).unwrap_or(0) + 1
    );
    let last = plots.response.last().expect("the response has points");
    assert!(
        (last.time_seconds - 10.0).abs() <= 1e-9,
        "{}",
        last.time_seconds
    );
}

#[test]
fn the_comparison_context_holds_for_candidate_edits_and_changes_with_the_model_or_the_baseline() {
    // identify-tune: edits to the candidate keep one model, baseline and method; a changed model or baseline does not.
    let plant = model(LowOrderModelKind::FirstOrder, 1.0, Some(4.0), None);
    let current = PredictedGains::parallel(0.4, 0.1, 0.0);
    let candidate = PredictedGains::parallel(0.8, 0.2, 0.0);
    let settings = conditions(0.05, 20.0);
    let base = plots_of(&plant, &current, &candidate, &settings).expect("the plots are valid");
    let next = PredictedGains::parallel(1.6, 0.2, 0.0);
    let edited = plots_of(&plant, &current, &next, &settings).expect("the plots are valid");
    assert_eq!(base.comparison.context, edited.comparison.context);
    assert_ne!(base.comparison.candidate, edited.comparison.candidate);
    let changed_model = model(LowOrderModelKind::FirstOrder, 1.1, Some(4.0), None);
    let other_model =
        plots_of(&changed_model, &current, &candidate, &settings).expect("the plots are valid");
    assert_ne!(base.comparison.context, other_model.comparison.context);
    let other_baseline =
        plots_of(&plant, &candidate, &candidate, &settings).expect("the plots are valid");
    assert_ne!(base.comparison.context, other_baseline.comparison.context);
    assert_eq!(
        base.comparison.context.method,
        "unit-step-model-prediction-v1"
    );
}

#[test]
fn a_candidate_that_cannot_be_predicted_is_refused_with_its_sentence() {
    let plant = model(LowOrderModelKind::FirstOrder, 1.0, Some(4.0), None);
    let current = PredictedGains::parallel(0.4, 0.1, 0.0);
    let broken = PredictedGains::parallel(f64::NAN, 0.1, 0.0);
    let refusal = plots_of(&plant, &current, &broken, &conditions(0.05, 20.0))
        .expect_err("a gain that is not finite is refused");
    assert_eq!(refusal.0, crate::tuning::texts::CANDIDATE_NOT_FINITE);
}
