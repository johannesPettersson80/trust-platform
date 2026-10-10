#![allow(clippy::float_cmp)] // Declared limits are compared exactly: they are typed, not computed.

use serde_json::{json, Value};

use super::*;
use crate::failure::NextAction;
use crate::loop_definition::{Direction, Topology};

/// The fixture loop of the acquisition-limits test: a manipulated variable in 0 to 100 %, a measured output in 20 to
/// 80 degC, output limits of 0 to 100, and a 20 s observation.
fn limits_loop() -> LoopDefinition {
    serde_json::from_value(json!({
        "schemaVersion": 2,
        "id": "limits",
        "name": "Limits",
        "revision": "sha256:limits",
        "projectId": "test",
        "runtimeId": "test",
        "controller": {
            "instance": "P1.Controller",
            "type": "Control.PID",
            "revision": "sha256:controller",
            "action": "direct",
            "taskPeriodMs": 100.0
        },
        "observedSampleTiming": { "nominalPeriodMs": 100.0 },
        "signals": {
            "manipulatedVariable": {
                "symbol": "P1.MV", "address": "%QW0", "iecType": "REAL", "direction": "write",
                "unit": "%", "range": { "minimum": 0.0, "maximum": 100.0 }
            },
            "processVariable": {
                "symbol": "P1.PV", "address": "%IW0", "iecType": "REAL", "direction": "read",
                "unit": "degC", "range": { "minimum": 20.0, "maximum": 80.0 }
            }
        },
        "limits": { "outputMinimum": 0.0, "outputMaximum": 100.0 },
        "experiment": {
            "execution": "control-studio", "kind": "observation", "amplitude": 10.0,
            "durationSeconds": 20.0, "seed": 1
        }
    }))
    .expect("the fixture loop decodes")
}

#[allow(clippy::needless_pass_by_value)] // json! only borrows its arguments.
fn settings_with(method: &str, ranges: Value) -> Value {
    let mut settings = json!({
        "method": method,
        "durationSeconds": 12.0,
        "amplitude": 0.0,
        "reuseRecordings": true,
    });
    if let (Some(object), Some(extra)) = (settings.as_object_mut(), ranges.as_object()) {
        for (key, value) in extra {
            object.insert(key.clone(), value.clone());
        }
    }
    settings
}

#[allow(clippy::needless_pass_by_value)] // json! only borrows its arguments.
fn automatic_settings(input: Value, output: Value) -> Value {
    settings_with(
        "automatic",
        json!({ "inputRange": input, "outputRange": output }),
    )
}

#[test]
fn the_native_experiment_is_constrained_to_the_reviewed_ranges_and_the_source_loop_is_not_changed()
{
    let source = limits_loop();
    let settings = decode_acquisition_settings(&automatic_settings(
        json!({ "minimum": 10.0, "maximum": 70.0 }),
        json!({ "minimum": 25.0, "maximum": 60.0 }),
    ))
    .expect("the settings decode");
    let bounded =
        configured_acquisition(&source, Some(&settings)).expect("the ranges are within the limits");
    assert_eq!(
        bounded.signals.manipulated_variable.range,
        Range {
            minimum: 10.0,
            maximum: 70.0
        }
    );
    assert_eq!(
        bounded.signals.process_variable.range,
        Range {
            minimum: 25.0,
            maximum: 60.0
        }
    );
    assert_eq!(bounded.limits.output_minimum, Some(10.0));
    assert_eq!(bounded.limits.output_maximum, Some(70.0));
    assert_eq!(
        source.signals.manipulated_variable.range,
        Range {
            minimum: 0.0,
            maximum: 100.0
        },
        "a study limit must not change the project's limits"
    );
}

