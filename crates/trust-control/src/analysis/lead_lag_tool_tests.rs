//! The lead/lag tool (`leadLag`, B9.9; N7.14): its input checks and its answer. Ported assertions name the TypeScript test they come from.

use serde_json::json;

use super::{checked_request, lead_lag_answer, DESIGN_DETAILS};
use crate::analysis::lead_lag::LeadLagMode;
use crate::messages::AnalysisInputs;
use crate::model::{decode_general_model, GeneralModel};

/// The transfer function `2 / (s + 2)`, a single-input, single-output model.
fn plant() -> GeneralModel {
    decode_general_model(&json!({
        "kind": "transfer-function", "revision": "plant", "source": "provided", "validation": "unvalidated",
        "evidence": [], "delaySeconds": 0.0,
        "inputs": [{ "id": "u", "unit": "V", "operatingRange": { "minimum": -10.0, "maximum": 10.0 } }],
        "outputs": [{ "id": "y", "unit": "m", "operatingRange": { "minimum": -10.0, "maximum": 10.0 } }],
        "numerator": [2.0], "denominator": [1.0, 2.0],
    }))
    .expect("the plant is valid")
}

fn request(mode: LeadLagMode, crossover: f64, value: f64) -> AnalysisInputs {
    AnalysisInputs {
        mode: Some(mode),
        crossover_rad_per_second: Some(crossover),
        value: Some(value),
        ..AnalysisInputs::default()
    }
}

#[test]
fn a_lead_design_at_one_radian_per_second_keeps_its_figures_and_its_formula() {
    // Ported from `routes lead/lag design through the host owner and retains the selected model` (control-studio-general-model-session.test.ts):
    // the request is `{mode: lead, crossoverRadPerSecond: 1, value: 30}` and the design has finite figures. The phase added at the
    // crossover is the requested 30°, and the zero lies below the pole.
    let answer = lead_lag_answer(&plant(), &request(LeadLagMode::Lead, 1.0, 30.0))
        .expect("the lead design qualifies");
    let design = answer.design;
    assert!(design.gain.is_finite());
    assert!(design.zero_rad_per_second > 0.0);
    assert!(design.pole_rad_per_second > design.zero_rad_per_second);
    assert!((design.phase_contribution_degrees - 30.0).abs() < 1e-6);
    assert!(
        answer.summary[0].starts_with("Lead compensator: C(s) = "),
        "{:?}",
        answer.summary
    );
    assert!(answer.summary[0].contains("(1 + s/"));
    assert_eq!(answer.details, vec![DESIGN_DETAILS.to_owned()]);
}

#[test]
fn a_lag_design_keeps_its_low_frequency_gain_ratio() {
    let answer = lead_lag_answer(&plant(), &request(LeadLagMode::Lag, 1.0, 5.0))
        .expect("the lag design qualifies");
    let ratio = answer
        .design
        .low_frequency_gain_ratio
        .expect("a lag has its ratio");
    assert!((ratio - 5.0).abs() < 1e-6);
    assert!(answer.summary[0].starts_with("Lag compensator: C(s) = "));
}

#[test]
fn each_input_rule_is_checked_and_its_refusal_names_the_input() {
    // The crossover must be finite and above zero.
    let crossover =
        checked_request(&request(LeadLagMode::Lead, 0.0, 30.0)).expect_err("zero is refused");
    assert!(
        crossover.message.contains("crossover"),
        "{}",
        crossover.message
    );
    // A lead adds a phase above 0° and below 85°.
    let lead = checked_request(&request(LeadLagMode::Lead, 1.0, 85.0)).expect_err("85° is refused");
    assert!(lead.message.contains("85"), "{}", lead.message);
    assert!(checked_request(&request(LeadLagMode::Lead, 1.0, 0.0)).is_err());
    // A lag has a ratio above 1.
    let lag =
        checked_request(&request(LeadLagMode::Lag, 1.0, 1.0)).expect_err("a ratio of 1 is refused");
    assert!(lag.message.contains("lag ratio"), "{}", lag.message);
    // The compensator itself must be chosen.
    let missing = checked_request(&AnalysisInputs {
        crossover_rad_per_second: Some(1.0),
        value: Some(30.0),
        ..AnalysisInputs::default()
    })
    .expect_err("a mode is required");
    assert!(
        missing.message.contains("compensator"),
        "{}",
        missing.message
    );
    assert!(checked_request(&request(LeadLagMode::Lead, 1.0, 30.0)).is_ok());
}
