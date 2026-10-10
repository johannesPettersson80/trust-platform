//! Recorded channels (`recordingChannels.ts`) and the values one sample of a recording holds for the loop's
//! signals (`runtimeRecording.ts`, `toWorkflowSamples`).
//!
//! A channel is the actuator or an additional signal of the loop. Its value, forced flag and quality come
//! from one sample of the runtime's recorded run (annex E4.4); the loop definition names the channel and its
//! role. Quality follows specification 35, A4.2: bad (a missing value or a flag that is not 1), then forced,
//! then good.
//!
//! A status page of a running recording (annex E4.6) has no run to read from. The value and the quality of a sample are
//! therefore read against the recording's bindings alone, and [`follow_samples`] turns a page into the samples that an
//! identification campaign reads.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::failure::Failure;
use crate::identification::campaign::FollowSample;
use crate::limits::RECORDING_CHANNEL_LIMIT;
use crate::loop_definition::{AuxiliaryRole, LoopDefinition, ModeWhenTrue};
use crate::recording::samples::{invalid_samples, seconds_between};
use crate::recording::{RecordedRun, RecordedSample};
use crate::signals::{controller_mode_address, Quality, SignalBinding};

/// The binding id of the setpoint in a recording (as `signals::loop_inspection_bindings` names it).
pub const SETPOINT_BINDING: &str = "setpoint";
/// The binding id of the measurement in a recording.
pub const PROCESS_VARIABLE_BINDING: &str = "processVariable";
/// The binding id of the manipulated variable in a recording.
pub const MANIPULATED_VARIABLE_BINDING: &str = "manipulatedVariable";
/// The binding id of a declared actuator's measured position in a recording.
pub const ACTUATOR_BINDING: &str = "actuator";

/// The quality of a value in a recording: good, forced or bad (A4.2). The worst quality wins, in this order:
/// bad, forced, good.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ChannelQuality {
    /// A valid value that is not forced.
    Good,
    /// A valid value that is forced.
    Forced,
    /// A missing value, or a quality flag that is not 1.
    Bad,
}

/// One channel of a recording: an additional signal of the loop or its actuator, with its value at the
/// sample (`recordingChannels.ts`, `RecordedChannel`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct RecordedChannel {
    /// The channel's identifier in the loop (`actuator` for the actuator).
    pub id: String,
    /// The channel the signal belongs to; a quality flag is read within its channel.
    pub channel: String,
    /// The signal's role in the channel.
    pub role: AuxiliaryRole,
    /// The signal's unit.
    pub unit: String,
    /// The value; absent when missing.
    pub value: Option<f64>,
    /// The channel's quality at the sample.
    pub quality: ChannelQuality,
}

/// The value of the binding `id` in `sample` (annex E4.4): `None` when the run has no such binding or its
/// value is null.
#[must_use]
pub fn binding_value(run: &RecordedRun, sample: &RecordedSample, id: &str) -> Option<f64> {
    value_of(&run.bindings, sample, id)
}

/// Whether the binding `id` is forced in `sample`; `false` when the run has no such binding.
#[must_use]
pub fn binding_forced(run: &RecordedRun, sample: &RecordedSample, id: &str) -> bool {
    binding_index(&run.bindings, id)
        .and_then(|index| sample.forced.get(index).copied())
        .unwrap_or(false)
}

/// The value of the binding `id` in `sample`, when `bindings` are the signals in the order of the sample's arrays.
fn value_of(bindings: &[SignalBinding], sample: &RecordedSample, id: &str) -> Option<f64> {
    let index = binding_index(bindings, id)?;
    sample.values.get(index).copied().flatten()
}

fn binding_index(bindings: &[SignalBinding], id: &str) -> Option<usize> {
    bindings.iter().position(|binding| binding.id == id)
}

/// The quality of one sample as a whole (`runtimeRecording.ts`, `quality` of a workflow sample): forced when any
/// value is forced, bad when any value is missing, any binding is not good, or a quality flag of the loop is not
/// 1, and good otherwise.
#[must_use]
pub fn sample_quality(
    run: &RecordedRun,
    loop_definition: &LoopDefinition,
    sample: &RecordedSample,
) -> ChannelQuality {
    quality_of(&run.bindings, loop_definition, sample)
}

/// The quality of `sample` as a whole, when `bindings` are the signals in the order of the sample's arrays.
fn quality_of(
    bindings: &[SignalBinding],
    loop_definition: &LoopDefinition,
    sample: &RecordedSample,
) -> ChannelQuality {
    if sample.forced.iter().any(|forced| *forced) || sample.quality.contains(&Quality::Forced) {
        return ChannelQuality::Forced;
    }
    let flag_lowered = loop_definition
        .signals
        .additional
        .iter()
        .flatten()
        .any(|signal| {
            signal.role == AuxiliaryRole::Quality && !is_one(value_of(bindings, sample, &signal.id))
        });
    let bad = flag_lowered
        || sample.values.iter().any(Option::is_none)
        || sample
            .quality
            .iter()
            .any(|quality| *quality != Quality::Good);
    if bad {
        ChannelQuality::Bad
    } else {
        ChannelQuality::Good
    }
}