#[test]
fn a_plant_input_limit_the_loop_does_not_set_is_not_filled_in_from_the_input_range() {
    let mut source = limits_loop();
    source.limits.output_maximum = None;
    let settings = decode_acquisition_settings(&automatic_settings(
        json!({ "minimum": 10.0, "maximum": 70.0 }),
        json!({ "minimum": 25.0, "maximum": 60.0 }),
    ))
    .expect("the settings decode");
    let bounded = configured_acquisition(&source, Some(&settings))
        .expect("the ranges are within the declared ones");
    assert_eq!(bounded.limits.output_minimum, Some(10.0));
    assert_eq!(
        bounded.limits.output_maximum, None,
        "the missing limit stays missing, so Identify names it (B3.4 step 1)"
    );
}

#[test]
fn a_range_outside_the_configured_plant_limits_is_refused_and_named() {
    let below = decode_acquisition_settings(&automatic_settings(
        json!({ "minimum": -1.0, "maximum": 100.0 }),
        json!({ "minimum": 25.0, "maximum": 60.0 }),
    ))
    .expect("the settings decode");
    let error = configured_acquisition(&limits_loop(), Some(&below))
        .expect_err("the input is below its limit");
    assert!(
        matches!(error, AcquisitionError::RangeOutsideLimits { ref signal, .. } if signal == "P1.MV")
    );

    let above = decode_acquisition_settings(&automatic_settings(
        json!({ "minimum": 10.0, "maximum": 70.0 }),
        json!({ "minimum": 20.0, "maximum": 90.0 }),
    ))
    .expect("the settings decode");
    let error = configured_acquisition(&limits_loop(), Some(&above))
        .expect_err("the output is above its limit");
    assert!(
        matches!(error, AcquisitionError::RangeOutsideLimits { ref signal, .. } if signal == "P1.PV")
    );
    assert_eq!(
        error.failure().next_action,
        NextAction::OpenLimits,
        "a range outside its limits is fixed in Limits"
    );
}

#[test]
fn a_reversed_range_or_a_missing_bound_is_refused_before_planning() {
    assert!(
        decode_acquisition_settings(&automatic_settings(
            json!({ "minimum": 50.0, "maximum": 40.0 }),
            Value::Null,
        ))
        .is_none(),
        "a reversed range"
    );
    assert!(
        decode_acquisition_settings(&automatic_settings(
            json!({ "minimum": 0.0, "maximum": Value::Null }),
            Value::Null,
        ))
        .is_none(),
        "a bound that is not a number: JSON cannot carry an infinite bound"
    );
}

#[test]
fn the_duration_must_be_between_one_second_and_twenty_four_hours() {
    let with_duration = |seconds: f64| {
        decode_acquisition_settings(&json!({
            "method": "automatic", "durationSeconds": seconds, "amplitude": 0.0, "reuseRecordings": true
        }))
    };
    assert!(with_duration(0.5).is_none());
    assert!(with_duration(86_401.0).is_none());
    assert!(with_duration(86_400.0).is_some());
    assert!(with_duration(1.0).is_some());
}

#[test]
fn an_experiment_without_a_declared_duration_is_left_to_the_precondition_that_names_it() {
    let mut source = limits_loop();
    source.experiment = None;
    assert_eq!(
        configured_acquisition(&source, None).expect("the campaign's precondition names the field"),
        source,
        "a missing duration is not refused here (B3.4 step 1)"
    );
    let mut zero = limits_loop();
    if let Some(experiment) = zero.experiment.as_mut() {
        experiment.duration_seconds = Some(0.0);
    }
    let error = configured_acquisition(&zero, None).expect_err("a zero duration is not a duration");
    assert_eq!(error, AcquisitionError::LimitsRequired);
    let failure = error.failure();
    assert_eq!(
        failure.message,
        "Set the maximum experiment duration in Limits before identification."
    );
    assert_eq!(failure.next_action, NextAction::OpenLimits);
    assert!(
        configured_acquisition(&limits_loop(), None).is_ok(),
        "a declared duration needs no settings"
    );
}

#[test]
fn a_relay_needs_its_first_direction() {
    let relay_without_direction = settings_with("relay", json!({}));
    assert!(decode_acquisition_settings(&relay_without_direction).is_none());
    let relay = settings_with("relay", json!({ "relayDirection": "positive" }));
    assert!(decode_acquisition_settings(&relay).is_some());
}

