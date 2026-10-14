// The plans' numbers are exact fixture values or the specification's formulas, compared with a tolerance.
#![allow(clippy::float_cmp)]

use super::*;
use crate::commissioning::fixture_loops::{parallel_pid_loop, proportional_loop, requirements};
use crate::loop_definition::LoopDefinition;
use crate::model::general::decode_general_model;
use crate::model::GeneralModel;
use crate::validation::engineer_verification_plan::engineer_requirements;
use crate::validation::run_validation::RunCheck;
use serde_json::json;

fn close(actual: f64, expected: f64, tolerance: f64) -> bool {
    (actual - expected).abs() <= tolerance
}

/// A first-order-style model of one input and one output at the given operating point, in `%`.
fn model_at(input: f64, output: f64) -> GeneralModel {
    decode_general_model(&json!({
        "kind": "transfer-function", "revision": "model:1", "source": "provided", "validation": "validated",
        "evidence": ["bench"], "delaySeconds": 0, "numerator": [1], "denominator": [1, 1],
        "inputs": [{ "id": "u", "unit": "%", "operatingRange": { "minimum": 0, "maximum": 100 } }],
        "outputs": [{ "id": "y", "unit": "%", "operatingRange": { "minimum": 0, "maximum": 100 } }],
        "operatingPoint": { "inputs": [input], "outputs": [output] }
    }))
    .expect("the model decodes")
}

/// The bounds of the requirement with `id`, and its window.
fn bounds_of(requirements: &[RunRequirement], id: &str) -> (f64, f64, Window) {
    let requirement = requirements
        .iter()
        .find(|requirement| requirement.id == id)
        .expect("the requirement is present");
    match requirement.check {
        RunCheck::Bounds { minimum, maximum } => (minimum, maximum, requirement.window),
        ref other => panic!("{id} is not a bounds check: {other:?}"),
    }
}

fn gains(kp: f64, ki: f64, kd: f64) -> BTreeMap<String, f64> {
    BTreeMap::from([
        ("Main.Parameters.Kp".to_owned(), kp),
        ("Main.Parameters.Ki".to_owned(), ki),
        ("Main.Parameters.Kd".to_owned(), kd),
    ])
}

#[test]
fn a_parallel_pid_start_holds_its_history_and_setpoint_and_enters_auto_at_the_transfer() {
    let loop_definition = parallel_pid_loop();
    let model = model_at(0.0, 2.0);
    let start = pid_initialization_plan(
        &loop_definition,
        &model,
        &requirements(),
        &gains(1.0, 0.0, 0.0),
        None,
    )
    .expect("the start");
    assert!(
        close(start.integral_state, 0.0, 1e-12),
        "u₀ − Kp·(β − 1)·y₀ = 0"
    );
    assert!(
        close(start.derivative_state, -2.0, 1e-12),
        "(γ − 1)·y₀ = −2"
    );
    assert!(
        close(start.plan.reference, 2.0, 1e-12),
        "the setpoint starts at the process value"
    );
    assert!(
        close(start.plan.tolerance, 1e-4, 1e-12),
        "the exact check: 1e-6 times the 100 span"
    );
    let run = &start.plan.run;
    assert_eq!(run.step_ms, 10);
    assert_eq!(run.duration_ms, 60, "six task periods");
    assert_eq!(run.max_total_cycles, 7);
    assert!(close(run.initial_values["Main.Integral"], 0.0, 1e-12));
    assert!(close(run.initial_values["Main.PreviousError"], -2.0, 1e-12));
    assert!(
        close(run.initial_values["Main.Auto"], 0.0, 1e-12),
        "the run starts in Manual, which is false here"
    );
    assert!(close(run.initial_values["LevelSP"], 2.0, 1e-12));
    let at_transfer: Vec<(&str, f64)> = run
        .events
        .iter()
        .map(|event| (event.symbol.as_str(), event.value))
        .collect();
    assert!(
        run.events.iter().all(|event| event.at_ms == 30),
        "the transfer is after three periods"
    );
    assert!(at_transfer.contains(&("LevelSP", 2.0)));
    assert!(
        at_transfer.contains(&("Main.Auto", 1.0)),
        "the mode enters Auto at the transfer"
    );
    let (minimum, maximum, window) = bounds_of(&start.plan.requirements, "transfer-continuity");
    assert!(
        close(minimum, -1e-4, 1e-12) && close(maximum, 1e-4, 1e-12),
        "the output stays within the allowance of its operating value 0"
    );
    assert!(
        close(window.start, 0.0, 1e-12) && close(window.end, 0.03, 1e-12),
        "the continuity bounds the Manual scans and the first output after the transfer (A6.4), not the law's response after it"
    );
    for requirement in &start.plan.requirements {
        let on_a_scan = |edge: f64| {
            (0..=run.duration_ms / run.step_ms)
                .any(|scan| close(edge, seconds_of(scan * run.step_ms), 1e-9))
        };
        assert!(
            on_a_scan(requirement.window.start) && on_a_scan(requirement.window.end),
            "{}: a window's edges lie on the run's sample times, or the evaluation refuses the run (N8.4)",
            requirement.id
        );
    }
    let (auto, _, auto_window) = bounds_of(&start.plan.requirements, "auto-mode");
    assert!(close(auto, 1.0, 1e-12));
    assert!(
        close(auto_window.start, 0.03, 1e-12),
        "the Auto mode is checked from the transfer"
    );
}

