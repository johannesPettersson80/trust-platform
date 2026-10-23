//! The context that a connected run notes for the recording it keeps (specification 35, B10.2 and B10.3), against the fake runtime of
//! `identify::test_support`: each experiment is noted when its hold ends, however it ended, before its recording is kept, so that a
//! recording from an aborted or stopped test keeps its stop reason and its restoration. A failure that makes the run abort its
//! experiment, and the engineer's Cancel of a journaled campaign or Test, note the experiment as it ended too. The plant is noted when
//! the runtime proves it.

use std::fs;
use std::sync::Arc;

use serde_json::{json, Value};
use trust_control::experiment::{ExperimentTerminalStatus, ExperimentValue};
use trust_control::failure::Failure;
use trust_control::recording::context::{
    ExperimentContext, PlantContext, RestorationResult, SIMULATION_APPROVAL,
};
use trust_control::recording::evidence::RecordingEvidence;
use trust_control::recording::RecordedRun;

use super::test_support::{
    assert_no_model_of_a_held_input, cancel_journaled, empty_journal, folder, journaled_step,
    passes, plan as step_plan, reopened, run_tank, tank_loop, FakeClock, FakeRuntime, FakeState,
    StartBehaviour,
};
use super::{ConnectedRuntime, IdentifyRequest, IdentifyRuntime, RecordingRetention, RunPurpose};
use crate::journal::JournalEntry;
use crate::runtime_client::fake_runtime::{answer, FakeRuntime as FakeEndpoint};
use crate::runtime_client::runtime_recording::PendingReleases;
use crate::workflow::identify_recovery::cancel_operation_on;

/// A retention that keeps the notes it receives, and how many experiments it had been told of when each recording was kept.
#[derive(Default)]
struct Notes {
    experiments: Vec<ExperimentContext>,
    plant: Option<PlantContext>,
    noted_at_keep: Vec<usize>,
}

impl RecordingRetention for Notes {
    fn retain(&mut self, _run: &RecordedRun, _evidence: &RecordingEvidence) -> Result<(), Failure> {
        self.noted_at_keep.push(self.experiments.len());
        Ok(())
    }

    fn note_experiment(&mut self, experiment: ExperimentContext) {
        self.experiments.push(experiment);
    }

    fn note_plant(&mut self, plant: PlantContext) {
        self.plant = Some(plant);
    }
}

/// The plant the fake runtime proves it runs.
fn plant() -> PlantContext {
    PlantContext {
        id: "tank".to_owned(),
        configuration_revision: "sha256:plant-1".to_owned(),
        configuration: Some(json!({ "id": "tank", "model": "first_order_dead_time" })),
        physical_topology_reference: None,
        model: "first_order_dead_time".to_owned(),
        effective_seed: Some("42".to_owned()),
        uncertainties: Vec::new(),
        scheduled_events: Vec::new(),
        actuator: "Not declared".to_owned(),
        validity_range: "PV 0…100 degC; MV 0…100 %".to_owned(),
    }
}

