//! The identification of a project-scripted experiment against a real runtime (checklist V1.6; specification 35, C12, B3.3
//! and B3.5; runtime protocol annex E5.2 to E5.7 and E11.2): lesson 5, the PI heater whose project owns its scenario, on a
//! copy at ten times real time. The helpers are those of the crate root, `control_studio_identify.rs`.
//!
//! # What the lesson's ST gives
//!
//! The lesson is `examples/control/01-feedback-foundations-and-temperature-control/05-water-heater-pi`, with the plant of its
//! `simulation.toml`: the heater, `%QL0` (`P1.AppliedHeaterPower`), is plant input 0, and the extraction, `%QL8`
//! (`P1.HeatExtractionKw`), is plant input 1.
//! - Before the scenario the heater is 0 %: Manual writes `ManualOutput := 0.0` (`src/Main.st` line 84), and the library
//!   returns the Manual output (`libraries/control/src/control.st` line 135).
//! - Writing `P1.ExperimentEnabled` TRUE sets `AutoMode` (`src/Main.st` line 55). At the first Auto scan the PI is seeded
//!   with `Kp (SP - PV) = 2 (35 - 20) = 30` (`src/Main.st` line 76), and its integral step adds `Ki dt (SP - PV) = 0.75`
//!   (`libraries/control/src/control.st` lines 148 and 149). The heater goes from 0 % to 30.75 % (`src/Main.st` line 88),
//!   a change of the plant input. The setpoint is held at 35 degC (`src/Main.st` line 45), so the PI raises the heater to
//!   about 75 %.
//! - The scenario's own step is the extraction: `HeatExtractionKw := 30` from the 601st Auto scan, 60 s after the activation
//!   (lines 57 to 59). It is not the heater, so the heater moves only through the controller. The PI raises it to about 90 %
//!   against the extraction, and the mean plant input over the hold is about 78 %.
//!
//! Which outcome holds: the recording is fitted, and the N2.6 refusal for a missing step does not apply. N2.2 refuses a fit
//! only when the mean plant input over the hold equals its baseline (`Δu = 0`), and here the input rises from 0 % to about
//! 78 %. So the result is a provisional model of the heater (B3.5), and its gain is positive: the heater has direct action,
//! and a negative gain would contradict the controller's action (B3.9).
//!
//! The model's family is not asserted. The closed-loop data is fitted as any other excitation (B3.5), so the fit states what
//! the record gives, and only its gain sign is checked here.

use serde_json::Value;
use trust_control::experiment::{ControlExperimentKind, ControlExperimentPlan, ExperimentValue};
use trust_control::study::is_recording_id;
use trust_control_studio::journal::{Journal, JournalEntry};

use super::{
    check_plot, check_study_keeps_the_model, result_of, stage_status, Findings, Lab, RUN_LIMIT,
};

/// The project of lesson 5, whose program owns the experiment (C12).
const WATER_HEATER_EXAMPLE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../examples/control/01-feedback-foundations-and-temperature-control/05-water-heater-pi"
);
/// The identifier of the example's loop (`control-loop.json`).
const WATER_HEATER_LOOP: &str = "control-lesson-05";

/// The plan of the experiment that the loop's journal holds: the plan of its `Experiment` entry, which carries it from the
/// dispatch on (E11.2). `None` while no experiment is journaled.
fn journaled_plan(lab: &Lab) -> Option<ControlExperimentPlan> {
    Journal::open(
        &lab.journal,
        &lab.loop_definition.project_id,
        &lab.loop_definition.id,
    )
    .ok()?
    .entries()
    .iter()
    .find_map(|entry| match entry {
        JournalEntry::Experiment { recovery } => recovery.plan.clone(),
        JournalEntry::RecordingOwner { .. }
        | JournalEntry::IdentificationCampaign { .. }
        | JournalEntry::IsolatedJob { .. }
        | JournalEntry::Commissioning { .. }
        | JournalEntry::IdentificationMode { .. } => None,
    })
}

/// Checks the plan of the scenario: the project's sequence writes the activation alone, from FALSE to TRUE, restores it to
/// FALSE, and writes no setpoint and no output: it has no trajectory and no mode binding (C12, E5.2, E5.3).
fn check_scenario_plan(findings: &mut Findings, plan: Option<&ControlExperimentPlan>) {
    let Some(plan) = plan else {
        findings.check(
            false,
            "the journal should hold the plan of the scenario while it runs (E11.2)",
        );
        return;
    };
    findings.check(
        plan.kind == ControlExperimentKind::ProjectSequence,
        format!(
            "the plan should be the project's sequence, it is {:?}",
            plan.kind
        ),
    );
    findings.check(
        plan.affected_signal == "P1.ExperimentEnabled"
            && plan.address.as_deref() == Some("%MX80.0"),
        format!(
            "the plan should write the activation alone, it writes {} at {:?}",
            plan.affected_signal, plan.address
        ),
    );
    findings.check(
        plan.from == ExperimentValue::Boolean(false) && plan.to == ExperimentValue::Boolean(true),
        format!(
            "the activation should be written TRUE from FALSE, the plan writes {:?} to {:?}",
            plan.from, plan.to
        ),
    );
    findings.check(
        plan.restoration_value == ExperimentValue::Boolean(false),
        format!(
            "the activation should be restored to FALSE, the plan restores {:?}",
            plan.restoration_value
        ),
    );
    findings.check(
        plan.trajectory.is_empty() && plan.mode_binding.is_none(),
        format!(
            "the plan should write no setpoint and no output, and carry no mode binding: {plan:?}"
        ),
    );
    findings.check(
        (plan.duration_ms - 120_100.0).abs() < 1e-6,
        format!(
            "the scenario should last its declared 120.1 s, whose last instant is the restoration sample (B3.4 step 4, C12), its plan says {} ms",
            plan.duration_ms
        ),
    );
}

