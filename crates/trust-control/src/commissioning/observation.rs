//! The channels and requirements of a commissioning observation, and the scan it is assessed on
//! (`commissioningObservation.ts`; specification 35, B6.2 and B6.3; numerics N8, A4.2, A7).
//!
//! The rules decide which signals an observation records and checks, which reference an Apply targets, the sample times
//! and values of the run the requirements are checked on, and whether a stored observation is valid. The multichannel
//! connection is left out except a PID group's reference targets (C7): any other loop with channel bindings is refused, as the Rust
//! engine refuses it (a parked feature, V7).

use std::collections::{BTreeMap, BTreeSet};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::initial_state::operating_output;
use super::texts;
use super::transfer::{rounded_transfer_value, TransferBinding, TransferMember};
use crate::failure::{Cause, Failure, NextAction};
use crate::identification::connected_record::{
    RunConditions, RunEvidence, ValidationChannel, ValidationRun, ValidationSample, ValidationValue,
};
use crate::json;
use crate::limits::{record_period_executions, APPLY_OBSERVATION_LIMIT_MS, RECORDING_SAMPLE_LIMIT};
use crate::loop_definition::{LoopDefinition, LoopSignal, Signals};
use crate::model::GeneralModel;
use crate::recording::channels::ChannelQuality;
use crate::validation::commissioning_assessment::CommissioningSample;
use crate::validation::engineer_verification_plan::{
    engineer_verification_plan, output_requirements_missing, output_task_of, EngineerRequirements,
};
use crate::validation::model_route::bound_channels;
use crate::validation::run_validation::{check_requirement, RunRequirement};

/// The source of the setpoint channel (`source` of `commissioningObservation`).
const SETPOINT: &str = "setpoint";
/// The source of the process-variable channel.
const PROCESS_VARIABLE: &str = "processVariable";
/// The source of the manipulated-variable channel.
const MANIPULATED_VARIABLE: &str = "manipulatedVariable";
/// The source of the actuator channel.
const ACTUATOR: &str = "actuator";
/// The most channels one observation records (A7: channels per recording).
const CHANNEL_LIMIT: usize = 32;
/// The most requirements one observation checks.
const REQUIREMENT_LIMIT: usize = 64;
/// The identifier of the run an observation's requirements are checked on; the run is not stored.
const OBSERVATION_RUN_ID: &str = "observation";

/// One signal an observation records (`channels` of `CommissioningObservation`): its identifier, its source in the loop and its
/// unit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ObservedChannel {
    /// The signal's symbol, which the requirements name.
    pub id: String,
    /// Where the value comes from: `setpoint`, `processVariable`, `manipulatedVariable`, `actuator`, or the identifier of an
    /// additional signal.
    pub source: String,
    /// The signal's unit.
    pub unit: String,
}

/// What an Apply observation records and checks (`CommissioningObservation`): its channels and its requirements. It is stored
/// with the criteria of the workflow result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CommissioningObservation {
    /// The recorded channels, in the order the loop declares them.
    pub channels: Vec<ObservedChannel>,
    /// The requirements the response must meet, each on one of the channels.
    pub requirements: Vec<RunRequirement>,
}

/// A recorded channel of a scan that is not the setpoint, the process variable or the manipulated variable: its identifier,
/// its unit, its value and its quality (`RecordedChannel`).
#[derive(Debug, Clone, PartialEq)]
pub struct AuxiliaryValue {
    /// The additional signal's identifier in the loop.
    pub id: String,
    /// The signal's unit.
    pub unit: String,
    /// The value at the scan, when it is recorded.
    pub value: Option<f64>,
    /// The value's quality at the scan.
    pub quality: ChannelQuality,
}

