//! The scans of an Apply observation as the commissioning rules read them (`toWorkflowSamples` of `runtimeRecording.ts`; specification
//! 35, A3.2, A4.2 and B6.3; annex E4.4 and E4.6). Each recorded sample becomes one `ResponseSample`: its process time from the
//! recording's first sample, whether the loop's mode channel reads Auto, the setpoint, the measurement, the applied output, the sample's
//! quality and the additional channels. Each value is read by trust-control's channel rules (`recording::channels`); this module only
//! puts them into the response's record.

use trust_control::commissioning::observation::{AuxiliaryValue, ResponseSample};
use trust_control::failure::Failure;
use trust_control::loop_definition::{LoopDefinition, ModeWhenTrue};
use trust_control::recording::channels::{
    applied_mv, binding_forced, binding_value, capture_additional_channels, process_value,
    sample_controller_mode, sample_quality, SETPOINT_BINDING,
};
use trust_control::recording::{
    Acquisition, Completeness, RecordedRun, RecordedSample, StopReason,
};
use trust_control::signals::SignalBinding;

/// The scans of one status page of a running recording (E4.6), whose signals are `bindings` in the order of every sample's arrays.
/// `origin` is the runtime time of the recording's first sample: it is set from the first sample followed, and passed again with every
/// later page, so that all pages count their process time from the same origin (A3.2).
///
/// # Errors
/// A sample carries no runtime time, so its process time is unknown (E4.5, E4.6).
pub(crate) fn page_samples(
    loop_definition: &LoopDefinition,
    bindings: &[SignalBinding],
    samples: &[RecordedSample],
    origin: &mut Option<i64>,
) -> Result<Vec<ResponseSample>, Failure> {
    let view = bindings_view(bindings);
    samples_of(loop_definition, &view, samples, origin)
}

/// The scans of a stopped recording (E4.7), from its own first sample.
///
/// # Errors
/// A sample carries no runtime time.
pub(crate) fn run_samples(
    loop_definition: &LoopDefinition,
    run: &RecordedRun,
) -> Result<Vec<ResponseSample>, Failure> {
    samples_of(loop_definition, run, &run.samples, &mut None)
}

/// The scans of `samples`, each read against the bindings of `run`.
fn samples_of(
    loop_definition: &LoopDefinition,
    run: &RecordedRun,
    samples: &[RecordedSample],
    origin: &mut Option<i64>,
) -> Result<Vec<ResponseSample>, Failure> {
    samples
        .iter()
        .map(|sample| -> Result<ResponseSample, Failure> {
            let time = sample
                .runtime_time_ns
                .ok_or_else(|| Failure::invalid_response("control.recording.status"))?;
            let first = *origin.get_or_insert(time);
            Ok(ResponseSample {
                elapsed_seconds: seconds_between(first, time),
                scan: sample.scan,
                controller_in_auto: sample_controller_mode(run, loop_definition, sample)
                    == Some(ModeWhenTrue::Auto),
                setpoint: binding_value(run, sample, SETPOINT_BINDING),
                process_value: process_value(run, sample),
                applied_output: applied_mv(run, loop_definition, sample),
                quality: sample_quality(run, loop_definition, sample),
                additional: capture_additional_channels(
                    loop_definition,
                    |id| binding_value(run, sample, id),
                    |id| binding_forced(run, sample, id),
                )
                .into_iter()
                .map(|channel| AuxiliaryValue {
                    id: channel.id,
                    unit: channel.unit,
                    value: channel.value,
                    quality: channel.quality,
                })
                .collect(),
            })
        })
        .collect()
}

/// The recording's bindings as a run without samples. The channel rules read only a run's bindings and the one sample they are given,
/// so a page of a running recording, which carries no run, is read through this view; the fields left empty are not read by them.
fn bindings_view(bindings: &[SignalBinding]) -> RecordedRun {
    RecordedRun {
        recording_id: String::new(),
        loop_revision: String::new(),
        program_revision: None,
        bindings: bindings.to_vec(),
        stop_reason: StopReason::Stopped,
        samples: Vec::new(),
        acquisition: Acquisition {
            method: String::new(),
            completeness: Completeness::Incomplete,
            observed_sample_count: 0,
            scan_range: None,
            missing_scan_ranges: Vec::new(),
            dropped_observed_samples: 0,
            actual_cycle_timing_available: false,
            freshness_available: false,
            reason: String::new(),
        },
    }
}

/// The seconds from `origin` to `time`, both in nanoseconds of process time (A3.2).
#[allow(clippy::cast_precision_loss)] // Differences of process time stay far below 2^53 ns.
fn seconds_between(origin: i64, time: i64) -> f64 {
    time.saturating_sub(origin) as f64 / 1e9
}
