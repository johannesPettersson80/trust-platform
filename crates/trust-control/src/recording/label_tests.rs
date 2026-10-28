//! The words that name a kept recording (specification 35, B9.9, B10.3). Every expected text is worked out by hand: the dates by
//! counting days from 1970-01-01 (55 years and 14 leap days to 2025-01-01, day 20,089), the times from the seconds of the day.

use super::{recording_label, utc_text};
use crate::experiment::{ControlExperimentKind, ControlExperimentPoint, ExperimentTerminalStatus};
use crate::recording::context::{
    ExperimentContext, RestorationContext, RestorationResult, RunContext, SIMULATION_APPROVAL,
};
use crate::recording::evidence::recording_evidence;
use crate::recording::fixtures;
use crate::recording::{EvidenceSource, RecordingFile};
use crate::signals::Quality;

/// 2026-10-22 14:05:31 UTC: day 20,383 + 365 = 20,748 after the epoch, and 50,731 s into the day (14 h, 5 min, 31 s).
const RECORDED_AT_MS: u64 = 1_792_677_931_000;

/// A kept recording of one sample captured at `captured_at_ms`.
fn kept(captured_at_ms: Option<u64>) -> RecordingFile {
    let samples = captured_at_ms
        .map(|at| {
            let mut sample = fixtures::sample(1, Some(0), &[Some(2.5)], &[Quality::Good]);
            sample.captured_at_ms = at;
            vec![sample]
        })
        .unwrap_or_default();
    let run = fixtures::run(
        vec![fixtures::binding("processVariable", "%IL0", "LREAL")],
        samples,
    );
    let evidence = recording_evidence(&run, None);
    RecordingFile::new("loop:tank", EvidenceSource::SimulatedPlant, run, evidence)
}

/// A finished experiment of `kind`, as a run notes it (B10.2).
fn experiment(kind: ControlExperimentKind) -> ExperimentContext {
    ExperimentContext {
        kind,
        affected_signal: Some("Main.ValveMV".to_owned()),
        address: None,
        requested_value: 55.0,
        applied_value: Some(55.0),
        duration_ms: Some(60_000.0),
        interval_ms: None,
        seed: None,
        frequencies_hz: None,
        trajectory: Some(vec![ControlExperimentPoint {
            at_ms: 0.0,
            value: 55.0,
        }]),
        stop_conditions: None,
        minimum: 0.0,
        maximum: 100.0,
        approval: SIMULATION_APPROVAL.to_owned(),
        status: ExperimentTerminalStatus::Completed,
        stop_reason: Some("duration_complete".to_owned()),
        aborted: false,
        restoration: RestorationContext {
            value: 50.0,
            status: RestorationResult::Confirmed,
            reason: None,
        },
    }
}

#[test]
fn a_recording_is_named_by_its_experiment_and_the_time_of_its_first_sample() {
    // Catches the page's Recording field showing the study's `rec-…` digest, which says nothing to the engineer.
    let mut file = kept(Some(RECORDED_AT_MS));
    file.context = Some(RunContext {
        experiments: vec![experiment(ControlExperimentKind::Prbs)],
        plant: None,
    });
    assert_eq!(
        recording_label(&file),
        "PRBS recorded 2026-10-22 14:05:31 UTC"
    );
    file.context = Some(RunContext {
        experiments: vec![experiment(ControlExperimentKind::MvStep)],
        plant: None,
    });
    assert_eq!(
        recording_label(&file),
        "Step response recorded 2026-10-22 14:05:31 UTC"
    );
}

#[test]
fn a_recording_without_an_experiment_says_whether_it_was_opened() {
    let file = kept(Some(RECORDED_AT_MS));
    assert_eq!(
        recording_label(&file),
        "Recording of 2026-10-22 14:05:31 UTC"
    );
    let mut opened = file;
    opened.opened = true;
    assert_eq!(
        recording_label(&opened),
        "Opened recording of 2026-10-22 14:05:31 UTC"
    );
    assert_eq!(
        recording_label(&kept(None)),
        "Recording",
        "no sample names no time"
    );
}

#[test]
fn the_time_is_the_civil_utc_date_with_leap_days_counted() {
    assert_eq!(utc_text(0), "1970-01-01 00:00:00 UTC");
    // 2024-02-29: 2024-01-01 is day 19,723 (54 years, 13 leap days), and 29 February is 59 days later.
    assert_eq!(utc_text(1_709_164_800_000), "2024-02-29 00:00:00 UTC");
    // The last millisecond of that day: milliseconds are cut, never rounded into the next day.
    assert_eq!(utc_text(1_709_251_199_999), "2024-02-29 23:59:59 UTC");
    // 2025-10-22 14:05:31, a year before the recording above.
    assert_eq!(utc_text(1_761_141_931_000), "2025-10-22 14:05:31 UTC");
}