#[test]
fn the_parallel_pid_allowance_includes_the_integral_and_derivative_terms_of_a6_4() {
    // A6.4 with Kp 1, Ki 2, Kd 3, cycle 0.01 s and noise 0.05: 2·Kp·n + Ki·Δt·n + 4·Kd·n/Δt = 0.1 + 0.001 + 60 = 60.101.
    // The TypeScript left out the integral and derivative terms, which the specification requires.
    let start = pid_initialization_plan(
        &parallel_pid_loop(),
        &model_at(0.0, 2.0),
        &requirements(),
        &gains(1.0, 2.0, 3.0),
        Some(0.05),
    )
    .expect("the start");
    assert!(
        close(start.plan.tolerance, 60.101, 1e-9),
        "tolerance {}",
        start.plan.tolerance
    );
}

#[test]
fn a_proportional_start_refuses_a_reviewed_tolerance_beyond_the_output_span() {
    let mut loop_definition: LoopDefinition = proportional_loop();
    loop_definition.limits.output_maximum = Some(40.0);
    let task = engineer_requirements(&json!({
        "referenceChange": 1, "maximumError": 0.1, "settlingBand": 0.02, "settlingTimeSeconds": 2,
        "durationSeconds": 4, "transferTolerance": 41
    }))
    .expect("the requirements");
    let refusal = proportional_initialization_plan(
        &loop_definition,
        &model_at(0.0, 2.0),
        &task,
        &gains(36.5, 0.0, 0.0),
        None,
    )
    .expect_err("41 is beyond the 40 span");
    assert!(refusal.message.contains("output span"));
}

#[test]
fn a_proportional_start_aligns_its_reference_so_the_first_output_equals_the_held_output() {
    // r = PV + MV / Kp = 10 + 2 / 36.5.
    let plan = proportional_initialization_plan(
        &proportional_loop(),
        &model_at(2.0, 10.0),
        &requirements(),
        &gains(36.5, 0.0, 0.0),
        None,
    )
    .expect("the start");
    assert!(close(plan.reference, 10.0 + 2.0 / 36.5, 1e-12));
    assert!(
        close(plan.run.initial_values["LevelSP"], 10.0, 1e-12),
        "the setpoint starts at the process value"
    );
    assert!(
        close(plan.run.initial_values["Main.ManualCommand"], 2.0, 1e-12),
        "the command starts at the applied output"
    );
    let aligned = plan
        .run
        .events
        .iter()
        .find(|event| event.symbol == "LevelSP")
        .expect("the setpoint is aligned at the transfer");
    assert!(close(aligned.value, 10.0 + 2.0 / 36.5, 1e-12));
    assert!(
        close(plan.tolerance, 1e-4, 1e-12),
        "without a noise measurement the exact check applies"
    );
}

#[test]
fn a_proportional_start_allows_twice_the_gain_times_the_measured_noise_as_c13_states() {
    // C13 item 5: the allowance is 2·|Kp|·n, here 2 · 36.5 · 0.05 = 3.65.
    let plan = proportional_initialization_plan(
        &proportional_loop(),
        &model_at(2.0, 10.0),
        &requirements(),
        &gains(36.5, 0.0, 0.0),
        Some(0.05),
    )
    .expect("the start");
    assert!(close(plan.tolerance, 3.65, 1e-9));
}