/// One scan of a commissioning response, with the values an observation reads (`WorkflowResultSample`).
#[derive(Debug, Clone, PartialEq)]
pub struct ResponseSample {
    /// The time of the scan in s from the start of the recording (`elapsedSeconds`).
    pub elapsed_seconds: f64,
    /// The scan's sequence value (`scan`).
    pub scan: u64,
    /// Whether the controller is in Auto at the scan (`controllerMode` is `Auto`).
    pub controller_in_auto: bool,
    /// The reference at the scan (`sp`), when it is recorded.
    pub setpoint: Option<f64>,
    /// The measurement at the scan (`pv`), when it is recorded.
    pub process_value: Option<f64>,
    /// The applied output at the scan (`appliedMv`), when it is recorded.
    pub applied_output: Option<f64>,
    /// The scan's quality (`quality`).
    pub quality: ChannelQuality,
    /// The additional channels the scan records (`additional`).
    pub additional: Vec<AuxiliaryValue>,
}

impl ResponseSample {
    /// The fields the recorded-data assessment reads, as its sample (`CommissioningSample`).
    #[must_use]
    pub fn commissioning_sample(&self) -> CommissioningSample {
        CommissioningSample {
            elapsed_seconds: self.elapsed_seconds,
            scan: self.scan,
            in_auto: self.controller_in_auto,
            setpoint: self.setpoint,
            process_value: self.process_value,
            applied_output: self.applied_output,
            quality: self.quality,
        }
    }
}

/// The channels an observation records, in the order the loop declares them (`commissioningObservation`, B6.3): the setpoint
/// when the loop has one, the process variable, the manipulated variable, the actuator when it has one, then each additional
/// signal with its identifier as the source. An open loop records no setpoint.
#[must_use]
pub fn observed_channels_of(signals: &Signals) -> Vec<ObservedChannel> {
    let mut channels = Vec::new();
    if let Some(setpoint) = &signals.setpoint {
        channels.push(channel_of(setpoint, SETPOINT));
    }
    channels.push(channel_of(&signals.process_variable, PROCESS_VARIABLE));
    channels.push(channel_of(
        &signals.manipulated_variable,
        MANIPULATED_VARIABLE,
    ));
    if let Some(actuator) = &signals.actuator {
        channels.push(channel_of(actuator, ACTUATOR));
    }
    for signal in signals.additional.iter().flatten() {
        channels.push(ObservedChannel {
            id: signal.symbol.clone(),
            source: signal.id.clone(),
            unit: signal.unit.clone(),
        });
    }
    channels
}

/// The observed channel of a loop signal with the given source.
#[must_use]
fn channel_of(signal: &LoopSignal, source: &str) -> ObservedChannel {
    ObservedChannel {
        id: signal.symbol.clone(),
        source: source.to_owned(),
        unit: signal.unit.clone(),
    }
}

/// The observation of an Apply (`commissioningObservation`, B6.3): the channels of the loop, and the requirements of its
/// validation plan that name one of those channels.
///
/// # Errors
/// Returns the refusal of the validation plan when the loop cannot be verified.
pub fn commissioning_observation(
    loop_definition: &LoopDefinition,
    model: &GeneralModel,
    task: &EngineerRequirements,
    values: &BTreeMap<String, f64>,
) -> Result<CommissioningObservation, Failure> {
    let channels = observed_channels_of(&loop_definition.signals);
    // The initial controller state is not given: the requirements do not depend on it, as the TypeScript does not pass it.
    let plan = engineer_verification_plan(loop_definition, model, task, values, &BTreeMap::new())?;
    let requirements: Vec<RunRequirement> = {
        let observed: BTreeSet<&str> = channels.iter().map(|channel| channel.id.as_str()).collect();
        plan.requirements
            .into_iter()
            .filter(|requirement| observed.contains(requirement.channel.as_str()))
            .collect()
    };
    Ok(CommissioningObservation {
        channels,
        requirements,
    })
}

