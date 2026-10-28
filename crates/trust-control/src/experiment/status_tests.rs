//! Tests of the experiment status and recovery decoding. The assertions follow `control-studio-experiment.test.ts`
//! (the classification and the recovery of a running experiment and of a Test recording); the others are worked
//! out by hand.

use serde_json::{json, Value};

use super::*;
use crate::experiment::plan::{
    build_control_experiment_plan, ControlExperimentPlanResult, ControlExperimentRequestKind,
    ControllerMode, ExperimentSample, PlanInput, PlanSignal,
};
use crate::recording::fixtures::bound_loop;
use crate::recording::EvidenceSource;
use crate::signals::Quality;

fn scope() -> ControlExperimentRecoveryScope {
    ControlExperimentRecoveryScope {
        project_id: "project:fixture".to_string(),
        loop_id: "loop:tank".to_string(),
        loop_revision: "sha256:loop-1".to_string(),
    }
}

fn signal(symbol: &str, address: &str, value: f64) -> PlanSignal {
    PlanSignal {
        symbol: symbol.to_string(),
        address: Some(address.to_string()),
        value,
        unit: "%".to_string(),
        quality: Quality::Good,
    }
}

/// The reviewed setpoint step of the fixture loop (the plan of the `TypeScript` recovery test).
fn reviewed_plan() -> ControlExperimentPlan {
    let loop_definition = bound_loop();
    let sample = ExperimentSample {
        scan: 41,
        source: EvidenceSource::SimulatedPlant,
        controller_mode: ControllerMode::Auto,
        setpoint: Some(signal("LevelSP", "%ML0", 50.0)),
        process_variable: signal("LevelPV", "%IL0", 48.0),
        manipulated_variable: signal("ValveMV", "%QL0", 32.0),
        additional: Vec::new(),
    };
    match build_control_experiment_plan(&PlanInput {
        loop_definition: &loop_definition,
        sample: Some(&sample),
        kind: ControlExperimentRequestKind::SetpointStep,
        amplitude: 5.0,
        duration_seconds: 30.0,
        physical_topology_reviewed: false,
        seed: None,
        process_center: None,
        process_noise: None,
        hold_seconds: None,
    }) {
        ControlExperimentPlanResult::Accepted { plan } => *plan,
        ControlExperimentPlanResult::Rejected { reason } => panic!("the plan is refused: {reason}"),
    }
}

fn recording_owner() -> Value {
    json!({
        "schemaVersion": 1, "recordingId": "recording-test-1", "projectId": "project:fixture",
        "runtimeId": "runtime:local", "loopId": "loop:tank", "loopRevision": "sha256:loop-1",
        "source": "Simulation", "programRevision": "program-test-1",
        "bindings": [
            { "id": "setpoint", "address": "%ML0", "iec_type": "LREAL" },
            { "id": "processVariable", "address": "%IL0", "iec_type": "LREAL" },
            { "id": "manipulatedVariable", "address": "%QL0", "iec_type": "LREAL" }
        ]
    })
}

#[test]
fn a_running_or_ended_experiment_is_polled_until_its_restoration_is_settled() {
    let classify = |status: &str, restoration: &str| {
        classify_control_experiment_status(&json!({ "status": status, "restoration": restoration }))
    };
    assert_eq!(
        classify("running", "pending"),
        ControlExperimentRuntimePhase::Running
    );
    assert_eq!(
        classify("running", "not_required"),
        ControlExperimentRuntimePhase::Running
    );
    assert_eq!(
        classify("aborted", "pending"),
        ControlExperimentRuntimePhase::AwaitingRestoration
    );
    assert_eq!(
        classify("failed", "pending"),
        ControlExperimentRuntimePhase::AwaitingRestoration
    );
    assert_eq!(
        classify("completed", "restored"),
        ControlExperimentRuntimePhase::Terminal
    );
    assert_eq!(
        classify("aborted", "restored"),
        ControlExperimentRuntimePhase::Terminal
    );
    assert_eq!(
        classify("failed_to_restore", "failed"),
        ControlExperimentRuntimePhase::Terminal
    );
    assert_eq!(
        classify("aborted", "failed"),
        ControlExperimentRuntimePhase::Invalid
    );
    assert_eq!(
        classify("mystery", "pending"),
        ControlExperimentRuntimePhase::Invalid
    );
    assert_eq!(
        classify_control_experiment_status(&json!("not an object")),
        ControlExperimentRuntimePhase::Invalid,
        "an answer that is not an object is invalid"
    );
}

#[test]
fn a_version_one_recovery_is_accepted_only_for_the_exact_loop_revision() {
    let stored = json!({ "schemaVersion": 1, "experimentId": "experiment-1", "projectId": "project:fixture",
        "loopId": "loop:tank", "loopRevision": "sha256:loop-1" });
    assert_eq!(
        decode_control_experiment_recovery(&stored, &scope()),
        Some(DecodedControlExperimentRecovery {
            experiment_id: "experiment-1".to_string(),
            plan: None,
            recording_owner: None,
        })
    );
    let mut stale = stored.clone();
    stale["loopRevision"] = json!("sha256:stale");
    assert_eq!(decode_control_experiment_recovery(&stale, &scope()), None);
    let mut extra = stored.clone();
    extra["extra"] = json!(true);
    assert_eq!(
        decode_control_experiment_recovery(&extra, &scope()),
        None,
        "no key beyond the version-1 keys"
    );
}

