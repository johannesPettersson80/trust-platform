//! Tests of the measured frequency response (N2.8, `measuredFrequency`): a retained recording is read as the page reads it, a
//! steady gain is estimated at its tone, a record too short for its segments is rejected with its reason, and the answer
//! names and refuses recordings by the study's identifier, never by the identifier the runtime gave the run.

use std::time::Duration;

use super::*;
use crate::analysis::budget::{ANALYSIS_BUDGET, BUDGET_EXCEEDED};
use crate::recording::fixtures::{binding, loop_definition, run, sample};
use crate::signals::Quality;
use trust_control_math::frequency_identification::FrequencyRejection;

/// The recording's identifier in the study: the identifier `importRecording` answers (`rec-` and its digest).
const RECORDING_ID: &str = "rec-00000000000000000000000000000001";
/// The words that name the recording, which the answer passes on as it is given them (`recording::label`).
const RECORDING_LABEL: &str = "PRBS recorded 2026-10-22 14:05:31 UTC";
/// The identifier the runtime gave the run, which the analysis never names (§0.1a).
const RUN_ID: &str = "periodic-import";
const REVISION: &str = "sha256:loop-1";
const SETPOINT: f64 = 50.0;

/// The runtime times in nanoseconds of scans that follow `cycles_ns`, starting at zero.
fn times_ns(cycles_ns: &[i64]) -> Vec<i64> {
    let mut time = 0;
    let mut times = vec![time];
    for cycle in cycles_ns {
        time += cycle;
        times.push(time);
    }
    times
}

/// A recorded run of the scans `scans` at the runtime times `times`, where sample `i` has the applied input `input(i)` and the
/// measured output `output(i)`. The runtime named the run `RUN_ID`; the study keeps it at `REVISION` under `RECORDING_ID`.
fn recording(
    scans: &[u64],
    times: &[i64],
    input: impl Fn(usize) -> f64,
    output: impl Fn(usize) -> f64,
) -> RecordedRun {
    let bindings = vec![
        binding("setpoint", "%ML0", "LREAL"),
        binding("processVariable", "%IL0", "LREAL"),
        binding("manipulatedVariable", "%QL0", "LREAL"),
    ];
    let samples: Vec<RecordedSample> = scans
        .iter()
        .zip(times)
        .enumerate()
        .map(|(index, (scan, time))| {
            sample(
                *scan,
                Some(*time),
                &[Some(SETPOINT), Some(output(index)), Some(input(index))],
                &[Quality::Good; 3],
            )
        })
        .collect();
    let mut recorded = run(bindings, samples);
    recorded.recording_id = RUN_ID.to_string();
    recorded.loop_revision = REVISION.to_string();
    recorded
}

/// The index of a sample as a float, for the fixtures' arithmetic.
fn at(index: usize) -> f64 {
    f64::from(u32::try_from(index).unwrap_or(0))
}

/// A tone of 1 rad/s sampled every 0.1 s.
fn tone(index: usize) -> f64 {
    (at(index) * 0.1).sin()
}

/// The steady gain of the process: the input `1 + 0.5·sin` gives the output `2 + sin`, so the gain at the tone is 2.
fn steady_gain_recording(count: u64) -> RecordedRun {
    let scans: Vec<u64> = (1..=count).collect();
    let times = times_ns(&vec![
        100_000_000_i64;
        usize::try_from(count).unwrap_or(0) - 1
    ]);
    recording(
        &scans,
        &times,
        |index| 1.0 + 0.5 * tone(index),
        |index| 2.0 + tone(index),
    )
}

/// The ramp of control-studio-general-model-session.test.ts: 32 scans of 0.1 s, too few for four segments after the startup.
fn short_ramp_recording() -> RecordedRun {
    let scans: Vec<u64> = (1..=32).collect();
    let times = times_ns(&[100_000_000; 31]);
    recording(
        &scans,
        &times,
        |index| 1.0 + at(index) / 100.0,
        |index| 2.0 + at(index) / 50.0,
    )
}

fn inputs(recording_id: &str, revision: &str) -> AnalysisInputs {
    AnalysisInputs {
        recording_id: Some(recording_id.to_string()),
        recording_revision: Some(revision.to_string()),
        frequencies: Some(vec![1.0]),
        ..AnalysisInputs::default()
    }
}

