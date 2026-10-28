// Ported from editors/vscode/src/test/suite/control-studio-identify-tune.test.ts: the comparison context of the plots of a
// named sweep (candidate edits keep it, a changed model or baseline invalidates it), and the word `unbounded` that an
// unbounded gain margin takes in the plot message.

use super::*;
use trust_control_math::identify::LowOrderModelKind;
use trust_control_math::margins::MarginValue;

/// The integrating shaft of the `TypeScript` test: the rate gain `gain`, a time constant of one second, and no dead time.
fn shaft(gain: f64) -> IdentifiedModel {
    IdentifiedModel {
        kind: LowOrderModelKind::Integrating,
        gain,
        time_constant_seconds: Some(1.0),
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

/// A proportional gain with the measured-velocity feedback of the `TypeScript` test (`Kd = 7`) and no integral term.
fn measured_velocity(proportional_gain: f64) -> PredictedGains {
    PredictedGains {
        derivative_gain: 7.0,
        ..PredictedGains::parallel(proportional_gain, 0.0, 0.0)
    }
}

/// The conditions of the `TypeScript` defaults: a period of 0.05 s over the given horizon, and the output and rate limits of
/// the first test's records. The margins do not read the limits, so the second test's records (0 to 100, and plus or minus
/// 100) leave its margins unchanged.
fn default_conditions(duration_seconds: f64) -> PredictionConditions {
    PredictionConditions {
        period_seconds: 0.05,
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

/// The plots of a model with the given baseline and candidate settings on the given conditions.
fn plots_of(
    model: &IdentifiedModel,
    current: &PredictedGains,
    candidate: &PredictedGains,
    conditions: &PredictionConditions,
) -> EngineeringPlots {
    let units = SeriesUnits {
        process_value: "unit".to_string(),
        manipulated: "unit".to_string(),
    };
    build_engineering_plots(&PlotInputs {
        model,
        current: Some(current),
        candidate,
        conditions,
        units: &units,
        requirement: None,
        sweep: None,
    })
    .expect("the plots of the model and the settings are valid")
}

// control-studio-identify-tune: "named root-locus sweep varies Kp while retaining measured-velocity feedback"
#[test]
fn the_comparison_context_ignores_candidate_edits_and_follows_the_model_and_the_baseline() {
    let plant = shaft(1.0);
    let twelve = measured_velocity(12.0);
    let twenty = measured_velocity(20.0);
    let settings = default_conditions(10.0);
    let base = plots_of(&plant, &twelve, &twelve, &settings);
    assert_eq!(base.comparison.candidate, twelve);

    let edited = plots_of(&plant, &twelve, &twenty, &settings);
    assert_eq!(
        base.comparison.context, edited.comparison.context,
        "candidate edits retain one common model, baseline, and comparison method"
    );

    let changed_model = plots_of(&shaft(1.1), &twelve, &twenty, &settings);
    assert_ne!(
        base.comparison.context, changed_model.comparison.context,
        "a changed model invalidates comparisons"
    );

    let changed_baseline = plots_of(&plant, &twenty, &twenty, &settings);
    assert_ne!(
        base.comparison.context, changed_baseline.comparison.context,
        "a changed runtime baseline invalidates comparisons"
    );
}

// control-studio-identify-tune: "reports an unbounded gain margin when a stable no-delay loop has no phase crossover"
#[test]
fn an_unbounded_gain_margin_serialises_as_the_word_unbounded() {
    let plant = IdentifiedModel {
        kind: LowOrderModelKind::FirstOrder,
        gain: 1.45928,
        time_constant_seconds: Some(4.0),
        second_time_constant_seconds: None,
        dead_time_seconds: None,
        normalized_error: Some(0.0998),
        normalized_estimation_error: None,
        normalized_validation_error: None,
        estimation_residuals: None,
        validation_residuals: None,
        uncertainty: None,
        stable: true,
        on_search_boundary: false,
    };
    let current = PredictedGains::parallel(0.4, 0.1, 0.0);
    let candidate = PredictedGains::parallel(0.57285, 0.38139, 0.0);
    let plots = plots_of(&plant, &current, &candidate, &default_conditions(40.0));
    assert_eq!(plots.margins.gain_margin_db, MarginValue::Unbounded);
    let message = serde_json::to_value(&plots.margins).expect("the margins serialize");
    assert_eq!(message["gainMarginDb"], "unbounded");
}
