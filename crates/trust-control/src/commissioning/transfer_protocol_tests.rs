//! Tests of the transfer protocol (annex E6). The malformed-evidence cases are those of `control-studio-parameter-application.test.ts`
//! ("rejects malformed optional tracking evidence before using it"); the approval case is that of `control-studio-runtime-target.test.ts`
//! ("a transfer sends the caller's source and approval"); the rejection messages are those of `control-studio-apply-process-time.test.ts`
//! (`transferRejection`). Each check also has a case on each side of its boundary.
#![allow(clippy::float_cmp)] // The tests compare the values they set, exactly.

use std::collections::BTreeMap;

use super::*;
use crate::commissioning::declared_initialization::DeclaredStateTracking;
use crate::commissioning::transfer::{TransferMember, TransferModeBinding, TransferValueType};
use crate::commissioning::transfer_tracking::{
    DeclaredStateKind, ParallelPidKind, ParallelPidTracking, ProportionalKind, ProportionalTracking,
};

/// A change that corrupts one part of a transfer status.
type Corruption = Box<dyn Fn(&mut TransferStatus)>;

fn member(symbol: &str, minimum: f64, maximum: f64) -> TransferMember {
    TransferMember {
        symbol: symbol.to_owned(),
        iec_type: TransferValueType::Lreal,
        unit: "%".to_owned(),
        minimum,
        maximum,
    }
}

/// The binding of the tests: the reference and the Manual command are states, the gain and the process value are guards, and the
/// output is held. The mode is `P1.Manual`, which reads true in Manual.
fn binding() -> TransferBinding {
    TransferBinding {
        loop_id: "loop:tank".to_owned(),
        task_name: "MainTask".to_owned(),
        mode: TransferModeBinding {
            symbol: "P1.Manual".to_owned(),
            manual_value: true,
        },
        additional_modes: Vec::new(),
        tracking_outputs: BTreeMap::new(),
        states: vec![
            member("P1.Ref", -100.0, 100.0),
            member("P1.ManualCommand", 0.0, 100.0),
        ],
        guards: vec![
            member("P1.Gain", -10.0, 10.0),
            member("P1.PV", -100.0, 100.0),
        ],
        output: member("P1.MV", 0.0, 100.0),
        additional_outputs: Vec::new(),
    }
}

/// The binding of a parallel PID: the integral and derivative states are added after the Manual command.
fn pid_binding() -> TransferBinding {
    let mut binding = binding();
    binding.states.push(member("P1.Integral", -100.0, 100.0));
    binding.states.push(member("P1.Derivative", -100.0, 100.0));
    binding
}

fn map<V: Clone>(pairs: &[(&str, V)]) -> BTreeMap<String, V> {
    pairs
        .iter()
        .map(|(symbol, value)| ((*symbol).to_owned(), value.clone()))
        .collect()
}

fn number(value: f64) -> TransferValue {
    TransferValue::Number(value)
}

fn flag(value: bool) -> TransferValue {
    TransferValue::Boolean(value)
}

/// Every value of the binding: the mode, and the members at a gain of 2, a process value of 10 and a held output of 40.
fn coherent_values(manual: bool) -> BTreeMap<String, TransferValue> {
    map(&[
        ("P1.Manual", flag(manual)),
        ("P1.Ref", number(0.0)),
        ("P1.ManualCommand", number(0.0)),
        ("P1.Gain", number(2.0)),
        ("P1.PV", number(10.0)),
        ("P1.MV", number(40.0)),
    ])
}

/// The values of the parallel PID binding: the proportional values plus the integral and derivative states.
fn pid_values(manual: bool) -> BTreeMap<String, TransferValue> {
    let mut values = coherent_values(manual);
    values.insert("P1.Integral".to_owned(), number(0.0));
    values.insert("P1.Derivative".to_owned(), number(0.0));
    values
}

fn status(
    values: BTreeMap<String, TransferValue>,
    transaction: Option<TransferTransaction>,
    state: TransferState,
) -> TransferStatus {
    TransferStatus {
        program_revision: "program".to_owned(),
        binding_revision: "binding".to_owned(),
        values,
        runtime_time_ns: None,
        status: state,
        reason: None,
        supported_tracking: None,
        transaction,
    }
}

