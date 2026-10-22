//! The context of a run (specification 35, B10.2 and B10.3): the experiment as it ended, the plant the runtime proves, the rules of both,
//! the recording file that keeps them, and the context a saved result states.

use serde_json::{json, Value};

use super::{
    saved_context, ExperimentContext, PlantContext, RestorationContext, RestorationResult,
    RunContext, NO_ACTUATOR, PHYSICAL_APPROVAL, SIMULATION_APPROVAL,
};
use crate::experiment::{
    ControlExperimentKind, ControlExperimentPlan, ControlExperimentPoint, ExperimentTerminalStatus,
};
use crate::loop_definition::LoopDefinition;
use crate::recording::evidence::recording_evidence;
use crate::recording::fixtures;
use crate::recording::{EvidenceSource, RecordingFile};
use crate::signals::Quality;

/// The plan of a step of the tank's setpoint from `from` to `to`, restored to `restoration`, on a `source` target.
fn plan(
    source: &str,
    kind: &str,
    from: &Value,
    to: &Value,
    restoration: &Value,
) -> ControlExperimentPlan {
    // A project's sequence writes no plant input, so its trajectory is empty (annex E5.2).
    let trajectory = if kind == "project_sequence" {
        json!([])
    } else {
        json!([{ "atMs": 0, "value": to }])
    };
    let physical = source == "physical";
    crate::json::decode_value(json!({
        "id": "experiment-1", "source": source, "kind": kind,
        "affectedSignal": "Main.TIC101_Setpoint", "valueType": "LREAL", "from": from, "to": to, "unit": "degC",
        "minimum": 0, "maximum": 100, "durationMs": 570_000, "expectedScan": 7, "task": "$background",
        "stopConditions": ["Abort"], "restorationValue": restoration,
        "requiresPhysicalApproval": physical, "trajectory": trajectory
    }))
    .expect("the plan decodes")
}

/// The setpoint step of the tank from 45 to 47 degC on a simulated target.
fn setpoint_step() -> ControlExperimentPlan {
    plan(
        "simulation",
        "setpoint_step",
        &json!(45),
        &json!(47),
        &json!(45),
    )
}

/// The status record of an experiment (annex E5.6), with every key the annex names.
fn status(
    state: &str,
    stop_reason: Option<&str>,
    restoration: &str,
    reason: Option<&str>,
    applied: Option<f64>,
) -> Value {
    json!({
        "experiment_id": "experiment-1", "source": "simulation", "kind": "setpoint_step", "task": "$background",
        "symbol": "Main.TIC101_Setpoint", "status": state, "stop_reason": stop_reason, "restoration": restoration,
        "restoration_reason": reason, "elapsed_ms": 570_000, "duration_ms": 570_000, "reviewed_scan": 7,
        "started_scan": 8,
        "last_applied": applied.map(|value| json!({ "index": 0, "value": value, "time_ms": 0, "scan": 8, "process_time_ms": 100 })),
        "relay_feedback": null, "abort_binding": null
    })
}

/// The context of the setpoint step above when it ran to its end and the runtime confirmed its restoration.
fn completed_step() -> ExperimentContext {
    ExperimentContext {
        kind: ControlExperimentKind::SetpointStep,
        affected_signal: Some("Main.TIC101_Setpoint".to_owned()),
        address: None,
        requested_value: 47.0,
        applied_value: Some(47.0),
        duration_ms: Some(570_000.0),
        interval_ms: None,
        seed: None,
        frequencies_hz: None,
        trajectory: Some(vec![ControlExperimentPoint {
            at_ms: 0.0,
            value: 47.0,
        }]),
        stop_conditions: Some(vec!["Abort".to_owned()]),
        minimum: 0.0,
        maximum: 100.0,
        approval: SIMULATION_APPROVAL.to_owned(),
        status: ExperimentTerminalStatus::Completed,
        stop_reason: Some("duration_complete".to_owned()),
        aborted: false,
        restoration: RestorationContext {
            value: 45.0,
            status: RestorationResult::Confirmed,
            reason: None,
        },
    }
}

#[test]
fn an_experiment_that_ended_keeps_its_plan_and_how_it_ended() {
    let ended = status(
        "completed",
        Some("duration_complete"),
        "restored",
        None,
        Some(47.0),
    );
    let context = ExperimentContext::of(&setpoint_step(), &ended).expect("an ended experiment");
    assert_eq!(context, completed_step());
    context
        .check()
        .expect("a context of a real run keeps the rules");
}

