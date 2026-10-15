// The allowance tests compare exact formula values and the exact check on purpose.
#![allow(clippy::float_cmp)]

use super::*;
use crate::commissioning::fixture_loops::{
    parallel_pid_loop, project_p_loop, project_pid_loop, proportional_loop, requirements,
};
use serde_json::json;

/// The exact check of the fixture's 100 % output span: `1e-6 · 100`.
const EXACT: f64 = 1e-4;

fn gains(kp: f64, ki: f64, kd: f64) -> BTreeMap<String, f64> {
    BTreeMap::from([
        ("Main.Parameters.Kp".to_owned(), kp),
        ("Main.Parameters.Ki".to_owned(), ki),
        ("Main.Parameters.Kd".to_owned(), kd),
    ])
}

#[test]
fn the_noise_allowance_covers_the_proportional_integral_and_derivative_response_of_one_cycle() {
    // Transfer-noise test of the `TypeScript`: Kp·2n (P on Δy), Ki·Δt·n (I), Kd·4n/Δt (D on the change of the sampled rate).
    let loop_definition = parallel_pid_loop();
    let noise = 0.001;
    let terms = AllowanceTerms {
        integral_gain: Some(12.0),
        derivative_gain: Some(7.0),
        cycle_seconds: Some(0.02),
        entry_error: None,
    };
    let pid =
        noise_aware_transfer_tolerance(&loop_definition, None, Some(16.0), Some(noise), terms)
            .expect("the allowance");
    assert!(
        (pid - (2.0 * 16.0 * noise + 12.0 * 0.02 * noise + 4.0 * 7.0 * noise / 0.02)).abs() < 1e-12,
        "PID allowance {pid}"
    );

    let proportional = noise_aware_transfer_tolerance(
        &loop_definition,
        None,
        Some(16.0),
        Some(noise),
        AllowanceTerms::default(),
    )
    .expect("the allowance");
    assert!(
        (proportional - 2.0 * 16.0 * noise).abs() < 1e-12,
        "P alone keeps its allowance"
    );

    let reviewed =
        noise_aware_transfer_tolerance(&loop_definition, Some(0.5), Some(16.0), Some(noise), terms)
            .expect("the allowance");
    assert_eq!(reviewed, 0.5, "a reviewed tolerance wins");

    let without_noise = noise_aware_transfer_tolerance(
        &loop_definition,
        None,
        Some(16.0),
        None,
        AllowanceTerms {
            derivative_gain: Some(7.0),
            cycle_seconds: Some(0.02),
            ..AllowanceTerms::default()
        },
    )
    .expect("the allowance");
    assert!(
        (without_noise - EXACT).abs() < 1e-12,
        "without measured noise the check stays exact ({without_noise}); the fixture's span is 100 %"
    );
}

#[test]
fn a_zero_or_missing_noise_and_a_non_finite_gain_keep_the_exact_check() {
    let loop_definition = parallel_pid_loop();
    for noise in [None, Some(0.0), Some(f64::NAN)] {
        let allowance = noise_aware_transfer_tolerance(
            &loop_definition,
            None,
            Some(16.0),
            noise,
            AllowanceTerms::default(),
        )
        .expect("the allowance");
        assert!(
            (allowance - EXACT).abs() < 1e-12,
            "{noise:?} keeps the exact check"
        );
    }
    let allowance = noise_aware_transfer_tolerance(
        &loop_definition,
        None,
        Some(f64::INFINITY),
        Some(0.05),
        AllowanceTerms::default(),
    )
    .expect("the allowance");
    assert!(
        (allowance - EXACT).abs() < 1e-12,
        "an infinite gain has no allowance"
    );
}

#[test]
fn the_transfer_tolerance_is_exact_unless_reviewed_and_must_lie_inside_the_span() {
    let loop_definition = parallel_pid_loop();
    assert!((transfer_tolerance(&loop_definition, None).expect("exact") - EXACT).abs() < 1e-12);
    assert_eq!(transfer_tolerance(&loop_definition, Some(2.5)), Ok(2.5));
    assert_eq!(
        transfer_tolerance(&loop_definition, Some(100.0)),
        Ok(100.0),
        "the span itself is allowed"
    );

    for outside in [-0.1, 100.5, f64::NAN] {
        let failure =
            transfer_tolerance(&loop_definition, Some(outside)).expect_err("outside the span");
        assert_eq!(
            failure.message,
            "Auto-entry tolerance must be between zero and the declared output span."
        );
        assert_eq!(
            (failure.cause, failure.next_action),
            (Cause::Settings, NextAction::OpenRequirements)
        );
    }
}

#[test]
fn the_entry_allowance_of_a_pid_is_the_noise_of_its_proportional_gain_at_entry() {
    // Kp 1, Ki 0, Kd 0 and a noise of 0.05: the allowance is 2·Kp·n, as A6.4 gives for the restored gains.
    let loop_definition = parallel_pid_loop();
    let allowance = auto_entry_tolerance(
        &loop_definition,
        &requirements(),
        &gains(1.0, 0.0, 0.0),
        Some(0.05),
        Some(0.0),
    )
    .expect("the allowance");
    assert!(
        (allowance - 0.1).abs() < 1e-12,
        "2 · 1 · 0.05 = {allowance}"
    );
}