/// Checks the model of the scenario's recording: provisional and labelled so (B3.5, B3.8), with its equation and key numbers
/// stated, and a positive gain (B3.9, the gain sign follows the controller's action).
fn check_scenario_model(findings: &mut Findings, result: &Value, section: &Value) {
    findings.check(
        result["provisional"].as_bool() == Some(true),
        "the result should be provisional (B3.5)",
    );
    let readable = result["text"].as_str().unwrap_or_default();
    findings.check(
        readable.contains("Provisional"),
        format!("the readable text should label the model Provisional (B3.5): {readable:?}"),
    );
    for field in ["modelType", "equation", "keyNumbers"] {
        findings.check(
            result[field].as_str().is_some_and(|text| !text.is_empty()),
            format!("the result should state its {field}"),
        );
    }
    let empty = Vec::new();
    let parameters = section["presentation"]["parameters"]
        .as_array()
        .unwrap_or(&empty);
    let gain = parameters
        .iter()
        .find(|item| item["name"] == "Gain")
        .and_then(|item| item["value"].as_f64());
    findings.check(
        gain.is_some_and(|value| value > 0.0),
        format!("the heater's gain should be positive, it is {gain:?}"),
    );
}

/// Identify with Automatic on lesson 5 runs the project's scenario and fits its recording (C12, B3.5).
///
/// 1. `identify --json` plans the project-scripted experiment. Its plan writes the activation alone, `P1.ExperimentEnabled`
///    from FALSE to TRUE, with no trajectory and no mode binding (the `project_sequence` of annex E5.2). The declared
///    maximum duration is 120.1 s, and the plan lasts all of it: the runtime counts a duration to the execution at which the
///    restoration takes effect, so the 120 s of the scenario and the restoration sample fit in it (B3.4 step 4, C12). The
///    journal holds the plan while the experiment runs, and it is empty once the identification has settled.
/// 2. The experiment ends normally, at its duration (`duration_complete`) or earlier once the response has settled
///    (`finished_early`, E5.7), and the activation is restored to FALSE and confirmed.
/// 3. The result is provisional (B3.5) with a positive gain, and the study keeps its recording, which is complete (B10.3).
/// 4. The controller output is back at 0 %, its Manual output, so the heater is left to the project's own program.
///
/// Run time at ten times real time: the recording starts with the plan and waits for no steady baseline (C12), the experiment
/// lasts 120.1 s, and its restoration is confirmed by the third execution after the write (0.3 s). That is at most 120.5 s of
/// process time, about 12 s at ten times real time. With the start of the runtime and the build of the project, a few seconds
/// each, the test takes about 20 s.
#[test]
fn identify_runs_the_project_scenario_and_fits_its_recording() {
    let lab = Lab::start_example("heater", WATER_HEATER_EXAMPLE, WATER_HEATER_LOOP);
    let mut findings = Findings::default();
    let original_command = lab.controller_command();
    findings.check(
        original_command.abs() < 1e-6,
        format!(
            "the heater should hold 0 % in Manual before the scenario (Main.st line 84), it reads {original_command} %"
        ),
    );

    // 1. Identify with Automatic. The plan is read from the journal while the experiment runs, because only the journal
    // names it before the answer (E11.2).
    let mut identify = lab.start_identify("identify", &["--json"]);
    let mut plan = None;
    let status = identify.wait_polling(RUN_LIMIT, || {
        if plan.is_none() {
            plan = journaled_plan(&lab);
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
    let recording_id = result["recordingId"]
        .as_str()
        .filter(|id| is_recording_id(id))
        .map(str::to_string);
    findings.check(
        recording_id.is_some(),
        format!(
            "the result should name a recording file of the study (A8.3), it names {}",
            result["recordingId"]
        ),
    );
    findings.check(
        stage_status(&result, "Identify") == Some("complete"),
        format!(
            "the Identify stage should be complete after the result, it is {:?}",
            stage_status(&result, "Identify")
        ),
    );
    check_plot(&mut findings, &result);
    let section = check_study_keeps_the_model(
        &mut findings,
        &lab,
        recording_id.as_deref().unwrap_or("(none)"),
    );
    check_scenario_model(&mut findings, &result, &section);
    lab.check_journal_empty(&mut findings, "once the identification has settled");

    // 2. The plan writes the activation alone, and the runtime ends and restores the experiment (E5.6, E5.7).
    check_scenario_plan(&mut findings, plan.as_ref());
    let record = plan
        .as_ref()
        .and_then(|reviewed| lab.check_ended_and_restored(&mut findings, &reviewed.id));
    findings.check(
        record.as_ref().is_some_and(|record| {
            record["status"] == "completed"
                && (record["stop_reason"] == "duration_complete"
                    || record["stop_reason"] == "finished_early")
                && record["restoration"] == "restored"
        }),
        format!("the scenario should end normally with its activation restored (E5.7): {record:?}"),
    );
    lab.check_command_is(&mut findings, original_command);
    findings.conclude();
}
