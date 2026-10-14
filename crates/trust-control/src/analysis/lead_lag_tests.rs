use super::*;
use crate::model::decode_general_model;
use serde_json::json;

fn selected_model() -> GeneralModel {
    decode_general_model(&json!({
        "revision": "plant-lead", "source": "provided", "validation": "unvalidated", "evidence": [], "delaySeconds": 0.0,
        "kind": "transfer-function",
        "inputs": [{ "id": "torque", "unit": "Nm", "operatingRange": { "minimum": -100.0, "maximum": 100.0 } }],
        "outputs": [{ "id": "angle", "unit": "rad", "operatingRange": { "minimum": -100.0, "maximum": 100.0 } }],
        "numerator": [1.0], "denominator": [1.0, 1.0, 0.0]
    }))
    .expect("a valid model")
}

fn lead(crossover: f64, phase: f64) -> LeadLagRequest {
    LeadLagRequest {
        mode: LeadLagMode::Lead,
        crossover_rad_per_second: crossover,
        value: phase,
    }
}

#[test]
fn a_lead_at_the_crossover_adds_the_requested_phase_with_unit_loop_gain() {
    // Ported: the plant 1/(s² + s) at ω = 1 rad/s with a 30° lead.
    let saved = run_lead_lag(&selected_model(), lead(1.0, 30.0)).expect("the design");
    assert!((saved.result.phase_contribution_degrees - 30.0).abs() <= 1e-9);
    assert!((saved.result.open_loop_magnitude_at_crossover - 1.0).abs() <= 1e-9);
    assert!(saved.result.zero_rad_per_second < saved.result.pole_rad_per_second);
    assert_eq!(saved.result.status, LeadLagStatus::Qualified);
}

#[test]
fn a_lag_keeps_the_crossover_gain_and_states_its_ratio() {
    let request = LeadLagRequest {
        mode: LeadLagMode::Lag,
        crossover_rad_per_second: 1.0,
        value: 5.0,
    };
    let saved = run_lead_lag(&selected_model(), request).expect("the design");
    assert!(saved.result.pole_rad_per_second < saved.result.zero_rad_per_second);
    assert!(saved.result.low_frequency_gain_ratio.expect("the ratio") > 4.9);
}

#[test]
fn an_input_that_is_not_finite_and_positive_is_refused() {
    assert_eq!(
        run_lead_lag(&selected_model(), lead(0.0, 30.0))
            .expect_err("refused")
            .message,
        texts::LEAD_LAG_INPUT
    );
}

#[test]
fn a_saved_design_is_kept_only_for_the_model_it_was_made_for() {
    let model = selected_model();
    let mut session = LeadLagSession::new();
    session.run(&model, lead(1.0, 30.0)).expect("the design");
    assert!(session.snapshot(&model).is_some());
    let other = decode_general_model(&json!({
        "revision": "other", "source": "provided", "validation": "unvalidated", "evidence": [], "delaySeconds": 0.0,
        "kind": "transfer-function",
        "inputs": [{ "id": "torque", "unit": "Nm", "operatingRange": { "minimum": -100.0, "maximum": 100.0 } }],
        "outputs": [{ "id": "angle", "unit": "rad", "operatingRange": { "minimum": -100.0, "maximum": 100.0 } }],
        "numerator": [2.0], "denominator": [1.0, 1.0]
    }))
    .expect("a valid model");
    assert!(session.snapshot(&other).is_none());
    let saved = session.snapshot(&model).expect("the saved design");
    let refused = session
        .restore(Some(saved), Some(&other))
        .expect_err("another model");
    assert_eq!(refused.message, texts::LEAD_LAG_OTHER_MODEL);
    session.dispose();
    assert!(session.snapshot(&model).is_none());
}