/// The last transaction of the tests, on the program and binding of `status`, under the output image.
fn transaction(
    state: TransferState,
    applied: Option<BTreeMap<String, f64>>,
    observed: Option<BTreeMap<String, TransferValue>>,
) -> TransferTransaction {
    TransferTransaction {
        id: "transfer".to_owned(),
        program_revision: "program".to_owned(),
        binding_revision: "binding".to_owned(),
        status: state,
        reason: None,
        observed_values: observed,
        confirmation: Some(TransferConfirmation::OutputImage),
        initial_values: Some(coherent_values(true)),
        applied_values: applied,
        restored: None,
    }
}

fn proportional(align_reference: bool) -> TransferTracking {
    TransferTracking::Proportional(ProportionalTracking {
        kind: ProportionalKind::Proportional,
        reference_symbol: "P1.Ref".to_owned(),
        manual_command_symbol: "P1.ManualCommand".to_owned(),
        process_variable: "P1.PV".to_owned(),
        gain_symbol: "P1.Gain".to_owned(),
        align_reference,
    })
}

fn owner() -> TransferOwner {
    TransferOwner {
        id: "transfer".to_owned(),
        program: "program".to_owned(),
        binding: "binding".to_owned(),
    }
}

#[test]
fn the_apply_request_carries_the_fields_of_annex_e6_5_and_omits_an_absent_tracking() {
    let reviewed = status(coherent_values(false), None, TransferState::Idle);
    let values = map(&[("P1.Ref", 0.0)]);
    let input = TransferApplyInput {
        transaction_id: "transfer-1",
        values: &values,
        manual: true,
        maximum_output_change: 0.5,
        entry_error_gain: None,
        entry_measurement_gain: None,
        tracking: None,
        source: ApplySource::Simulation,
        physical_approved: false,
    };
    let request = transfer_apply_request(&binding(), &reviewed, &input)
        .expect("a simulated apply is allowed");
    let stored = serde_json::to_value(&request).expect("the request serializes");
    assert_eq!(stored["transaction_id"], "transfer-1");
    assert_eq!(stored["expected_program_revision"], "program");
    assert_eq!(stored["expected_binding_revision"], "binding");
    assert_eq!(stored["expected_values"]["P1.Manual"], false);
    assert_eq!(stored["expected_values"]["P1.MV"], 40.0);
    assert_eq!(stored["values"]["P1.Ref"], 0.0);
    assert_eq!(stored["manual"], true);
    assert_eq!(stored["maximum_output_change"], 0.5);
    assert_eq!(stored["source"], "simulation");
    assert_eq!(stored["physical_approved"], false);
    assert_eq!(stored["binding"]["loop_id"], "loop:tank");
    assert!(
        stored.get("tracking").is_none(),
        "an apply without tracking omits the field (E6.5)"
    );
}

#[test]
fn a_physical_apply_without_approval_is_refused_before_it_is_sent() {
    let reviewed = status(coherent_values(false), None, TransferState::Idle);
    let values = map(&[("P1.Ref", 0.0)]);
    let input = TransferApplyInput {
        transaction_id: "transfer-1",
        values: &values,
        manual: true,
        maximum_output_change: 0.5,
        entry_error_gain: None,
        entry_measurement_gain: None,
        tracking: None,
        source: ApplySource::Physical,
        physical_approved: false,
    };
    let failure = transfer_apply_request(&binding(), &reviewed, &input)
        .expect_err("a physical transfer needs approval");
    assert_eq!(
        failure.message,
        "Physical approval is required for controller transfer on a physical target."
    );
    assert_eq!(
        (failure.cause, failure.next_action),
        (Cause::Approval, NextAction::Approve)
    );
}

