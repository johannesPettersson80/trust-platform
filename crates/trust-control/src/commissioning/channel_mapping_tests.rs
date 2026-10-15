// The fixture's bounds and limits are exact values, which these tests compare on purpose.
#![allow(clippy::float_cmp)]

use super::*;
use crate::commissioning::fixture_loops::{control_pid_loop, first_order_model};
use crate::commissioning::transfer::TransferValueType;

#[test]
fn a_loop_without_channels_declares_its_setpoint_process_value_output_and_manual_command() {
    let channels =
        declared_channels(&control_pid_loop()).expect("the fixture declares its channels");
    assert_eq!(channels.reference.symbol, "LevelSP");
    assert_eq!(channels.reference.iec_type, TransferValueType::Lreal);
    assert_eq!(channels.process.symbol, "LevelPV");
    assert_eq!(channels.output.symbol, "ValveMV");
    assert_eq!(
        channels.output.minimum, 0.0,
        "the output takes the output limits"
    );
    assert_eq!(
        channels.output.maximum, 100.0,
        "the output takes the output limits"
    );
    assert_eq!(channels.command.symbol, "Main.ManualCommand");
    assert_eq!(channels.command.iec_type, TransferValueType::Lreal);
    let measured: Vec<String> = channels
        .measurements()
        .into_iter()
        .map(|member| member.symbol)
        .collect();
    assert_eq!(
        measured,
        vec!["LevelPV".to_owned()],
        "the measurement of a loop without channels is its process value"
    );
}

#[test]
fn a_loop_without_a_setpoint_or_with_channel_bindings_is_refused() {
    let mut without_setpoint = control_pid_loop();
    without_setpoint.signals.setpoint = None;
    assert!(
        declared_channels(&without_setpoint).is_err(),
        "the reference is needed"
    );
    let mut with_channels = control_pid_loop();
    with_channels.channels = Some(crate::loop_definition::LoopChannels {
        inputs: Vec::new(),
        outputs: Vec::new(),
        initial_state: None,
    });
    assert!(
        declared_channels(&with_channels).is_err(),
        "channels are parked"
    );
    assert!(
        connected_model_channels(&with_channels, &first_order_model()).is_err(),
        "channels are parked"
    );
}

#[test]
fn a_dimensionless_on_off_output_is_a_boolean_command_and_a_non_real_process_value_is_refused() {
    let mut on_off = control_pid_loop();
    on_off.signals.manipulated_variable.iec_type = "BOOL".to_owned();
    on_off.signals.manipulated_variable.unit = "1".to_owned();
    on_off.signals.manipulated_variable.range.minimum = 0.0;
    on_off.signals.manipulated_variable.range.maximum = 1.0;
    let channels = declared_channels(&on_off).expect("a dimensionless level is an actuator");
    assert_eq!(channels.output.iec_type, TransferValueType::Bool);
    let mut integer = control_pid_loop();
    integer.signals.process_variable.iec_type = "INT".to_owned();
    assert!(
        declared_channels(&integer).is_err(),
        "the process value is REAL or LREAL"
    );
}

#[test]
fn the_plant_connection_is_the_manipulated_variable_and_the_process_variable_of_a_one_input_one_output_model(
) {
    let loop_definition = control_pid_loop();
    let connected =
        connected_model_channels(&loop_definition, &first_order_model()).expect("the connection");
    assert_eq!(connected.input.symbol, "ValveMV");
    assert_eq!(connected.output.symbol, "LevelPV");
    assert_eq!(connected.input.range.maximum, 100.0);
}