#[test]
fn an_identification_notes_its_experiment_and_its_plant_before_its_recording_is_kept() {
    let state = FakeState::new(StartBehaviour::Acknowledged, 0);
    state.prove_plant(plant());
    let journal_folder = folder("context-notes-completed");
    let journal = empty_journal(&journal_folder);
    let mut notes = Notes::default();
    let loop_definition = tank_loop("rev-1");
    let request = IdentifyRequest {
        loop_definition: &loop_definition,
        expected_settling_seconds: None,
        evaluate: &passes,
    };
    let Err(failure) = run_tank(&state, &journal, &mut notes, &request, RunPurpose::Identify)
    else {
        panic!("the fit of the held plant input finds no model");
    };
    assert_no_model_of_a_held_input(&failure);
    let [experiment] = notes.experiments.as_slice() else {
        panic!("one experiment is noted: {:?}", notes.experiments);
    };
    let started = state.started_plans();
    let [plan] = started.as_slice() else {
        panic!("one experiment is started: {started:?}");
    };
    assert_eq!(experiment.kind, plan.kind, "the plan the runtime received");
    assert_eq!(
        experiment.affected_signal.as_deref(),
        Some(plan.affected_signal.as_str())
    );
    assert_eq!(
        experiment.trajectory.as_deref(),
        Some(plan.trajectory.as_slice()),
        "the complete commanded trajectory"
    );
    assert_eq!(experiment.duration_ms, Some(plan.duration_ms));
    assert_eq!(experiment.status, ExperimentTerminalStatus::Completed);
    assert!(!experiment.aborted);
    assert!(
        matches!(
            experiment.stop_reason.as_deref(),
            Some("finished_early" | "duration_complete")
        ),
        "{:?}",
        experiment.stop_reason
    );
    assert_eq!(experiment.restoration.status, RestorationResult::Confirmed);
    let ExperimentValue::Number(restored) = plan.restoration_value else {
        panic!("a setpoint is restored to a number: {plan:?}");
    };
    assert!(
        (experiment.restoration.value - restored).abs() < 1e-9,
        "the signal returns to the value the plan restores: {}",
        experiment.restoration.value
    );
    assert_eq!(experiment.approval, SIMULATION_APPROVAL);
    assert!(
        experiment
            .stop_conditions
            .as_ref()
            .is_some_and(|conditions| !conditions.is_empty()),
        "the stop conditions are kept: {experiment:?}"
    );
    assert_eq!(notes.plant, Some(plant()));
    assert_eq!(
        notes.noted_at_keep,
        [1],
        "the experiment is noted before the recording is kept, and the recording is kept once"
    );
    fs::remove_dir_all(&journal_folder).expect("clean up");
}

#[test]
fn an_experiment_the_runtime_stopped_is_noted_with_its_stop_reason_and_its_recording_is_still_kept()
{
    // B10.3: a recording from a test that did not run to its end stays available, and keeps its stop reason and its restoration.
    let state = FakeState::new(StartBehaviour::Acknowledged, 0);
    state.end_experiments_at_start("failed", "signal_forced");
    let journal_folder = folder("context-notes-stopped");
    let journal = empty_journal(&journal_folder);
    let mut notes = Notes::default();
    let loop_definition = tank_loop("rev-1");
    let request = IdentifyRequest {
        loop_definition: &loop_definition,
        expected_settling_seconds: None,
        evaluate: &passes,
    };
    let outcome = run_tank(&state, &journal, &mut notes, &request, RunPurpose::Identify);
    assert!(outcome.is_err(), "a stopped experiment ends the run");
    let [experiment] = notes.experiments.as_slice() else {
        panic!("one experiment is noted: {:?}", notes.experiments);
    };
    assert_eq!(experiment.status, ExperimentTerminalStatus::Failed);
    assert_eq!(experiment.stop_reason.as_deref(), Some("signal_forced"));
    assert!(!experiment.aborted, "a stop is not an abort");
    assert_eq!(experiment.restoration.status, RestorationResult::Confirmed);
    assert_eq!(
        notes.noted_at_keep,
        [1],
        "the recording is kept with the experiment noted"
    );
    assert_eq!(notes.plant, None, "the runtime proved no plant");
    fs::remove_dir_all(&journal_folder).expect("clean up");
}

