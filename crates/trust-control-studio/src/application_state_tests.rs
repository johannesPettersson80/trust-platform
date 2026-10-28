//! Tests of the Validate page's offers and the block, as the engine reads them from the loop's journal and study (specification 35,
//! B6.4, B6.7, B7, A8.6; annex E11.7). A reopened loop holds the block of an Apply's checkpoint that waits for its Restore, and the
//! engine refuses the actions the block holds with its reason and the next action Restore. The page's own recovery
//! (`threePageRecovery.ts`) held that lock only for a failure received in its session; its tests are replaced by these and by the
//! block's own (`commissioning::checkpoint_block` in trust-control). A Restore on a runtime that was replaced since Apply keeps the
//! checkpoint as history, and the block ends (B6.4, B6.7).

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::{json, Value};
use trust_control::commissioning::checkpoint::{CheckpointStage, CommissioningCheckpoint};
use trust_control::commissioning::checkpoint_block::UNRESOLVED_APPLY;
use trust_control::commissioning::found_mode::FoundMode;
use trust_control::commissioning::parameter_application::ParameterOwner;
use trust_control::commissioning::parameter_status::{ParameterStatus, RecordState};
use trust_control::commissioning::transfer::proportional_transfer_binding;
use trust_control::commissioning::transfer_tracking::{proportional_tracking, TransferTracking};
use trust_control::failure::NextAction;
use trust_control::loop_definition::LoopDefinition;
use trust_control::messages::OperationUpdate;
use trust_control::stages::{Stage, IDENTIFY_FIRST, TUNE_FIRST};
use trust_control::study::apply::{ApplySection, RestorationReceipt};
use trust_control::study::inputs::InputsSection;
use trust_control::study::Study;

use super::{journaled_checkpoint, refuse_applied_validation, state_now};
use crate::apply_action::keep_restoration;
use crate::commands::{ControlStudio, IDENTIFY, INPUTS, OPEN, RESTORE, TUNE, VALIDATE};
use crate::inputs_action::journal_folder;
use crate::journal::{Journal, JournalEntry};
use crate::operation::UpdateSink;
use crate::runtime_client::fake_runtime::{answer, refusal, FakeRuntime};
use crate::settings::{self, Platform};
use crate::storage;
use crate::workflow::apply::test_support;

const EXAMPLE: &str = include_str!("../../../examples/control-pid-commissioning/control-loop.json");

/// The first sentence of [`UNRESOLVED_APPLY`], which a refusal's message is (A11.1).
const UNRESOLVED_MESSAGE: &str =
    "An Apply or a Restore of this loop stopped before the runtime confirmed its end.";

/// What a Restore answers on a runtime restarted since the Apply of [`checkpoint`], worked out by hand from B6.4 ("nothing of the
/// checkpoint can be restored: Restore says so, the checkpoint is kept as history, and the loop continues from a new validation") with
/// the two program revisions named (A11.1): program:1 at Apply, program:2 now.
const RETIRED_REASON: &str = "Restore is unavailable: the runtime was restarted or its program reloaded since Apply (program revision program:1 at Apply, program:2 now), so nothing of the checkpoint can be restored. The checkpoint is kept as history, and the loop continues from a new validation.";

struct Ignore;

impl UpdateSink for Ignore {
    fn send(&self, _update: OperationUpdate) {}
}

