//! The project experiment of the PID heater lesson, run through the engine's Identify and kept in the study (checklist V4.8; specification 35,
//! C12, B3, B10.3; the `TypeScript` journey `control-studio-heater-pid-lessons.integration.test.ts`).
//!
//! The lessons are the journey's water heaters, `examples/control/01-feedback-foundations-and-temperature-control/04-water-heater-p`
//! (P) and `05-water-heater-pi` (PI), each a controller whose program owns its experiment (`execution: "project-scripted"`): the program's
//! warming and load trials run while `P1.ExperimentEnabled` is TRUE, which is the experiment's activation (C12), for the declared 120.1 s. Identify, run with the loop's own method (Automatic), observes the project's scenario whatever the mode (C12), and the
//! engine keeps the recording in the study (B10.3). The test then reads the recording and checks it as the TypeScript journey does: it is
//! complete, it has the 120-second experiment's samples, and in the warming window (50 to 60 s) and the load window (110 to 120 s) the
//! measured temperature matches the published comparison.
//!
//! The expected values are the TypeScript journey's and the lessons' published figures: P holds 24.285714 °C while warming and 22.142857 °C
//! under the load, within 0.05 °C; PI holds 35 °C in both windows within 0.2 °C.
//!
//! Run time at real time (1×, as the TypeScript journey runs it): about three minutes, the 120.1 s experiment and the start of the runtime
//! and of the verbs. The file is Unix only: the runtime's endpoint is a Unix socket.
#![cfg(unix)]

mod common;

use std::fs;
use std::time::Duration;

use serde_json::Value;
use trust_control::recording::{Completeness, RecordedSample, RecordingFile};
use trust_control_studio::storage;

use common::engine::{study_of, Findings, Lab};

/// The P water heater, a copy of which the first test runs (see the module documentation).
const LESSON_04: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../examples/control/01-feedback-foundations-and-temperature-control/04-water-heater-p"
);
/// The PI water heater, a copy of which the second test runs.
const LESSON_05: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../examples/control/01-feedback-foundations-and-temperature-control/05-water-heater-pi"
);
/// The loop's file in the copy.
const LOOP_FILE: &str = "control-loop.json";
/// The speed of the simulation: the lesson's own speed, which the TypeScript journey runs at.
const TIME_SCALE: u32 = 1;
/// The samples that the 120-second experiment retains with the authority on (the `TypeScript` journey's 1190).
const EXPERIMENT_SAMPLES: usize = 1190;
/// The samples that each assessment window holds at the 0.1 s task interval (the `TypeScript` journey's 99).
const WINDOW_SAMPLES: usize = 99;
/// The longest that Identify may take: the experiment's 120.1 s at real time, and the verbs' own work.
const LIMIT: Duration = Duration::from_secs(600);
/// The name of the experiment's authority signal in the loop's `signals.additional` is its `id`; the activation names its address.
const AUTHORITY_ID_FALLBACK: &str = "experiment-authority";

/// The id of the signal that carries the experiment's authority: the additional signal at the activation's address.
fn authority_id(loop_json: &Value) -> String {
    let activation = loop_json["experiment"]["activation"]["address"].as_str();
    loop_json["signals"]["additional"]
        .as_array()
        .and_then(|signals| {
            signals
                .iter()
                .find(|signal| signal["address"].as_str() == activation)
        })
        .and_then(|signal| signal["id"].as_str())
        .unwrap_or(AUTHORITY_ID_FALLBACK)
        .to_string()
}

/// The process time of a recorded sample in ms (A3.2): the runtime's time of its scan, or its capture time when the runtime gave none.
#[allow(clippy::cast_precision_loss)] // Process times in ns stay far below 2⁵³ for a recording of minutes.
fn process_ms(sample: &RecordedSample) -> f64 {
    sample
        .runtime_time_ns
        .map_or(sample.captured_at_ms as f64, |nanoseconds| {
            nanoseconds as f64 / 1_000_000.0
        })
}

/// The value of the column at `index` in a recorded sample, when it has one.
fn value_at(sample: &RecordedSample, index: Option<usize>) -> Option<f64> {
    sample.values.get(index?).copied().flatten()
}

/// The project experiment of the P water heater, run through Identify, retained in the study and checked against the published comparison
/// (V4.8).
///
/// Run time: ×1 (real time, as the TypeScript journey runs it), about three minutes.
#[test]
fn the_p_heaters_project_experiment_is_recorded_by_identify_and_kept_in_the_study() {
    check_heater("heater-04", LESSON_04, (24.285_714, 22.142_857, 0.05));
}