/// The manipulated input the process received in `sample`: the actuator's measured position when the loop
/// declares an actuator, else the manipulated variable (`runtimeRecording.ts`, `appliedMv`).
#[must_use]
pub fn applied_mv(
    run: &RecordedRun,
    loop_definition: &LoopDefinition,
    sample: &RecordedSample,
) -> Option<f64> {
    let id = if loop_definition.signals.actuator.is_some() {
        ACTUATOR_BINDING
    } else {
        MANIPULATED_VARIABLE_BINDING
    };
    binding_value(run, sample, id)
}

/// The measurement in `sample` (`runtimeRecording.ts`, `pv`).
#[must_use]
pub fn process_value(run: &RecordedRun, sample: &RecordedSample) -> Option<f64> {
    binding_value(run, sample, PROCESS_VARIABLE_BINDING)
}

/// The samples of one status page of a running recording (annex E4.6) as an identification campaign reads them
/// (`runtimeRecording.ts`, `toWorkflowSamples`, reduced to what the campaign uses): each sample's process time in
/// seconds from the recording's first sample (A3.2), whether its quality is good (the rule of [`sample_quality`]), and
/// its measurement (that of [`process_value`]), which is absent when the sample has none.
///
/// `bindings` are the recording's signals, in the order of every sample's arrays. `first_time_ns` is the runtime time of
/// the recording's first sample, the origin of every process time. It is `None` until a sample has been followed, it is
/// set from the first sample of the first page that has one, and the caller passes it again with every later page, so
/// that all pages count from the same origin.
///
/// # Errors
/// A sample carries no runtime time, so its process time is unknown (E4.5, E4.6); the failure is that of a malformed sample.
pub fn follow_samples(
    loop_definition: &LoopDefinition,
    bindings: &[SignalBinding],
    samples: &[RecordedSample],
    first_time_ns: &mut Option<i64>,
) -> Result<Vec<FollowSample>, Failure> {
    samples
        .iter()
        .map(|sample| -> Result<FollowSample, Failure> {
            let time = sample.runtime_time_ns.ok_or_else(invalid_samples)?;
            let origin = *first_time_ns.get_or_insert(time);
            Ok(FollowSample {
                elapsed_seconds: seconds_between(origin, time),
                good: quality_of(bindings, loop_definition, sample) == ChannelQuality::Good,
                output: value_of(bindings, sample, PROCESS_VARIABLE_BINDING),
            })
        })
        .collect()
}

/// The controller mode a recorded sample shows (`runtimeRecording.ts`, `toWorkflowSamples`): the loop's mode channel decides
/// it. TRUE is the mode the loop declares for TRUE, and FALSE is the other mode. A mode channel that is forced, not good, or
/// neither 0 nor 1 gives no mode, which the caller shows as unavailable (`None`).
#[must_use]
pub fn sample_controller_mode(
    run: &RecordedRun,
    loop_definition: &LoopDefinition,
    sample: &RecordedSample,
) -> Option<ModeWhenTrue> {
    let index = mode_channel(run, loop_definition)?;
    let readable = !sample.forced.get(index).copied().unwrap_or(true)
        && sample.quality.get(index) == Some(&Quality::Good);
    let value = if readable {
        sample.values.get(index).copied().flatten()
    } else {
        None
    };
    let when_true = loop_definition
        .controller
        .mode_when_true
        .unwrap_or(ModeWhenTrue::Auto);
    mode_from_value(value, when_true)
}

/// The index of the loop's mode channel in `run`: the BOOL binding at the mode signal's address, or the one with its symbol
/// when the mode has no address (`runtimeRecording.ts`, `modeIndex`).
fn mode_channel(run: &RecordedRun, loop_definition: &LoopDefinition) -> Option<usize> {
    let mode = loop_definition.controller.mode_signal.as_deref()?;
    let address = controller_mode_address(loop_definition, mode);
    run.bindings.iter().position(|binding| {
        binding.iec_type.eq_ignore_ascii_case("BOOL")
            && match &address {
                Some(address) => binding.address.as_deref() == Some(address.as_str()),
                None => binding.symbol.as_deref() == Some(mode),
            }
    })
}

/// The mode a value of the mode channel gives: `when_true` for 1, the other mode for 0, and none for any other value.
#[allow(clippy::float_cmp)] // A mode channel is exactly 0 or 1 (E0.3).
fn mode_from_value(value: Option<f64>, when_true: ModeWhenTrue) -> Option<ModeWhenTrue> {
    match value {
        Some(1.0) => Some(when_true),
        Some(0.0) => Some(match when_true {
            ModeWhenTrue::Auto => ModeWhenTrue::Manual,
            ModeWhenTrue::Manual => ModeWhenTrue::Auto,
        }),
        _ => None,
    }
}