#[test]
fn a_test_recording_is_recovered_only_with_its_exact_reviewed_plan_and_recording_owner() {
    let plan = reviewed_plan();
    let plan_json = serde_json::to_value(&plan).expect("the plan encodes");
    let owner = recording_owner();
    let stored = json!({ "schemaVersion": 2, "experimentId": plan.id, "projectId": "project:fixture",
        "loopId": "loop:tank", "loopRevision": "sha256:loop-1", "plan": plan_json, "recordingOwner": owner });
    let recovered =
        decode_control_experiment_recovery(&stored, &scope()).expect("the recovery is exact");
    assert_eq!(recovered.experiment_id, plan.id);
    assert_eq!(recovered.plan.as_ref(), Some(&plan));
    assert_eq!(
        recovered.recording_owner,
        Some(RecordingRecoveryOwner {
            schema_version: 1,
            recording_id: "recording-test-1".to_string(),
            project_id: "project:fixture".to_string(),
            runtime_id: "runtime:local".to_string(),
            loop_id: "loop:tank".to_string(),
            loop_revision: "sha256:loop-1".to_string(),
            source: ControlEvidenceSource::Simulation,
            program_revision: Some("program-test-1".to_string()),
            start_acknowledgement_unconfirmed: None,
            bindings: vec![
                RecordingRecoveryBinding {
                    id: "setpoint".to_string(),
                    address: Some("%ML0".to_string()),
                    symbol: None,
                    iec_type: "LREAL".to_string(),
                },
                RecordingRecoveryBinding {
                    id: "processVariable".to_string(),
                    address: Some("%IL0".to_string()),
                    symbol: None,
                    iec_type: "LREAL".to_string(),
                },
                RecordingRecoveryBinding {
                    id: "manipulatedVariable".to_string(),
                    address: Some("%QL0".to_string()),
                    symbol: None,
                    iec_type: "LREAL".to_string(),
                },
            ],
        })
    );
    let mut other_experiment = stored.clone();
    other_experiment["experimentId"] = json!("other");
    assert_eq!(
        decode_control_experiment_recovery(&other_experiment, &scope()),
        None
    );
    let mut other_plan = stored.clone();
    other_plan["plan"]["id"] = json!("other");
    assert_eq!(
        decode_control_experiment_recovery(&other_plan, &scope()),
        None,
        "the plan names the experiment"
    );
    let mut stale_owner = stored.clone();
    stale_owner["recordingOwner"]["loopRevision"] = json!("sha256:stale");
    assert_eq!(
        decode_control_experiment_recovery(&stale_owner, &scope()),
        None,
        "the recording belongs to the revision"
    );
}

#[test]
fn a_recording_source_must_match_the_plan_source() {
    let plan_json = serde_json::to_value(reviewed_plan()).expect("the plan encodes");
    let mut physical_owner = recording_owner();
    physical_owner["source"] = json!("Physical");
    let stored = json!({ "schemaVersion": 2, "experimentId": reviewed_plan().id, "projectId": "project:fixture",
        "loopId": "loop:tank", "loopRevision": "sha256:loop-1", "plan": plan_json, "recordingOwner": physical_owner });
    assert_eq!(
        decode_control_experiment_recovery(&stored, &scope()),
        None,
        "a simulated plan cannot reattach a physical recording"
    );
}

#[test]
fn a_stored_plan_with_an_unknown_key_or_an_out_of_range_point_is_refused() {
    let plan = reviewed_plan();
    let owner = recording_owner();
    let recovery = |plan_json: Value| {
        json!({ "schemaVersion": 2, "experimentId": plan.id, "projectId": "project:fixture",
            "loopId": "loop:tank", "loopRevision": "sha256:loop-1", "plan": plan_json, "recordingOwner": owner })
    };
    let plan_json = serde_json::to_value(&plan).expect("the plan encodes");
    let mut unknown = plan_json.clone();
    unknown["surprise"] = json!(1);
    assert_eq!(
        decode_control_experiment_recovery(&recovery(unknown), &scope()),
        None
    );
    let mut late = plan_json.clone();
    late["trajectory"] = json!([{ "atMs": 31_000.0, "value": 55.0 }]);
    assert_eq!(
        decode_control_experiment_recovery(&recovery(late), &scope()),
        None,
        "a point after the 30 s duration is refused"
    );
    let mut stale_field = plan_json.clone();
    stale_field["stopOnStaleMs"] = json!(5000);
    assert!(
        decode_control_experiment_recovery(&recovery(stale_field), &scope()).is_some(),
        "a plan journaled before the runtime owned staleness is accepted, and its field ignored"
    );
}

