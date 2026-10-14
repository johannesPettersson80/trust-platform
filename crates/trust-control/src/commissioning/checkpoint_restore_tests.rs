#![allow(clippy::float_cmp)] // The rules copy the values the runtime reports, and the tests compare them exactly, as the TypeScript tests do.

use serde_json::json;

use super::*;
use crate::commissioning::commissioning_result::{TransferOwner, TransferTransaction};
use crate::commissioning::fixture_loops::{
    first_order_model, library_pid_loop, proportional_loop, requirements,
};
use crate::commissioning::parameter_application::{ParameterOwner, ReviewKind};
use crate::commissioning::pid_apply_fixture::{pid_loop, record, status, values, NOISE};
use crate::commissioning::texts::restore_unavailable;
use crate::commissioning::transfer_tracking::DeclaredStateKind;
use crate::experiment::{ControlEvidenceSource, RecordingRecoveryBinding};

/// The candidate the TypeScript tests apply over the record at `Kp 1`: `Kp 2, Ki 0.5, Kd 0`.
fn candidate() -> BTreeMap<String, f64> {
    BTreeMap::from([
        ("Main.Kp".to_owned(), 2.0),
        ("Main.Ki".to_owned(), 0.5),
        ("Main.Kd".to_owned(), 0.0),
    ])
}

/// The checkpoint of an Apply of `loop_definition` interrupted after its settings were applied (A6.2, A8.6; the TypeScript tests'
/// `interruptedCheckpoint`): the origin is `origin`, the reference `reference` held 2, the tolerance is 0.1, and the loop was found in
/// `found`. The binding and the recovery tracking are the adapter's.
fn checkpoint_of(
    loop_definition: &LoopDefinition,
    origin: ParameterStatus,
    candidate: BTreeMap<String, f64>,
    reference: &str,
    found: Option<FoundMode>,
) -> CommissioningCheckpoint {
    let adapter = controller_adapter(loop_definition).expect("the fixture loop is adapted");
    let model = first_order_model();
    let binding = adapter.transfer_binding().expect("the fixture binds");
    let recovery_tracking = adapter
        .recovery_tracking(&model)
        .expect("the fixture tracks");
    CommissioningCheckpoint {
        id: "checkpoint-1".to_owned(),
        loop_revision: loop_definition.revision.clone(),
        stage: CheckpointStage::Parameters,
        model,
        task: requirements(),
        parameter_operation: ParameterOwner {
            id: "apply-1".to_owned(),
            program: origin.program_revision.clone(),
            binding: origin.binding_revision.clone(),
        },
        origin,
        candidate,
        references: BTreeMap::from([(reference.to_owned(), 2.0)]),
        binding,
        recovery_tracking,
        tolerance: 0.1,
        replacement_program_revision: None,
        transfer: None,
        recording: None,
        recording_result: None,
        found_mode: found,
        process_noise: None,
        validation_identity: "sha256:validation".to_owned(),
        transfers: Vec::new(),
    }
}

/// The checkpoint of the PID apply fixture: the origin at `Kp 1, Ki 0, Kd 0` under `original`, the reference `Main.SP`.
fn pid_checkpoint(
    loop_definition: &LoopDefinition,
    found: Option<FoundMode>,
) -> CommissioningCheckpoint {
    checkpoint_of(
        loop_definition,
        record(1.0, 0.0, 0.0, "original"),
        candidate(),
        "Main.SP",
        found,
    )
}

/// The record Apply left: the candidate in force under `applied-1`, with the origin retained as its previous values (E7.2).
fn applied_record() -> ParameterStatus {
    ParameterStatus {
        active_revision: "applied-1".to_owned(),
        values: candidate(),
        previous_revision: Some("original".to_owned()),
        previous_values: Some(record(1.0, 0.0, 0.0, "original").values),
        ..record(2.0, 0.5, 0.0, "applied-1")
    }
}

/// A transfer status of the PID fixture whose last transaction is `id`, in `state`, on `binding` (E6.3, E6.8).
fn transfer_with(id: &str, state: TransferState, binding: &str) -> TransferStatus {
    let mut current = status(values(true, 0.0));
    current.binding_revision = binding.to_owned();
    current.status = state;
    current.transaction = Some(TransferTransaction {
        id: id.to_owned(),
        program_revision: "program".to_owned(),
        binding_revision: binding.to_owned(),
        status: state,
        reason: None,
        observed_values: None,
        confirmation: None,
        initial_values: None,
        applied_values: None,
        restored: None,
    });
    current
}