#[test]
fn an_aborted_experiment_keeps_its_stop_reason_and_a_failed_restoration_keeps_its_reason() {
    let aborted = status("aborted", Some("user_abort"), "restored", None, Some(47.0));
    let context = ExperimentContext::of(&setpoint_step(), &aborted).expect("an aborted experiment");
    assert!(context.aborted, "the abort state is kept");
    assert_eq!(context.status, ExperimentTerminalStatus::Aborted);
    assert_eq!(context.stop_reason.as_deref(), Some("user_abort"));
    assert_eq!(context.restoration.status, RestorationResult::Confirmed);

    let unrestored = status(
        "failed_to_restore",
        Some("duration_complete"),
        "failed",
        Some("confirmation_timeout"),
        None,
    );
    let context =
        ExperimentContext::of(&setpoint_step(), &unrestored).expect("an unrestored experiment");
    assert_eq!(context.status, ExperimentTerminalStatus::FailedToRestore);
    assert!(!context.aborted);
    assert_eq!(
        context.restoration,
        RestorationContext {
            value: 45.0,
            status: RestorationResult::Failed,
            reason: Some("confirmation_timeout".to_owned()),
        },
        "the restoration result and the reason it failed are kept"
    );
    assert_eq!(
        context.applied_value, None,
        "a value the runtime reported none of stays missing"
    );
    context.check().expect("the rules hold");
}

#[test]
fn an_experiment_that_has_not_ended_has_no_context() {
    let plan = setpoint_step();
    let running = status("running", None, "pending", None, None);
    let pending = status(
        "completed",
        Some("duration_complete"),
        "pending",
        None,
        None,
    );
    for unfinished in [
        running,
        pending,
        json!({ "status": "completed" }),
        json!(null),
    ] {
        assert_eq!(
            ExperimentContext::of(&plan, &unfinished),
            None,
            "{unfinished}"
        );
    }
}

#[test]
fn a_boolean_experiment_keeps_its_values_as_numbers_and_a_physical_one_its_approval() {
    let sequence = plan(
        "physical",
        "project_sequence",
        &json!(false),
        &json!(true),
        &json!(false),
    );
    let ended = status(
        "completed",
        Some("duration_complete"),
        "not_required",
        None,
        None,
    );
    let context = ExperimentContext::of(&sequence, &ended).expect("an ended experiment");
    assert_eq!(context.kind, ControlExperimentKind::ProjectSequence);
    assert_eq!(context.approval, PHYSICAL_APPROVAL);
    assert_eq!(
        (
            context.requested_value.to_bits(),
            context.restoration.value.to_bits()
        ),
        (1.0_f64.to_bits(), 0.0_f64.to_bits()),
        "a Boolean is 0 or 1"
    );
    assert_eq!(context.restoration.status, RestorationResult::NotRequired);
}

/// A change to an experiment's context that makes it break one rule of the file.
type Breakage = fn(&mut ExperimentContext);

#[test]
fn an_experiment_context_that_breaks_a_rule_is_refused_and_left_out_of_a_kept_context() {
    let cases: [(&str, Breakage); 6] = [
        ("bounds", |context| context.minimum = 101.0),
        ("approval", |context| context.approval = " ".to_owned()),
        ("abort state", |context| context.aborted = true),
        ("duration", |context| context.duration_ms = Some(0.0)),
        ("trajectory", |context| {
            context.trajectory = Some(vec![ControlExperimentPoint {
                at_ms: 0.0,
                value: f64::NAN,
            }]);
        }),
        ("stop reason", |context| {
            context.stop_reason = Some(String::new());
        }),
    ];
    for (name, change) in cases {
        let mut context = completed_step();
        change(&mut context);
        let refusal = context.check().expect_err(name);
        assert!(
            refusal.message.contains("not consistent"),
            "{name}: {}",
            refusal.message
        );
    }
    let mut broken = completed_step();
    broken.minimum = 101.0;
    let kept = RunContext {
        experiments: vec![broken.clone(), completed_step()],
        plant: None,
    }
    .checked(EvidenceSource::SimulatedPlant)
    .expect("one experiment is left");
    assert_eq!(kept.experiments, vec![completed_step()]);
    assert_eq!(
        RunContext {
            experiments: vec![broken],
            plant: None
        }
        .checked(EvidenceSource::SimulatedPlant),
        None,
        "a context with nothing left is no context"
    );
}