#[test]
fn an_experiment_the_run_aborts_after_a_failure_is_noted_as_aborted_and_its_recording_is_kept() {
    // B10.3: the connection is lost during the hold, the run aborts its experiment through the restoration path, and the recording of
    // that aborted test keeps its stop reason and its restoration.
    let state = FakeState::new(StartBehaviour::Acknowledged, 0);
    state.refuse_follows_once_started();
    let journal_folder = folder("context-notes-aborted");
    let journal = empty_journal(&journal_folder);
    let mut notes = Notes::default();
    let loop_definition = tank_loop("rev-1");
    let request = IdentifyRequest {
        loop_definition: &loop_definition,
        expected_settling_seconds: None,
        evaluate: &passes,
    };
    let outcome = run_tank(&state, &journal, &mut notes, &request, RunPurpose::Identify);
    assert!(outcome.is_err(), "a lost connection ends the run");
    assert_eq!(
        state.count("experiment.abort"),
        1,
        "the run aborts the experiment it owns"
    );
    let [experiment] = notes.experiments.as_slice() else {
        panic!("one experiment is noted: {:?}", notes.experiments);
    };
    assert_eq!(experiment.status, ExperimentTerminalStatus::Aborted);
    assert!(experiment.aborted);
    assert_eq!(experiment.stop_reason.as_deref(), Some("user_abort"));
    assert_eq!(experiment.restoration.status, RestorationResult::Confirmed);
    assert_eq!(
        notes.noted_at_keep,
        [1],
        "the recording is kept with the experiment noted"
    );
    fs::remove_dir_all(&journal_folder).expect("clean up");
}

#[test]
fn the_cancel_of_a_journaled_campaign_notes_the_experiment_it_aborts_and_the_plant_before_the_recording_is_kept(
) {
    // B10.3: the engineer's Cancel from a reopened panel aborts the journaled experiment and keeps its recording.
    let state = FakeState::new(StartBehaviour::Acknowledged, 30_000);
    state.restart("experiment-1", 30_000, 540_000, 2.0);
    state.prove_plant(plant());
    let journal_folder = folder("context-cancel-campaign");
    let journal = journaled_step(
        &journal_folder,
        &step_plan("experiment-1", 540_000),
        30.0,
        0,
    );
    let mut notes = Notes::default();
    cancel_journaled(
        &FakeRuntime::new(&state),
        &FakeClock::new(&state),
        &journal,
        &tank_loop("rev-1"),
        &mut notes,
    )
    .expect("the Cancel ends the experiment and keeps the recording");
    let [experiment] = notes.experiments.as_slice() else {
        panic!("one experiment is noted: {:?}", notes.experiments);
    };
    assert_eq!(experiment.status, ExperimentTerminalStatus::Aborted);
    assert!(experiment.aborted);
    assert_eq!(experiment.stop_reason.as_deref(), Some("user_abort"));
    assert_eq!(experiment.restoration.status, RestorationResult::Confirmed);
    assert_eq!(
        experiment.trajectory.as_ref().map(Vec::len),
        Some(1),
        "the commanded trajectory of the journaled plan"
    );
    assert_eq!(notes.plant, Some(plant()));
    assert_eq!(notes.noted_at_keep, [1]);
    assert_eq!(reopened(&journal_folder), Vec::<JournalEntry>::new());
    fs::remove_dir_all(&journal_folder).expect("clean up");
}

#[test]
fn the_cancel_of_a_journaled_test_experiment_notes_it_as_the_cancel_of_a_campaign_does() {
    let state = FakeState::new(StartBehaviour::Acknowledged, 30_000);
    state.restart("experiment-1", 30_000, 540_000, 2.0);
    // The journal of a Test holds the experiment and its recording owner, and no campaign: it is the experiment entry of a journaled step.
    let step_folder = folder("context-cancel-test-step");
    journaled_step(&step_folder, &step_plan("experiment-1", 540_000), 30.0, 0);
    let recovery = reopened(&step_folder)
        .into_iter()
        .find_map(|entry| match entry {
            JournalEntry::Experiment { recovery } => Some(recovery),
            _ => None,
        })
        .expect("the journaled step holds its experiment");
    let journal_folder = folder("context-cancel-test");
    let journal = empty_journal(&journal_folder);
    journal
        .lock()
        .persist(JournalEntry::Experiment { recovery })
        .expect("the experiment is journaled");
    let mut notes = Notes::default();
    cancel_operation_on(
        &FakeRuntime::new(&state),
        &FakeClock::new(&state),
        &journal,
        &tank_loop("rev-1"),
        "experiment-1",
        &mut notes,
    )
    .expect("the Cancel ends the experiment and keeps the recording");
    let [experiment] = notes.experiments.as_slice() else {
        panic!("one experiment is noted: {:?}", notes.experiments);
    };
    assert_eq!(experiment.status, ExperimentTerminalStatus::Aborted);
    assert_eq!(experiment.stop_reason.as_deref(), Some("user_abort"));
    assert_eq!(notes.plant, None, "the runtime proved no plant");
    assert_eq!(notes.noted_at_keep, [1]);
    fs::remove_dir_all(&step_folder).expect("clean up");
    fs::remove_dir_all(&journal_folder).expect("clean up");
}