/// A reading of every mode and member of `binding`: each member at 0 except those `overrides` names, and the mode at `mode`.
fn reading(
    binding: &TransferBinding,
    mode: bool,
    overrides: &[(&str, f64)],
) -> BTreeMap<String, TransferValue> {
    let mut readings: BTreeMap<String, TransferValue> = binding
        .states
        .iter()
        .chain(&binding.guards)
        .chain(std::iter::once(&binding.output))
        .map(|member| (member.symbol.clone(), TransferValue::Number(0.0)))
        .collect();
    for (symbol, value) in overrides {
        readings.insert((*symbol).to_owned(), TransferValue::Number(*value));
    }
    readings.insert(binding.mode.symbol.clone(), TransferValue::Boolean(mode));
    readings
}

/// B6.4, A8.5: the checkpoint is the one record a Restore starts from; a recovered one has nothing left, a retired one is history.
#[test]
fn restore_starts_only_from_a_journaled_checkpoint_whose_settings_are_not_restored_yet() {
    let loop_definition = pid_loop("LREAL", json!([]));
    let none = restorable(None, "loop:pid-apply").expect_err("no checkpoint");
    assert_eq!(
        (none.cause, none.next_action),
        (Cause::Settings, NextAction::None)
    );
    assert!(
        none.message.contains("no applied settings to restore"),
        "{}",
        none.message
    );
    assert!(none.message.contains("loop:pid-apply"), "{}", none.message);

    let mut recovered = pid_checkpoint(&loop_definition, Some(FoundMode::Auto));
    recovered.stage = CheckpointStage::Recovered;
    let refusal = restorable(Some(recovered), "loop:pid-apply").expect_err("recovered");
    assert!(
        refusal
            .message
            .contains("checkpoint-1 are restored already"),
        "{}",
        refusal.message
    );
    let mut retired = pid_checkpoint(&loop_definition, Some(FoundMode::Auto));
    retired.stage = CheckpointStage::Retired;
    let refusal = restorable(Some(retired), "loop:pid-apply").expect_err("retired");
    assert!(
        refusal.message.contains("kept as history"),
        "{}",
        refusal.message
    );

    let mut completed = pid_checkpoint(&loop_definition, Some(FoundMode::Auto));
    completed.stage = CheckpointStage::Completed;
    assert_eq!(
        restorable(Some(completed.clone()), "loop:pid-apply").expect("completed settings restore"),
        completed
    );
}

/// B6.4: when the runtime was restarted or its program reloaded since Apply, nothing of the checkpoint can be restored, and the loop
/// continues from a new validation. A pending record of another program is left to the ownership check (reviewInterruptedRollback).
#[test]
fn a_restarted_or_reloaded_runtime_makes_restore_unavailable_and_names_both_programs() {
    let loop_definition = pid_loop("LREAL", json!([]));
    let checkpoint = pid_checkpoint(&loop_definition, Some(FoundMode::Auto));
    assert_eq!(
        check_runtime_identity(&checkpoint, &applied_record()),
        Ok(())
    );

    let restarted = ParameterStatus {
        program_revision: "program-2".to_owned(),
        ..applied_record()
    };
    let refusal = check_runtime_identity(&checkpoint, &restarted).expect_err("a new program");
    assert_eq!(
        (refusal.cause, refusal.next_action),
        (Cause::Runtime, NextAction::Validate)
    );
    let text = refusal.detail.unwrap_or(refusal.message);
    assert!(
        text.contains("program revision program at Apply, program-2 now"),
        "{text}"
    );
    assert!(text.contains("The checkpoint is kept as history"), "{text}");

    let pending = ParameterStatus {
        status: RecordState::Pending,
        ..restarted
    };
    assert_eq!(check_runtime_identity(&checkpoint, &pending), Ok(()));
    assert!(
        check_reviewed_parameters(&checkpoint, &pending).is_err(),
        "the ownership check refuses the other program"
    );
}