/// The open loop of the setpoint test: no setpoint of its own, and a declared further signal that is a setpoint in
/// the measured output's unit.
fn open_loop() -> LoopDefinition {
    let mut loop_definition = limits_loop();
    loop_definition.id = "limits-reference-selection".to_string();
    loop_definition.topology = Some(Topology::OpenLoop);
    loop_definition.signals.setpoint = None;
    loop_definition.signals.additional = Some(vec![AuxiliarySignal {
        id: "additional-reference".to_string(),
        channel: "primary".to_string(),
        role: AuxiliaryRole::Setpoint,
        symbol: "Main.Reference".to_string(),
        address: Some("%ML64".to_string()),
        iec_type: "LREAL".to_string(),
        direction: Direction::Read,
        unit: "degC".to_string(),
        range: Range {
            minimum: 20.0,
            maximum: 80.0,
        },
    }]);
    loop_definition
}

fn setpoint_settings(id: &str) -> AcquisitionSettings {
    decode_acquisition_settings(&json!({
        "method": "setpoint_step",
        "durationSeconds": 10.0,
        "amplitude": 1.0,
        "reuseRecordings": true,
        "setpointSignalId": id,
    }))
    .expect("the settings decode")
}

#[test]
fn only_a_declared_reference_is_used_and_the_controller_is_not_changed() {
    let loop_definition = open_loop();
    let configured = configured_acquisition(
        &loop_definition,
        Some(&setpoint_settings("additional-reference")),
    )
    .expect("the declared reference is used");
    assert_eq!(
        configured
            .signals
            .setpoint
            .as_ref()
            .map(|signal| signal.symbol.as_str()),
        Some("Main.Reference")
    );
    assert!(
        loop_definition.signals.setpoint.is_none(),
        "the source loop keeps no setpoint"
    );
    assert_eq!(configured.controller, loop_definition.controller);

    for id in ["unknown", "manipulatedVariable"] {
        assert_eq!(
            configured_acquisition(&loop_definition, Some(&setpoint_settings(id))),
            Err(AcquisitionError::SetpointReference),
            "{id} is not a declared setpoint"
        );
    }
    let mut wrong_unit = open_loop();
    if let Some(signals) = wrong_unit.signals.additional.as_mut() {
        signals[0].unit = "wrong-unit".to_string();
    }
    assert_eq!(
        configured_acquisition(
            &wrong_unit,
            Some(&setpoint_settings("additional-reference"))
        ),
        Err(AcquisitionError::SetpointReference),
        "the reference must be in the measured output's unit"
    );
}

#[test]
fn a_configured_method_keeps_the_loop_kind_and_an_automatic_one_observes() {
    let mut loop_definition = limits_loop();
    if let Some(experiment) = loop_definition.experiment.as_mut() {
        experiment.kind = ExperimentKind::MvStep;
    }
    let configured = decode_acquisition_settings(&settings_with("configured", json!({})))
        .expect("the settings decode");
    let kept = configured_acquisition(&loop_definition, Some(&configured)).expect("configured");
    assert_eq!(
        kept.experiment.as_ref().map(|experiment| experiment.kind),
        Some(ExperimentKind::MvStep)
    );
    let automatic = decode_acquisition_settings(&settings_with("automatic", json!({})))
        .expect("the settings decode");
    let observed = configured_acquisition(&loop_definition, Some(&automatic)).expect("automatic");
    assert_eq!(
        observed
            .experiment
            .as_ref()
            .map(|experiment| experiment.kind),
        Some(ExperimentKind::Observation)
    );
}