#[test]
fn a_physical_apply_with_approval_carries_its_source_and_the_approval() {
    let reviewed = status(coherent_values(false), None, TransferState::Idle);
    let values = map(&[("P1.Ref", 0.0)]);
    let input = TransferApplyInput {
        transaction_id: "transfer-1",
        values: &values,
        manual: true,
        maximum_output_change: 0.5,
        entry_error_gain: None,
        entry_measurement_gain: None,
        tracking: None,
        source: ApplySource::Physical,
        physical_approved: true,
    };
    let request = transfer_apply_request(&binding(), &reviewed, &input).expect("approved");
    let stored = serde_json::to_value(&request).expect("serializes");
    assert_eq!(stored["source"], "physical");
    assert_eq!(stored["physical_approved"], true);
}

#[test]
fn a_coherent_status_is_accepted_and_each_malformed_part_is_refused() {
    let binding = binding();
    let coherent = status(
        coherent_values(true),
        Some(transaction(
            TransferState::Applied,
            Some(map(&[("P1.Ref", 0.0), ("P1.ManualCommand", 0.0)])),
            Some(coherent_values(true)),
        )),
        TransferState::Applied,
    );
    assert_eq!(check_transfer_status(&binding, &coherent), Ok(()));

    // The mode is a BOOL, and a number in its place is not evidence (`control-studio-parameter-application.test.ts`).
    let mut mode_as_number = coherent.clone();
    mode_as_number
        .values
        .insert("P1.Manual".to_owned(), number(1.0));
    let mut missing_member = coherent.clone();
    missing_member.values.remove("P1.PV");
    let mut too_many_tracking = coherent.clone();
    too_many_tracking.supported_tracking = Some(vec!["proportional".to_owned(); 17]);
    for malformed in [&mode_as_number, &missing_member, &too_many_tracking] {
        let failure = check_transfer_status(&binding, malformed).expect_err("malformed status");
        assert_eq!(
            failure.message,
            "The runtime returned invalid controller-transfer status."
        );
    }

    // The evidence of the last transaction: its initial and applied values must hold the binding's members.
    let transaction_cases: Vec<Corruption> = vec![
        Box::new(|value: &mut TransferStatus| {
            let evidence = value.transaction.as_mut().expect("a transaction");
            evidence
                .initial_values
                .as_mut()
                .expect("initial values")
                .insert("P1.Manual".to_owned(), number(1.0));
        }),
        Box::new(|value: &mut TransferStatus| {
            let evidence = value.transaction.as_mut().expect("a transaction");
            evidence
                .initial_values
                .as_mut()
                .expect("initial values")
                .remove("P1.MV");
        }),
        Box::new(|value: &mut TransferStatus| {
            let evidence = value.transaction.as_mut().expect("a transaction");
            evidence.applied_values = Some(map(&[("P1.Ref", 0.0)]));
        }),
        Box::new(|value: &mut TransferStatus| {
            let evidence = value.transaction.as_mut().expect("a transaction");
            evidence.applied_values = Some(map(&[
                ("P1.Ref", 0.0),
                ("P1.ManualCommand", 0.0),
                ("P1.Gain", 2.0),
            ]));
        }),
        Box::new(|value: &mut TransferStatus| {
            let evidence = value.transaction.as_mut().expect("a transaction");
            evidence.status = TransferState::Pending;
        }),
    ];
    for mutate in transaction_cases {
        let mut malformed = coherent.clone();
        mutate(&mut malformed);
        let failure = check_transfer_status(&binding, &malformed).expect_err("malformed evidence");
        assert_eq!(
            failure.message,
            "The runtime returned invalid transfer outcome evidence."
        );
    }
}