/// interruptedCommissioningRecovery.ts `review`: the checkpoint must name the loop's revision, its transfer binding and its recovery
/// tracking as the adapter gives them now (B6.4).
#[test]
fn a_checkpoint_of_another_loop_revision_binding_or_tracking_is_refused() {
    let loop_definition = pid_loop("LREAL", json!([]));
    let checkpoint = pid_checkpoint(&loop_definition, Some(FoundMode::Auto));
    let adapter = controller_adapter(&loop_definition).expect("adapted");
    let binding = adapter.transfer_binding().expect("binds");
    let tracking = adapter
        .recovery_tracking(&checkpoint.model)
        .expect("tracks");
    assert_eq!(
        check_checkpoint_identity(&loop_definition.revision, &binding, &tracking, &checkpoint),
        Ok(())
    );

    let refusal = check_checkpoint_identity("sha256:other", &binding, &tracking, &checkpoint)
        .expect_err("another loop revision");
    assert_eq!(
        (refusal.cause, refusal.next_action),
        (Cause::Settings, NextAction::None)
    );
    assert!(
        refusal.message.starts_with(
            "The commissioning checkpoint does not match the current controller binding"
        ),
        "{}",
        refusal.message
    );
    assert!(
        refusal
            .message
            .contains("loop revision sha256:pid-apply-1, and the loop is at revision sha256:other"),
        "{}",
        refusal.message
    );

    let mut moved = binding.clone();
    moved.task_name = "OtherTask".to_owned();
    let refusal =
        check_checkpoint_identity(&loop_definition.revision, &moved, &tracking, &checkpoint)
            .expect_err("another binding");
    assert!(
        refusal
            .message
            .contains("transfer binding or recovery tracking differs"),
        "{}",
        refusal.message
    );
    let aligned = TransferTracking::ParallelPid(
        parallel_pid_tracking(&loop_definition, &binding, true).expect("tracks"),
    );
    assert!(
        check_checkpoint_identity(&loop_definition.revision, &binding, &aligned, &checkpoint)
            .is_err(),
        "another tracking"
    );
}

/// interruptedCommissioningRecovery.ts `assertParameterOwner` (A6.8, B6.4, E11.7): the record Apply found, or the candidate written over
/// it with the origin retained; a pending transaction only when it is the checkpoint's own, and only at the review.
#[test]
fn the_record_is_the_checkpoint_origin_or_its_candidate_and_another_operation_is_refused() {
    let loop_definition = pid_loop("LREAL", json!([]));
    let checkpoint = pid_checkpoint(&loop_definition, Some(FoundMode::Auto));
    let origin = record(1.0, 0.0, 0.0, "original");
    for held in [origin.clone(), applied_record()] {
        assert_eq!(check_owned_parameters(&checkpoint, &held), Ok(()));
        assert_eq!(check_reviewed_parameters(&checkpoint, &held), Ok(()));
    }

    let changed = ParameterStatus {
        active_revision: "someone-else".to_owned(),
        values: BTreeMap::from([
            ("Main.Kp".to_owned(), 3.0),
            ("Main.Ki".to_owned(), 0.5),
            ("Main.Kd".to_owned(), 0.0),
        ]),
        ..applied_record()
    };
    let refusal = check_owned_parameters(&checkpoint, &changed).expect_err("another operation");
    assert_eq!(
        refusal,
        Failure::new(
            &pid_commission_ownership_changed(),
            Cause::Runtime,
            NextAction::None
        )
    );
    assert!(check_reviewed_parameters(&checkpoint, &changed).is_err());

    let owned_pending = ParameterStatus {
        status: RecordState::Pending,
        ..applied_record()
    };
    assert_eq!(
        check_reviewed_parameters(&checkpoint, &owned_pending),
        Ok(()),
        "the review allows the checkpoint's own pending operation, which the Restore cancels first"
    );
    assert!(
        check_owned_parameters(&checkpoint, &owned_pending).is_err(),
        "after the cancels nothing may be pending"
    );
    let other_pending = ParameterStatus {
        transaction_id: Some("someone-else".to_owned()),
        ..owned_pending
    };
    assert!(check_reviewed_parameters(&checkpoint, &other_pending).is_err());

    let rebound = ParameterStatus {
        binding_revision: "parameters-2".to_owned(),
        ..origin
    };
    assert!(check_owned_parameters(&checkpoint, &rebound).is_err());
}

/// B6.4 (`restoreUnavailableReason`): Restore returns only the settings Apply wrote; a record already back at the one Apply found is no
/// reason to refuse, since that step is not repeated.
#[test]
fn restore_is_unavailable_once_another_change_replaced_the_applied_settings() {
    let loop_definition = pid_loop("LREAL", json!([]));
    let checkpoint = pid_checkpoint(&loop_definition, Some(FoundMode::Auto));
    let applied = applied_record();
    assert_eq!(
        check_restore_available(&checkpoint, &applied, Some(&applied)),
        Ok(())
    );
    assert_eq!(
        check_restore_available(
            &checkpoint,
            &record(1.0, 0.0, 0.0, "original"),
            Some(&applied)
        ),
        Ok(()),
        "the rollback the runtime confirmed is not repeated"
    );
    assert_eq!(check_restore_available(&checkpoint, &applied, None), Ok(()));

    let replaced = ParameterStatus {
        active_revision: "applied-2".to_owned(),
        values: BTreeMap::from([
            ("Main.Kp".to_owned(), 4.0),
            ("Main.Ki".to_owned(), 0.5),
            ("Main.Kd".to_owned(), 0.0),
        ]),
        ..applied.clone()
    };
    assert_eq!(
        check_restore_available(&checkpoint, &replaced, Some(&applied)),
        Err(Failure::new(
            &restore_unavailable(),
            Cause::Settings,
            NextAction::None
        ))
    );
}