/// A project folder with the example loop and its working study, and the arguments of `open` for it.
fn example_project(name: &str) -> (PathBuf, LoopDefinition, Value) {
    let root = std::env::temp_dir().join(format!(
        "trust-control-application-state-{name}-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("project");
    fs::write(root.join("control-loop.json"), EXAMPLE).expect("loop");
    let example = LoopDefinition::decode(EXAMPLE).expect("example");
    storage::save_study(&root, &Study::new(&example.id, &example.revision), &[])
        .expect("the working study");
    let request = json!({
        "projectRoot": root.display().to_string(),
        "runtimeId": example.runtime_id,
        "loopId": example.id,
        "storageFolder": root.join("storage").display().to_string(),
    });
    (root, example, request)
}

/// The folder of the operation journals that `request` names (A8.3).
fn storage_folder(request: &Value) -> PathBuf {
    PathBuf::from(request["storageFolder"].as_str().expect("a storage folder"))
}

/// An Apply's checkpoint at `stage`, with the apply tests' proportional loop as its content: opening and the block read only its stage
/// and whether it holds a recording (A8.5), never its bindings, so the example loop's journal may hold it.
fn checkpoint(stage: CheckpointStage) -> CommissioningCheckpoint {
    let loop_definition = test_support::proportional_loop();
    let binding =
        proportional_transfer_binding(&loop_definition).expect("the proportional binding");
    let tracking =
        proportional_tracking(&loop_definition, &binding, true).expect("the proportional tracking");
    CommissioningCheckpoint {
        id: "commissioning-test".to_owned(),
        loop_revision: loop_definition.revision.clone(),
        stage,
        model: test_support::model(),
        task: test_support::task(),
        origin: ParameterStatus {
            supports_request_id: Some(true),
            transaction_id: None,
            program_revision: "program:1".to_owned(),
            binding_revision: "binding:1".to_owned(),
            active_revision: "active:1".to_owned(),
            values: BTreeMap::from([("Main.Kp".to_owned(), 1.0)]),
            status: RecordState::Applied,
            previous_revision: None,
            previous_values: None,
            reason: None,
        },
        candidate: BTreeMap::from([("Main.Kp".to_owned(), 2.0)]),
        parameter_operation: ParameterOwner {
            id: "parameter:1".to_owned(),
            program: "program:1".to_owned(),
            binding: "binding:1".to_owned(),
        },
        references: BTreeMap::new(),
        binding,
        recovery_tracking: TransferTracking::Proportional(tracking),
        tolerance: 0.1,
        replacement_program_revision: None,
        transfer: None,
        recording: None,
        recording_result: None,
        found_mode: Some(FoundMode::Manual),
        process_noise: None,
        validation_identity: "sha256:applied".to_owned(),
        transfers: Vec::new(),
        schedule_origin: None,
    }
}

/// Journals `checkpoint` in the journal of `example` that `request` names, as an Apply journals it before its first write (A8.5).
fn journal(request: &Value, example: &LoopDefinition, checkpoint: CommissioningCheckpoint) {
    let mut journal = Journal::open(&storage_folder(request), &example.project_id, &example.id)
        .expect("the journal");
    journal
        .persist(JournalEntry::Commissioning {
            checkpoint: Box::new(checkpoint),
        })
        .expect("journaled");
}

/// The stage named `name` of an answer's `stages`.
fn stage<'a>(answer: &'a Value, name: &str) -> &'a Value {
    answer["stages"]
        .as_array()
        .and_then(|stages| stages.iter().find(|stage| stage["stage"] == name))
        .unwrap_or_else(|| panic!("{name}: {answer}"))
}

/// R3 (B6.4: "Restore previous settings is offered while the loop's journal holds the Apply's checkpoint"; B6.7: "dependent actions stay
/// blocked until confirmed"; A8.6): a reopened loop whose journal holds an Apply's checkpoint that stopped at its transfer holds the block
/// in its stages, offers Restore in the position and no Apply, Keep or Resume. A completed checkpoint, whose settings stand, holds
/// nothing back (B7). The wrong behaviour caught: the block living only in the page's session, so that a reopened loop offers Identify,
/// Tune and Validate beside an unresolved Apply.
#[test]
fn a_waiting_checkpoint_blocks_after_a_reopen() {
    let (root, example, request) = example_project("waiting-reopen");
    journal(&request, &example, checkpoint(CheckpointStage::Transfer));
    let studio = ControlStudio::new(Arc::new(Ignore));
    let opened = studio
        .execute(OPEN, std::slice::from_ref(&request))
        .expect("answered");
    let result = &opened["result"];
    assert_eq!(result["kind"], "opened", "{opened}");
    for name in ["Test", "Identify", "Tune", "Validate", "Apply"] {
        assert_eq!(
            stage(result, name)["available"],
            json!(false),
            "{name}: {opened}"
        );
        assert_eq!(
            stage(result, name)["reason"],
            UNRESOLVED_APPLY,
            "{name}: {opened}"
        );
    }
    assert_eq!(
        stage(result, "Monitor")["available"],
        json!(true),
        "{opened}"
    );
    assert_eq!(result["applyAvailable"], json!(false), "{opened}");
    assert_eq!(result["restoreAvailable"], json!(true), "{opened}");
    assert_eq!(result["keepAvailable"], json!(false), "{opened}");
    assert_eq!(
        result["applyObservationResumable"],
        json!(false),
        "{opened}"
    );
    assert_eq!(result["primaryAction"], "restore", "{opened}");
    assert_eq!(
        result["reattached"],
        json!([]),
        "a checkpoint is no operation (B6.4): {opened}"
    );

    journal(&request, &example, checkpoint(CheckpointStage::Completed));
    let standing = studio.execute(OPEN, &[request]).expect("answered");
    let result = &standing["result"];
    assert_eq!(
        stage(result, "Identify")["available"],
        json!(true),
        "{standing}"
    );
    assert!(
        stage(result, "Identify").get("reason").is_none(),
        "{standing}"
    );
    assert_eq!(result["restoreAvailable"], json!(true), "{standing}");
    assert_eq!(
        result["primaryAction"], "restore",
        "the settings stand: {standing}"
    );
    fs::remove_dir_all(&root).expect("clean up");
}