/// The manipulated variable the controller requested in `sample`, when the loop declares an actuator: the command, as opposed
/// to the actuator's measured position (`runtimeRecording.ts`, `requestedMv`). Without an actuator there is no separate request.
#[must_use]
pub fn requested_mv(
    run: &RecordedRun,
    loop_definition: &LoopDefinition,
    sample: &RecordedSample,
) -> Option<f64> {
    loop_definition.signals.actuator.as_ref()?;
    binding_value(run, sample, MANIPULATED_VARIABLE_BINDING)
}

/// The channels of a loop at one sample (`recordingChannels.ts`, `captureAdditionalChannels`): the actuator, then
/// the additional signals. `value` gives a channel's value by identifier and `forced` whether it is forced.
#[must_use]
pub fn capture_additional_channels(
    loop_definition: &LoopDefinition,
    value: impl Fn(&str) -> Option<f64>,
    forced: impl Fn(&str) -> bool,
) -> Vec<RecordedChannel> {
    let bindings = channel_bindings(loop_definition);
    bindings
        .iter()
        .map(|binding| {
            let measured = value(binding.id.as_str());
            let bad_channel = bindings.iter().any(|flag| {
                flag.role == AuxiliaryRole::Quality
                    && flag.channel == binding.channel
                    && !is_one(value(flag.id.as_str()))
            });
            let quality = if forced(binding.id.as_str()) {
                ChannelQuality::Forced
            } else if measured.is_none() || bad_channel {
                ChannelQuality::Bad
            } else {
                ChannelQuality::Good
            };
            RecordedChannel {
                id: binding.id.clone(),
                channel: binding.channel.clone(),
                role: binding.role,
                unit: binding.unit.clone(),
                value: measured,
                quality,
            }
        })
        .collect()
}

/// Whether a list of recorded channels is well formed (`recordingChannels.ts`, `validRecordedChannels`): at most 32
/// channels, each with a unique non-empty identifier and a non-empty channel, and each value missing or finite.
#[must_use]
pub fn valid_recorded_channels(channels: &[RecordedChannel]) -> bool {
    let count = u64::try_from(channels.len()).unwrap_or(u64::MAX);
    if count > RECORDING_CHANNEL_LIMIT {
        return false;
    }
    let mut ids = std::collections::BTreeSet::new();
    channels.iter().all(|channel| {
        !channel.id.is_empty()
            && ids.insert(channel.id.as_str())
            && !channel.channel.is_empty()
            && channel.value.is_none_or(f64::is_finite)
    })
}

/// The identity of a list of channels (`recordingChannels.ts`, `recordedChannelIdentity`): the JSON of each
/// channel's identifier, channel, role and unit, sorted by identifier in UTF-16 order. Two lists with the same
/// identity describe the same channels.
#[must_use]
pub fn recorded_channel_identity(channels: Option<&[RecordedChannel]>) -> String {
    let mut rows: Vec<(&str, &str, AuxiliaryRole, &str)> = channels
        .unwrap_or_default()
        .iter()
        .map(|channel| {
            (
                channel.id.as_str(),
                channel.channel.as_str(),
                channel.role,
                channel.unit.as_str(),
            )
        })
        .collect();
    rows.sort_by(|left, right| left.0.encode_utf16().cmp(right.0.encode_utf16()));
    serde_json::to_string(&rows).unwrap_or_default()
}

/// A signal a channel reads, with the identity it has in the loop.
struct ChannelBinding {
    id: String,
    channel: String,
    role: AuxiliaryRole,
    unit: String,
}

/// The channels of a loop in order: the actuator as `actuator` in the `primary` channel, then the additional signals.
fn channel_bindings(loop_definition: &LoopDefinition) -> Vec<ChannelBinding> {
    let actuator = loop_definition
        .signals
        .actuator
        .as_ref()
        .map(|actuator| ChannelBinding {
            id: ACTUATOR_BINDING.to_string(),
            channel: "primary".to_string(),
            role: AuxiliaryRole::Actuator,
            unit: actuator.unit.clone(),
        });
    actuator
        .into_iter()
        .chain(
            loop_definition
                .signals
                .additional
                .iter()
                .flatten()
                .map(|signal| ChannelBinding {
                    id: signal.id.clone(),
                    channel: signal.channel.clone(),
                    role: signal.role,
                    unit: signal.unit.clone(),
                }),
        )
        .collect()
}

/// Whether a quality flag reads exactly 1, as a BOOL does; a missing flag is not 1.
#[allow(clippy::float_cmp)] // A flag is exactly 1 or it is not.
fn is_one(value: Option<f64>) -> bool {
    value == Some(1.0)
}

#[cfg(test)]
#[path = "channels_tests.rs"]
mod tests;