/// interruptedCommissioningRecovery.ts `review` (B6.4, E6.3): the program Apply found, the binding of the owned transfer, and no
/// pending transaction of another owner.
#[test]
fn the_transfer_of_another_program_or_owner_is_refused_at_the_review() {
    let loop_definition = pid_loop("LREAL", json!([]));
    let mut checkpoint = pid_checkpoint(&loop_definition, Some(FoundMode::Auto));
    assert_eq!(
        check_transfer_owner(&checkpoint, &status(values(true, 0.0))),
        Ok(())
    );

    checkpoint.transfer = Some(TransferOwner {
        id: "target-1".to_owned(),
        program: "program".to_owned(),
        binding: "transfer".to_owned(),
    });
    assert_eq!(
        check_transfer_owner(
            &checkpoint,
            &transfer_with("target-1", TransferState::Applied, "transfer")
        ),
        Ok(())
    );
    assert_eq!(
        check_transfer_owner(
            &checkpoint,
            &transfer_with("target-1", TransferState::Pending, "transfer")
        ),
        Ok(()),
        "the owned transfer is cancelled by its identity first"
    );
    let refusal = check_transfer_owner(
        &checkpoint,
        &transfer_with("transfer-9", TransferState::Pending, "transfer"),
    )
    .expect_err("another owner's pending transfer");
    assert_eq!(
        (refusal.cause, refusal.next_action),
        (Cause::Runtime, NextAction::None)
    );
    assert_eq!(
        refusal.message,
        "Another program or transfer owns the current runtime; recovery cannot overwrite it."
    );
    assert!(
        refusal
            .detail
            .as_deref()
            .is_some_and(|text| text.contains("transaction transfer-9")),
        "{refusal:?}"
    );
    assert!(check_transfer_owner(
        &checkpoint,
        &transfer_with("target-1", TransferState::Applied, "transfer-2")
    )
    .is_err());
    let mut reloaded = status(values(true, 0.0));
    reloaded.program_revision = "program-2".to_owned();
    assert!(check_transfer_owner(&checkpoint, &reloaded).is_err());
}

/// interruptedCommissioningRecovery.ts `rollback` (E6.10, E11.7, B6.7): a transaction still pending is restored again once the runtime
/// has ended it; a transfer of another program is not written to.
#[test]
fn after_the_cancels_a_pending_or_reloaded_transfer_leaves_the_ownership_unresolved() {
    let loop_definition = pid_loop("LREAL", json!([]));
    let checkpoint = pid_checkpoint(&loop_definition, Some(FoundMode::Auto));
    assert_eq!(
        check_transfer_settled(&checkpoint, &status(values(true, 0.0))),
        Ok(())
    );
    let pending = check_transfer_settled(
        &checkpoint,
        &transfer_with("transfer-9", TransferState::Pending, "transfer"),
    )
    .expect_err("still pending");
    assert_eq!(pending.next_action, NextAction::Restore);
    assert_eq!(
        pending.message,
        "Controller transfer ownership remains unresolved: transfer transaction transfer-9 is still pending."
    );
    let mut reloaded = status(values(true, 0.0));
    reloaded.program_revision = "program-2".to_owned();
    let refusal = check_transfer_settled(&checkpoint, &reloaded).expect_err("another program");
    assert_eq!(refusal.next_action, NextAction::None);
    assert!(
        refusal
            .message
            .contains("program revision program-2, not program"),
        "{}",
        refusal.message
    );
}

/// interruptedCommissioningRecovery.ts `rollback` (A6.2, B6.4): a loop in Manual with every reference held needs no transfer.
#[test]
fn a_manual_transfer_is_written_only_while_the_mode_or_a_reference_differs() {
    let loop_definition = pid_loop("LREAL", json!([]));
    let checkpoint = pid_checkpoint(&loop_definition, Some(FoundMode::Auto));
    assert!(!restore_transfer_needed(&checkpoint, &values(true, 0.0)));
    assert!(
        restore_transfer_needed(&checkpoint, &values(false, 0.0)),
        "the loop is in Auto"
    );
    let mut moved = values(true, 0.0);
    moved.insert("Main.SP".to_owned(), TransferValue::Number(2.5));
    assert!(
        restore_transfer_needed(&checkpoint, &moved),
        "a reference moved"
    );
    let mut unread = values(true, 0.0);
    unread.remove("Main.SP");
    assert!(
        restore_transfer_needed(&checkpoint, &unread),
        "an unread reference differs"
    );
}

