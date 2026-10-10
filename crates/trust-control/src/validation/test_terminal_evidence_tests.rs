use super::*;
use crate::experiment::{IdentificationOutcome, IdentificationRole};
use crate::recording::fixtures::{binding, run, sample};
use crate::recording::{Completeness, RecordedSample};
use crate::signals::Quality;
use serde_json::json;

/// The Test of the `TypeScript` tests: a project sequence, which the identification reads as its applied output.
fn project_plan() -> ControlExperimentPlan {
    crate::json::decode_value(json!({
        "id": "test:saturation",
        "source": "simulation",
        "kind": "project_sequence",
        "affectedSignal": "P1.Sequence",
        "valueType": "BOOL",
        "from": false,
        "to": true,
        "unit": "",
        "minimum": 0.0,
        "maximum": 1.0,
        "durationMs": 600.0,
        "expectedScan": 0,
        "stopConditions": [],
        "restorationValue": false,
        "requiresPhysicalApproval": false,
        "trajectory": []
    }))
    .expect("the plan decodes")
}

fn loop_of_the_test() -> LoopDefinition {
    crate::recording::fixtures::loop_definition()
}

/// The seven scans of the `TypeScript` test (`recording()`): set point, measurement and applied output, 100 ms apart.
fn recording_run() -> RecordedRun {
    let rows = [
        (35.0, 75.0),
        (45.0, 100.0),
        (45.0, 100.0),
        (45.0, 100.0),
        (35.0, 100.0),
        (35.0, 100.0),
        (35.0, 95.0),
    ];
    let samples: Vec<RecordedSample> = rows
        .iter()
        .enumerate()
        .map(|(index, &(setpoint, applied))| {
            let scan = u64::try_from(index).expect("seven scans");
            let runtime_time_ns = i64::try_from(index).expect("seven scans") * 100_000_000;
            sample(
                scan,
                Some(runtime_time_ns),
                &[Some(setpoint), Some(setpoint), Some(applied)],
                &[Quality::Good; 3],
            )
        })
        .collect();
    run(
        vec![
            binding("setpoint", "%ML0", "LREAL"),
            binding("processVariable", "%IL0", "LREAL"),
            binding("manipulatedVariable", "%QL0", "LREAL"),
        ],
        samples,
    )
}

fn completed_answer() -> Value {
    json!({
        "experiment_id": "test:saturation",
        "status": "completed",
        "stop_reason": "duration_complete",
        "restoration": "restored"
    })
}

#[test]
fn a_completed_test_keeps_its_runtime_record_and_its_identification() {
    // control-studio-test-terminal-evidence.test.ts: the result keeps its terminal fields, its seven samples and its
    // recording, unchanged.
    let plan = project_plan();
    let loop_definition = loop_of_the_test();
    let recording = recording_run();
    let mut ledger = ExperimentIdentificationLedger::default();
    let record = terminal_test_evidence(
        &mut ledger,
        Some(&plan),
        Some(&loop_definition),
        &completed_answer(),
        &recording,
        None,
    )
    .expect("the evidence is recorded")
    .expect("the answer is terminal");
    assert_eq!(
        record.identification.terminal,
        ExperimentIdentificationTerminal {
            experiment_id: "test:saturation".to_owned(),
            status: ExperimentTerminalStatus::Completed,
            stop_reason: Some("duration_complete".to_owned()),
            restoration: TerminalRestoration::Restored,
            restoration_reason: None,
        }
    );
    assert_eq!(
        record.identification.samples.len(),
        7,
        "the result must retain the actual runtime scan count"
    );
    assert_eq!(record.recording_evidence.observed_sample_count, 7);
    assert_eq!(
        record.recording, recording,
        "the display receives the same native samples"
    );
    assert_eq!(record.result["stop_reason"], "duration_complete");
    assert_eq!(record.identification.role, IdentificationRole::Estimation);
    assert_eq!(
        record.identification.outcome,
        IdentificationOutcome::Completed
    );
    assert_eq!(ledger.records().len(), 1, "the Test is recorded once");
}

#[test]
fn an_incomplete_recording_keeps_its_samples_as_evidence_that_never_qualifies() {
    // The fall-back of the `TypeScript`: the complete samples are refused, so the kept samples are the evidence.
    let plan = project_plan();
    let loop_definition = loop_of_the_test();
    let mut recording = recording_run();
    recording.acquisition.completeness = Completeness::Incomplete;
    let mut ledger = ExperimentIdentificationLedger::default();
    let record = terminal_test_evidence(
        &mut ledger,
        Some(&plan),
        Some(&loop_definition),
        &completed_answer(),
        &recording,
        None,
    )
    .expect("the kept samples are recorded")
    .expect("the answer is terminal");
    assert_eq!(record.identification.samples.len(), 7);
    assert_eq!(
        record.identification.outcome,
        IdentificationOutcome::Failed,
        "an incomplete recording never fits a model"
    );
}

#[test]
fn an_answer_that_is_not_the_terminal_answer_of_the_plan_stays_as_it_is() {
    let plan = project_plan();
    let loop_definition = loop_of_the_test();
    let recording = recording_run();
    let mut ledger = ExperimentIdentificationLedger::default();
    let unchanged = |plan: Option<&ControlExperimentPlan>,
                     loop_definition: Option<&LoopDefinition>,
                     result: &Value,
                     ledger: &mut ExperimentIdentificationLedger| {
        terminal_test_evidence(ledger, plan, loop_definition, result, &recording, None)
            .expect("no refusal is made")
            .is_none()
    };
    assert!(
        unchanged(
            None,
            Some(&loop_definition),
            &completed_answer(),
            &mut ledger
        ),
        "no plan"
    );
    assert!(
        unchanged(Some(&plan), None, &completed_answer(), &mut ledger),
        "no loop"
    );
    assert!(
        unchanged(Some(&plan), Some(&loop_definition), &json!([]), &mut ledger),
        "no record"
    );
    let mut other = completed_answer();
    other["experiment_id"] = json!("test:other");
    assert!(
        unchanged(Some(&plan), Some(&loop_definition), &other, &mut ledger),
        "another experiment"
    );
    let mut running = completed_answer();
    running["status"] = json!("running");
    assert!(
        unchanged(Some(&plan), Some(&loop_definition), &running, &mut ledger),
        "not ended"
    );
    let mut unknown = completed_answer();
    unknown["restoration"] = json!("unknown");
    assert!(
        unchanged(Some(&plan), Some(&loop_definition), &unknown, &mut ledger),
        "unknown restoration"
    );
    assert!(
        ledger.records().is_empty(),
        "no unchanged answer is recorded"
    );
}

#[test]
fn the_terminal_words_and_the_reasons_are_read_as_the_runtime_writes_them() {
    let plan = project_plan();
    let failed = json!({
        "experiment_id": "test:saturation",
        "status": "failed_to_restore",
        "stop_reason": 5,
        "restoration": "not_required",
        "restoration_reason": "No restoring write was queued."
    });
    assert_eq!(
        terminal_result(&plan, &failed),
        Some(ExperimentIdentificationTerminal {
            experiment_id: "test:saturation".to_owned(),
            status: ExperimentTerminalStatus::FailedToRestore,
            stop_reason: None,
            restoration: TerminalRestoration::NotRequired,
            restoration_reason: Some("No restoring write was queued.".to_owned()),
        }),
        "a reason that is not text is absent"
    );
}
