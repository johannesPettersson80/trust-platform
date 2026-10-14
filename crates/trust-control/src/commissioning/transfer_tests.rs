// The rounding tests compare the values IEC rounding produces, exactly, on purpose.
#![allow(clippy::float_cmp)]

use super::*;
use crate::commissioning::fixture_loops::{control_pid_loop, parallel_pid_loop, proportional_loop};
use crate::loop_definition::ManualOutputType;

fn member(iec_type: TransferValueType, minimum: f64, maximum: f64) -> TransferMember {
    TransferMember {
        symbol: "Main.Value".into(),
        iec_type,
        unit: "%".into(),
        minimum,
        maximum,
    }
}

#[test]
fn a_real_is_rounded_to_single_precision_and_an_lreal_is_kept() {
    let single = member(TransferValueType::Real, 0.0, 100.0);
    assert_eq!(rounded_transfer_value(&single, 0.1), Ok(f64::from(0.1_f32)));
    let double = member(TransferValueType::Lreal, 0.0, 100.0);
    assert_eq!(rounded_transfer_value(&double, 0.1), Ok(0.1));
}

#[test]
fn a_value_is_checked_against_its_bounds_after_rounding() {
    let outside = member(TransferValueType::Real, 0.0, 100.0);
    let failure = rounded_transfer_value(&outside, 100.5).expect_err("above the maximum");
    assert_eq!(
        failure.message,
        "Main.Value is outside its declared range after IEC rounding."
    );
    assert_eq!(failure.cause, Cause::Settings);

    // 0.1 is below its REAL maximum as a double, but its single-precision value is above it: IEC rounding decides.
    let tight = member(TransferValueType::Real, 0.0, 0.1);
    assert!(
        rounded_transfer_value(&tight, 0.1).is_err(),
        "the rounded value leaves the bounds"
    );
    let non_finite = member(TransferValueType::Lreal, 0.0, 100.0);
    assert!(
        rounded_transfer_value(&non_finite, f64::NAN).is_err(),
        "NaN is not a value"
    );
}

#[test]
fn a_bool_state_takes_only_zero_or_one() {
    let flag = member(TransferValueType::Bool, 0.0, 1.0);
    assert_eq!(rounded_transfer_value(&flag, 0.0), Ok(0.0));
    assert_eq!(rounded_transfer_value(&flag, 1.0), Ok(1.0));
    let failure = rounded_transfer_value(&flag, 0.5).expect_err("a BOOL is 0 or 1");
    assert_eq!(failure.message, "BOOL transfer state must be 0 or 1.");
    assert_eq!(
        (failure.cause, failure.next_action),
        (Cause::Runtime, NextAction::None)
    );
}

#[test]
fn a_transfer_value_is_a_number_only_when_it_is_one() {
    assert_eq!(TransferValue::Number(2.5).as_number(), Some(2.5));
    assert_eq!(TransferValue::Boolean(true).as_number(), None);
    assert_eq!(
        TransferValueType::from_iec("LREAL"),
        Some(TransferValueType::Lreal)
    );
    assert_eq!(
        TransferValueType::from_iec("INT"),
        None,
        "only REAL, LREAL and BOOL"
    );
}

#[test]
fn the_proportional_binding_writes_the_setpoint_and_the_manual_command_and_holds_the_output() {
    let binding = proportional_transfer_binding(&proportional_loop()).expect("the fixture binds");
    let symbols = |members: &[TransferMember]| -> Vec<String> {
        members.iter().map(|member| member.symbol.clone()).collect()
    };
    assert_eq!(
        symbols(binding.states.as_slice()),
        ["LevelSP", "Main.ManualCommand"]
    );
    let manual = &binding.states[1];
    assert_eq!(manual.iec_type, TransferValueType::Lreal);
    assert_eq!(
        (manual.minimum, manual.maximum),
        (0.0, 100.0),
        "the output limits"
    );
    assert_eq!(binding.output.symbol, "ValveMV");
    assert_eq!(
        (binding.output.minimum, binding.output.maximum),
        (0.0, 100.0)
    );
    assert_eq!(
        symbols(binding.guards.as_slice()),
        ["Main.Parameters.Kp", "LevelPV", "ValvePosition"],
        "the gain, the process value, then the measured actuator"
    );
    assert_eq!(
        binding.mode,
        TransferModeBinding {
            symbol: "Main.Auto".into(),
            manual_value: false
        }
    );
    assert_eq!(
        binding.tracking_outputs,
        BTreeMap::from([("ValveMV".to_owned(), "ValvePosition".to_owned())])
    );
}