#[test]
fn a_proportional_outcome_must_hold_the_boundary_inputs_it_was_aligned_to() {
    let binding = binding();
    let tracking = proportional(true);
    // The held output is 40 at a gain of 2 and a process value of 10: the reference aligns to 10 + 40 / 2 = 30, and the Manual
    // command to the held output, 40.
    let applied = map(&[("P1.ManualCommand", 40.0), ("P1.Ref", 30.0)]);
    let mut observed = coherent_values(true);
    observed.insert("P1.Ref".to_owned(), number(30.0));
    observed.insert("P1.ManualCommand".to_owned(), number(40.0));
    let values = BTreeMap::new();

    let outcome = status(
        observed.clone(),
        Some(transaction(
            TransferState::Applied,
            Some(applied.clone()),
            Some(observed.clone()),
        )),
        TransferState::Applied,
    );
    assert_eq!(
        verify_transfer_outcome(&binding, &values, true, Some(&tracking), &outcome),
        Ok(())
    );

    let mut wrong_reference = applied.clone();
    wrong_reference.insert("P1.Ref".to_owned(), 31.0);
    let outcome = status(
        observed.clone(),
        Some(transaction(
            TransferState::Applied,
            Some(wrong_reference),
            Some(observed.clone()),
        )),
        TransferState::Applied,
    );
    let failure = verify_transfer_outcome(&binding, &values, true, Some(&tracking), &outcome)
        .expect_err("the reference differs from its alignment");
    assert_eq!(
        failure.message,
        "Applied tracking values disagree with the retained boundary inputs."
    );

    let mut auto = observed.clone();
    auto.insert("P1.Manual".to_owned(), flag(false));
    let outcome = status(
        auto.clone(),
        Some(transaction(
            TransferState::Applied,
            Some(applied),
            Some(auto),
        )),
        TransferState::Applied,
    );
    let failure = verify_transfer_outcome(&binding, &values, true, Some(&tracking), &outcome)
        .expect_err("the loop is in Auto, not in Manual");
    assert_eq!(
        failure.message,
        "The observed transfer does not match the reviewed mode and state."
    );
}

#[test]
fn an_outcome_counts_only_when_it_is_applied_under_the_output_image() {
    let mut post_st = transaction(TransferState::Applied, None, None);
    post_st.confirmation = Some(TransferConfirmation::PostSt);
    let outcome = status(coherent_values(true), Some(post_st), TransferState::Applied);
    let failure = verify_transfer_outcome(&binding(), &BTreeMap::new(), true, None, &outcome)
        .expect_err("a post-ST check is not an applied transfer");
    assert_eq!(failure.message, "Transfer applied.");
}

#[test]
fn a_rejected_transfer_keeps_the_runtime_reason_and_a_reverted_one_asks_to_validate() {
    let mut rejected = status(coherent_values(true), None, TransferState::Rejected);
    rejected.reason = Some("stale program, binding, or expected values".to_owned());
    let failure = transfer_rejection(&rejected);
    assert_eq!(
        failure.message,
        "stale program, binding, or expected values"
    );
    assert_eq!(failure.detail, None);
    assert_eq!(
        (failure.cause, failure.next_action),
        (Cause::Runtime, NextAction::None)
    );

    let mut reverted = status(
        coherent_values(true),
        Some(TransferTransaction {
            restored: Some(true),
            ..transaction(TransferState::Failed, None, None)
        }),
        TransferState::Failed,
    );
    reverted.reason =
        Some("first controller output Main.MV violates reviewed transfer tolerance".to_owned());
    let failure = transfer_rejection(&reverted);
    assert_eq!(
        failure.message,
        "first controller output Main.MV violates reviewed transfer tolerance."
    );
    assert_eq!(
        failure.detail.as_deref(),
        Some(
            "first controller output Main.MV violates reviewed transfer tolerance. The runtime reverted the transfer in the same scan, so the loop kept its mode and values."
        )
    );
    assert_eq!(
        (failure.cause, failure.next_action),
        (Cause::Settings, NextAction::Validate)
    );

    let silent = status(coherent_values(true), None, TransferState::Rejected);
    assert_eq!(transfer_rejection(&silent).message, "Transfer rejected.");
}