/// R3 (A8.4 Reset; B6.7; E11.7: "New work cannot bypass an unresolved application owner"): Reset is refused while an Apply's checkpoint
/// waits for its Restore, with the block's reason and the next action Restore, and the working study stays where it is. The wrong
/// behaviour caught: the engine's generic "wait" refusal, which offers no way out, or a Reset that moves the study to history beside an
/// unresolved Apply.
#[test]
fn reset_is_refused_while_a_checkpoint_waits() {
    let (root, example, request) = example_project("waiting-reset");
    journal(&request, &example, checkpoint(CheckpointStage::Parameters));
    let studio = ControlStudio::new(Arc::new(Ignore));
    let mut reset = request;
    reset["action"] = json!({ "action": "reset" });
    let refused = studio.execute(IDENTIFY, &[reset]).expect("answered");
    assert_eq!(refused["failure"]["nextAction"], "restore", "{refused}");
    assert_eq!(
        refused["failure"]["nextActionLabel"], "Restore previous settings",
        "{refused}"
    );
    assert_eq!(
        refused["failure"]["message"], UNRESOLVED_MESSAGE,
        "{refused}"
    );
    assert_eq!(refused["failure"]["detail"], UNRESOLVED_APPLY, "{refused}");
    assert!(
        !storage::study_directory(&root, &example.id)
            .join("history")
            .exists(),
        "the study is not moved"
    );
    fs::remove_dir_all(&root).expect("clean up");
}

/// R3: Tune, Validate and the imports wait for the Restore too, each refused with the block's reason and the next action Restore before
/// it reads anything else. The wrong behaviour caught: an action that the block holds starting, or answering a refusal of its own that
/// names no Restore.
#[test]
fn tune_validate_and_the_imports_are_refused_while_a_checkpoint_waits() {
    let (root, example, request) = example_project("waiting-actions");
    journal(&request, &example, checkpoint(CheckpointStage::Transfer));
    let studio = ControlStudio::new(Arc::new(Ignore));
    let mut asked = vec![(TUNE, request.clone()), (VALIDATE, request.clone())];
    for action in [
        json!({ "action": "importModel", "json": "{}" }),
        json!({ "action": "importRecording", "json": "{}" }),
    ] {
        let mut import = request.clone();
        import["action"] = action;
        asked.push((IDENTIFY, import));
    }
    for (method, arguments) in asked {
        let refused = studio.execute(method, &[arguments]).expect("answered");
        assert_eq!(
            refused["failure"]["nextAction"], "restore",
            "{method}: {refused}"
        );
        assert_eq!(
            refused["failure"]["message"], UNRESOLVED_MESSAGE,
            "{method}: {refused}"
        );
    }
    fs::remove_dir_all(&root).expect("clean up");
}

