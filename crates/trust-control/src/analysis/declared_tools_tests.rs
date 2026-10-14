//! Tests of the declared analysis tools (`rootLocus`, `routh`): the selected value, the sentences of the shaft of
//! control-studio-declared-analysis.test.ts, and the budget.

use std::time::Duration;

use serde_json::json;

use super::*;
use crate::analysis::budget::{ANALYSIS_BUDGET, BUDGET_EXCEEDED};

/// The shaft of control-studio-declared-analysis.test.ts: 1/(s² + s) with the derivative gain Kd fixed at 7 and the
/// proportional gain Kp swept over 12, 16 and 20.
fn shaft() -> DeclaredControlAnalysis {
    crate::json::decode_value(json!({
        "plant": { "kind": "transfer-function", "numerator": [1.0], "denominator": [1.0, 1.0, 0.0], "delaySeconds": 0.0,
            "inputUnit": "N·m", "outputUnit": "rad", "revision": "shaft-1", "source": "provided", "validation": "unvalidated" },
        "controller": { "numerator": [{ "parameter": "Kd" }, { "parameter": "Kp" }], "denominator": [1.0],
            "parameters": { "Kp": { "value": 12.0, "minimum": 0.0, "maximum": 100.0, "unit": "N·m/rad" },
                "Kd": { "value": 7.0, "minimum": 7.0, "maximum": 7.0, "unit": "N·m·s/rad" } } },
        "rootLocus": { "parameter": "Kp", "values": [12.0, 16.0, 20.0], "selectedValue": 12.0 }
    }))
    .expect("the shaft")
}

/// The elapsed time that a budget reports, in whole seconds.
fn budget_with(elapsed_seconds: u64) -> impl Fn() -> Duration {
    move || Duration::from_secs(elapsed_seconds)
}

#[test]
fn the_root_locus_names_its_parameter_its_selection_and_its_fixed_parameter() {
    let elapsed = budget_with(0);
    let cancelled = || false;
    let budget = Budget::new(ANALYSIS_BUDGET, &elapsed, &cancelled);
    let inputs = AnalysisInputs {
        selected_value: Some(16.0),
        ..AnalysisInputs::default()
    };
    let answer = root_locus_answer(&shaft(), &inputs, &budget).expect("the root locus");
    assert!((answer.selected_value - 16.0).abs() < 1e-12);
    assert_eq!(
        answer.summary,
        vec!["Kp varies in N·m/rad; selected Kp = 16. Fixed: Kd = 7 N·m·s/rad.".to_string()]
    );
    assert_eq!(answer.details.len(), 1);
    assert!(
        answer.details[0].ends_with("No model delay."),
        "{:?}",
        answer.details
    );
    assert_eq!(answer.branches.len(), 3);
}

#[test]
fn a_selected_value_outside_the_sweep_is_refused_by_name() {
    let elapsed = budget_with(0);
    let cancelled = || false;
    let budget = Budget::new(ANALYSIS_BUDGET, &elapsed, &cancelled);
    let inputs = AnalysisInputs {
        selected_value: Some(18.0),
        ..AnalysisInputs::default()
    };
    let error = root_locus_answer(&shaft(), &inputs, &budget).expect_err("18 is not in the sweep");
    assert!(error.message.contains("selectedValue"), "{}", error.message);
}

#[test]
fn the_root_locus_stops_when_the_budget_runs_out() {
    let elapsed = budget_with(11);
    let cancelled = || false;
    let budget = Budget::new(ANALYSIS_BUDGET, &elapsed, &cancelled);
    let error = root_locus_answer(&shaft(), &AnalysisInputs::default(), &budget)
        .expect_err("the budget is exceeded");
    assert_eq!(error.message, BUDGET_EXCEEDED);
}

#[test]
fn the_routh_reading_at_kp_16_is_stable_with_the_interval_of_kp_above_zero() {
    let elapsed = budget_with(0);
    let cancelled = || false;
    let budget = Budget::new(ANALYSIS_BUDGET, &elapsed, &cancelled);
    let inputs = AnalysisInputs {
        selected_value: Some(16.0),
        ..AnalysisInputs::default()
    };
    // s² + 8s + Kp: stable for Kp > 0, and at Kp = 16 the first column is positive throughout.
    let answer = routh_answer(&shaft(), &inputs, &budget).expect("the reading");
    assert_eq!(
        answer.summary,
        vec![
            "Kp = 16. No right-half-plane poles were established by the Routh first column; the declared polynomial is asymptotically stable.".to_string(),
            "Stable interval: (0, ∞).".to_string(),
        ]
    );
    assert!(
        answer.details[0]
            .starts_with("First-column signs of the table: positive, positive, positive."),
        "{:?}",
        answer.details
    );
    assert!(
        answer.details[0].contains("Allowed domain [0, 100]."),
        "{:?}",
        answer.details
    );
}

#[test]
fn the_routh_reading_refuses_a_value_that_is_not_finite_by_name() {
    let elapsed = budget_with(0);
    let cancelled = || false;
    let budget = Budget::new(ANALYSIS_BUDGET, &elapsed, &cancelled);
    let inputs = AnalysisInputs {
        selected_value: Some(f64::NAN),
        ..AnalysisInputs::default()
    };
    let error = routh_answer(&shaft(), &inputs, &budget).expect_err("NaN is not a value");
    assert!(error.message.contains("selectedValue"), "{}", error.message);
}