/// interruptedCommissioningRecovery.ts `recoveryStates` (B6.4, C0): the states as the runtime reports them, the references written over
/// them; a state that is not reported refuses the transfer.
#[test]
fn the_manual_transfer_holds_each_state_as_observed_with_the_references_the_checkpoint_kept() {
    let loop_definition = pid_loop("LREAL", json!([]));
    let checkpoint = pid_checkpoint(&loop_definition, Some(FoundMode::Auto));
    let mut observed = values(false, 0.36);
    observed.insert("Main.SP".to_owned(), TransferValue::Number(3.0));
    observed.insert("Main.Integral".to_owned(), TransferValue::Number(0.7));
    assert_eq!(
        restore_states(&checkpoint, &observed).expect("every state is reported"),
        BTreeMap::from([
            ("Main.SP".to_owned(), 2.0),
            ("Main.ManualCommand".to_owned(), 0.0),
            ("Main.Integral".to_owned(), 0.7),
            ("Main.PreviousError".to_owned(), 0.0),
        ])
    );
    observed.remove("Main.Integral");
    let refusal = restore_states(&checkpoint, &observed).expect_err("a state is missing");
    assert_eq!(
        refusal.message,
        "Recovery state Main.Integral is unavailable."
    );
}

/// interruptedCommissioningRecovery.ts `rollback` (B6.4, E7.5).
#[test]
fn the_record_is_rolled_back_only_while_it_differs_and_the_rollback_must_return_to_the_record_apply_found(
) {
    let loop_definition = pid_loop("LREAL", json!([]));
    let checkpoint = pid_checkpoint(&loop_definition, Some(FoundMode::Auto));
    assert!(rollback_needed(&checkpoint, &applied_record()));
    assert!(!rollback_needed(
        &checkpoint,
        &record(1.0, 0.0, 0.0, "original")
    ));

    let mut review = ParameterReview {
        transaction_id: "rollback-1".to_owned(),
        observed: applied_record(),
        values: record(1.0, 0.0, 0.0, "original").values,
        kind: ReviewKind::Rollback {
            previous_revision: "original".to_owned(),
        },
    };
    assert_eq!(check_rollback_values(&checkpoint, &review), Ok(()));
    review.values.insert("Main.Kp".to_owned(), 3.0);
    let refusal = check_rollback_values(&checkpoint, &review).expect_err("another rollback record");
    assert_eq!(
        refusal.message,
        "The original parameter record is no longer the runtime's rollback record: the runtime would roll back to Main.Kd 0, Main.Ki 0, Main.Kp 3, and Apply found Main.Kd 0, Main.Ki 0, Main.Kp 1."
    );
}

/// control-studio-found-mode.test.ts, "a PID loop found in Auto returns to Auto once its previous settings are back" (A6.2, A6.4): the
/// entry writes the states as observed with the kept reference, aligns the output, and allows 2·Kp·n for the restored Kp 1 and n 0.05.
#[test]
fn a_pid_loop_found_in_auto_returns_to_auto_with_its_states_references_and_the_noise_allowance() {
    let loop_definition = pid_loop("LREAL", json!([]));
    let mut checkpoint = pid_checkpoint(&loop_definition, Some(FoundMode::Auto));
    checkpoint.process_noise = Some(NOISE);
    let entry = restore_auto_entry(&loop_definition, &checkpoint, &values(true, 0.0))
        .expect("the entry builds")
        .expect("a loop found in Auto re-enters Auto");
    assert_eq!(
        entry.values,
        BTreeMap::from([
            ("Main.SP".to_owned(), 2.0),
            ("Main.ManualCommand".to_owned(), 0.0),
            ("Main.Integral".to_owned(), 0.0),
            ("Main.PreviousError".to_owned(), 0.0),
        ])
    );
    assert!(
        (entry.tolerance - 2.0 * 1.0 * NOISE).abs() < 1e-12,
        "A6.4 gives {}, the entry allows {}",
        2.0 * NOISE,
        entry.tolerance
    );
    assert!(
        matches!(entry.tracking, TransferTracking::ParallelPid(ref tracking) if tracking.align_output),
        "{:?}",
        entry.tracking
    );
    assert!(reenterable(&loop_definition, &checkpoint).expect("decides"));
}