/// R1 (B6.4: the engine refuses "a change of the study inputs" with the block's reason and the next action Restore previous settings;
/// annex E11.7: editable task inputs wait until the operation is reconciled): while an Apply's checkpoint waits for its Restore, a change
/// of the study inputs is refused with the reason and the next action Restore, and the study records nothing. Once the checkpoint
/// completed, so that the Apply's settings stand and nothing waits, the same change is recorded. The wrong behaviour caught: the engine
/// recording what the page's editors send beside an unresolved Apply, so that only a lock in the page held the inputs (N1).
#[test]
fn an_inputs_change_is_refused_while_a_checkpoint_waits() {
    let (root, example, request) = example_project("waiting-inputs");
    journal(&request, &example, checkpoint(CheckpointStage::Transfer));
    let studio = ControlStudio::new(Arc::new(Ignore));
    let mut change = request.clone();
    change["change"] = json!({ "family": "PI" });
    let refused = studio
        .execute(INPUTS, std::slice::from_ref(&change))
        .expect("answered");
    assert_eq!(refused["failure"]["nextAction"], "restore", "{refused}");
    assert_eq!(
        refused["failure"]["nextActionLabel"], "Restore previous settings",
        "{refused}"
    );
    assert_eq!(
        refused["failure"]["message"], UNRESOLVED_MESSAGE,
        "{refused}"
    );
    assert_eq!(refused["failure"]["detail"], UNRESOLVED_APPLY, "{refused}");
    let study =
        storage::open_study(&root, &example.id, &example.revision).expect("the working study");
    assert_eq!(
        study
            .section::<InputsSection>()
            .expect("the inputs section reads"),
        None,
        "a refused change records nothing"
    );
    journal(&request, &example, checkpoint(CheckpointStage::Completed));
    let recorded = studio.execute(INPUTS, &[change]).expect("answered");
    assert_eq!(
        recorded["result"]["inputs"]["structure"], "PI",
        "the settings stand and nothing waits: {recorded}"
    );
    fs::remove_dir_all(&root).expect("clean up");
}

/// R1 (A8.3: the extension and the command line read one journal per loop, in the engine's journal folder): an inputs request that names
/// no journal folder, or an empty one, reads the engine's folder, and a named folder is read as it is. The wrong behaviour caught: a
/// request that leaves the folder out reading no journal, and so escaping the refusal of a checkpoint that waits.
#[test]
fn an_inputs_request_without_a_journal_folder_reads_the_engines() {
    let environment = |name: &str| {
        matches!(name, "HOME" | "LOCALAPPDATA").then(|| OsString::from("/home/engineer"))
    };
    let engines = settings::default_journal_folder(Platform::current(), &environment)
        .expect("the engine's folder");
    assert_eq!(
        journal_folder(None, &environment).expect("the folder"),
        engines
    );
    assert_eq!(
        journal_folder(Some(String::new()), &environment).expect("the folder"),
        engines
    );
    assert_eq!(
        journal_folder(Some("/journals".to_owned()), &environment).expect("the folder"),
        PathBuf::from("/journals")
    );
}

/// R2 (B7: every answer that states the offers states them by one rule; A8.4: the inputs answer carries the stages and what the change
/// invalidated): an inputs change answers Apply and the action that holds the Validate page's position, as the journal and the study hold
/// them. With no Apply journaled and no validation that passed, nothing is offered: `applyAvailable` is false and no action holds the
/// position. Once an Apply's settings stand (its checkpoint completed) and its receipt allows Keep in project, Keep in project is the
/// primary action, and Apply is not offered, because no validation of the current candidate passed. The wrong behaviour caught: an inputs
/// answer without the offers, so that the page combines it with the Apply offer of an older answer (N7), or offers that ignore the
/// journal's standing Apply.
#[test]
fn an_inputs_answer_states_the_apply_offer() {
    let (root, example, request) = example_project("inputs-offers");
    let studio = ControlStudio::new(Arc::new(Ignore));
    let mut change = request.clone();
    change["change"] = json!({ "family": "PI" });
    let free = studio
        .execute(INPUTS, std::slice::from_ref(&change))
        .expect("answered");
    assert_eq!(free["result"]["applyAvailable"], json!(false), "{free}");
    assert!(
        free["result"].get("primaryAction").is_none(),
        "nothing holds the position: {free}"
    );
    journal(&request, &example, checkpoint(CheckpointStage::Completed));
    keep_apply_receipt(&root, &example);
    change["change"] = json!({ "family": "PID" });
    let standing = studio.execute(INPUTS, &[change]).expect("answered");
    assert_eq!(
        standing["result"]["applyAvailable"],
        json!(false),
        "{standing}"
    );
    assert_eq!(standing["result"]["primaryAction"], "keep", "{standing}");
    fs::remove_dir_all(&root).expect("clean up");
}