#[test]
fn the_confirmation_waits_while_pending_ends_on_a_terminal_status_and_refuses_another_identity() {
    let pending = status(
        coherent_values(true),
        Some(transaction(TransferState::Pending, None, None)),
        TransferState::Pending,
    );
    assert_eq!(
        transfer_finish_step(&owner(), &pending),
        Ok(TransferStep::Wait)
    );

    let applied = status(
        coherent_values(true),
        Some(transaction(TransferState::Applied, None, None)),
        TransferState::Applied,
    );
    assert_eq!(
        transfer_finish_step(&owner(), &applied),
        Ok(TransferStep::Terminal(applied.clone()))
    );

    let other = status(
        coherent_values(true),
        Some(TransferTransaction {
            id: "other".to_owned(),
            ..transaction(TransferState::Pending, None, None)
        }),
        TransferState::Pending,
    );
    let failure =
        transfer_finish_step(&owner(), &other).expect_err("another transaction is not the owner's");
    assert_eq!(
        failure.message,
        "The owned controller-transfer identity changed; confirmation is unresolved."
    );

    let none = status(coherent_values(true), None, TransferState::Idle);
    assert!(transfer_finish_step(&owner(), &none).is_err());
}

#[test]
fn a_cancel_settles_only_the_owner_before_dispatch() {
    let mut answer = TransferCancelAnswer {
        status: Some(TransferState::Cancelled),
        confirmed: true,
        reason: None,
        transaction_id: Some("transfer".to_owned()),
        program_revision: Some("program".to_owned()),
        binding_revision: Some("binding".to_owned()),
        before_dispatch: true,
    };
    assert!(transfer_cancel_acknowledged(&owner(), &answer));

    answer.before_dispatch = false;
    assert!(!transfer_cancel_acknowledged(&owner(), &answer));
    answer.before_dispatch = true;

    answer.transaction_id = Some("other".to_owned());
    assert!(!transfer_cancel_acknowledged(&owner(), &answer));
    answer.transaction_id = Some("transfer".to_owned());

    answer.confirmed = false;
    assert!(!transfer_cancel_acknowledged(&owner(), &answer));
    answer.confirmed = true;

    answer.status = Some(TransferState::Pending);
    assert!(!transfer_cancel_acknowledged(&owner(), &answer));
}

#[test]
fn a_parallel_pid_outcome_checks_the_reference_and_the_command_exactly_and_the_other_states_by_bounds(
) {
    let binding = pid_binding();
    let tracking = TransferTracking::ParallelPid(ParallelPidTracking {
        kind: ParallelPidKind::ParallelPidV1,
        reference_symbol: "P1.Ref".to_owned(),
        manual_command_symbol: "P1.ManualCommand".to_owned(),
        process_variable: "P1.PV".to_owned(),
        proportional_gain_symbol: "P1.Gain".to_owned(),
        integral_gain_symbol: "P1.Gain".to_owned(),
        derivative_gain_symbol: "P1.Gain".to_owned(),
        integral_state_symbol: "P1.Integral".to_owned(),
        derivative_state_symbol: "P1.Derivative".to_owned(),
        proportional_setpoint_weight: 1.0,
        derivative_setpoint_weight: 0.0,
        cycle_seconds: 0.1,
        align_output: true,
    });
    let applied = map(&[
        ("P1.Ref", 5.0),
        ("P1.ManualCommand", 40.0),
        ("P1.Integral", 3.0),
        ("P1.Derivative", 0.0),
    ]);
    // The integral state moved in the scan: it is compared by its bounds only, so 3.5 is accepted.
    let mut observed = pid_values(true);
    observed.insert("P1.Ref".to_owned(), number(5.0));
    observed.insert("P1.ManualCommand".to_owned(), number(40.0));
    observed.insert("P1.Integral".to_owned(), number(3.5));
    let outcome = status(
        observed.clone(),
        Some(transaction(
            TransferState::Applied,
            Some(applied.clone()),
            Some(observed.clone()),
        )),
        TransferState::Applied,
    );
    assert_eq!(
        verify_transfer_outcome(&binding, &BTreeMap::new(), true, Some(&tracking), &outcome),
        Ok(())
    );

    observed.insert("P1.Integral".to_owned(), number(150.0));
    let outcome = status(
        observed.clone(),
        Some(transaction(
            TransferState::Applied,
            Some(applied),
            Some(observed),
        )),
        TransferState::Applied,
    );
    let failure =
        verify_transfer_outcome(&binding, &BTreeMap::new(), true, Some(&tracking), &outcome)
            .expect_err("the integral state is outside its bounds");
    assert_eq!(
        failure.message,
        "The observed transfer does not match the reviewed mode and state."
    );
}