#[test]
fn a_steady_gain_is_measured_as_its_magnitude_at_the_tone() {
    let recorded = steady_gain_recording(256);
    let definition = loop_definition();
    let elapsed = || Duration::from_secs(0);
    let cancelled = || false;
    let budget = Budget::new(ANALYSIS_BUDGET, &elapsed, &cancelled);
    let answer = measured_frequency_answer(
        RECORDING_ID,
        RECORDING_LABEL,
        &recorded,
        &definition,
        &inputs(RECORDING_ID, REVISION),
        &budget,
    )
    .expect("the measured response");
    assert_eq!(answer.status, MeasuredStatus::Qualified);
    assert!((answer.sample_time_seconds - 0.1).abs() < 1e-9);
    assert_eq!(answer.points.len(), 1);
    // control-studio-general-model-session.test.ts asserts the measured magnitude within 0.05 of its gain.
    assert!(
        (answer.points[0].magnitude - 2.0).abs() < 0.05,
        "{:?}",
        answer.points
    );
    assert!((answer.points[0].magnitude_db - 20.0 * 2f64.log10()).abs() < 0.1);
    assert!(answer.points[0].coherence > 0.99, "{:?}", answer.points);
    assert_eq!(
        answer.summary,
        vec!["Measured 1 frequency point.".to_string()]
    );
}

#[test]
fn a_record_too_short_for_its_segments_is_rejected_with_its_reason() {
    let recorded = short_ramp_recording();
    let definition = loop_definition();
    let elapsed = || Duration::from_secs(0);
    let cancelled = || false;
    let budget = Budget::new(ANALYSIS_BUDGET, &elapsed, &cancelled);
    let answer = measured_frequency_answer(
        RECORDING_ID,
        RECORDING_LABEL,
        &recorded,
        &definition,
        &inputs(RECORDING_ID, REVISION),
        &budget,
    )
    .expect("the answer names the rejection");
    assert_eq!(answer.status, MeasuredStatus::Rejected);
    assert!(answer.reason.is_some());
    assert_eq!(answer.points.len(), 0);
    assert!(
        answer.summary[0].starts_with("Measured frequency response unavailable: "),
        "{:?}",
        answer.summary
    );
}

#[test]
fn the_answer_refuses_a_recording_or_a_revision_that_it_does_not_name() {
    let recorded = steady_gain_recording(256);
    let definition = loop_definition();
    let elapsed = || Duration::from_secs(0);
    let cancelled = || false;
    let budget = Budget::new(ANALYSIS_BUDGET, &elapsed, &cancelled);
    let other_recording = inputs("rec-00000000000000000000000000000002", REVISION);
    let error = measured_frequency_answer(
        RECORDING_ID,
        RECORDING_LABEL,
        &recorded,
        &definition,
        &other_recording,
        &budget,
    )
    .expect_err("another recording");
    assert!(error.message.contains("recordingId"), "{}", error.message);
    // The run's own identifier is not the study's: a request that names it is refused (§0.1a).
    let run_id = inputs(RUN_ID, REVISION);
    let error = measured_frequency_answer(
        RECORDING_ID,
        RECORDING_LABEL,
        &recorded,
        &definition,
        &run_id,
        &budget,
    )
    .expect_err("the runtime's identifier of the run");
    assert!(error.message.contains("recordingId"), "{}", error.message);
    let other_revision = inputs(RECORDING_ID, "sha256:other");
    let error = measured_frequency_answer(
        RECORDING_ID,
        RECORDING_LABEL,
        &recorded,
        &definition,
        &other_revision,
        &budget,
    )
    .expect_err("another revision");
    assert!(
        error.message.contains("recordingRevision"),
        "{}",
        error.message
    );
}