/// control-studio-found-mode.test.ts, "a proportional loop found in Auto returns to Auto once its previous gain is back" (A6.2, C13.3,
/// A6.4): the reference r = PV + u/Kp at the restored Kp 0.5, the Manual command at the held output 0.36 (the measured actuator), and the
/// allowance 2·|Kp|·n = 0.05.
#[test]
fn a_proportional_loop_found_in_auto_returns_to_auto_aligned_to_the_current_output_at_the_restored_gain(
) {
    let loop_definition = proportional_loop();
    let origin = ParameterStatus {
        values: BTreeMap::from([("Main.Parameters.Kp".to_owned(), 0.5)]),
        ..record(1.0, 0.0, 0.0, "original")
    };
    let mut checkpoint = checkpoint_of(
        &loop_definition,
        origin,
        BTreeMap::from([("Main.Parameters.Kp".to_owned(), 2.0)]),
        "LevelSP",
        Some(FoundMode::Auto),
    );
    checkpoint.process_noise = Some(NOISE);
    let current = reading(
        &checkpoint.binding,
        false,
        &[
            ("LevelSP", 2.0),
            ("LevelPV", 2.004),
            ("ValveMV", 0.36),
            ("ValvePosition", 0.36),
            ("Main.Parameters.Kp", 0.5),
        ],
    );
    let entry = restore_auto_entry(&loop_definition, &checkpoint, &current)
        .expect("the entry builds")
        .expect("a loop found in Auto re-enters Auto");
    assert_eq!(
        entry.values,
        BTreeMap::from([
            ("LevelSP".to_owned(), 2.004 + 0.36 / 0.5),
            ("Main.ManualCommand".to_owned(), 0.36),
        ])
    );
    assert!(
        (entry.tolerance - 2.0 * 0.5 * NOISE).abs() < 1e-12,
        "{}",
        entry.tolerance
    );
    assert!(
        matches!(entry.tracking, TransferTracking::Proportional(ref tracking) if tracking.align_reference),
        "{:?}",
        entry.tracking
    );
}

/// control-studio-found-mode.test.ts, "a Control.PID loop found in Auto returns to Auto through its declared initialization, allowing
/// Apply's measured noise" (checklist V4.6; A6.2, A6.4, C1): one entry, through the declared-state tracking, aligned to the current
/// output, allowing 2·Kp·n for the restored Kp 1, Ki 0, Kd 0 and n 0.05.
#[test]
fn a_control_pid_loop_found_in_auto_returns_to_auto_through_its_declared_initialization_with_the_noise_allowance(
) {
    let loop_definition = library_pid_loop();
    let origin = ParameterStatus {
        values: BTreeMap::from([
            ("Main.Parameters.Kp".to_owned(), 1.0),
            ("Main.Parameters.Ki".to_owned(), 0.0),
            ("Main.Parameters.Kd".to_owned(), 0.0),
        ]),
        ..record(1.0, 0.0, 0.0, "original")
    };
    let mut checkpoint = checkpoint_of(
        &loop_definition,
        origin,
        BTreeMap::from([
            ("Main.Parameters.Kp".to_owned(), 2.0),
            ("Main.Parameters.Ki".to_owned(), 0.5),
            ("Main.Parameters.Kd".to_owned(), 0.0),
        ]),
        "LevelSP",
        Some(FoundMode::Auto),
    );
    checkpoint.process_noise = Some(NOISE);
    let reenters = reenterable(&loop_definition, &checkpoint).expect("decides");
    assert!(
        reenters,
        "the library PID declares its commissioning profile"
    );
    assert!(
        texts::restore_description(checkpoint.stage, checkpoint.found_mode, reenters)
            .ends_with("Then the loop returns to Auto, the mode it was found in."),
        "the review says the loop returns to Auto"
    );
    let current = reading(
        &checkpoint.binding,
        false,
        &[
            ("LevelSP", 2.0),
            ("Main.TankController.StateValue.ProcessValue", 2.004),
            ("ValveMV", 0.36),
            ("ValvePosition", 0.36),
            ("Main.Parameters.Kp", 1.0),
        ],
    );
    let entry = restore_auto_entry(&loop_definition, &checkpoint, &current)
        .expect("the entry builds")
        .expect("a loop found in Auto re-enters Auto");
    match &entry.tracking {
        TransferTracking::DeclaredState(tracking) => {
            assert_eq!(tracking.kind, DeclaredStateKind::DeclaredStateV1);
            assert!(
                tracking.align_output,
                "the entry starts from the current output"
            );
        }
        other => panic!("the declared-state tracking, not {other:?}"),
    }
    assert!(
        (entry.tolerance - 2.0 * 1.0 * NOISE).abs() < 1e-12,
        "A6.4 gives {}, the entry allows {}",
        2.0 * NOISE,
        entry.tolerance
    );
    assert_eq!(
        entry.values.len(),
        checkpoint.binding.states.len(),
        "every declared state as observed"
    );
}