#[test]
fn a_stored_recording_names_its_bindings_and_acknowledgement_exactly() {
    let plan_json = serde_json::to_value(reviewed_plan()).expect("the plan encodes");
    let recovery = |owner: Value| {
        json!({ "schemaVersion": 2, "experimentId": reviewed_plan().id, "projectId": "project:fixture",
            "loopId": "loop:tank", "loopRevision": "sha256:loop-1", "plan": plan_json, "recordingOwner": owner })
    };
    let mut null_address = recording_owner();
    null_address["bindings"][0]["address"] = Value::Null;
    assert_eq!(
        decode_control_experiment_recovery(&recovery(null_address), &scope()),
        None,
        "null is not an address"
    );
    let mut no_selector = recording_owner();
    no_selector["bindings"][0] = json!({ "id": "setpoint", "iec_type": "LREAL" });
    assert_eq!(
        decode_control_experiment_recovery(&recovery(no_selector), &scope()),
        None,
        "a binding names an address or a symbol"
    );
    let mut unconfirmed = recording_owner();
    unconfirmed["startAcknowledgementUnconfirmed"] = json!(true);
    assert!(
        decode_control_experiment_recovery(&recovery(unconfirmed), &scope()).is_some(),
        "an unconfirmed start is recovered as such"
    );
    let mut false_flag = recording_owner();
    false_flag["startAcknowledgementUnconfirmed"] = json!(false);
    assert_eq!(
        decode_control_experiment_recovery(&recovery(false_flag), &scope()),
        None,
        "only true is stored"
    );
}

#[test]
fn a_recording_owner_is_recovered_only_for_the_loop_it_was_journaled_for() {
    let loop_definition = bound_loop();
    let owner = RecordingRecoveryOwner {
        schema_version: 1,
        recording_id: "recording-1".into(),
        project_id: loop_definition.project_id.clone(),
        runtime_id: loop_definition.runtime_id.clone(),
        loop_id: loop_definition.id.clone(),
        loop_revision: loop_definition.revision.clone(),
        source: ControlEvidenceSource::Simulation,
        program_revision: None,
        start_acknowledgement_unconfirmed: None,
        bindings: vec![RecordingRecoveryBinding {
            id: "setpoint".into(),
            address: Some("%ML0".into()),
            symbol: None,
            iec_type: "LREAL".into(),
        }],
    };
    assert!(check_recording_owner_scope(&owner, &loop_definition).is_ok());

    let mut revised = loop_definition.clone();
    revised.revision = "sha256:changed".into();
    let refusal = check_recording_owner_scope(&owner, &revised)
        .expect_err("another loop revision is another scope");
    assert!(
        refusal
            .message
            .contains("does not match the active loop scope"),
        "{refusal:?}"
    );

    let mut other_runtime = loop_definition.clone();
    other_runtime.runtime_id = "runtime:other".into();
    assert!(
        check_recording_owner_scope(&owner, &other_runtime).is_err(),
        "the runtime is part of the scope"
    );

    let mut empty = owner.clone();
    empty.bindings.clear();
    assert!(
        check_recording_owner_scope(&empty, &loop_definition).is_err(),
        "a recording reads one binding at least"
    );

    let mut too_many = owner.clone();
    too_many.bindings = vec![owner.bindings[0].clone(); 33];
    assert!(
        check_recording_owner_scope(&too_many, &loop_definition).is_err(),
        "a recording reads 32 bindings at most"
    );
}

#[test]
fn a_stored_test_recovery_restores_its_plan_and_its_recording_together() {
    // The restore of `control-studio-experiment.test.ts`'s combined recovery transaction: the stored plan and its recording
    // come back together, for the loop revision they were reviewed against. The journal's file round trip is tested with
    // the journal in trust-control-studio.
    let plan = reviewed_plan();
    let owner = RecordingRecoveryOwner {
        schema_version: 1,
        recording_id: "recording-test-2".to_string(),
        project_id: "project:fixture".to_string(),
        runtime_id: "runtime:local".to_string(),
        loop_id: "loop:tank".to_string(),
        loop_revision: "sha256:loop-1".to_string(),
        source: ControlEvidenceSource::Simulation,
        program_revision: None,
        start_acknowledgement_unconfirmed: None,
        bindings: vec![RecordingRecoveryBinding {
            id: "processVariable".to_string(),
            address: Some("%IL0".to_string()),
            symbol: None,
            iec_type: "LREAL".to_string(),
        }],
    };
    let stored = ControlExperimentRecovery {
        schema_version: 2,
        experiment_id: plan.id.clone(),
        project_id: "project:fixture".to_string(),
        loop_id: "loop:tank".to_string(),
        loop_revision: "sha256:loop-1".to_string(),
        plan: Some(plan.clone()),
        recording_owner: Some(owner.clone()),
    };
    let encoded = serde_json::to_value(&stored).expect("the stored recovery encodes");
    let restored = decode_control_experiment_recovery(&encoded, &scope())
        .expect("the stored recovery is accepted for its loop revision");
    assert_eq!(restored.experiment_id, plan.id);
    assert_eq!(restored.plan.as_ref(), Some(&plan));
    assert_eq!(restored.recording_owner, Some(owner));
}