/// B6.4, B6.7, B9.4: the answer that ends an Apply or a Restore states the stages as the study and the journal hold them then
/// (`state_now`): a checkpoint the operation leaves unresolved holds them, and one it resolved or completed does not. The wrong behaviour
/// caught: the page keeping the block after a confirmed Restore, or not showing it after an Apply whose own Restore was not confirmed,
/// until the loop is reopened.
#[test]
fn the_end_of_an_apply_or_a_restore_states_the_stages_it_leaves() {
    let (root, example, request) = example_project("state-now");
    let folder = storage_folder(&request);
    let (free, evidence) = state_now(&root, &folder, &example).expect("the state");
    assert_eq!(free.block, None, "no checkpoint, no block");
    // Without a checkpoint no stage carries a block's reason; a stage that cannot run yet states only why (B9.4).
    assert!(free
        .stages(&evidence)
        .iter()
        .all(|stage| matches!(stage.reason, None | Some(IDENTIFY_FIRST | TUNE_FIRST))));
    journal(&request, &example, checkpoint(CheckpointStage::Transfer));
    let (held, evidence) = state_now(&root, &folder, &example).expect("the state");
    assert_eq!(held.block, Some(UNRESOLVED_APPLY));
    let stages = held.stages(&evidence);
    let identify = stages
        .iter()
        .find(|stage| stage.stage == Stage::Identify)
        .expect("Identify");
    assert!(!identify.available, "Identify waits for the Restore");
    assert_eq!(identify.reason, Some(UNRESOLVED_APPLY));
    let mut cleared =
        Journal::open(&folder, &example.project_id, &example.id).expect("the journal");
    cleared
        .clear("commissioning-test")
        .expect("a confirmed Restore clears it");
    let (resolved, _) = state_now(&root, &folder, &example).expect("the state");
    assert_eq!(resolved.block, None, "the Restore ended the block");
    assert_eq!(
        resolved.primary_action(),
        None,
        "nothing is offered after a confirmed Restore"
    );
    fs::remove_dir_all(&root).expect("clean up");
}

/// A runtime restarted since the Apply of [`checkpoint`] (annex E9.3: the session epoch changed, so the program revision did): it runs
/// program:2, where the checkpoint names program:1, on a simulated target, and its parameter record holds each parameter the example
/// loop binds at 1 with no transaction, because the old program's transactions ended with it (A6.7). It answers the runtime's status and
/// the parameter record, and refuses everything else.
fn restarted_runtime(example: &LoopDefinition) -> FakeRuntime {
    let values: serde_json::Map<String, Value> = example
        .parameter_binding
        .iter()
        .flat_map(|binding| binding.parameters.iter())
        .map(|parameter| (parameter.symbol.clone(), json!(1.0)))
        .collect();
    FakeRuntime::start(move |request| {
        Some(match request["type"].as_str() {
            Some("status") => answer(
                request,
                json!({
                    "state": "running", "session_epoch": 2, "simulation_enabled": true, "simulation_only_io": true,
                    "simulation_time_scale": 1, "simulation_achieved_time_scale": null, "program_revision": "program:2",
                    "tasks": [{ "name": "MainTask", "interval_ms": 100 }], "cycle_interval_ms": 100
                }),
            ),
            Some("control.parameters.status") => answer(
                request,
                json!({
                    "supports_request_id": true, "transaction_id": null, "program_revision": "program:2",
                    "binding_revision": "binding:2", "active_revision": "active:2", "values": values, "status": "idle",
                    "previous_revision": null, "previous_values": null, "reason": null
                }),
            ),
            _ => refusal(
                request,
                "unknown_operation",
                "The restarted runtime answers its status and its parameter record only.",
            ),
        })
    })
}

/// Keeps in the example's study the receipt of the Apply that journaled [`checkpoint`] (A8.1): its settings applied, its result passed.
fn keep_apply_receipt(root: &Path, example: &LoopDefinition) {
    let mut study =
        storage::open_study(root, &example.id, &example.revision).expect("the working study");
    study.sections.insert(
        "apply".to_owned(),
        json!({
            "applications": [{
                "checkpoint": "commissioning-test",
                "validationIdentity": "sha256:applied",
                "settingsApplied": true,
                "result": {
                    "status": "passed",
                    "reason": "The response was assessed.",
                    "transfers": [],
                    "recoveryConfirmed": false,
                    "target": 38.0,
                    "modelRevision": "model:1",
                    "channels": [],
                    "criteria": {
                        "target": 38.0,
                        "durationSeconds": 60.0,
                        "settlingTimeSeconds": 30.0,
                        "maximumError": 0.2,
                        "taskPeriodMs": 100.0,
                        "processRange": { "minimum": 0.0, "maximum": 80.0 },
                        "outputLimits": { "minimum": 0.0, "maximum": 100.0 }
                    }
                }
            }]
        }),
    );
    storage::save_study(root, &study, &[]).expect("the study is saved");
}

