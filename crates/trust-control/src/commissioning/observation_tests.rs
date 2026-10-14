#![allow(clippy::float_cmp)]

use serde_json::json;

use super::*;
use crate::commissioning::transfer::TransferValueType;
use crate::loop_definition::{AuxiliaryRole, AuxiliarySignal, Direction, Range};
use crate::validation::run_validation::{RunCheck, Window};

fn loop_signal(symbol: &str, unit: &str, minimum: f64, maximum: f64) -> LoopSignal {
    LoopSignal {
        symbol: symbol.to_owned(),
        address: None,
        iec_type: "LREAL".to_owned(),
        direction: Direction::Read,
        unit: unit.to_owned(),
        range: Range { minimum, maximum },
    }
}

fn auxiliary(id: &str, role: AuxiliaryRole, symbol: &str, unit: &str) -> AuxiliarySignal {
    AuxiliarySignal {
        id: id.to_owned(),
        channel: "shaft".to_owned(),
        role,
        symbol: symbol.to_owned(),
        address: None,
        iec_type: "LREAL".to_owned(),
        direction: Direction::Read,
        unit: unit.to_owned(),
        range: Range {
            minimum: 0.0,
            maximum: 100.0,
        },
    }
}

fn signals(setpoint: bool, actuator: bool, additional: Vec<AuxiliarySignal>) -> Signals {
    Signals {
        setpoint: setpoint.then(|| loop_signal("Main.SP", "degC", 0.0, 100.0)),
        process_variable: loop_signal("Main.PV", "degC", 0.0, 100.0),
        manipulated_variable: loop_signal("Main.MV", "%", 0.0, 100.0),
        actuator: actuator.then(|| loop_signal("Main.Act", "%", 0.0, 100.0)),
        additional: Some(additional),
    }
}

fn transfer_member(symbol: &str) -> TransferMember {
    TransferMember {
        symbol: symbol.to_owned(),
        iec_type: TransferValueType::Real,
        unit: "degC".to_owned(),
        minimum: -1000.0,
        maximum: 1000.0,
    }
}

/// A stored observation of the setpoint, the process variable and the manipulated variable, with one bounds requirement on the
/// process variable from 0 s to `window_end`.
fn observation_json(window_end: f64) -> Value {
    json!({
        "channels": [
            {"id": "Main.SP", "source": "setpoint", "unit": "degC"},
            {"id": "Main.PV", "source": "processVariable", "unit": "degC"},
            {"id": "Main.MV", "source": "manipulatedVariable", "unit": "%"}
        ],
        "requirements": [{
            "id": "pv-bounds", "channel": "Main.PV", "window": {"start": 0.0, "end": window_end},
            "kind": "bounds", "minimum": 0.0, "maximum": 10.0
        }]
    })
}

#[test]
fn the_channels_follow_the_loop_declaration_with_the_auxiliary_identifier_as_source() {
    let channels = observed_channels_of(&signals(
        true,
        true,
        vec![auxiliary("speed", AuxiliaryRole::State, "M1.Speed", "rpm")],
    ));
    let described: Vec<(&str, &str, &str)> = channels
        .iter()
        .map(|channel| {
            (
                channel.id.as_str(),
                channel.source.as_str(),
                channel.unit.as_str(),
            )
        })
        .collect();
    assert_eq!(
        described,
        vec![
            ("Main.SP", "setpoint", "degC"),
            ("Main.PV", "processVariable", "degC"),
            ("Main.MV", "manipulatedVariable", "%"),
            ("Main.Act", "actuator", "%"),
            ("M1.Speed", "speed", "rpm"),
        ]
    );
}

#[test]
fn an_open_loop_records_no_setpoint_and_so_no_setpoint_unit() {
    let channels = observed_channels_of(&signals(false, false, Vec::new()));
    let sources: Vec<&str> = channels
        .iter()
        .map(|channel| channel.source.as_str())
        .collect();
    assert_eq!(sources, vec!["processVariable", "manipulatedVariable"]);
}

