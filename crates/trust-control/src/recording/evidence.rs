//! The evidence of a stopped recording (runtime protocol annex, E4.4, E4.5 and E11.4; specification 35, A4.2): the
//! acquisition claims of a stop answer must agree with its samples, and a recording is complete only when its acquisition
//! is complete, it holds at least two samples and its record period is steady. A start whose acknowledgement was lost keeps
//! its recording incomplete (`decode` and `stop` of `runtimeRecording.ts`).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::failure::{Cause, Failure, NextAction};
use crate::recording::samples::{check_bool_values, invalid_samples, samples_increase};
use crate::recording::sequence::steady_record_period;
use crate::recording::{Completeness, RecordedRun, StopReason};

/// The reason of a recording whose start acknowledgement was lost before the host reloaded (`fromRecoveryOwner`).
pub const UNCONFIRMED_START_RECOVERED: &str =
    "Start acknowledgement was not confirmed before Extension Host reload.";

/// The verdict on a stopped recording: whether it is complete, and the reason the engineer reads. The recording file stores
/// it as its `evidence`, with exactly these two fields (`completeness` and `reason`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RecordingEvidence {
    /// Complete only when every execution of the interval is held.
    pub completeness: Completeness,
    /// Why the recording is complete or incomplete.
    pub reason: String,
}

/// Checks the acquisition claims of a stop answer (E4.5): the method and the observed sample count, which must equal the
/// samples the answer carries, and the ranges. A complete acquisition has no missing range, no dropped sample and known
/// cycle timing.
///
/// # Errors
/// The method or the observed count is wrong, or the claims contradict each other.
pub fn check_acquisition_claims(run: &RecordedRun) -> Result<(), Failure> {
    let acquisition = &run.acquisition;
    let observed = u64::try_from(run.samples.len()).unwrap_or(u64::MAX);
    if acquisition.method != "runtime_scan_buffer" || acquisition.observed_sample_count != observed
    {
        return Err(Failure::new(
            "The runtime recording identity or acquisition data is invalid.",
            Cause::Runtime,
            NextAction::None,
        ));
    }
    let contradictory = acquisition
        .missing_scan_ranges
        .iter()
        .any(|range| range.last < range.first)
        || (acquisition.completeness == Completeness::Complete
            && (!acquisition.missing_scan_ranges.is_empty()
                || acquisition.dropped_observed_samples != 0
                || !acquisition.actual_cycle_timing_available));
    if contradictory {
        return Err(Failure::new(
            "The runtime acquisition claims are inconsistent.",
            Cause::Runtime,
            NextAction::None,
        ));
    }
    Ok(())
}

/// Checks the samples of a stop answer. A BOOL channel reads exactly 0 or 1 (E0.3). A complete acquisition must step strictly
/// forward in scan and runtime time, or its answer contradicts its own claim. An incomplete acquisition keeps its samples as
/// evidence in whatever order they came (E4.5).
///
/// # Errors
/// A BOOL value is neither 0 nor 1, or a complete acquisition's samples do not step forward.
pub fn check_run_samples(run: &RecordedRun) -> Result<(), Failure> {
    check_bool_values(&run.bindings, &run.samples)?;
    if run.acquisition.completeness == Completeness::Complete
        && !samples_increase(&run.samples, None)
    {
        return Err(invalid_samples());
    }
    Ok(())
}

/// The evidence of a stopped recording. It is complete only when its acquisition is complete, it holds at least two samples
/// and its record period is steady (E4.4, E4.5; A4.2). A recording whose start acknowledgement was not confirmed is always
/// incomplete, and its reason keeps the stopped data and says why (E11.4).
#[must_use]
pub fn recording_evidence(run: &RecordedRun, start_failure: Option<&str>) -> RecordingEvidence {
    let sequences: Vec<u64> = run.samples.iter().map(|sample| sample.scan).collect();
    let steady = steady_record_period(&sequences, true);
    let claims_complete = run.acquisition.completeness == Completeness::Complete;
    let complete = claims_complete && run.samples.len() >= 2 && steady;
    let reason = if complete {
        format!(
            "Complete recording — runtime captured {} scans. Stop: {}.",
            run.samples.len(),
            stop_reason_name(run.stop_reason)
        )
    } else if claims_complete && !steady {
        "Incomplete recording — the sample sequence changes its record period, so an execution is missing.".to_string()
    } else {
        format!("Incomplete recording — {}", run.acquisition.reason)
    };
    match start_failure {
        Some(failure) => RecordingEvidence {
            completeness: Completeness::Incomplete,
            reason: format!(
                "Start acknowledgement unconfirmed ({failure}); stopped data retained. {reason}"
            ),
        },
        None => RecordingEvidence {
            completeness: if complete {
                Completeness::Complete
            } else {
                Completeness::Incomplete
            },
            reason,
        },
    }
}

/// The word the annex gives a stop reason (E4.7).
fn stop_reason_name(reason: StopReason) -> &'static str {
    match reason {
        StopReason::Stopped => "stopped",
        StopReason::SampleLimit => "sample_limit",
        StopReason::DurationLimit => "duration_limit",
        StopReason::ScanReset => "scan_reset",
        StopReason::RuntimeBoundary => "runtime_boundary",
    }
}

#[cfg(test)]
#[path = "evidence_tests.rs"]
mod tests;