/// The reference targets of a loop: for a loop without channel bindings, the setpoint at the output's operating point plus the reviewed
/// reference change (`commissioningReferenceTargets`, B6.2); an open loop has none. A PID group's targets are those of every controlled
/// output (C7).
///
/// # Errors
/// Returns a refusal when the loop has channel bindings that are not a PID group's, which the Rust engine does not run yet (V7); when a
/// group's channels do not bind the model, or one of its controlled outputs has no requirements; or when a loop without channels has a
/// model that is not one input and one output.
pub fn commissioning_reference_targets(
    loop_definition: &LoopDefinition,
    model: &GeneralModel,
    task: &EngineerRequirements,
) -> Result<BTreeMap<String, f64>, Failure> {
    if loop_definition.channels.is_some() {
        if loop_definition.controller.group.is_some() {
            return channel_reference_targets(loop_definition, model, task);
        }
        return Err(Failure::new(
            &texts::observe_channels_parked(),
            Cause::ControllerCapability,
            NextAction::None,
        ));
    }
    let identity = model.identity();
    if !single_channel(identity.inputs.len(), identity.outputs.len()) {
        return Err(Failure::new(
            &texts::observe_model_channels_unmapped(),
            Cause::Unclassified,
            NextAction::None,
        ));
    }
    let operating_output = identity
        .operating_point
        .as_ref()
        .and_then(|point| point.outputs.first())
        .copied()
        .unwrap_or(0.0);
    Ok(reference_targets(
        loop_definition.signals.setpoint.as_ref(),
        operating_output,
        task.reference_change,
    ))
}

/// The reference targets of a loop with channel bindings (`commissioningReferenceTargets`, B6.2, C7): for each output that has a
/// setpoint, the setpoint at the output's operating point plus the reference change the output is held to. The output is held to its own
/// record in the task, else to its channel's own requirements, else, for the loop's process variable, to the task itself, as Validate
/// holds it (`output_task_of`), so that Apply and Validate aim at the same targets.
///
/// # Errors
/// The channels do not bind the model (see `bound_channels`), or an output with a setpoint has no requirements to hold it to.
fn channel_reference_targets(
    loop_definition: &LoopDefinition,
    model: &GeneralModel,
    task: &EngineerRequirements,
) -> Result<BTreeMap<String, f64>, Failure> {
    let bound = bound_channels(loop_definition, model)?;
    let process_variable = loop_definition.signals.process_variable.symbol.as_str();
    let mut targets = BTreeMap::new();
    for (index, output) in bound.outputs.iter().enumerate() {
        let Some(reference) = output.reference else {
            continue;
        };
        let required = output_task_of(
            task,
            &output.channel.id,
            output.requirements,
            output.signal.symbol == process_variable,
        )
        .ok_or_else(|| output_requirements_missing(&output.channel.id))?;
        targets.insert(
            reference.symbol.to_owned(),
            operating_output(model, index) + required.reference_change,
        );
    }
    Ok(targets)
}

/// Whether a model has exactly one input and one output, the only shape a loop without channel bindings connects.
#[must_use]
fn single_channel(inputs: usize, outputs: usize) -> bool {
    inputs == 1 && outputs == 1
}

/// The reference targets of a loop without channel bindings: the setpoint, when the loop has one, at the operating output plus
/// the reviewed change.
#[must_use]
fn reference_targets(
    setpoint: Option<&LoopSignal>,
    operating_output: f64,
    reference_change: f64,
) -> BTreeMap<String, f64> {
    setpoint.map_or_else(BTreeMap::new, |reference| {
        BTreeMap::from([(
            reference.symbol.clone(),
            operating_output + reference_change,
        )])
    })
}

/// The reviewed Auto target of the setpoint, rounded to its IEC type as the transfer writes it and the runtime records it
/// (`commissioningTarget`, B6.3). None when the loop has no setpoint or the binding has no reference member.
///
/// # Errors
/// Returns the refusal of [`commissioning_reference_targets`].
pub fn commissioning_target(
    loop_definition: &LoopDefinition,
    model: &GeneralModel,
    task: &EngineerRequirements,
    binding: &TransferBinding,
) -> Result<Option<f64>, Failure> {
    let targets = commissioning_reference_targets(loop_definition, model, task)?;
    target_of(
        &targets,
        loop_definition.signals.setpoint.as_ref(),
        binding.states.first(),
    )
}