/// The Apply section of the example's study, when it has one (A8.1).
fn apply_section(root: &Path, example: &LoopDefinition) -> Option<ApplySection> {
    storage::open_study(root, &example.id, &example.revision)
        .expect("the working study")
        .section::<ApplySection>()
        .expect("the Apply section reads")
}

/// Checks that the journal that `request` names keeps [`checkpoint`] as history: still journaled, retired, and naming the program the
/// replacement runs (B6.4).
fn assert_kept_as_history(request: &Value, example: &LoopDefinition) {
    let journal = Journal::open(&storage_folder(request), &example.project_id, &example.id)
        .expect("the journal");
    let kept = journaled_checkpoint(&journal).expect("the checkpoint is kept, not cleared");
    assert_eq!(
        (
            kept.id.as_str(),
            kept.stage,
            kept.replacement_program_revision.as_deref()
        ),
        (
            "commissioning-test",
            CheckpointStage::Retired,
            Some("program:2")
        )
    );
}

/// R0 (B6.4: "When the runtime was restarted or its program reloaded since Apply, nothing of the checkpoint can be restored: Restore
/// says so, the checkpoint is kept as history, and the loop continues from a new validation"; B6.7: "None; the checkpoint is kept as
/// history; validate again"; annex E9.3, E11.7): the runtime runs program:2 and the checkpoint was journaled on program:1. A Restore
/// answers at once, without a review and with one: not restored, with the sentence, nothing offered (B7), and the stages free of the
/// block. It only reads the runtime. The journal keeps the checkpoint, retired, with program:2 as the replacement's program; the study
/// keeps a retired receipt beside the Apply's receipt when it has one, and an interrupted Apply without a receipt leaves the journal its
/// only record. A further Restore says the checkpoint is history and keeps no second receipt. The wrong behaviour caught: a Restore
/// that refuses and leaves the unresolved checkpoint journaled, so that its block holds the loop for good; one that clears the
/// checkpoint without history; one that writes to the new program; or a receipt that names no Apply, which the study refuses to keep.
#[test]
fn a_restore_after_a_runtime_restart_retires_the_checkpoint_as_history() {
    let (root, example, mut request) = example_project("restart-retires");
    let fake = restarted_runtime(&example);
    request["connection"] = serde_json::to_value(fake.connection(None)).expect("a connection");
    let studio = ControlStudio::new(Arc::new(Ignore));

    journal(&request, &example, checkpoint(CheckpointStage::Transfer));
    let ended = studio
        .execute(RESTORE, std::slice::from_ref(&request))
        .expect("answered");
    let result = &ended["result"];
    assert_eq!(result["restored"], json!(false), "{ended}");
    assert_eq!(result["reason"], RETIRED_REASON, "{ended}");
    assert_eq!(result["restoreAvailable"], json!(false), "{ended}");
    assert_eq!(result["keepAvailable"], json!(false), "{ended}");
    assert_eq!(result["applyAvailable"], json!(false), "{ended}");
    assert!(result.get("primaryAction").is_none(), "{ended}");
    for name in ["Test", "Identify", "Tune", "Validate", "Apply"] {
        assert_ne!(
            stage(result, name).get("reason"),
            Some(&json!(UNRESOLVED_APPLY)),
            "{name}: {ended}"
        );
    }
    assert_eq!(
        stage(result, "Identify")["available"],
        json!(true),
        "{ended}"
    );
    assert_kept_as_history(&request, &example);
    assert!(
        apply_section(&root, &example).is_none(),
        "an Apply interrupted before its receipt leaves no receipt for the Restore to stand beside"
    );

    journal(&request, &example, checkpoint(CheckpointStage::Completed));
    keep_apply_receipt(&root, &example);
    let mut confirmation = request.clone();
    confirmation["review"] = json!("sha256:reviewed-before-the-restart");
    let ended = studio.execute(RESTORE, &[confirmation]).expect("answered");
    assert_eq!(ended["result"]["restored"], json!(false), "{ended}");
    assert_eq!(ended["result"]["reason"], RETIRED_REASON, "{ended}");
    assert_kept_as_history(&request, &example);
    let section = apply_section(&root, &example).expect("the Apply section");
    assert_eq!(
        section.restorations,
        vec![RestorationReceipt {
            checkpoint: "commissioning-test".to_owned(),
            restored: false,
            retired: true,
            reason: RETIRED_REASON.to_owned(),
            mode: None,
        }]
    );
    assert!(
        !section.restore_available() && !section.keep_available(),
        "the retired receipt closes the Apply's settings"
    );

    let refused = studio.execute(RESTORE, &[request]).expect("answered");
    assert_eq!(
        refused["failure"]["message"],
        "There are no applied settings to restore: checkpoint commissioning-test is kept as history, because the runtime was replaced after its Apply.",
        "{refused}"
    );
    assert_eq!(
        apply_section(&root, &example)
            .expect("the Apply section")
            .restorations
            .len(),
        1,
        "no second receipt"
    );
    let sent: Vec<String> = fake
        .requests()
        .iter()
        .map(|sent| sent["type"].as_str().unwrap_or_default().to_owned())
        .collect();
    assert!(
        sent.iter()
            .all(|kind| kind == "status" || kind == "control.parameters.status"),
        "a retirement only reads the runtime: {sent:?}"
    );
    fs::remove_dir_all(&root).expect("clean up");
}