#[test]
fn the_pid_binding_adds_the_integral_and_derivative_states_after_the_manual_command() {
    let binding = pid_transfer_binding(&parallel_pid_loop()).expect("the fixture binds");
    let symbols: Vec<&str> = binding
        .states
        .iter()
        .map(|member| member.symbol.as_str())
        .collect();
    assert_eq!(
        symbols,
        [
            "LevelSP",
            "Main.ManualCommand",
            "Main.Integral",
            "Main.PreviousError"
        ]
    );
    let guards: Vec<&str> = binding
        .guards
        .iter()
        .map(|member| member.symbol.as_str())
        .collect();
    assert_eq!(
        guards,
        [
            "Main.Parameters.Kp",
            "Main.Parameters.Ki",
            "Main.Parameters.Kd",
            "LevelPV",
            "ValvePosition"
        ]
    );
}

#[test]
fn a_control_pid_whose_states_the_library_declares_has_no_parallel_pid_contract() {
    let failure = pid_transfer_binding(&control_pid_loop())
        .expect_err("control-pid-v1 is not parallel-pid-v1");
    assert_eq!(
        failure.message,
        "Typed PI/PID initialization adapter is required for Auto commissioning; choose Manual staging after Verify."
    );
    assert_eq!(
        (failure.cause, failure.next_action),
        (Cause::ControllerCapability, NextAction::ChooseMethod)
    );
}

#[test]
fn a_pid_loop_is_not_a_proportional_commissioning_and_a_bool_command_is_not_transferred() {
    let failure = proportional_transfer_binding(&parallel_pid_loop()).expect_err("a PID law");
    assert_eq!(
        failure.message,
        "Confirm the proportional law, gain binding and setpoint before tuning."
    );

    let mut bool_command = proportional_loop();
    bool_command.controller.manual_output_type = Some(ManualOutputType::Bool);
    let failure = proportional_transfer_binding(&bool_command).expect_err("a BOOL command");
    assert_eq!(
        failure.message,
        "Configure the compiled reference, mode and manual-command type before Auto commissioning."
    );
    assert_eq!(failure.next_action, NextAction::OpenLoopSetup);
}

#[test]
fn an_output_without_its_limits_is_refused_with_the_limits_as_the_next_action() {
    let mut loop_definition = parallel_pid_loop();
    loop_definition.limits.output_minimum = None;
    let failure = pid_transfer_binding(&loop_definition).expect_err("no output minimum");
    assert_eq!(
        failure.message,
        "Auto commissioning needs the output minimum and maximum, and one of them is not set."
    );
    assert_eq!(
        failure.detail.as_deref(),
        Some("Auto commissioning needs the output minimum and maximum, and one of them is not set. Set both under Limits.")
    );
    assert_eq!(
        (failure.cause, failure.next_action),
        (Cause::Settings, NextAction::OpenLimits)
    );
}

#[test]
fn a_binding_keeps_the_names_the_protocol_uses_and_reads_back_unchanged() {
    let binding = pid_transfer_binding(&parallel_pid_loop()).expect("the fixture binds");
    let json = serde_json::to_value(&binding).expect("the binding serializes");
    assert_eq!(json["loop_id"], "loop:tank");
    assert_eq!(json["task_name"], "MainTask");
    assert_eq!(json["mode"]["manual_value"], false);
    assert_eq!(json["states"][1]["iec_type"], "LREAL");
    assert_eq!(json["tracking_outputs"]["ValveMV"], "ValvePosition");
    assert!(
        json.get("additional_modes").is_none(),
        "an empty optional list is omitted"
    );
    let back: TransferBinding = crate::json::decode_value(json).expect("the binding decodes");
    assert_eq!(back, binding);
}
