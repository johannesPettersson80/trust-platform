//! Tests of the controlled outputs of the Requirements editor (B4.3, C7): which outputs the editor lists, and what the rows of each
//! start from.

use std::collections::BTreeMap;

use super::{requirement_outputs, RequirementOutput};
use crate::loop_definition::{ChannelRequirements, LoopDefinition};
use crate::tuning::output_requirements::OutputRequirements;
use crate::tuning::requirements::TuningRequirements;
use crate::validation::engineer_verification_plan::output_task_of;

/// The shipped two-input, two-output loop (lesson 61): its outputs are the process variable `vessel-1` and a second vessel `vessel-2`,
/// and neither declares a setpoint.
fn mimo_loop() -> LoopDefinition {
    LoopDefinition::decode(include_str!(
        "../../../../examples/control/10-multivariable-control/61-mimo-paths/control-loop.json"
    ))
    .expect("lesson 61's loop")
}

/// Lesson 61's loop with a setpoint on each output, so that both are controlled. The second output declares `second` as its own
/// requirements, and the first `first`.
fn controlled_pair(
    first: Option<ChannelRequirements>,
    second: Option<ChannelRequirements>,
) -> LoopDefinition {
    let mut loop_definition = mimo_loop();
    let channels = loop_definition
        .channels
        .as_mut()
        .expect("the lesson declares channels");
    channels.outputs[0].setpoint_signal_id = Some("model-1".to_owned());
    channels.outputs[0].requirements = first;
    channels.outputs[1].setpoint_signal_id = Some("model-2".to_owned());
    channels.outputs[1].requirements = second;
    loop_definition
}

/// The shipped state-feedback loop (lesson 69): its first output, the upper tank, is only measured, and its second output, the process
/// variable, has the setpoint.
fn lqr_loop() -> LoopDefinition {
    LoopDefinition::decode(include_str!(
        "../../../../examples/control/11-state-space-estimation-and-predictive-control/69-lqr/control-loop.json"
    ))
    .expect("lesson 69's loop")
}

/// A requirement record with the setpoint change `reference_change`, a 5 % overshoot limit and the given record of each output.
fn record(
    reference_change: f64,
    channels: Option<BTreeMap<String, OutputRequirements>>,
) -> TuningRequirements {
    TuningRequirements {
        reference_change,
        maximum_error: 0.4,
        settling_band: 0.1,
        settling_time_seconds: 45.0,
        duration_seconds: 90.0,
        constant_input_disturbance: None,
        measurement_noise_band: None,
        gain_uncertainty_fraction: None,
        transfer_tolerance: None,
        maximum_overshoot_percent: Some(5.0),
        tuning_method: None,
        tuning_objective: None,
        automatic_criteria: None,
        channel_requirements: channels,
    }
}

/// An output's own record: a reverse step with a 7 % overshoot limit.
fn own_record() -> OutputRequirements {
    OutputRequirements {
        reference_change: -3.0,
        maximum_error: 0.8,
        settling_band: 0.2,
        settling_time_seconds: 30.0,
        maximum_overshoot_percent: Some(7.0),
    }
}

/// Requirements that a loop file declares for a channel.
fn declared() -> ChannelRequirements {
    ChannelRequirements {
        reference_change: 1.0,
        maximum_error: 0.3,
        settling_band: 0.1,
        settling_time_seconds: 40.0,
    }
}

/// What the rows of each listed output start from.
fn starts(
    loop_definition: &LoopDefinition,
    held: Option<&TuningRequirements>,
) -> Vec<Option<OutputRequirements>> {
    requirement_outputs(loop_definition, held)
        .into_iter()
        .map(|output| output.requirements)
        .collect()
}

#[test]
fn a_loop_without_channels_lists_no_output() {
    let loop_definition = LoopDefinition::decode(include_str!(
        "../../../../examples/control-pid-commissioning/control-loop.json"
    ))
    .expect("the commissioning example's loop");
    assert!(
        loop_definition.channels.is_none(),
        "the example declares no channels"
    );
    assert_eq!(
        requirement_outputs(&loop_definition, Some(&record(2.0, None))),
        [] as [RequirementOutput; 0]
    );
}

#[test]
fn the_outputs_that_declare_a_setpoint_are_listed_and_a_measured_output_is_not() {
    let held = record(2.0, None);
    assert_eq!(
        requirement_outputs(&lqr_loop(), Some(&held)),
        vec![RequirementOutput {
            channel_id: "tank-2-level".to_owned(),
            signal_id: "processVariable".to_owned(),
            requirements: Some(OutputRequirements::of_record(&held)),
        }],
        "the measured upper tank has no requirements; the process variable takes the record's own values"
    );
}

#[test]
fn a_loop_whose_outputs_declare_no_setpoint_lists_every_output_in_channel_order() {
    let outputs = requirement_outputs(&mimo_loop(), None);
    let listed: Vec<(&str, &str)> = outputs
        .iter()
        .map(|output| (output.channel_id.as_str(), output.signal_id.as_str()))
        .collect();
    assert_eq!(
        listed,
        [("vessel-1", "processVariable"), ("vessel-2", "vessel-2")]
    );
    assert!(
        outputs.iter().all(|output| output.requirements.is_none()),
        "nothing is recorded or declared"
    );
}

#[test]
fn an_output_starts_from_its_own_record_in_the_requirements_the_study_holds() {
    let held = record(
        2.0,
        Some(BTreeMap::from([("vessel-2".to_owned(), own_record())])),
    );
    assert_eq!(
        starts(&controlled_pair(None, None), Some(&held)),
        [
            Some(OutputRequirements::of_record(&held)),
            Some(own_record())
        ],
        "the process variable has no record of its own, so it takes the record's own values"
    );
}

#[test]
fn an_output_without_a_record_starts_from_the_requirements_its_channel_declares() {
    let held = record(2.0, None);
    let channel = declared();
    assert_eq!(
        starts(&controlled_pair(None, Some(channel)), Some(&held)),
        [
            Some(OutputRequirements::of_record(&held)),
            Some(OutputRequirements::of_channel(channel))
        ]
    );
    assert_eq!(
        starts(&controlled_pair(Some(channel), None), Some(&held)),
        [Some(OutputRequirements::of_channel(channel)), None],
        "the declared requirements come before the record's own values, and an output that is not the process variable has no others"
    );
    assert_eq!(
        starts(&controlled_pair(None, Some(channel)), None),
        [None, Some(OutputRequirements::of_channel(channel))],
        "a study that holds no requirements yet still offers what the loop declares"
    );
}

#[test]
fn what_each_output_starts_from_is_what_validate_holds_it_to() {
    let held = record(
        2.0,
        Some(BTreeMap::from([("vessel-2".to_owned(), own_record())])),
    );
    let flat = record(2.0, None);
    let channel = declared();
    for (loop_definition, held) in [
        (controlled_pair(None, None), &held),
        (controlled_pair(None, None), &flat),
        (controlled_pair(None, Some(channel)), &flat),
        (controlled_pair(Some(channel), None), &flat),
        (controlled_pair(Some(channel), Some(channel)), &held),
    ] {
        let validated: Vec<Option<OutputRequirements>> = loop_definition
            .channels
            .iter()
            .flat_map(|channels| channels.outputs.iter())
            .map(|output| {
                let process_variable = output.signal_id == "processVariable";
                output_task_of(
                    held,
                    &output.model_channel_id,
                    output.requirements,
                    process_variable,
                )
                .as_ref()
                .map(OutputRequirements::of_record)
            })
            .collect();
        assert_eq!(starts(&loop_definition, Some(held)), validated);
    }
}
