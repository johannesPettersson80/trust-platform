//! A loop whose program stops writing its output in Manual, so that the output itself is its manual output (the CODESYS
//! toolkit's S3; specification 35, B1.4 and B3.4). Many CODESYS programs are written so: in Manual the controller's output is
//! simply not assigned, and whatever wrote the output last keeps it. Setup has no rule against naming the output as the manual
//! output; this file runs it end to end against a real runtime.
//!
//! The heater of lesson 30 is copied and changed in two places:
//! - `src/Main.st`: in Manual the program no longer writes `AppliedPowerPct` (`%QL0`); the controller tracks the output it
//!   finds there, so that a later return to Auto is bumpless;
//! - `control-loop.json`: the manual output is `P1.AppliedPowerPct` at `%QL0`, the manipulated variable itself.
//!
//! `identify --method step` then steps `%QL0` from 75 % in Manual and restores it, as it steps the lesson's own manual command.
//!
//! # Running
//!
//! `cargo test -p trust-control-studio --test control_studio_identify_mv_manual`, after `cargo build -p trust-runtime --bin
//! trust-runtime`, or with `TRUST_RUNTIME_BIN` naming the executable. Unix only, as the other runtime tests are.
#![cfg(unix)]

mod common;
mod identify_lab;

use std::fs;
use std::path::Path;

use trust_control::experiment::ControlExperimentKind;

use identify_lab::{
    check_plausible_model, check_plot, check_study_keeps_the_model, result_of, result_recording_id,
    stage_status, Findings, Lab, HOLDING_COMMAND, LESSON_30, LESSON_30_LOOP, RUN_LIMIT,
};

/// Replaces the one occurrence of `old` in the file at `path` with `new`.
fn replace_once(path: &Path, old: &str, new: &str) {
    let text = fs::read_to_string(path).expect("read the copied file");
    assert_eq!(
        text.matches(old).count(),
        1,
        "{} holds {old:?} once",
        path.display()
    );
    fs::write(path, text.replacen(old, new, 1)).expect("write the copied file");
}

/// Makes the copied heater's output its own manual output (module documentation).
fn output_is_the_manual_output(project: &Path) {
    let program = project.join("src/Main.st");
    replace_once(
        &program,
        "    CommandActivityPoints : LREAL := 0.0;\nEND_VAR",
        "    CommandActivityPoints : LREAL := 0.0;\n    ControllerOutput : LREAL := 75.0;\nEND_VAR",
    );
    replace_once(
        &program,
        "AppliedPowerPct := Controller.Update(Input := Input);",
        "IF Input.Mode = PIDMode#Manual THEN\n    \
         (* In Manual the output is not written: the controller tracks the output it finds. *)\n    \
         Input.ManualOutput := AppliedPowerPct;\n    \
         Input.TrackingOutput := AppliedPowerPct;\n\
         END_IF;\n\
         ControllerOutput := Controller.Update(Input := Input);\n\
         IF Input.Mode = PIDMode#Auto THEN\n    \
         AppliedPowerPct := ControllerOutput;\n\
         END_IF;",
    );
    let loop_file = project.join("control-loop.json");
    replace_once(
        &loop_file,
        "\"manualOutputSignal\": \"P1.ManualOutput\"",
        "\"manualOutputSignal\": \"P1.AppliedPowerPct\"",
    );
    replace_once(
        &loop_file,
        "\"manualOutputAddress\": \"%ML72\"",
        "\"manualOutputAddress\": \"%QL0\"",
    );
}

/// B3.4 steps 3 to 6 with the output as the manual output: the heater holds 75 % in Manual; `identify --method step` writes
/// the step to `%QL0` itself, holds it, restores 75 % and keeps a provisional first-order model with a positive gain, and the
/// experiment ends completed and restored with an empty journal. Wrong behaviour caught: a step on the output that the program
/// or the runtime overwrites before the plant sees it (no response, so no model), and a restoration that is never confirmed.
#[test]
fn identify_steps_a_loop_whose_manual_output_is_its_output_itself() {
    let lab = Lab::start_edited(
        "mv-manual",
        LESSON_30,
        LESSON_30_LOOP,
        output_is_the_manual_output,
    );
    let mut findings = Findings::default();
    let before = lab.controller_command();
    findings.check(
        (before - HOLDING_COMMAND).abs() < 1e-6,
        format!("the heater should hold {HOLDING_COMMAND} % in Manual, it reads {before} %"),
    );
    let mut identify = lab.start_verb("identify", &["--method", "step", "--json"], "step");
    let mut plan = None;
    let status = identify.wait_polling(RUN_LIMIT, || {
        if plan.is_none() {
            plan = lab.journaled_plan();
        }
    });
    findings.check(
        status.code() == Some(0),
        format!(
            "identify should finish with its result (status 0):\n{}",
            lab.describe(&identify, status)
        ),
    );
    let result = result_of(&mut findings, &lab, &identify, status);
    let recording_id = result_recording_id(&mut findings, &result);
    findings.check(
        stage_status(&result, "Identify") == Some("complete"),
        format!(
            "the Identify stage should be complete, it is {:?}",
            stage_status(&result, "Identify")
        ),
    );
    check_plot(&mut findings, &result);
    let section = check_study_keeps_the_model(
        &mut findings,
        &lab,
        recording_id.as_deref().unwrap_or("(none)"),
    );
    check_plausible_model(&mut findings, &result, &section);
    lab.check_journal_empty(&mut findings, "once the identification has settled");
    match &plan {
        Some(experiment) => {
            findings.check(
                experiment.kind == ControlExperimentKind::MvStep
                    && experiment.affected_signal == "P1.AppliedPowerPct"
                    && experiment.address.as_deref() == Some("%QL0"),
                format!(
                    "the step should write the output itself, P1.AppliedPowerPct at %QL0: {:?} {} {:?}",
                    experiment.kind, experiment.affected_signal, experiment.address
                ),
            );
            let record = lab.check_ended_and_restored(&mut findings, &experiment.id);
            findings.check(
                record
                    .as_ref()
                    .is_some_and(|record| record["status"] == "completed"),
                format!("the experiment should end normally (status completed): {record:?}"),
            );
        }
        None => findings.check(
            false,
            "the journal should hold the campaign's step while its experiment runs (E11.2)",
        ),
    }
    lab.check_command_is(&mut findings, before);
    findings.conclude();
}