/// The rounded target of the setpoint, when the setpoint, its transfer member and its reference target all exist.
/// A target the transfer member cannot hold is refused by `rounded_transfer_value`.
fn target_of(
    targets: &BTreeMap<String, f64>,
    setpoint: Option<&LoopSignal>,
    member: Option<&TransferMember>,
) -> Result<Option<f64>, Failure> {
    let (Some(setpoint), Some(member)) = (setpoint, member) else {
        return Ok(None);
    };
    targets
        .get(&setpoint.symbol)
        .map(|target| rounded_transfer_value(member, *target))
        .transpose()
}

/// Whether a stored observation is valid for a run of `duration_seconds` at a task period of `step_ms`
/// (`validCommissioningObservation`; N8.1, N8.3, A4.2, A7). It has one to 32 channels with unique identifiers and non-empty
/// sources and units, one to 64 requirements with unique identifiers, a run within the Apply observation limit, and every
/// requirement that decodes on the sample grid of the run. The value must decode as an observation: an unknown field refuses it.
#[must_use]
pub fn valid_commissioning_observation(value: Value, duration_seconds: f64, step_ms: f64) -> bool {
    let Ok(observed) = json::decode_value::<CommissioningObservation>(value) else {
        return false;
    };
    if !channels_valid(&observed.channels) || !requirement_count_valid(&observed.requirements) {
        return false;
    }
    let Some(times) = observation_grid(duration_seconds, step_ms) else {
        return false;
    };
    let end = times.last().copied().unwrap_or(0.0);
    // The grid checks read only the channels, the sample times and the duration, so the evidence class does not matter here.
    let run = observation_run(
        &observed.channels,
        Vec::new(),
        times,
        end,
        RunEvidence::PhysicalRecorded,
    );
    observed
        .requirements
        .iter()
        .all(|requirement| check_requirement(requirement, &run).is_ok())
        && unique_requirement_ids(&observed.requirements)
}

/// Whether the channels of an observation are valid: a count from one to the channel limit, unique identifiers, and a
/// non-empty identifier, source and unit for each.
#[must_use]
fn channels_valid(channels: &[ObservedChannel]) -> bool {
    let identifiers: BTreeSet<&str> = channels.iter().map(|channel| channel.id.as_str()).collect();
    (1..=CHANNEL_LIMIT).contains(&channels.len())
        && identifiers.len() == channels.len()
        && channels.iter().all(|channel| {
            !channel.id.is_empty() && !channel.source.is_empty() && !channel.unit.is_empty()
        })
}

/// Whether the number of requirements of an observation is from one to the requirement limit.
#[must_use]
fn requirement_count_valid(requirements: &[RunRequirement]) -> bool {
    (1..=REQUIREMENT_LIMIT).contains(&requirements.len())
}

/// Whether every requirement of an observation has its own identifier.
#[must_use]
fn unique_requirement_ids(requirements: &[RunRequirement]) -> bool {
    let identifiers: BTreeSet<&str> = requirements
        .iter()
        .map(|requirement| requirement.id.as_str())
        .collect();
    identifiers.len() == requirements.len()
}

/// The sample times of an observation of `duration_seconds` at a task period of `step_ms`, as the recorder keeps them (A4.2):
/// one time per record period from the start, and then the end of the run. None when the run is negative or longer than the
/// Apply observation limit, or when its task period is not a whole number of milliseconds of at least 1.
#[must_use]
fn observation_grid(duration_seconds: f64, step_ms: f64) -> Option<Vec<f64>> {
    let count = (duration_seconds * 1_000.0 / step_ms).ceil();
    let planned_ms = count * step_ms;
    if !count.is_finite()
        || count < 0.0
        || step_ms.fract() != 0.0
        || step_ms < 1.0
        || planned_ms > exact_f64(APPLY_OBSERVATION_LIMIT_MS)
    {
        return None;
    }
    let step = whole_ms(step_ms);
    let every = record_period_executions(whole_ms(planned_ms), step, RECORDING_SAMPLE_LIMIT, None);
    let last_index = whole_ms(count) / every;
    let end = planned_ms / 1_000.0;
    let mut times: Vec<f64> = (0..=last_index)
        .map(|index| exact_f64(index * every) * step_ms / 1_000.0)
        .collect();
    if times
        .last()
        .is_none_or(|last| last.to_bits() != end.to_bits())
    {
        times.push(end);
    }
    Some(times)
}