/// The project experiment of the PI water heater, as the P one (V4.8).
///
/// Run time: ×1, about three minutes.
#[test]
fn the_pi_heaters_project_experiment_is_recorded_by_identify_and_kept_in_the_study() {
    check_heater("heater-05", LESSON_05, (35.0, 35.0, 0.2));
}

/// Runs the lesson's project experiment through Identify and checks the retained recording against `(warming, load, tolerance)`: the
/// measured temperature of the warming window (50 to 60 s) and of the load window (110 to 120 s), within the tolerance.
#[allow(clippy::too_many_lines)] // The copy, Identify, the study and the two windows, in the order the journey checks them.
fn check_heater(name: &str, lesson: &str, (warming, load, tolerance): (f64, f64, f64)) {
    let lab = Lab::start(name, lesson, LOOP_FILE, TIME_SCALE);
    let loop_json: Value = serde_json::from_str(
        &fs::read_to_string(lab.project.join(LOOP_FILE)).expect("the loop file"),
    )
    .expect("JSON");
    let loop_id = lab.loop_definition.id.clone();
    let mut findings = Findings::default();

    // Identify with the loop's own method: the project's scenario runs whatever the controller mode (C12). The journey checks the
    // recording, not a model: a closed-loop experiment with the project's load step may have no simple model that explains it, which
    // Identify reports as such (B3.9) while it keeps the recording (B10.3).
    let (program, status) = lab.run("identify", "identify", &["--json"], true, LIMIT);
    let answer = program.answer();
    findings.check(
        (status.code() == Some(0) && answer.get("result").is_some())
            || (status.code() == Some(1) && answer["failure"]["cause"] == "model-or-data"),
        format!(
            "Identify runs the project's experiment and answers its model, or that no simple model explains it (C12, B3, B3.9):\n{}",
            program.report(status)
        ),
    );

    // The recording is retained and stored in the study (B10.3): complete, and the one that holds the most samples.
    let study = study_of(&lab.project, &loop_id);
    let recordings: Vec<String> = study["recordings"]
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    findings.check(
        !recordings.is_empty(),
        "the study keeps the experiment's recording (B10.3)",
    );
    let recording: Option<RecordingFile> = recordings
        .iter()
        .filter_map(|id| storage::read_recording(&lab.project, &loop_id, id).ok())
        .max_by_key(|file| file.run.samples.len());
    findings.check(
        recording
            .as_ref()
            .is_some_and(|file| file.evidence.completeness == Completeness::Complete),
        "the recording is complete evidence (B10.3)",
    );

    // The published comparison: the samples of the experiment, and the two assessment windows of the measured temperature. Without a
    // complete recording there is nothing to compare, so the test ends with the findings so far.
    let Some(file) = recording else {
        findings.conclude();
        return;
    };
    let authority = authority_id(&loop_json);
    let column = |id: &str| {
        file.run
            .bindings
            .iter()
            .position(|binding| binding.id == id)
    };
    let authority_column = column(&authority);
    let pv_column = column("processVariable");
    let active: Vec<&RecordedSample> = file
        .run
        .samples
        .iter()
        .filter(|sample| value_at(sample, authority_column).is_some_and(|value| value > 0.5))
        .collect();
    findings.check(
        active.len() >= EXPERIMENT_SAMPLES,
        format!(
            "the recording retains the 120-second experiment: {} samples with the authority on, at least {EXPERIMENT_SAMPLES} expected",
            active.len()
        ),
    );
    let Some(first) = active.first().map(|sample| process_ms(sample)) else {
        findings.conclude();
        return;
    };
    // (`first` is the first sample with the authority on: the experiment's start, as the journey's windows count from it. The
    // windows count process time, the runtime's time of each sample (A3.2), so the wall clock's jitter moves no sample.)
    for (start_ms, end_ms, expected, window) in [
        (50_000.0, 60_000.0, warming, "warming (50 to 60 s)"),
        (110_000.0, 120_000.0, load, "load (110 to 120 s)"),
    ] {
        let inside: Vec<Option<f64>> = active
            .iter()
            .filter(|sample| {
                let elapsed = process_ms(sample) - first;
                elapsed >= start_ms && elapsed < end_ms
            })
            .map(|sample| value_at(sample, pv_column))
            .collect();
        findings.check(
            inside.len() >= WINDOW_SAMPLES,
            format!(
                "the {window} window retains {WINDOW_SAMPLES} samples at the 0.1 s interval: {} found",
                inside.len()
            ),
        );
        findings.check(
            inside
                .iter()
                .all(|pv| pv.is_some_and(|pv| (pv - expected).abs() <= tolerance)),
            format!(
                "the measured temperature in the {window} window is {expected} °C within {tolerance} °C (the published comparison)"
            ),
        );
    }
    findings.conclude();
}