/// The tank loop of the recording fixtures, which names the plant `tank`.
fn loop_with_plant() -> LoopDefinition {
    let mut loop_definition = fixtures::loop_definition();
    loop_definition.plant_id = Some("tank".to_owned());
    loop_definition
}

/// The plant of the tank as the project saves it: a first-order plant with an uncertainty and an actuator, beside another plant.
fn saved_plant() -> Value {
    json!({
        "id": "tank", "model": "first_order_dead_time", "parameters": { "gain": 1.5 }, "initial_state": [20],
        "actuator": { "minimum": 0, "maximum": 100 },
        "uncertainties": [{ "target": "gain", "mode": "fixed", "value": 0.1 }]
    })
}

/// The `plant.read` answer of a project that saves two plants and an event of each (specification 34, §20.2).
fn plant_read(revision: &str) -> Value {
    json!({
        "contract": "trust.plant", "schema_version": 1, "operation": "plant.read", "project_id": "project:sha256:abc",
        "configuration_revision": revision,
        "saved_configuration": {
            "simulation": { "seed": "random" },
            "plants": [saved_plant(), { "id": "other", "model": "integrator" }],
            "plant_events": [
                { "plant": "tank", "at_ms": 5000, "kind": "set_parameter", "parameter": "gain", "value": 1.2 },
                { "plant": "other", "at_ms": 1, "kind": "reset" }
            ]
        },
        "plants": [], "restart_required": false
    })
}

/// The `plant.status` answer of a runtime that runs the saved configuration `revision` with the seed 77 (specification 34, §20.4).
fn plant_status(revision: &str, plant_ids: &[&str]) -> Value {
    let plants: Vec<Value> = plant_ids
        .iter()
        .map(|id| json!({ "id": id, "model": "first_order_dead_time", "operating_state": "running" }))
        .collect();
    json!({
        "contract": "trust.plant", "schema_version": 1, "operation": "plant.status", "project_id": "project:sha256:abc",
        "available": true, "active_configuration_revision": revision, "effective_seed": "77", "plants": plants
    })
}

