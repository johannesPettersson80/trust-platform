//! Rust ports of the chart-data assertions of `control-studio-chart-visuals.test.ts`: the crossover points that the
//! chart markers read, the uncertainty method as words, and the notation of units. The colour, tick, axis-range and
//! line-style assertions of that file are presentation and stay in `TypeScript`.

use trust_control_math::identify::LowOrderModelKind;

use super::*;
use crate::identification::plant::IdentifiedUncertainty;
use crate::identification::recording_import::display_engineering_unit;

/// The first-order model of the chart evidence: a gain of 1.4 and a time constant of 4 s, with the given dead time and
/// uncertainty.
fn first_order_model(
    dead_time_seconds: Option<f64>,
    uncertainty: Option<IdentifiedUncertainty>,
) -> IdentifiedModel {
    IdentifiedModel {
        kind: LowOrderModelKind::FirstOrder,
        gain: 1.4,
        time_constant_seconds: Some(4.0),
        second_time_constant_seconds: None,
        dead_time_seconds,
        normalized_error: Some(0.05),
        normalized_estimation_error: None,
        normalized_validation_error: None,
        estimation_residuals: None,
        validation_residuals: None,
        uncertainty,
        stable: true,
        on_search_boundary: false,
    }
}

/// The prediction conditions of the chart evidence: a period of 0.05 s over the given horizon, a setpoint step of one
/// from zero, and the base candidate's output limits (0 to 100) and rate limits (-100 to 100 per second).
fn conditions_of(duration_seconds: f64) -> PredictionConditions {
    PredictionConditions {
        period_seconds: 0.05,
        duration_seconds,
        reference_change: 1.0,
        operating_input: 0.0,
        operating_output: 0.0,
        input_disturbance: 0.0,
        output_minimum: 0.0,
        output_maximum: 100.0,
        rate_minimum: -100.0,
        rate_maximum: 100.0,
    }
}

/// The plots of a model with the base settings (proportional gain 0.4, integral gain 0.1, no derivative) as both the
/// current and the candidate settings, with no requirement and no sweep.
fn plots_of(model: &IdentifiedModel, conditions: &PredictionConditions) -> EngineeringPlots {
    let settings = PredictedGains::parallel(0.4, 0.1, 0.0);
    let units = SeriesUnits {
        process_value: "m".to_string(),
        manipulated: "V".to_string(),
    };
    build_engineering_plots(&PlotInputs {
        model,
        current: Some(&settings),
        candidate: &settings,
        conditions,
        units: &units,
        requirement: None,
        sweep: None,
    })
    .expect("the first-order model and the base settings are valid")
}

/// Whether the text has the shape of a dash-case identifier, `^[a-z0-9]+(-[a-z0-9]+)+$`: at least two segments of
/// lower-case letters and digits joined by dashes.
fn is_dash_case(text: &str) -> bool {
    let parts: Vec<&str> = text.split('-').collect();
    parts.len() >= 2
        && parts.iter().all(|segment| {
            !segment.is_empty()
                && segment
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        })
}

#[test]
fn the_candidate_evidence_exposes_the_crossover_points_the_markers_need() {
    // control-studio-chart-visuals: "V-05/engineeringPlots: the candidate evidence exposes the exact crossover points the markers need"
    // Spec: control-studio-numerics.md N7.7. The gain crossover is where |L| = 1 (0 dB). The phase crossover is
    // where the phase of L is -180 degrees, the frequency that the gain margin reads.
    // The model has a dead time of 0.6 s. The horizon, 46 s, is the TypeScript default max(10, 10 * (4 + 0 + 0.6)).
    let model = first_order_model(Some(0.6), None);
    let plots = plots_of(&model, &conditions_of(46.0));
    let crossover = plots.gain_crossover.expect("this loop crosses 0 dB");
    assert!(
        crossover.frequency_rad_per_second.is_finite(),
        "the gain crossover has a finite frequency"
    );
    assert!(
        crossover.magnitude_db.abs() < 1e-6,
        "the gain crossover is defined at 0 dB, not {}",
        crossover.magnitude_db
    );
    assert!(plots
        .phase_crossover
        .is_none_or(|point| point.frequency_rad_per_second.is_finite()));
}

#[test]
fn the_uncertainty_method_reaches_the_plot_as_words_never_as_a_raw_enum() {
    // control-studio-chart-visuals: "V-17/engineeringPlots: the uncertainty method reaches the plot as words, never a raw enum"
    // Spec: control-studio-numerics.md N2.4. A single run's envelope is labelled as a sample-resolution envelope; it is
    // never a confidence interval.
    // The gain interval is 1.2 to 1.6 by the sample-resolution envelope. The horizon, 40 s, is the TypeScript default.
    let uncertainty = IdentifiedUncertainty {
        method: UncertaintyMethod::SampleResolutionEnvelope,
        gain: ParameterInterval {
            minimum: 1.2,
            maximum: 1.6,
        },
        time_constant_seconds: None,
        second_time_constant_seconds: None,
        dead_time_seconds: None,
    };
    let model = first_order_model(None, Some(uncertainty));
    let plots = plots_of(&model, &conditions_of(40.0));
    assert_eq!(
        plots.uncertainty.method,
        "sample-resolution envelope from the estimation record"
    );
    assert!(
        !is_dash_case(&plots.uncertainty.method),
        "the method must not reach the plot as a dash-case identifier"
    );
}

#[test]
fn one_unit_display_helper_maps_configuration_unit_spellings_to_what_a_student_reads() {
    // control-studio-chart-visuals: "V-02/V-07: one unit-display helper maps configuration unit spellings to what a student reads"
    // The owner is identification::recording_import::display_engineering_unit. Its existing test asserts only some of
    // these cases, so the whole TypeScript assertion list is ported here.
    assert_eq!(display_engineering_unit("degC"), "°C");
    assert_eq!(display_engineering_unit("pp"), "percentage points");
    assert_eq!(display_engineering_unit("%/degC"), "%/°C");
    assert_eq!(display_engineering_unit("%/(degC*s)"), "%/(°C·s)");
    assert_eq!(display_engineering_unit("V*s/m"), "V·s/m");
    assert_eq!(
        display_engineering_unit("pp/pp"),
        "percentage points/percentage points"
    );
    assert_eq!(
        display_engineering_unit("level pp"),
        "level percentage points"
    );
    assert_eq!(display_engineering_unit("%"), "%");
    assert_eq!(display_engineering_unit("  degC  "), "°C");
}