/// R0 (B6.4: the checkpoint "is kept as history, and the loop continues from a new validation"; B6.7: "validate again"; A8.5: a retired
/// checkpoint grants nothing): a reopened loop whose journal keeps a retired checkpoint holds no stage back. Its stages carry no block,
/// Identify can start, Restore previous settings and Keep in project are not offered and nothing holds the position, and Validate goes
/// on to its own first check, the study's model (B5.5), which the empty study does not have. The wrong behaviour caught: a retired
/// checkpoint that still blocks the stages, offers Restore, or keeps Validate waiting for an operation that ended with the old runtime.
#[test]
fn after_a_retired_checkpoint_the_stages_are_no_longer_blocked() {
    let (root, example, request) = example_project("retired-free");
    journal(&request, &example, checkpoint(CheckpointStage::Retired));
    let studio = ControlStudio::new(Arc::new(Ignore));
    let opened = studio
        .execute(OPEN, std::slice::from_ref(&request))
        .expect("answered");
    let result = &opened["result"];
    assert_eq!(result["kind"], "opened", "{opened}");
    for name in ["Test", "Identify", "Tune", "Validate", "Apply"] {
        assert_ne!(
            stage(result, name).get("reason"),
            Some(&json!(UNRESOLVED_APPLY)),
            "{name}: {opened}"
        );
    }
    assert_eq!(
        stage(result, "Identify")["available"],
        json!(true),
        "{opened}"
    );
    assert_eq!(result["restoreAvailable"], json!(false), "{opened}");
    assert_eq!(result["keepAvailable"], json!(false), "{opened}");
    assert_eq!(
        result["applyObservationResumable"],
        json!(false),
        "{opened}"
    );
    assert!(result.get("primaryAction").is_none(), "{opened}");
    let refused = studio.execute(VALIDATE, &[request]).expect("answered");
    assert_eq!(
        refused["failure"]["nextAction"], "identify",
        "Validate's own first check, not the block: {refused}"
    );
    fs::remove_dir_all(&root).expect("clean up");
}