#[test]
fn the_reference_target_is_the_operating_output_plus_the_reviewed_change() {
    let setpoint = loop_signal("Main.SP", "degC", 0.0, 100.0);
    assert_eq!(
        reference_targets(Some(&setpoint), 2.0, 1.0),
        BTreeMap::from([("Main.SP".to_owned(), 3.0)])
    );
    assert!(
        reference_targets(None, 2.0, 1.0).is_empty(),
        "an open loop has no reference target"
    );
}

#[test]
fn only_a_model_with_one_input_and_one_output_connects_to_a_loop_without_channel_bindings() {
    assert!(single_channel(1, 1));
    assert!(!single_channel(2, 1));
    assert!(!single_channel(1, 2));
    assert!(!single_channel(0, 1));
}

#[test]
fn the_target_is_the_setpoint_reference_target_rounded_to_its_transfer_member() {
    let setpoint = loop_signal("Main.SP", "degC", 0.0, 100.0);
    let member = transfer_member("Main.SP");
    let targets = BTreeMap::from([("Main.SP".to_owned(), 3.0)]);
    assert_eq!(
        target_of(&targets, Some(&setpoint), Some(&member)),
        rounded_transfer_value(&member, 3.0).map(Some)
    );
    assert_eq!(
        target_of(&targets, None, Some(&member)),
        Ok(None),
        "a loop without a setpoint has no target"
    );
    assert_eq!(
        target_of(&targets, Some(&setpoint), None),
        Ok(None),
        "a binding without a reference member has no target"
    );
    assert_eq!(
        target_of(&BTreeMap::new(), Some(&setpoint), Some(&member)),
        Ok(None),
        "a reference target that was not formed is no target"
    );
}

#[test]
fn a_four_hour_observation_decodes_on_the_record_grid_of_its_run() {
    // Ported from control-studio-scenario-duration.test.ts (A4.2, A7): the 4 h observation's requirements decode at a
    // record period. The TypeScript builds the observation from the loop; this test states an equivalent observation.
    assert!(valid_commissioning_observation(
        observation_json(14_400.0),
        14_400.0,
        100.0
    ));
    assert!(
        !valid_commissioning_observation(observation_json(14_401.0), 14_400.0, 100.0),
        "a requirement that ends after the run does not decode"
    );
}

#[test]
fn an_observation_longer_than_the_apply_limit_is_refused() {
    // 24 h is the limit (A7); 24 h and 1 s at a 100 ms task is over it.
    assert!(!valid_commissioning_observation(
        observation_json(86_401.0),
        86_401.0,
        100.0
    ));
}

#[test]
fn duplicate_channels_or_requirements_and_an_empty_requirement_list_are_refused() {
    let mut duplicate_channel = observation_json(10.0);
    duplicate_channel["channels"][1]["id"] = json!("Main.SP");
    assert!(!valid_commissioning_observation(
        duplicate_channel,
        10.0,
        100.0
    ));

    let mut duplicate_requirement = observation_json(10.0);
    let requirement = duplicate_requirement["requirements"][0].clone();
    duplicate_requirement["requirements"] = json!([requirement.clone(), requirement]);
    assert!(!valid_commissioning_observation(
        duplicate_requirement,
        10.0,
        100.0
    ));

    let mut none = observation_json(10.0);
    none["requirements"] = json!([]);
    assert!(!valid_commissioning_observation(none, 10.0, 100.0));
}

#[test]
fn a_requirement_on_a_channel_the_observation_does_not_record_is_refused() {
    let mut observed = observation_json(10.0);
    observed["requirements"][0]["channel"] = json!("Main.Missing");
    assert!(!valid_commissioning_observation(observed, 10.0, 100.0));
}

#[test]
fn an_observation_with_an_unknown_field_is_refused() {
    let mut observed = observation_json(10.0);
    observed["extra"] = json!(1);
    assert!(!valid_commissioning_observation(observed, 10.0, 100.0));
}