/// The `plant.read` answer of a project that saves the tank (specification 47, §20.2).
fn plant_read(revision: &str) -> Value {
    json!({
        "contract": "trust.plant", "schema_version": 1, "operation": "plant.read", "project_id": "project:sha256:abc",
        "configuration_revision": revision,
        "saved_configuration": {
            "simulation": { "seed": "random" },
            "plants": [{ "id": "tank", "model": "first_order_dead_time", "actuator": { "minimum": 0, "maximum": 100 } }],
            "plant_events": []
        },
        "plants": [], "restart_required": false
    })
}

/// The `plant.status` answer of a runtime that runs the tank under the saved configuration `revision` (specification 47, §20.4).
fn plant_status(revision: &str) -> Value {
    json!({
        "contract": "trust.plant", "schema_version": 1, "operation": "plant.status", "project_id": "project:sha256:abc",
        "available": true, "active_configuration_revision": revision, "effective_seed": "77",
        "plants": [{ "id": "tank", "model": "first_order_dead_time", "operating_state": "running" }]
    })
}

#[test]
fn the_connected_runtime_states_the_plant_it_proves_it_runs_and_none_it_does_not() {
    // Specification 47, §20.3: the saved configuration is evidence of the run only when the runtime reports it active.
    let proven = FakeEndpoint::start(|request| match request["type"].as_str() {
        Some("plant.read") => Some(answer(request, plant_read("sha256:plant-1"))),
        Some("plant.status") => Some(answer(request, plant_status("sha256:plant-1"))),
        _ => None,
    });
    let stale = FakeEndpoint::start(|request| match request["type"].as_str() {
        Some("plant.read") => Some(answer(request, plant_read("sha256:plant-2"))),
        Some("plant.status") => Some(answer(request, plant_status("sha256:plant-1"))),
        _ => None,
    });
    let pending = Arc::new(PendingReleases::new());
    let mut loop_definition = tank_loop("rev-1");
    loop_definition.plant_id = Some("tank".to_owned());

    let connection = proven.connection(None);
    let runtime = ConnectedRuntime::new(&connection, &pending);
    let plant = runtime
        .plant_context(&loop_definition)
        .expect("the runtime proves the plant it runs");
    assert_eq!(plant.id, "tank");
    assert_eq!(plant.configuration_revision, "sha256:plant-1");
    assert_eq!(plant.effective_seed.as_deref(), Some("77"));
    let statuses = proven.requests_of("plant.status");
    assert_eq!(statuses.len(), 1, "one read of the status");
    assert_eq!(
        statuses[0]["params"]["project_id"], "project:sha256:abc",
        "the status is asked for the project that plant.read named"
    );

    let connection = stale.connection(None);
    let runtime = ConnectedRuntime::new(&connection, &pending);
    assert_eq!(
        runtime.plant_context(&loop_definition),
        None,
        "a configuration the runtime does not run is no evidence of the run"
    );

    // A loop that names no plant makes no request at all.
    loop_definition.plant_id = None;
    let connection = proven.connection(None);
    let runtime = ConnectedRuntime::new(&connection, &pending);
    assert_eq!(runtime.plant_context(&loop_definition), None);
    assert_eq!(proven.requests_of("plant.read").len(), 1);
}
