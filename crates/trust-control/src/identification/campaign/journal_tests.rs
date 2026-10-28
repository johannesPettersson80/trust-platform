use serde_json::json;

use super::*;
use crate::experiment::{ControlEvidenceSource, ControlExperimentPlan, RecordingRecoveryBinding};

/// The owner of the tank's recording: its identity, with no data.
fn owner() -> RecordingRecoveryOwner {
    RecordingRecoveryOwner {
        schema_version: 1,
        recording_id: "recording-1".to_owned(),
        project_id: "project".to_owned(),
        runtime_id: "runtime".to_owned(),
        loop_id: "tank".to_owned(),
        loop_revision: "rev-1".to_owned(),
        source: ControlEvidenceSource::Simulation,
        program_revision: Some("program".to_owned()),
        start_acknowledgement_unconfirmed: None,
        bindings: vec![RecordingRecoveryBinding {
            id: "setpoint".to_owned(),
            address: None,
            symbol: Some("Main.TIC101_Setpoint".to_owned()),
            iec_type: "LREAL".to_owned(),
        }],
    }
}

/// A reviewed setpoint step of the tank, named `experiment_id`.
fn plan(experiment_id: &str) -> ControlExperimentPlan {
    serde_json::from_value(json!({
        "id": experiment_id, "source": "simulation", "kind": "setpoint_step",
        "affectedSignal": "Main.TIC101_Setpoint", "valueType": "LREAL", "from": 45, "to": 47, "unit": "degC",
        "minimum": 0, "maximum": 100, "durationMs": 540_000, "expectedScan": 300, "task": "$background",
        "stopConditions": ["Abort"], "restorationValue": 45, "requiresPhysicalApproval": false,
        "trajectory": [{ "atMs": 0, "value": 47 }]
    }))
    .expect("the plan")
}

/// The steady baseline of the tank: 45 degC, its noise and its half range, over a 30 s window.
fn baseline() -> BaselineSummary {
    BaselineSummary {
        mean: 45.0,
        noise: 0.03,
        half_range: 0.05,
        spacing_seconds: 1.0,
        window_seconds: 30.0,
        input_mean: None,
        input_noise: None,
    }
}

#[test]
fn a_step_journal_keeps_the_baseline_end_and_the_plans_of_the_inputs_stepped_before_it() {
    let stepped = vec![plan("experiment-1")];
    let journal = IdentificationCampaignJournal::step_phase(
        "rev-1",
        owner(),
        plan("experiment-2"),
        baseline(),
        40.0,
        0,
    )
    .with_progress(30.0, stepped.clone());
    assert_eq!(journal.baseline_end_seconds, Some(30.0));
    assert_eq!(journal.stepped_plans, stepped);
    let written = serde_json::to_value(&journal).expect("the journal encodes");
    let read: IdentificationCampaignJournal =
        serde_json::from_value(written).expect("the journal decodes");
    assert_eq!(read, journal, "the progress survives a reopened host");
}

#[test]
fn a_journal_without_any_progress_reads_with_no_baseline_end_and_no_stepped_inputs() {
    let journal = IdentificationCampaignJournal::step_phase(
        "rev-1",
        owner(),
        plan("experiment-1"),
        baseline(),
        30.0,
        1,
    );
    let written = serde_json::to_value(&journal).expect("the journal encodes");
    let object = written.as_object().expect("the journal is an object");
    assert!(
        !object.contains_key("steppedPlans") && !object.contains_key("baselineEndSeconds"),
        "a journal with no progress writes the keys of the schema it had"
    );
    let read: IdentificationCampaignJournal =
        serde_json::from_value(written).expect("the journal decodes");
    assert_eq!(read.baseline_end_seconds, None);
    assert_eq!(read.stepped_plans, [] as [ControlExperimentPlan; 0]);
    assert_eq!(read.channels_remaining, 1);
}

#[test]
fn only_a_dispatched_step_of_this_loop_revision_is_resumable() {
    let baseline_only = IdentificationCampaignJournal::baseline_phase("rev-1", owner(), 2);
    let refusal = baseline_only
        .resumable_step("rev-1")
        .expect_err("a campaign in its baseline has no running experiment");
    assert_eq!(refusal.detail.as_deref(), Some(NO_RUNNING_EXPERIMENT_TEXT));
    let step = IdentificationCampaignJournal::step_phase(
        "rev-1",
        owner(),
        plan("experiment-1"),
        baseline(),
        30.0,
        1,
    );
    assert!(step.resumable_step("rev-2").is_err(), "another revision");
    let (resumed, _) = step
        .resumable_step("rev-1")
        .expect("the step of this revision resumes");
    assert_eq!(resumed.id, "experiment-1");
}
