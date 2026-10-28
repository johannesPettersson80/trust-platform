use super::*;
use crate::commissioning::fixture_loops::{control_pid_loop, first_order_model, parallel_pid_loop};
use crate::loop_definition::PidInitialization;

fn close(actual: f64, expected: f64) -> bool {
    (actual - expected).abs() <= 1e-12
}

#[test]
fn the_history_starts_at_the_operating_point_and_the_first_derivative_contribution_is_zero() {
    // The fixture's setpoint weight β is 1 and derivative weight γ is 0, at the operating output y₀ = 2 and command u₀ = 0.
    let values = BTreeMap::from([("Main.Parameters.Kp".to_owned(), 1.0)]);
    let history = pid_operating_state(&parallel_pid_loop(), &first_order_model(), &values)
        .expect("the history");
    assert!(
        close(history["Main.Integral"], 0.0),
        "u₀ − Kp·(β − 1)·y₀ = 0"
    );
    assert!(close(history["Main.PreviousError"], -2.0), "(γ − 1)·y₀ = −2, so the first derivative term, c·r₀ − y₀ with r₀ = y₀, is zero when c is 0");
}

#[test]
fn a_weighted_proportional_term_and_a_derivative_weight_change_the_history() {
    let mut loop_definition = parallel_pid_loop();
    if let Some(PidInitialization::ParallelPid {
        proportional_setpoint_weight,
        derivative_setpoint_weight,
        ..
    }) = loop_definition.controller.pid_initialization.as_mut()
    {
        *proportional_setpoint_weight = 0.5;
        *derivative_setpoint_weight = 1.0;
    }
    let values = BTreeMap::from([("Main.Parameters.Kp".to_owned(), 1.0)]);
    let history =
        pid_operating_state(&loop_definition, &first_order_model(), &values).expect("the history");
    // u₀ − Kp·(β − 1)·y₀ = 0 − 1·(0.5 − 1)·2 = 1, and (γ − 1)·y₀ = 0.
    assert!(close(history["Main.Integral"], 1.0));
    assert!(close(history["Main.PreviousError"], 0.0));
}

#[test]
fn a_history_needs_a_finite_proportional_gain_and_a_control_pid_has_none() {
    let refusal = pid_operating_state(&parallel_pid_loop(), &first_order_model(), &BTreeMap::new())
        .expect_err("the gain is needed");
    assert!(refusal.message.contains("finite PID gain"));
    let none = pid_operating_state(&control_pid_loop(), &first_order_model(), &BTreeMap::new())
        .expect("no history");
    assert!(none.is_empty(), "control-pid-v1 has no parallel history");
}