/// B6.4, B7 (A8.1, A8.5): the record that a Restore of [`checkpoint`] leaves (`apply_action::keep_restoration`). For an Apply interrupted
/// before its receipt was kept, the journal is the only record. A Restore the runtime did not confirm leaves the checkpoint journaled as
/// it was, still waiting. One it confirmed cleared the checkpoint, and the journal keeps it again as history, recovered, with the
/// validation the Apply applied, while the study keeps no receipt, because every Restore receipt names an Apply's. The recovered
/// checkpoint holds no stage back and offers nothing: no Restore, no Keep, no Resume, nothing in the position. For an Apply whose receipt
/// the study keeps, the Restore's receipt stands beside it, and the journal keeps nothing. The wrong behaviour caught: the confirmed
/// Restore of an Apply without a receipt leaving no record of the validation it applied, so that a reopened loop offers Apply for it
/// again while the Restore's end offered none (B7); or an unconfirmed Restore closing a checkpoint that still waits (B6.7).
#[test]
fn a_confirmed_restore_of_an_apply_without_a_receipt_keeps_its_checkpoint_as_history() {
    let (root, example, request) = example_project("recovered-history");
    let folder = storage_folder(&request);
    let receipt = |restored: bool| RestorationReceipt {
        checkpoint: "commissioning-test".to_owned(),
        restored,
        retired: false,
        reason: "The previous settings were confirmed back.".to_owned(),
        mode: Some(FoundMode::Manual),
    };
    let journaled_stage = || {
        let journal =
            Journal::open(&folder, &example.project_id, &example.id).expect("the journal");
        journaled_checkpoint(&journal).map(|kept| (kept.stage, kept.validation_identity.clone()))
    };

    journal(&request, &example, checkpoint(CheckpointStage::Transfer));
    keep_restoration(
        &root,
        &folder,
        &example,
        checkpoint(CheckpointStage::Transfer),
        receipt(false),
    )
    .expect("the record is kept");
    assert_eq!(
        journaled_stage(),
        Some((CheckpointStage::Transfer, "sha256:applied".to_owned())),
        "an unconfirmed Restore leaves the checkpoint waiting"
    );

    let mut cleared =
        Journal::open(&folder, &example.project_id, &example.id).expect("the journal");
    cleared
        .clear("commissioning-test")
        .expect("a confirmed Restore clears it");
    keep_restoration(
        &root,
        &folder,
        &example,
        checkpoint(CheckpointStage::Transfer),
        receipt(true),
    )
    .expect("the record is kept");
    assert_eq!(
        journaled_stage(),
        Some((CheckpointStage::Recovered, "sha256:applied".to_owned())),
        "the journal keeps the checkpoint as history, with the validation the Apply applied"
    );
    assert!(
        apply_section(&root, &example).is_none(),
        "no receipt names the interrupted Apply"
    );
    let (history, _) = state_now(&root, &folder, &example).expect("the state");
    assert_eq!(
        history.block, None,
        "a recovered checkpoint holds nothing back"
    );
    let offers = history.offers;
    assert_eq!(
        (offers.apply, offers.restore, offers.keep, offers.resume),
        (false, false, false, false)
    );
    assert_eq!(history.primary_action(), None);

    // An Apply whose receipt the study keeps: its confirmed Restore cleared the checkpoint, and the receipts are its record.
    let mut cleared =
        Journal::open(&folder, &example.project_id, &example.id).expect("the journal");
    cleared
        .clear("commissioning-test")
        .expect("as the confirmed Restore clears it");
    keep_apply_receipt(&root, &example);
    keep_restoration(
        &root,
        &folder,
        &example,
        checkpoint(CheckpointStage::Completed),
        receipt(true),
    )
    .expect("the record is kept");
    assert_eq!(journaled_stage(), None, "the receipts are the record");
    assert_eq!(
        apply_section(&root, &example)
            .expect("the Apply section")
            .restorations,
        vec![receipt(true)]
    );
    fs::remove_dir_all(&root).expect("clean up");
}

/// B7: Apply's review keeps the rule the offers state, by the newest Apply's checkpoint while the journal holds it and by the study's
/// newest Apply receipt once it does not. The wrong behaviour caught: the review accepted the validation the newest Apply had already
/// applied, so only the page's withheld offer kept the engine from applying the same validation again after a Restore.
#[test]
fn an_apply_of_the_validation_the_newest_apply_applied_is_refused() {
    let (root, example, request) = example_project("applied-validation");
    let study = |root: &Path| {
        storage::open_study(root, &example.id, &example.revision).expect("the working study")
    };
    let open_journal = || {
        Journal::open(&storage_folder(&request), &example.project_id, &example.id)
            .expect("the journal")
    };

    // The journal holds the newest Apply's checkpoint, restored since: it still names the validation it applied.
    journal(&request, &example, checkpoint(CheckpointStage::Recovered));
    let refused = refuse_applied_validation(&open_journal(), &study(&root), "sha256:applied")
        .expect_err("the applied validation is refused");
    assert_eq!(
        refused.message,
        "Apply already applied this validation, so Apply is not available for it again."
    );
    assert_eq!(refused.next_action, NextAction::Validate);
    assert!(
        refuse_applied_validation(&open_journal(), &study(&root), "sha256:validated-after").is_ok(),
        "a validation that passed after the Apply may be applied (B7)"
    );

    // With no checkpoint journaled, the study's newest Apply receipt names the applied validation.
    open_journal()
        .clear("commissioning-test")
        .expect("the checkpoint is cleared");
    assert!(
        refuse_applied_validation(&open_journal(), &study(&root), "sha256:applied").is_ok(),
        "nothing records an Apply yet"
    );
    keep_apply_receipt(&root, &example);
    let refused = refuse_applied_validation(&open_journal(), &study(&root), "sha256:applied")
        .expect_err("the receipt's validation is refused");
    assert_eq!(refused.next_action, NextAction::Validate);
    fs::remove_dir_all(&root).expect("clean up");
}