#[test]
fn each_channel_takes_its_scan_value_or_the_additional_channel_of_its_identifier_and_unit() {
    let scan = ResponseSample {
        elapsed_seconds: 1.0,
        scan: 7,
        controller_in_auto: true,
        setpoint: Some(3.0),
        process_value: None,
        applied_output: Some(12.0),
        quality: ChannelQuality::Forced,
        additional: vec![AuxiliaryValue {
            id: "speed".to_owned(),
            unit: "rpm".to_owned(),
            value: Some(100.0),
            quality: ChannelQuality::Good,
        }],
    };
    let channel = |id: &str, source: &str, unit: &str| ObservedChannel {
        id: id.to_owned(),
        source: source.to_owned(),
        unit: unit.to_owned(),
    };
    assert_eq!(
        observed_value(&scan, &channel("Main.SP", "setpoint", "degC")),
        ValidationValue {
            value: Some(3.0),
            quality: ChannelQuality::Forced,
        }
    );
    assert_eq!(
        observed_value(&scan, &channel("Main.PV", "processVariable", "degC")),
        ValidationValue {
            value: None,
            quality: ChannelQuality::Forced,
        },
        "a scan without a measurement keeps the scan's quality"
    );
    assert_eq!(
        observed_value(&scan, &channel("Main.MV", "manipulatedVariable", "%")),
        ValidationValue {
            value: Some(12.0),
            quality: ChannelQuality::Forced,
        }
    );
    assert_eq!(
        observed_value(&scan, &channel("M1.Speed", "speed", "rpm")),
        ValidationValue {
            value: Some(100.0),
            quality: ChannelQuality::Good,
        }
    );
    assert_eq!(
        observed_value(&scan, &channel("M1.Speed", "speed", "bar")),
        ValidationValue {
            value: None,
            quality: ChannelQuality::Bad,
        },
        "an additional channel of another unit is not this channel"
    );
    assert_eq!(
        observed_value(&scan, &channel("M1.Temp", "temp", "degC")),
        ValidationValue {
            value: None,
            quality: ChannelQuality::Bad,
        },
        "a channel the scan does not hold is missing"
    );
}

#[test]
fn the_recorded_data_assessment_reads_the_scan_as_the_observation_holds_it() {
    let scan = ResponseSample {
        elapsed_seconds: 2.5,
        scan: 9,
        controller_in_auto: false,
        setpoint: Some(4.0),
        process_value: Some(3.5),
        applied_output: None,
        quality: ChannelQuality::Good,
        additional: Vec::new(),
    };
    let sample = scan.commissioning_sample();
    assert!(!sample.in_auto);
    assert_eq!(sample.scan, 9);
    assert_eq!(sample.elapsed_seconds, 2.5);
    assert_eq!(sample.setpoint, Some(4.0));
    assert_eq!(sample.process_value, Some(3.5));
    assert_eq!(sample.applied_output, None);
}

#[test]
fn a_sample_time_is_rounded_to_the_nanosecond_as_the_record_holds_it() {
    assert!((round_nanoseconds(0.123_456_789_4) - 0.123_456_789).abs() < 1e-12);
    assert_eq!(round_nanoseconds(2.0), 2.0);
}

#[test]
fn the_observation_grid_holds_one_time_per_record_period_and_the_end_of_the_run() {
    // 10 s on a 100 ms task: 100 executions fit the sample limit one by one, so every execution is a sample.
    let times = observation_grid(10.0, 100.0).expect("a 10 s run has a grid");
    assert_eq!(times.len(), 101);
    assert_eq!(times.first().copied(), Some(0.0));
    assert_eq!(times.last().copied(), Some(10.0));
    assert!(
        observation_grid(10.0, 0.5).is_none(),
        "a fractional task period has no whole-millisecond grid"
    );
}

#[test]
fn a_requirement_on_a_recorded_channel_is_checked_on_the_run_it_is_given() {
    let channels = vec![ObservedChannel {
        id: "Main.PV".to_owned(),
        source: "processVariable".to_owned(),
        unit: "degC".to_owned(),
    }];
    let requirement = RunRequirement {
        id: "bounds".to_owned(),
        channel: "Main.PV".to_owned(),
        window: Window {
            start: 0.0,
            end: 10.0,
        },
        check: RunCheck::Bounds {
            minimum: 0.0,
            maximum: 10.0,
        },
    };
    let run = observation_run(
        &channels,
        Vec::new(),
        vec![0.0, 10.0],
        10.0,
        RunEvidence::PhysicalRecorded,
    );
    assert!(check_requirement(&requirement, &run).is_ok());
}
