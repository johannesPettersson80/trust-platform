//! Fit refits any live recording the study keeps (specification 35, B3.6; checklist G104), against a real runtime through the
//! built program `trust-control-studio`. Two step identifications of the heater of lesson 30 keep two recordings; `identify
//! --recording` then fits the first one again from the request the study kept for it, and that fit becomes the Identify result.
//! The harness is `identify_lab/mod.rs`.
//!
//! # Running
//!
//! `cargo test -p trust-control-studio --test control_studio_identify_refit`, after `cargo build -p trust-runtime --bin
//! trust-runtime`, or with `TRUST_RUNTIME_BIN` naming the executable. Unix only, as the other runtime tests are.
#![cfg(unix)]

mod common;
mod identify_lab;

use serde_json::Value;
use trust_control_studio::storage;

use identify_lab::{
    result_of, result_recording_id, stage_status, Findings, Lab, LESSON_30, LESSON_30_LOOP,
    RUN_LIMIT,
};

/// Runs `identify` with `extra` to its end and answers its result, with a finding when it does not finish with status 0.
fn identify(lab: &Lab, findings: &mut Findings, extra: &[&str], name: &str) -> Value {
    let mut program = lab.start_verb("identify", extra, name);
    let status = program.wait(RUN_LIMIT);
    findings.check(
        status.code() == Some(0),
        format!(
            "identify {extra:?} should finish with its result (status 0):\n{}",
            lab.describe(&program, status)
        ),
    );
    result_of(findings, lab, &program, status)
}

/// B3.6: two step identifications keep the recordings A and B, and the Identify result is B's fit. `identify --recording A` fits
/// A again without live excitation, from the request the study kept for it: it finishes with status 0, names A, completes the
/// Identify stage, and the study's Identify section names A. The study still lists A and B and keeps the request of each. Wrong
/// behaviour caught: the earlier recording refused because only the recording of the Identify result kept its request ("Recording
/// A cannot be fitted again"), and a refit that runs an experiment on the plant (the journal holds nothing afterwards).
#[test]
fn identify_fits_again_an_earlier_live_recording_the_study_keeps() {
    let lab = Lab::start_example("refit-kept", LESSON_30, LESSON_30_LOOP);
    let mut findings = Findings::default();
    let first = identify(
        &lab,
        &mut findings,
        &["--method", "step", "--json"],
        "first",
    );
    let a = result_recording_id(&mut findings, &first);
    let second = identify(
        &lab,
        &mut findings,
        &["--method", "step", "--json"],
        "second",
    );
    let b = result_recording_id(&mut findings, &second);
    if let (Some(a), Some(b)) = (a, b) {
        findings.check(a != b, "each identification keeps a recording of its own");
        let refit = identify(
            &lab,
            &mut findings,
            &["--recording", a.as_str(), "--json"],
            "refit",
        );
        findings.check(
            refit["recordingId"].as_str() == Some(a.as_str()),
            format!(
                "the refit should name recording {a}, it names {}",
                refit["recordingId"]
            ),
        );
        findings.check(
            stage_status(&refit, "Identify") == Some("complete"),
            format!(
                "the Identify stage should be complete after the refit, it is {:?}",
                stage_status(&refit, "Identify")
            ),
        );
        let directory = storage::study_directory(&lab.scratch.project, &lab.loop_definition.id);
        let study = findings.json(&directory.join("study.json"));
        findings.check(
            study["sections"]["identify"]["estimationRecordingId"].as_str() == Some(a.as_str()),
            format!(
                "the Identify result should be the fit of {a}: {}",
                study["sections"]["identify"]["estimationRecordingId"]
            ),
        );
        let listed: Vec<&str> = study["recordings"].as_array().map_or_else(Vec::new, |ids| {
            ids.iter().filter_map(Value::as_str).collect()
        });
        findings.check(
            listed == [a.as_str(), b.as_str()],
            format!("the study should list both recordings, it lists {listed:?}"),
        );
        let requests = &study["sections"]["recordRequests"];
        findings.check(
            requests[a.as_str()].is_object() && requests[b.as_str()].is_object(),
            format!("the study should keep the request of both recordings: {requests}"),
        );
    }
    lab.check_journal_empty(&mut findings, "after the refit");
    findings.conclude();
}