#[test]
fn a_declared_state_outcome_tracks_the_committed_output_in_manual() {
    let binding = binding();
    let tracking = TransferTracking::DeclaredState(DeclaredStateTracking {
        kind: DeclaredStateKind::DeclaredStateV1,
        reference_symbol: "P1.Ref".to_owned(),
        manual_command_symbol: "P1.ManualCommand".to_owned(),
        process_variable: "P1.PV".to_owned(),
        align_output: false,
        initialization_formulas: None,
        initialization_values: BTreeMap::new(),
        bounded_state_symbols: Vec::new(),
        additional_references: Vec::new(),
        additional_measurements: Vec::new(),
        additional_commands: Vec::new(),
    });
    // The held output is 40, so the Manual command must be 40 after the boundary.
    let applied = map(&[("P1.Ref", 0.0), ("P1.ManualCommand", 40.0)]);
    let mut observed = coherent_values(true);
    observed.insert("P1.ManualCommand".to_owned(), number(40.0));
    let outcome = status(
        observed.clone(),
        Some(transaction(
            TransferState::Applied,
            Some(applied.clone()),
            Some(observed.clone()),
        )),
        TransferState::Applied,
    );
    assert_eq!(
        verify_transfer_outcome(&binding, &BTreeMap::new(), true, Some(&tracking), &outcome),
        Ok(())
    );

    let mut not_tracking = applied;
    not_tracking.insert("P1.ManualCommand".to_owned(), 39.0);
    let outcome = status(
        observed.clone(),
        Some(transaction(
            TransferState::Applied,
            Some(not_tracking),
            Some(observed),
        )),
        TransferState::Applied,
    );
    let failure =
        verify_transfer_outcome(&binding, &BTreeMap::new(), true, Some(&tracking), &outcome)
            .expect_err("the Manual command differs from the held output");
    assert_eq!(
        failure.message,
        "Manual output did not track the committed output."
    );
}

#[test]
fn the_request_echoes_the_identifier_and_the_reviewed_values() {
    let reviewed = status(coherent_values(false), None, TransferState::Idle);
    let values = map(&[("P1.Ref", 0.0)]);
    let input = TransferApplyInput {
        transaction_id: "transaction-minted",
        values: &values,
        manual: false,
        maximum_output_change: 1.0,
        entry_error_gain: None,
        entry_measurement_gain: None,
        tracking: None,
        source: ApplySource::Simulation,
        physical_approved: false,
    };
    let request = transfer_apply_request(&binding(), &reviewed, &input).expect("a simulated apply");
    assert_eq!(request.transaction_id, "transaction-minted");
    assert_eq!(request.expected_values, reviewed.values);
}

#[test]
fn a_further_controllers_mode_follows_the_loop_it_is_switched_with() {
    // Ported from `control-studio-parameter-application.test.ts`: a further controller's mode is part of the transfer (Specification 35,
    // §17.11), so an Auto transfer must leave it in the mode that is the reverse of its Manual value.
    let mut binding = binding();
    binding.additional_modes = vec![TransferModeBinding {
        symbol: "P1.OuterManual".to_owned(),
        manual_value: true,
    }];
    let states = map(&[("P1.Ref", 0.0), ("P1.ManualCommand", 0.0)]);
    let mut observed = coherent_values(false);
    observed.insert("P1.OuterManual".to_owned(), flag(false));
    let outcome = status(
        observed.clone(),
        Some(transaction(
            TransferState::Applied,
            Some(states.clone()),
            Some(observed.clone()),
        )),
        TransferState::Applied,
    );
    assert_eq!(
        verify_transfer_outcome(&binding, &states, false, None, &outcome),
        Ok(())
    );

    observed.insert("P1.OuterManual".to_owned(), flag(true));
    let outcome = status(
        observed.clone(),
        Some(transaction(
            TransferState::Applied,
            Some(states.clone()),
            Some(observed),
        )),
        TransferState::Applied,
    );
    assert!(
        verify_transfer_outcome(&binding, &states, false, None, &outcome).is_err(),
        "a further controller left in Manual does not match the Auto transfer"
    );
}
