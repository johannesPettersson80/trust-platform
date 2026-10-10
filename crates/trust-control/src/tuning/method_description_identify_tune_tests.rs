// Ported from editors/vscode/src/test/suite/control-studio-identify-tune.test.ts: the title, the status and the gain units of
// a feasible description (`describeTuningMethod`). The assumption, input and convention copy of that function is not in
// Rust yet; this module's header says so.

use super::*;

// control-studio-identify-tune: "describes formula assumptions, units, inputs, results, and unavailable reasons"
#[test]
fn a_feasible_simc_description_gives_each_gain_its_unit_in_the_input_and_output_units() {
    let model = IdentifiedModel {
        kind: LowOrderModelKind::FirstOrderDeadTime,
        gain: 1.5,
        time_constant_seconds: Some(4.0),
        second_time_constant_seconds: None,
        dead_time_seconds: Some(0.5),
        normalized_error: 0.0,
        normalized_estimation_error: None,
        normalized_validation_error: None,
        estimation_residuals: None,
        validation_residuals: None,
        uncertainty: None,
        stable: true,
        on_search_boundary: false,
    };
    // The fifth argument is the output unit and the sixth the input unit: the plant gain reads `°C/%`.
    let described = describe_method(
        &model,
        TuningMethod::Simc,
        TuningObjective::Balanced,
        ControllerAction::Direct,
        "°C",
        "%",
    );
    assert_eq!(described.status, DescriptionStatus::Feasible);
    assert_eq!(described.title, "SIMC PI (Skogestad 2003)");
    assert_eq!(
        described.units,
        GainUnits {
            plant_gain: "°C/%".to_string(),
            proportional_gain: "%/°C".to_string(),
            integral_gain: "%/(°C·s)".to_string(),
            derivative_gain: "(%·s)/°C".to_string(),
        }
    );
    let result = described.result.expect("a feasible method has a result");
    assert!(result.proportional_gain > 0.0 && result.integral_time_seconds > 0.0);
}