/// control-studio-found-mode.test.ts, "a loop found in Manual stays in Manual" and "a checkpoint written before the mode was journalled
/// stays in Manual" (A6.2).
#[test]
fn a_loop_found_in_manual_or_of_unknown_mode_or_not_read_in_manual_builds_no_auto_entry() {
    let loop_definition = pid_loop("LREAL", json!([]));
    for found in [Some(FoundMode::Manual), None] {
        let checkpoint = pid_checkpoint(&loop_definition, found);
        assert_eq!(
            restore_auto_entry(&loop_definition, &checkpoint, &values(true, 0.0)),
            Ok(None)
        );
    }
    let checkpoint = pid_checkpoint(&loop_definition, Some(FoundMode::Auto));
    assert_eq!(
        restore_auto_entry(&loop_definition, &checkpoint, &values(false, 0.0)),
        Ok(None),
        "a loop the runtime reads in Auto is left where it is"
    );
}

/// interruptedCommissioningRecovery.ts `rollbackDescription` and `modeClause` (B6.4, A6.2).
#[test]
fn the_review_describes_the_restore_and_the_mode_it_returns_the_loop_to() {
    assert_eq!(
        texts::restore_description(CheckpointStage::Completed, Some(FoundMode::Auto), true),
        "Return to Manual at the current output and restore the previous settings. Then the loop returns to Auto, the mode it was found in."
    );
    assert_eq!(
        texts::restore_description(CheckpointStage::Parameters, Some(FoundMode::Manual), true),
        "Recover the interrupted operation in Manual and restore the previous settings."
    );
    assert_eq!(
        texts::restore_description(CheckpointStage::Transfer, None, true),
        "Recover the interrupted operation in Manual and restore the previous settings. The mode the loop was found in is unknown, so it stays in Manual."
    );
    assert_eq!(
        texts::restore_description(CheckpointStage::Completed, Some(FoundMode::Auto), false),
        "Return to Manual at the current output and restore the previous settings. Then the loop stays in Manual: its Auto entry needs the controller initialization, which this recovery does not hold."
    );
}

/// interruptedCommissioningRecovery.ts `modeStatement` with the restored reason of engineerActions/application.ts (B6.4, A6.2).
#[test]
fn the_reason_names_the_mode_the_restore_left_the_loop_in() {
    assert_eq!(
        restored_reason(Some(FoundMode::Auto), true, true),
        "Previous values restored and observed after ST execution; the loop returned to Auto."
    );
    assert_eq!(
        restored_reason(Some(FoundMode::Manual), true, false),
        "Previous values restored and observed after ST execution; the loop stays in Manual."
    );
    assert!(restored_reason(None, true, false).contains("the mode it was found in is unknown"));
    assert!(restored_reason(Some(FoundMode::Auto), false, false)
        .contains("its Auto entry needs the controller initialization"));
}

/// control-studio-apply-reattach.test.ts, "a retained recording the runtime no longer holds is stored as incomplete evidence, so the
/// recovery goes on" (applyObservation.ts `stopRecovered`, A8.6, E4.5): no samples, an incomplete acquisition with the runtime's reason.
#[test]
fn a_recording_the_runtime_no_longer_holds_is_kept_as_incomplete_evidence() {
    let owner = RecordingRecoveryOwner {
        schema_version: 1,
        recording_id: "recording-1".to_owned(),
        project_id: "project:pid-apply".to_owned(),
        runtime_id: "runtime:pid-apply".to_owned(),
        loop_id: "loop:pid-apply".to_owned(),
        loop_revision: "sha256:pid-apply-1".to_owned(),
        source: ControlEvidenceSource::Simulation,
        program_revision: Some("program".to_owned()),
        start_acknowledgement_unconfirmed: None,
        bindings: vec![RecordingRecoveryBinding {
            id: "processVariable".to_owned(),
            address: Some("%IL0".to_owned()),
            symbol: None,
            iec_type: "LREAL".to_owned(),
        }],
    };
    let identity = retained_recording_request(&owner);
    assert_eq!(
        (
            identity.recording_id.as_str(),
            identity.loop_revision.as_str(),
            identity.program_revision.as_deref(),
            identity.task.as_deref(),
            identity.record_every
        ),
        (
            "recording-1",
            "sha256:pid-apply-1",
            Some("program"),
            None,
            1
        )
    );
    assert_eq!(identity.bindings.len(), 1);
    assert_eq!(identity.bindings[0].address.as_deref(), Some("%IL0"));

    let run = incomplete_recording(&owner, "No recording recording-1 is held.");
    assert_eq!(run.samples.len(), 0);
    assert_eq!(run.stop_reason, StopReason::RuntimeBoundary);
    assert_eq!(run.acquisition.completeness, Completeness::Incomplete);
    assert_eq!(run.acquisition.observed_sample_count, 0);
    assert_eq!(run.acquisition.method, "runtime_scan_buffer");
    assert_eq!(
        run.acquisition.reason,
        "The runtime no longer holds this recording. No recording recording-1 is held."
    );
    assert_eq!(run.bindings, identity.bindings);
}