/// The value and quality of one observed channel at a scan (`commissioningObservation`, B6.3). The setpoint, the process
/// variable and the manipulated variable come from the scan's own fields, with the scan's quality. Any other channel is the
/// additional channel with the same identifier and unit; a channel the scan does not hold is missing, which is `Bad`.
#[must_use]
pub(super) fn observed_value(
    sample: &ResponseSample,
    channel: &ObservedChannel,
) -> ValidationValue {
    let primary = match channel.source.as_str() {
        SETPOINT => Some(sample.setpoint),
        PROCESS_VARIABLE => Some(sample.process_value),
        MANIPULATED_VARIABLE => Some(sample.applied_output),
        _ => None,
    };
    match primary {
        Some(value) => ValidationValue {
            value,
            quality: sample.quality,
        },
        None => sample
            .additional
            .iter()
            .find(|found| found.id == channel.source && found.unit == channel.unit)
            .map_or_else(
                || ValidationValue {
                    value: None,
                    quality: ChannelQuality::Bad,
                },
                |found| ValidationValue {
                    value: found.value,
                    quality: found.quality,
                },
            ),
    }
}

/// The values of every observed channel at a scan, by channel identifier (`Object.fromEntries` of the observed sample).
#[must_use]
pub(super) fn observed_values(
    sample: &ResponseSample,
    channels: &[ObservedChannel],
) -> BTreeMap<String, ValidationValue> {
    channels
        .iter()
        .map(|channel| (channel.id.clone(), observed_value(sample, channel)))
        .collect()
}

/// The run an observation's requirements are checked on: its channels, its samples, its sample times and its duration. The
/// fields that the checks do not read are left empty.
#[must_use]
pub(super) fn observation_run(
    channels: &[ObservedChannel],
    samples: Vec<ValidationSample>,
    sample_times: Vec<f64>,
    duration_seconds: f64,
    evidence: RunEvidence,
) -> ValidationRun {
    ValidationRun {
        id: OBSERVATION_RUN_ID.to_owned(),
        evidence,
        parameters: BTreeMap::new(),
        conditions: RunConditions {
            initial_state: BTreeMap::new(),
            events: Vec::new(),
            duration_seconds,
            sample_times_seconds: sample_times,
            seed: 1,
            model_revision: String::new(),
            program_revision: String::new(),
            parameter_schema_revision: String::new(),
        },
        channels: channels
            .iter()
            .map(|channel| ValidationChannel {
                id: channel.id.clone(),
                unit: channel.unit.clone(),
            })
            .collect(),
        samples,
    }
}

/// A time rounded to the nanosecond as the record holds it (`Math.round(x * 1e9) / 1e9`, A4.2). The times of a response are
/// never negative, so the rounding of halves agrees with `Math.round`.
#[must_use]
pub(super) fn round_nanoseconds(seconds: f64) -> f64 {
    (seconds * 1e9).round() / 1e9
}

/// A count or a millisecond limit as a float. Every value converted here is at most the 24 h Apply observation limit, far below
/// 2^53, so the conversion is exact.
#[allow(clippy::cast_precision_loss)]
#[must_use]
pub(super) fn exact_f64(value: u64) -> f64 {
    value as f64
}

/// A whole number of milliseconds or executions from a float that holds one. The callers have checked that it is finite, whole
/// and not negative, and at most the Apply observation limit.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
#[must_use]
fn whole_ms(value: f64) -> u64 {
    value as u64
}

#[cfg(test)]
#[path = "observation_tests.rs"]
mod tests;
