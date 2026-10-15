//! Tests of the commissioning checkpoint and the mode record (annex E11.2; specification 35, A8.5). The checkpoint of the tests is built
//! from the shared fixtures and written as its JSON, so that the decode is tested on the form the engine stores. The checks mirror those
//! of `control-studio-commissioning-checkpoint.test.ts`: a coherent record decodes for its loop, and a record of another revision, a value
//! outside its bounds or an owner of another program is refused.
#![allow(clippy::float_cmp)] // The tests compare the values they set, exactly.

use std::collections::BTreeMap;

use serde_json::json;

use super::*;
use crate::commissioning::commissioning_result::TransferState;
use crate::commissioning::fixture_loops::{first_order_model, proportional_loop, requirements};
use crate::commissioning::parameter_status::RecordState;
use crate::commissioning::transfer::proportional_transfer_binding;
use crate::commissioning::transfer_tracking::proportional_tracking;
use crate::json::decode_value;
use crate::loop_definition::{LoopDefinition, RealType};

/// The origin record: the gain `Main.Parameters.Kp` at `gain`, under the program and binding `program:1` and `binding:1`.
fn origin_status(gain: f64) -> ParameterStatus {
    ParameterStatus {
        supports_request_id: Some(true),
        transaction_id: None,
        program_revision: "program:1".to_owned(),
        binding_revision: "binding:1".to_owned(),
        active_revision: "active:1".to_owned(),
        values: BTreeMap::from([("Main.Parameters.Kp".to_owned(), gain)]),
        status: RecordState::Applied,
        previous_revision: None,
        previous_values: None,
        reason: None,
    }
}

/// A completed checkpoint of the proportional loop: the candidate gain 2 was written over the origin's gain 1, and the transfer of the run
/// is owned under the origin's program.
fn checkpoint(loop_definition: &LoopDefinition) -> CommissioningCheckpoint {
    let binding = proportional_transfer_binding(loop_definition).expect("the proportional binding");
    let tracking =
        proportional_tracking(loop_definition, &binding, true).expect("the proportional tracking");
    CommissioningCheckpoint {
        id: "checkpoint:1".to_owned(),
        loop_revision: loop_definition.revision.clone(),
        stage: CheckpointStage::Completed,
        model: first_order_model(),
        task: requirements(),
        origin: origin_status(1.0),
        candidate: BTreeMap::from([("Main.Parameters.Kp".to_owned(), 2.0)]),
        parameter_operation: ParameterOwner {
            id: "parameter:1".to_owned(),
            program: "program:1".to_owned(),
            binding: "binding:1".to_owned(),
        },
        references: BTreeMap::from([("Main.SP".to_owned(), 0.0)]),
        binding,
        recovery_tracking: TransferTracking::Proportional(tracking),
        tolerance: 0.1,
        replacement_program_revision: None,
        transfer: Some(TransferOwner {
            id: "transfer:1".to_owned(),
            program: "program:1".to_owned(),
            binding: "transfer-binding:1".to_owned(),
        }),
        recording: None,
        recording_result: None,
        found_mode: Some(FoundMode::Auto),
        process_noise: Some(0.05),
        validation_identity: "validation:1".to_owned(),
        transfers: Vec::new(),
        schedule_origin: None,
    }
}

#[test]
fn a_completed_record_decodes_for_its_loop_and_views_the_stage_and_the_owners_the_rule_reads() {
    let loop_definition = proportional_loop();
    let stored = serde_json::to_value(checkpoint(&loop_definition)).expect("the record serializes");
    let decoded =
        decode_checkpoint(&stored, &loop_definition).expect("the record belongs to its loop");
    assert_eq!(decoded, checkpoint(&loop_definition));

    let view = decoded.view();
    assert_eq!(view.stage, JournalStage::Completed);
    assert_eq!(view.model_revision, first_order_model().identity().revision);
    assert_eq!(view.origin_program, "program:1");
    assert_eq!(view.origin_binding, "binding:1");
    assert_eq!(view.parameter_operation_id, "parameter:1");
    assert_eq!(
        view.candidate,
        BTreeMap::from([("Main.Parameters.Kp".to_owned(), 2.0)])
    );
    assert_eq!(view.transfer, decoded.transfer);
}

#[test]
fn a_record_of_another_revision_a_value_outside_its_bounds_or_an_owner_of_another_program_is_refused(
) {
    let loop_definition = proportional_loop();
    let stored = serde_json::to_value(checkpoint(&loop_definition)).expect("the record serializes");

    let mut other_revision = stored.clone();
    other_revision["loopRevision"] = json!("sha256:other");
    assert!(decode_checkpoint(&other_revision, &loop_definition).is_err());

    let mut outside = stored.clone();
    outside["candidate"]["Main.Parameters.Kp"] = json!(150.0);
    let failure = decode_checkpoint(&outside, &loop_definition)
        .expect_err("the candidate is above the parameter's maximum");
    assert_eq!(
        failure.message,
        "The retained commissioning checkpoint does not match this loop."
    );

    let mut other_program = stored.clone();
    other_program["parameterOperation"]["program"] = json!("program:other");
    assert!(decode_checkpoint(&other_program, &loop_definition).is_err());

    let mut unknown_field = stored;
    unknown_field["unexpected"] = json!(1);
    assert!(decode_checkpoint(&unknown_field, &loop_definition).is_err());
}

#[test]
fn a_real_parameter_is_exact_only_in_single_precision() {
    let mut loop_definition = proportional_loop();
    loop_definition
        .parameter_binding
        .as_mut()
        .expect("the loop has a parameter binding")
        .parameters[0]
        .iec_type = RealType::Real;
    let stored = serde_json::to_value(checkpoint(&loop_definition)).expect("the record serializes");

    // 0.1 is not exact in single precision, so a REAL record cannot hold it; 0.5 is.
    let mut inexact = stored.clone();
    inexact["candidate"]["Main.Parameters.Kp"] = json!(0.1);
    assert!(decode_checkpoint(&inexact, &loop_definition).is_err());

    let mut exact = stored;
    exact["candidate"]["Main.Parameters.Kp"] = json!(0.5);
    assert!(decode_checkpoint(&exact, &loop_definition).is_ok());
}

#[test]
fn the_mode_record_keeps_the_mode_the_loop_was_found_in_and_the_transfer_that_changed_it() {
    let loop_definition = proportional_loop();
    let record = IdentificationModeCheckpoint {
        loop_revision: loop_definition.revision.clone(),
        original: TransferStatus {
            program_revision: "program:1".to_owned(),
            binding_revision: "binding:1".to_owned(),
            values: BTreeMap::new(),
            runtime_time_ns: None,
            status: TransferState::Idle,
            reason: None,
            supported_tracking: None,
            transaction: None,
        },
        transfer: Some(ModeTransferOwner {
            binding: proportional_transfer_binding(&loop_definition).expect("the binding"),
            owner: TransferOwner {
                id: "transfer:2".to_owned(),
                program: "program:1".to_owned(),
                binding: "binding:2".to_owned(),
            },
        }),
    };
    let stored = serde_json::to_value(&record).expect("the record serializes");
    assert_eq!(stored["loopRevision"], "sha256:loop-1");
    assert_eq!(stored["transfer"]["owner"]["id"], "transfer:2");
    let back: IdentificationModeCheckpoint = decode_value(stored).expect("the record decodes");
    assert_eq!(back, record);
}