/// §0.1b and B6.4: each bound parameter, its unit, the value in force and the record Apply found.
#[test]
fn the_review_lists_each_parameter_with_its_running_value_and_the_value_restore_returns() {
    let loop_definition = pid_loop("LREAL", json!([]));
    let checkpoint = pid_checkpoint(&loop_definition, Some(FoundMode::Auto));
    let reviewed = reviewed_parameters(&loop_definition, &checkpoint, &applied_record());
    let parameter = |symbol: &str, unit: &str, running: f64, next: f64| ReviewedParameter {
        symbol: symbol.to_owned(),
        unit: unit.to_owned(),
        running,
        next,
    };
    assert_eq!(
        reviewed,
        vec![
            parameter("Main.Kp", "V/m", 2.0, 1.0),
            parameter("Main.Ki", "V/(m*s)", 0.5, 0.0),
            parameter("Main.Kd", "V*s/m", 0.0, 0.0),
        ]
    );
}

/// A2.5, E10.2: one approval covers the reviewed Restore; a simulated target needs none.
#[test]
fn a_physical_target_needs_the_engineers_approval_before_restore_writes() {
    assert_eq!(
        check_restore_approval(true, false),
        Err(physical_approval_needed())
    );
    assert_eq!(check_restore_approval(true, true), Ok(()));
    assert_eq!(check_restore_approval(false, false), Ok(()));
}

/// A6.2: the PID fixture's Manual value is TRUE.
#[test]
fn the_mode_is_read_from_the_runtime_by_the_checkpoint_mode_binding() {
    let loop_definition = pid_loop("LREAL", json!([]));
    let checkpoint = pid_checkpoint(&loop_definition, Some(FoundMode::Auto));
    assert_eq!(
        reported_mode(&checkpoint, &values(true, 0.0)),
        Some(FoundMode::Manual)
    );
    assert_eq!(
        reported_mode(&checkpoint, &values(false, 0.0)),
        Some(FoundMode::Auto)
    );
    assert_eq!(reported_mode(&checkpoint, &BTreeMap::new()), None);
}

#[test]
fn after_the_auto_entry_the_references_apply_found_return_as_a_reference_step() {
    // B6.4 and C13 step 8: the bumpless Auto entry aligned the reference to the measurement (2.004 m); the Restore then returns
    // the reference Apply found as a reference step, without the entry's alignment.
    let loop_definition = library_pid_loop();
    let origin = ParameterStatus {
        values: BTreeMap::from([
            ("Main.Parameters.Kp".to_owned(), 1.0),
            ("Main.Parameters.Ki".to_owned(), 0.0),
            ("Main.Parameters.Kd".to_owned(), 0.0),
        ]),
        ..record(1.0, 0.0, 0.0, "original")
    };
    let mut checkpoint = checkpoint_of(
        &loop_definition,
        origin,
        BTreeMap::from([
            ("Main.Parameters.Kp".to_owned(), 2.0),
            ("Main.Parameters.Ki".to_owned(), 0.5),
            ("Main.Parameters.Kd".to_owned(), 0.0),
        ]),
        "LevelSP",
        Some(FoundMode::Auto),
    );
    checkpoint.process_noise = Some(NOISE);
    let found = checkpoint.references.clone();
    let (reference, value) = found
        .iter()
        .next()
        .expect("the checkpoint keeps the reference Apply found");
    let aligned = reading(
        &checkpoint.binding,
        false,
        &[
            (reference.as_str(), value + 0.004),
            ("Main.TankController.StateValue.ProcessValue", value + 0.004),
            ("ValveMV", 0.36),
            ("ValvePosition", 0.36),
            ("Main.Parameters.Kp", 1.0),
        ],
    );
    let step = restore_reference_step(&loop_definition, &checkpoint, &aligned)
        .expect("the step builds")
        .expect("a reference away from the one Apply found is returned");
    assert_eq!(
        step.values.get(reference),
        Some(value),
        "the step writes the reference Apply found"
    );
    match &step.tracking {
        TransferTracking::DeclaredState(tracking) => assert!(
            !tracking.align_output,
            "a reference step keeps the states and writes the reference as given"
        ),
        other => panic!("the declared-state tracking, not {other:?}"),
    }
    let holding = reading(
        &checkpoint.binding,
        false,
        &[
            (reference.as_str(), *value),
            ("Main.TankController.StateValue.ProcessValue", value + 0.004),
            ("ValveMV", 0.36),
            ("ValvePosition", 0.36),
            ("Main.Parameters.Kp", 1.0),
        ],
    );
    assert_eq!(
        restore_reference_step(&loop_definition, &checkpoint, &holding).expect("decides"),
        None,
        "a loop that holds the reference Apply found needs no step"
    );
}