#[test]
fn the_plant_a_runtime_proves_it_runs_keeps_its_configuration_seed_events_and_actuator() {
    let context = PlantContext::of(
        &loop_with_plant(),
        &plant_read("sha256:plant-1"),
        &plant_status("sha256:plant-1", &["tank", "other"]),
    )
    .expect("the runtime runs the saved configuration");
    assert_eq!(
        context,
        PlantContext {
            id: "tank".to_owned(),
            configuration_revision: "sha256:plant-1".to_owned(),
            configuration: Some(saved_plant()),
            physical_topology_reference: None,
            model: "first_order_dead_time".to_owned(),
            effective_seed: Some("77".to_owned()),
            uncertainties: vec![r#"{"mode":"fixed","target":"gain","value":0.1}"#.to_owned()],
            scheduled_events: vec![
                r#"{"at_ms":5000,"kind":"set_parameter","parameter":"gain","plant":"tank","value":1.2}"#
                    .to_owned()
            ],
            actuator: r#"{"maximum":100,"minimum":0}"#.to_owned(),
            validity_range: "PV 0…100 %; MV 0…100 %".to_owned(),
        },
        "the effective seed is the runtime's, not the word the configuration saved"
    );
    context
        .check(EvidenceSource::SimulatedPlant)
        .expect("a simulated plant states its configuration");
    assert!(
        context.check(EvidenceSource::PhysicalPlant).is_err(),
        "a simulated plant is never physical evidence (A9)"
    );
}

#[test]
fn a_plant_the_runtime_does_not_prove_has_no_context() {
    let loop_definition = loop_with_plant();
    let read = plant_read("sha256:plant-1");
    let active = plant_status("sha256:plant-1", &["tank"]);
    assert!(PlantContext::of(&loop_definition, &read, &active).is_some());
    assert_eq!(
        PlantContext::of(
            &loop_definition,
            &read,
            &plant_status("sha256:plant-2", &["tank"])
        ),
        None,
        "the saved configuration is not the one the runtime runs"
    );
    assert_eq!(
        PlantContext::of(
            &loop_definition,
            &read,
            &plant_status("sha256:plant-1", &["other"])
        ),
        None,
        "the plant is not active"
    );
    let mut unnamed = loop_definition.clone();
    unnamed.plant_id = None;
    assert_eq!(
        PlantContext::of(&unnamed, &read, &active),
        None,
        "the loop names no plant"
    );
    let mut missing = loop_definition;
    missing.plant_id = Some("missing".to_owned());
    assert_eq!(
        PlantContext::of(
            &missing,
            &read,
            &plant_status("sha256:plant-1", &["missing"])
        ),
        None,
        "the saved configuration does not describe the plant"
    );
    let mut unseeded = active;
    unseeded
        .as_object_mut()
        .expect("an object")
        .remove("effective_seed");
    assert_eq!(
        PlantContext::of(&loop_with_plant(), &read, &unseeded),
        None,
        "without the effective seed the run cannot be replayed"
    );
}

#[test]
fn a_plant_without_a_declared_actuator_says_so() {
    let mut read = plant_read("sha256:plant-1");
    read["saved_configuration"]["plants"][0]
        .as_object_mut()
        .expect("an object")
        .remove("actuator");
    let context = PlantContext::of(
        &loop_with_plant(),
        &read,
        &plant_status("sha256:plant-1", &["tank"]),
    )
    .expect("the runtime runs the saved configuration");
    assert_eq!(context.actuator, NO_ACTUATOR);
}

/// A complete recording of the tank, kept as a simulated-plant recording file.
fn recording_file() -> RecordingFile {
    let run = fixtures::run(
        vec![fixtures::binding("processVariable", "%IL0", "LREAL")],
        vec![
            fixtures::sample(1, Some(0), &[Some(2.5)], &[Quality::Good]),
            fixtures::sample(2, Some(10_000_000), &[Some(2.5)], &[Quality::Good]),
        ],
    );
    let evidence = recording_evidence(&run, None);
    RecordingFile::new("loop:tank", EvidenceSource::SimulatedPlant, run, evidence)
}

#[test]
fn a_recording_file_keeps_the_context_of_its_run_and_an_older_file_keeps_its_identifier() {
    let plain = recording_file();
    let plain_id = plain.id().expect("identified");
    assert!(
        serde_json::to_value(&plain)
            .expect("json")
            .get("context")
            .is_none(),
        "a file without a context writes no such field, so its identifier is unchanged"
    );
    let mut kept = plain;
    kept.context = Some(RunContext {
        experiments: vec![completed_step()],
        plant: PlantContext::of(
            &loop_with_plant(),
            &plant_read("sha256:plant-1"),
            &plant_status("sha256:plant-1", &["tank"]),
        ),
    });
    let kept_id = kept.id().expect("identified");
    assert_ne!(kept_id, plain_id, "the context is part of the content");
    let text = kept.encode().expect("encoded");
    let read = RecordingFile::decode(&text, "loop:tank", &kept_id).expect("decoded");
    assert_eq!(read, kept, "the file reads back with its context");
}

#[test]
fn a_recording_file_whose_context_breaks_a_rule_or_contradicts_its_source_is_neither_written_nor_read(
) {
    let mut broken = completed_step();
    broken.minimum = 101.0;
    let mut file = recording_file();
    file.context = Some(RunContext {
        experiments: vec![broken],
        plant: None,
    });
    assert!(file
        .encode()
        .expect_err("not written")
        .message
        .contains("not consistent"));
    // The text that another writer or a hand edit would leave is refused as a whole when it is read (B10.4).
    let text = serde_json::to_string(&file).expect("the file serializes");
    assert!(
        RecordingFile::decode(&text, "loop:tank", "rec-00000000000000000000000000000000")
            .expect_err("not read")
            .message
            .contains("not consistent")
    );

    let plant = PlantContext::of(
        &loop_with_plant(),
        &plant_read("sha256:plant-1"),
        &plant_status("sha256:plant-1", &["tank"]),
    );
    let mut physical = recording_file();
    physical.source = EvidenceSource::PhysicalPlant;
    physical.context = Some(RunContext {
        experiments: Vec::new(),
        plant,
    });
    assert!(
        physical
            .encode()
            .expect_err("not written")
            .message
            .contains("not consistent"),
        "a simulated plant is never kept as physical evidence (A9)"
    );
}

/// A saved result in the form the `TypeScript` extension wrote (`ControlStudioWorkflowResult`), with the experiment and the plant a
/// Simulation result keeps (B10.2).
fn saved_result() -> Value {
    json!({
        "schemaVersion": 4, "source": "Simulation",
        "experiment": {
            "kind": "prbs", "affectedSignal": "ValveMV", "address": "%QL0", "requestedValue": 25, "appliedValue": 25,
            "durationMs": 10_000, "intervalMs": 500, "seed": 7,
            "trajectory": [{ "atMs": 0, "value": 25 }, { "atMs": 500, "value": 15 }],
            "stopConditions": ["Abort", "10 s timeout", "Stale runtime data"],
            "minimum": 0, "maximum": 100, "approval": "Simulation confirmation", "status": "failed_to_restore",
            "stopReason": "signal_forced", "aborted": false,
            "restoration": { "value": 20, "status": "failed", "reason": "confirmation_timeout" }
        },
        "plant": {
            "source": "Simulation", "id": "tank", "configurationRevision": "sha256:plant-1", "configuration": saved_plant(),
            "model": "first_order_dead_time", "effectiveSeed": "42",
            "uncertainties": [r#"{"target":"gain","mode":"fixed","value":0.1}"#],
            "scheduledEvents": [], "actuator": r#"{"minimum":0,"maximum":100}"#,
            "validityRange": "PV 0…100 °C; MV 0…100 %"
        },
        "recording": { "rawSamples": [] }
    })
}

#[test]
fn a_saved_result_keeps_the_experiment_and_the_plant_it_states() {
    let context = saved_context(&saved_result(), EvidenceSource::SimulatedPlant)
        .expect("the result states a context");
    let [experiment] = context.experiments.as_slice() else {
        panic!("one experiment: {context:?}");
    };
    assert_eq!(experiment.kind, ControlExperimentKind::Prbs);
    assert_eq!(experiment.seed, Some(7));
    assert_eq!(experiment.status, ExperimentTerminalStatus::FailedToRestore);
    assert_eq!(experiment.restoration.status, RestorationResult::Failed);
    assert_eq!(
        experiment.restoration.reason.as_deref(),
        Some("confirmation_timeout")
    );
    assert_eq!(
        experiment.trajectory.as_ref().map(Vec::len),
        Some(2),
        "the complete commanded trajectory is kept"
    );
    let plant = context.plant.expect("the plant");
    assert_eq!(plant.effective_seed.as_deref(), Some("42"));
    assert_eq!(plant.configuration, Some(saved_plant()));
}

#[test]
fn a_saved_result_states_its_context_in_the_provenance_of_a_copied_recording_and_never_as_another_source(
) {
    let mut copied = saved_result();
    copied["recording"]["provenance"] = json!({
        "experiment": copied["experiment"].clone(), "plant": copied["plant"].clone()
    });
    copied["experiment"]["kind"] = json!("mv_step");
    copied["plant"]["id"] = json!("elsewhere");
    let context = saved_context(&copied, EvidenceSource::SimulatedPlant).expect("a context");
    assert_eq!(
        context.experiments[0].kind,
        ControlExperimentKind::Prbs,
        "the recording's own provenance decides, as the result of a copied recording does"
    );
    assert_eq!(context.plant.map(|plant| plant.id), Some("tank".to_owned()));

    let physical = saved_context(&saved_result(), EvidenceSource::PhysicalPlant)
        .expect("the experiment is kept");
    assert_eq!(
        physical.plant, None,
        "a plant of another evidence source is not kept (A9)"
    );
}

#[test]
fn a_saved_result_that_states_a_part_in_a_form_the_engine_does_not_read_keeps_nothing_of_it() {
    let mut partial = saved_result();
    partial["experiment"] = json!({ "status": "completed" });
    partial["plant"] = json!(null);
    assert_eq!(
        saved_context(&partial, EvidenceSource::SimulatedPlant),
        None,
        "an experiment that states only its status, and no plant, leave no context"
    );
    assert_eq!(
        saved_context(
            &json!({ "source": "Simulation" }),
            EvidenceSource::SimulatedPlant
        ),
        None
    );
}