#[test]
fn a_reviewed_task_tolerance_replaces_the_allowance_of_a_pid_and_a_proportional_loop() {
    let mut task = requirements();
    task.transfer_tolerance = Some(0.3);
    let pid = auto_entry_tolerance(
        &parallel_pid_loop(),
        &task,
        &gains(1.0, 0.0, 0.0),
        Some(0.05),
        None,
    )
    .expect("the reviewed tolerance");
    assert_eq!(pid, 0.3);

    let proportional = auto_entry_tolerance(
        &proportional_loop(),
        &task,
        &gains(2.0, 0.0, 0.0),
        Some(0.05),
        None,
    )
    .expect("the reviewed tolerance");
    assert_eq!(proportional, 0.3);
}

#[test]
fn a_proportional_loop_allows_two_gain_times_noise_and_a_loop_without_a_law_keeps_the_exact_check()
{
    let proportional = auto_entry_tolerance(
        &proportional_loop(),
        &requirements(),
        &gains(2.0, 0.0, 0.0),
        Some(0.01),
        None,
    )
    .expect("the allowance");
    assert!(
        (proportional - 0.04).abs() < 1e-12,
        "2 · 2 · 0.01 = {proportional}"
    );

    let mut without_law = parallel_pid_loop();
    without_law.controller.law = None;
    let allowance = auto_entry_tolerance(
        &without_law,
        &requirements(),
        &gains(2.0, 0.0, 0.0),
        Some(0.01),
        None,
    )
    .expect("the exact check");
    assert!((allowance - EXACT).abs() < 1e-12);
}

#[test]
fn a_loop_that_declares_state_feedback_is_refused_until_the_rust_engine_runs_it() {
    let mut loop_definition = parallel_pid_loop();
    loop_definition.controller.state_feedback = Some(json!({}));
    let failure = auto_entry_tolerance(
        &loop_definition,
        &requirements(),
        &gains(1.0, 0.0, 0.0),
        Some(0.05),
        None,
    )
    .expect_err("state feedback is parked");
    assert_eq!(
        failure.message,
        "This loop uses state feedback, which Control Studio's Rust engine does not run yet."
    );
    assert_eq!(failure.cause, Cause::ControllerCapability);
}

#[test]
fn the_entry_error_is_the_setpoint_less_the_measurement_when_both_are_numbers() {
    let loop_definition = parallel_pid_loop();
    let values = BTreeMap::from([
        ("LevelSP".to_owned(), TransferValue::Number(2.0)),
        ("LevelPV".to_owned(), TransferValue::Number(1.5)),
    ]);
    assert_eq!(auto_entry_error(&loop_definition, &values), Some(0.5));

    let without_measurement = BTreeMap::from([("LevelSP".to_owned(), TransferValue::Number(2.0))]);
    assert_eq!(
        auto_entry_error(&loop_definition, &without_measurement),
        None
    );

    let boolean_setpoint = BTreeMap::from([
        ("LevelSP".to_owned(), TransferValue::Boolean(true)),
        ("LevelPV".to_owned(), TransferValue::Number(1.5)),
    ]);
    assert_eq!(
        auto_entry_error(&loop_definition, &boolean_setpoint),
        None,
        "a BOOL is not a setpoint"
    );
}

#[test]
fn a_program_pi_law_takes_its_allowance_from_the_gains_of_its_seed_variables() {
    // A6.4 with the program's own variables: Kp 2, Ki 0.5 and Kd 0 on the 10 ms task, a noise of 0.05 and an entry error of 2:
    // 2·Kp·n = 0.2 from P, Ki·Δt·(|e| + n) = 0.5 · 0.01 · 2.05 = 0.01025 from I, and nothing from D.
    let loop_definition = project_pid_loop();
    let values = BTreeMap::from([
        ("Main.Program.Kp".to_owned(), 2.0),
        ("Main.Program.Ki".to_owned(), 0.5),
        ("Main.Program.Kd".to_owned(), 0.0),
    ]);
    let allowance = auto_entry_tolerance(
        &loop_definition,
        &requirements(),
        &values,
        Some(0.05),
        Some(2.0),
    )
    .expect("the allowance");
    assert!((allowance - 0.21025).abs() < 1e-12, "{allowance}");
}

#[test]
fn a_program_p_law_takes_the_connected_p_allowance_of_its_seed_variable() {
    // C13 step 5: 2·|Kp|·n with Ki and Kd zero, so 2 · 2 · 0.05 = 0.2.
    let loop_definition = project_p_loop();
    let values = BTreeMap::from([("Main.Program.Kp".to_owned(), 2.0)]);
    let allowance =
        auto_entry_tolerance(&loop_definition, &requirements(), &values, Some(0.05), None)
            .expect("the allowance");
    assert!((allowance - 0.2).abs() < 1e-12, "{allowance}");
}