#[test]
fn a_request_that_names_no_recording_measures_the_recording_given_and_names_it() {
    let recorded = steady_gain_recording(256);
    let definition = loop_definition();
    let elapsed = || Duration::from_secs(0);
    let cancelled = || false;
    let budget = Budget::new(ANALYSIS_BUDGET, &elapsed, &cancelled);
    let unnamed = AnalysisInputs {
        frequencies: Some(vec![1.0]),
        ..AnalysisInputs::default()
    };
    let answer = measured_frequency_answer(
        RECORDING_ID,
        RECORDING_LABEL,
        &recorded,
        &definition,
        &unnamed,
        &budget,
    )
    .expect("the recording the study chose is measured");
    assert_eq!(
        answer.recording_label, RECORDING_LABEL,
        "the answer names the recording in the words the study gives it, not by its digest alone"
    );
    assert_eq!(
        answer.recording_id, RECORDING_ID,
        "the answer names the study's identifier of the recording it used (§0.1a)"
    );
    assert_eq!(answer.recording_revision, REVISION);
    let revision_only = AnalysisInputs {
        recording_revision: Some("sha256:other".to_string()),
        frequencies: Some(vec![1.0]),
        ..AnalysisInputs::default()
    };
    let error = measured_frequency_answer(
        RECORDING_ID,
        RECORDING_LABEL,
        &recorded,
        &definition,
        &revision_only,
        &budget,
    )
    .expect_err("a revision named without the recording is checked");
    assert!(
        error.message.contains("recordingRevision"),
        "{}",
        error.message
    );
}

#[test]
fn the_study_measures_its_last_periodic_recording_and_passes_over_one_that_is_not() {
    let definition = loop_definition();
    let periodic = steady_gain_recording(256);
    let too_short = steady_gain_recording(10);
    let recordings = ["rec-periodic".to_string(), "rec-short".to_string()];
    let (chosen_id, chosen) = last_periodic_recording(&recordings, &definition, |id| {
        Ok(if id == "rec-periodic" {
            periodic.clone()
        } else {
            too_short.clone()
        })
    })
    .expect("the periodic recording is chosen");
    assert_eq!(
        chosen_id, "rec-periodic",
        "the study's identifier of the recording it chose comes with it"
    );
    assert_eq!(
        chosen.samples.len(),
        256,
        "the recording before the short one is measured"
    );
    let only_short = last_periodic_recording(&["rec-short".to_string()], &definition, |_| {
        Ok(too_short.clone())
    })
    .expect_err("the study keeps no periodic recording");
    assert_eq!(only_short.message, NO_PERIODIC_RECORDING);
    assert!(
        matches!(only_short.next_action, NextAction::Identify),
        "the next action is Identify (B9.9)"
    );
}

#[test]
fn an_incomplete_recording_is_not_a_periodic_record() {
    let mut recorded = steady_gain_recording(256);
    recorded.acquisition.completeness = Completeness::Incomplete;
    let error = periodic_record(&recorded, &loop_definition()).expect_err("an incomplete record");
    assert_eq!(error.message, NOT_PERIODIC);
}

#[test]
fn the_answer_stops_when_the_budget_runs_out() {
    let recorded = steady_gain_recording(256);
    let definition = loop_definition();
    let elapsed = || Duration::from_secs(11);
    let cancelled = || false;
    let budget = Budget::new(ANALYSIS_BUDGET, &elapsed, &cancelled);
    let error = measured_frequency_answer(
        RECORDING_ID,
        RECORDING_LABEL,
        &recorded,
        &definition,
        &inputs(RECORDING_ID, REVISION),
        &budget,
    )
    .expect_err("the budget is exceeded");
    assert_eq!(error.message, BUDGET_EXCEEDED);
}

#[test]
fn a_response_whose_sign_changes_between_the_segments_is_refused_for_low_coherence() {
    // control-studio-numerics.test.ts ("fails closed for noisy, poorly excited, unstable, singular, and invalid optimization
    // cases"): a sine of five cycles per 100-sample segment, and an output that takes the sign of its segment, so the segments
    // cancel in the cross spectrum. The estimate reads the whole record, as the test does, so its startup is zero.
    let frequency = 2.0 * std::f64::consts::PI * 5.0 / (100.0 * 0.1);
    let input: Vec<f64> = (0..400)
        .map(|index| (frequency * at(index) * 0.1).sin())
        .collect();
    let output: Vec<f64> = input
        .iter()
        .enumerate()
        .map(|(index, value)| {
            if (index / 100) % 2 == 0 {
                *value
            } else {
                -value
            }
        })
        .collect();
    let refused = estimate_periodic_frequency(&input, &output, 0.1, &[frequency], 4, 0.8, 0);
    assert!(
        matches!(refused, Err(FrequencyRejection::LowCoherence)),
        "{refused:?}"
    );
}