#[test]
fn a_further_signal_takes_its_requested_range_within_its_declared_one() {
    // The further-signal range of the measured-output companion, as `control-studio-acquisition-limits.test.ts` (its
    // multichannel test) asserts it. That test's manipulated-variable companion is checked through its channel actuator,
    // which belongs to stage V7, because channels are parked.
    let mut source = limits_loop();
    source.signals.additional = Some(vec![
        AuxiliarySignal {
            id: "manipulatedVariable2".to_string(),
            channel: "input-2".to_string(),
            role: AuxiliaryRole::ManipulatedVariable,
            symbol: "P1.MV2".to_string(),
            address: Some("%QW1".to_string()),
            iec_type: "REAL".to_string(),
            direction: Direction::Write,
            unit: "%".to_string(),
            range: Range {
                minimum: 10.0,
                maximum: 90.0,
            },
        },
        AuxiliarySignal {
            id: "processVariable2".to_string(),
            channel: "output-2".to_string(),
            role: AuxiliaryRole::ProcessVariable,
            symbol: "P1.PV2".to_string(),
            address: Some("%IW1".to_string()),
            iec_type: "REAL".to_string(),
            direction: Direction::Read,
            unit: "degC".to_string(),
            range: Range {
                minimum: 25.0,
                maximum: 75.0,
            },
        },
    ]);
    let settings = decode_acquisition_settings(&settings_with(
        "automatic",
        json!({
            "inputRange": { "minimum": 20.0, "maximum": 70.0 },
            "outputRange": { "minimum": 35.0, "maximum": 65.0 },
            "inputRanges": {
                "P1.MV": { "minimum": 20.0, "maximum": 70.0 },
                "P1.MV2": { "minimum": 25.0, "maximum": 65.0 }
            },
            "outputRanges": {
                "P1.PV": { "minimum": 35.0, "maximum": 65.0 },
                "P1.PV2": { "minimum": 40.0, "maximum": 60.0 }
            }
        }),
    ))
    .expect("the settings decode");
    let bounded = configured_acquisition(&source, Some(&settings))
        .expect("the further ranges are within their declared ranges");
    let further = bounded.signals.additional.as_deref().unwrap_or_default();
    assert_eq!(
        further[1].range,
        Range {
            minimum: 40.0,
            maximum: 60.0
        }
    );
}

// Specification 35, §0.1b and B3.2: the command line starts from the loop's declared limits.
#[test]
fn the_declared_settings_use_the_loops_maximum_duration_and_the_declared_amplitude() {
    let example = LoopDefinition::decode(include_str!(
        "../../../../examples/control-pid-commissioning/control-loop.json"
    ))
    .expect("the example loop decodes");
    let settings = declared_settings(&example, AcquisitionMethod::MvStep);
    assert_eq!(
        settings,
        AcquisitionSettings {
            method: AcquisitionMethod::MvStep,
            duration_seconds: 16.0,
            amplitude: 0.0,
            setpoint_signal_id: None,
            relay_direction: None,
            reuse_recordings: false,
            input_range: None,
            output_range: None,
            input_ranges: None,
            output_ranges: None,
        },
        "the example declares a 16 s experiment"
    );
    assert_eq!(
        configured_acquisition(&example, Some(&settings))
            .expect("the declared settings configure the loop")
            .experiment
            .map(|experiment| (experiment.kind, experiment.duration_seconds)),
        Some((ExperimentKind::MvStep, Some(16.0)))
    );
    let mut undeclared = example;
    undeclared.experiment = None;
    assert_eq!(
        declared_settings(&undeclared, AcquisitionMethod::Automatic).duration_seconds,
        0.0,
        "no declared duration"
    );

    // The declared amplitude and relay direction are kept, so a relay passes the settings' checks.
    let mut relay_loop = limits_loop();
    if let Some(experiment) = relay_loop.experiment.as_mut() {
        experiment.relay_direction = Some(RelayDirection::Positive);
    }
    let relay = declared_settings(&relay_loop, AcquisitionMethod::Relay);
    assert_eq!(
        (
            relay.duration_seconds,
            relay.amplitude,
            relay.relay_direction
        ),
        (20.0, 10.0, Some(RelayDirection::Positive))
    );
    let value = serde_json::to_value(&relay).expect("the settings serialize");
    assert_eq!(decode_acquisition_settings(&value), Some(relay));
}
