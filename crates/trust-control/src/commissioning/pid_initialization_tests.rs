// The contract's weights are exact values from the fixture, compared on purpose.
#![allow(clippy::float_cmp)]

use super::*;
use crate::commissioning::fixture_loops::{control_pid_loop, parallel_pid_loop};
use crate::failure::{Cause, NextAction};

#[test]
fn a_parallel_pid_loop_declares_its_integral_and_derivative_states_and_its_setpoint_weights() {
    let loop_definition = parallel_pid_loop();
    let contract = pid_initialization_adapter(&loop_definition).expect("the parallel contract");
    assert_eq!(contract.integral_state.symbol, "Main.Integral");
    assert_eq!(contract.derivative_state.symbol, "Main.PreviousError");
    assert_eq!(contract.proportional_setpoint_weight, 1.0);
    assert_eq!(contract.derivative_setpoint_weight, 0.0);
}

#[test]
fn a_control_pid_loop_has_no_parallel_contract_so_auto_is_blocked_until_a_method_is_chosen() {
    let failure =
        pid_initialization_adapter(&control_pid_loop()).expect_err("no parallel contract");
    assert!(matches!(failure.cause, Cause::ControllerCapability));
    assert!(matches!(failure.next_action, NextAction::ChooseMethod));
}
