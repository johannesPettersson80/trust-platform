//! Tests of the client's rules (`parameterApplication.ts`): the binding, the record, the review, the requests, the confirmation and
//! the cancel. The first two tests port the assertions of `control-studio-parameter-application.test.ts`; the rest prove the rules
//! of annexes E7.1 to E7.6 and C0 that the TypeScript tests do not reach.

use super::*;

fn gain_binding() -> RuntimeBinding {
    RuntimeBinding {
        loop_id: "engineer-loop".to_owned(),
        task_name: "ControlTask".to_owned(),
        parameters: vec![gain_parameter("P1.Gain", RealType::Real, 0.0, 20.0)],
        mode: RuntimeMode {
            symbol: "P1.Enabled".to_owned(),
            manual_value: false,
        },
    }
}

fn gain_parameter(
    symbol: &str,
    iec_type: RealType,
    minimum: f64,
    maximum: f64,
) -> RuntimeParameter {
    RuntimeParameter {
        symbol: symbol.to_owned(),
        iec_type,
        unit: "N·m/rad".to_owned(),
        convention: "proportional gain".to_owned(),
        minimum,
        maximum,
    }
}

/// A record of the runtime with the given active revision and values, and no transaction.
fn record(active: &str, values: &[(&str, f64)], status: RecordState) -> ParameterStatus {
    ParameterStatus {
        supports_request_id: Some(true),
        transaction_id: None,
        program_revision: "program-1".to_owned(),
        binding_revision: "binding-1".to_owned(),
        active_revision: active.to_owned(),
        values: values
            .iter()
            .map(|(symbol, value)| ((*symbol).to_owned(), *value))
            .collect(),
        status,
        previous_revision: None,
        previous_values: None,
        reason: None,
    }
}

/// The text of a failure as the engineer reads it past its first sentence.
fn detail(error: &Failure) -> &str {
    error.detail.as_deref().unwrap_or(&error.message)
}

/// A review of `P1.Gain = 0.5` made against an idle record with `P1.Gain = 1`.
fn idle_review(transaction_id: &str) -> ParameterReview {
    let observed = check_status(
        &gain_binding(),
        record("active-1", &[("P1.Gain", 1.0)], RecordState::Idle),
    )
    .expect("an idle record is well formed");
    review_of(
        BTreeMap::from([("P1.Gain".to_owned(), 0.5)]),
        observed,
        transaction_id.to_owned(),
    )
    .expect("an idle record is reviewed")
}

#[test]
fn a_rollback_writes_the_retained_values_back_and_is_confirmed_after_execution() {
    // Ports "rolls back the exact retained values and confirms them after execution".
    let binding = gain_binding();
    let mut before = record("active-2", &[("P1.Gain", 2.0)], RecordState::Applied);
    before.transaction_id = Some("apply-1".to_owned());
    before.previous_revision = Some("active-1".to_owned());
    before.previous_values = Some(BTreeMap::from([("P1.Gain".to_owned(), 1.0)]));
    let checked = check_status(&binding, before).expect("the record is well formed");
    let review = rollback_review(checked, "rollback-1".to_owned()).expect("a rollback is retained");
    assert_eq!(
        review.values,
        BTreeMap::from([("P1.Gain".to_owned(), 1.0)]),
        "rollback review must retain original runtime values"
    );
    let request = rollback_request(&binding, &review, ParameterSource::Simulation, false)
        .expect("a rollback request");
    assert_eq!(request.previous_revision, "active-1");
    assert_eq!(request.expected_active_revision, "active-2");
    assert_eq!(
        request.expected_values,
        BTreeMap::from([("P1.Gain".to_owned(), 2.0)])
    );
    let mut after = record("active-1", &[("P1.Gain", 1.0)], RecordState::Applied);
    after.transaction_id = Some("rollback-1".to_owned());
    assert_eq!(
        confirmation_step(&review, &after),
        Ok(Confirmation::Confirmed)
    );
}

#[test]
fn a_review_writes_the_exact_real_value_and_is_confirmed_by_the_observed_adoption() {
    // Ports "reviews exact REAL values and confirms observed task-boundary adoption".
    let binding = gain_binding();
    let observed = check_status(
        &binding,
        record("active-1", &[("P1.Gain", 1.0)], RecordState::Idle),
    )
    .expect("the record is well formed");
    let values = review_input(&binding, &BTreeMap::from([("P1.Gain".to_owned(), 0.1)]))
        .expect("0.1 lies within the bounds");
    let review =
        review_of(values, observed, "apply-1".to_owned()).expect("an idle record is reviewed");
    assert_eq!(
        review.values,
        BTreeMap::from([("P1.Gain".to_owned(), f64::from(0.1_f32))]),
        "review must show exact IEC REAL value"
    );
    let request = apply_request(&binding, &review, ParameterSource::Simulation, false)
        .expect("an apply request");
    assert_eq!(
        request.expected_values,
        BTreeMap::from([("P1.Gain".to_owned(), 1.0)])
    );
    assert_eq!(request.expected_program_revision, "program-1");
    assert_eq!(request.expected_binding_revision, "binding-1");
    assert_eq!(request.expected_active_revision, "active-1");
    let mut adopted = record(
        "active-2",
        &[("P1.Gain", f64::from(0.1_f32))],
        RecordState::Applied,
    );
    adopted.transaction_id = Some("apply-1".to_owned());
    adopted.previous_revision = Some("active-1".to_owned());
    adopted.previous_values = Some(BTreeMap::from([("P1.Gain".to_owned(), 1.0)]));
    assert_eq!(
        confirmation_step(&review, &adopted),
        Ok(Confirmation::Confirmed)
    );
}

#[test]
fn a_binding_is_checked_before_any_runtime_call() {
    let binding = gain_binding();
    assert_eq!(check_binding(&binding, "engineer-loop"), Ok(()));

    let other_loop =
        check_binding(&binding, "other-loop").expect_err("the binding names another loop");
    assert_eq!(other_loop.message, "Invalid parameter binding.");
    assert!(detail(&other_loop).contains("other-loop"));

    let mut case_clash = gain_binding();
    case_clash
        .parameters
        .push(gain_parameter("p1.gain", RealType::Lreal, 0.0, 1.0));
    let clash = check_binding(&case_clash, "engineer-loop")
        .expect_err("symbols are unique without regard to case");
    assert!(detail(&clash).contains("p1.gain"));

    let mut mode_clash = gain_binding();
    mode_clash.mode.symbol = "P1.Gain".to_owned();
    let mode = check_binding(&mode_clash, "engineer-loop")
        .expect_err("a parameter cannot be the mode symbol");
    assert!(detail(&mode).contains("P1.Gain"));

    let mut reversed = gain_binding();
    reversed.parameters[0].minimum = 20.0;
    reversed.parameters[0].maximum = 0.0;
    let reversed_error = check_binding(&reversed, "engineer-loop")
        .expect_err("the minimum may not exceed the maximum");
    assert!(detail(&reversed_error).contains("P1.Gain"));

    let mut infinite = gain_binding();
    infinite.parameters[0].maximum = f64::INFINITY;
    assert!(
        check_binding(&infinite, "engineer-loop").is_err(),
        "a bound must be finite"
    );

    let mut no_unit = gain_binding();
    no_unit.parameters[0].unit = "  ".to_owned();
    let unit = check_binding(&no_unit, "engineer-loop").expect_err("a unit may not be blank");
    assert!(detail(&unit).contains("unit of P1.Gain"));

    let mut empty = gain_binding();
    empty.parameters.clear();
    let none =
        check_binding(&empty, "engineer-loop").expect_err("a binding holds one parameter at least");
    assert!(detail(&none).contains("not 0"));

    let mut too_many = gain_binding();
    too_many.parameters = (0..65)
        .map(|index| gain_parameter(&format!("P{index}.Gain"), RealType::Real, 0.0, 1.0))
        .collect();
    let many = check_binding(&too_many, "engineer-loop")
        .expect_err("a binding holds 64 parameters at most");
    assert!(detail(&many).contains("not 65"));
}

#[test]
fn a_recorded_status_is_refused_when_it_does_not_hold_the_declared_record() {
    let binding = gain_binding();
    assert!(check_status(
        &binding,
        record("active-1", &[("P1.Gain", 1.0)], RecordState::Idle)
    )
    .is_ok());

    let mut empty_revision = record("active-1", &[("P1.Gain", 1.0)], RecordState::Idle);
    empty_revision.program_revision.clear();
    let refused = check_status(&binding, empty_revision).expect_err("a revision must be set");
    assert_eq!(
        refused.message,
        "Runtime returned invalid parameter status."
    );
    assert_eq!(refused.next_action, NextAction::None);

    let missing = record("active-1", &[], RecordState::Idle);
    assert!(
        check_status(&binding, missing).is_err(),
        "the values hold every declared symbol"
    );

    let mut empty_transaction = record("active-1", &[("P1.Gain", 1.0)], RecordState::Idle);
    empty_transaction.transaction_id = Some(String::new());
    assert!(
        check_status(&binding, empty_transaction).is_err(),
        "a transaction identifier that is present is not empty"
    );

    let not_finite = record("active-1", &[("P1.Gain", f64::NAN)], RecordState::Idle);
    assert!(
        check_status(&binding, not_finite).is_err(),
        "every value is finite"
    );
}

#[test]
fn a_review_writes_only_the_declared_values_rounded_as_their_iec_type_and_within_their_bounds() {
    let binding = gain_binding();
    let missing =
        review_input(&binding, &BTreeMap::new()).expect_err("every declared symbol is written");
    assert_eq!(
        missing.message,
        "Provide exactly the declared finite parameter values."
    );

    let mut lreal = gain_binding();
    lreal.parameters[0].iec_type = RealType::Lreal;
    let exact = review_input(&lreal, &BTreeMap::from([("P1.Gain".to_owned(), 0.1)]))
        .expect("an LREAL keeps its value");
    assert_eq!(exact, BTreeMap::from([("P1.Gain".to_owned(), 0.1)]));

    let mut tight = gain_binding();
    tight.parameters[0].maximum = 0.1;
    let rounded_out = review_input(&tight, &BTreeMap::from([("P1.Gain".to_owned(), 0.1)]))
        .expect_err("0.1 rounds to single precision above the bound");
    assert_eq!(
        rounded_out.message,
        "P1.Gain is outside its declared bounds after IEC rounding."
    );
    assert!(
        detail(&rounded_out).contains("N·m/rad"),
        "the bounds are shown with their unit"
    );

    let not_finite = review_input(
        &binding,
        &BTreeMap::from([("P1.Gain".to_owned(), f64::NAN)]),
    )
    .expect_err("a value must be finite");
    assert_eq!(
        not_finite.message,
        "Provide exactly the declared finite parameter values."
    );
}

#[test]
fn a_pending_record_is_waited_for_before_a_review_and_a_rollback_needs_a_retained_set() {
    let binding = gain_binding();
    let pending = check_status(
        &binding,
        record("active-1", &[("P1.Gain", 1.0)], RecordState::Pending),
    )
    .expect("the record is well formed");
    let refused = review_of(
        BTreeMap::from([("P1.Gain".to_owned(), 0.5)]),
        pending.clone(),
        "review".to_owned(),
    )
    .expect_err("a pending record is not reviewed");
    assert_eq!(
        refused.message,
        "Wait for the pending parameter transaction before review."
    );
    assert_eq!(refused.next_action, NextAction::Wait);

    let rollback = rollback_review(pending, "rollback".to_owned())
        .expect_err("a pending record retains no rollback");
    assert_eq!(
        rollback.message,
        "No confirmed previous parameter set is available for rollback."
    );

    let mut no_previous = record("active-1", &[("P1.Gain", 1.0)], RecordState::Applied);
    no_previous.previous_revision = None;
    no_previous.previous_values = Some(BTreeMap::from([("P1.Gain".to_owned(), 1.0)]));
    assert!(
        rollback_review(no_previous, "rollback".to_owned()).is_err(),
        "a rollback needs a previous revision"
    );
}

#[test]
fn an_apply_or_rollback_needs_the_matching_review_the_approval_and_request_identities() {
    let binding = gain_binding();
    let review = idle_review("review-1");
    assert!(apply_request(&binding, &review, ParameterSource::Simulation, false).is_ok());

    let physical = apply_request(&binding, &review, ParameterSource::Physical, false)
        .expect_err("a physical write needs the engineer's approval");
    assert_eq!(
        (physical.cause, physical.next_action),
        (Cause::Approval, NextAction::Approve)
    );
    assert_eq!(
        physical.message,
        "Physical approval is required for parameter application on a physical target."
    );
    assert!(apply_request(&binding, &review, ParameterSource::Physical, true).is_ok());

    let mut unsupported = idle_review("review-2");
    unsupported.observed.supports_request_id = Some(false);
    let identity = apply_request(&binding, &unsupported, ParameterSource::Simulation, false)
        .expect_err("the runtime must keep request identities");
    assert_eq!(
        identity.message,
        "Reload the runtime with parameter-operation identity support before application."
    );

    let rollback_for_apply =
        rollback_request(&binding, &review, ParameterSource::Simulation, false)
            .expect_err("an Apply review cannot roll back");
    assert_eq!(
        rollback_for_apply.message,
        "Review the retained rollback values on this runtime first."
    );

    let mut retained = record("active-2", &[("P1.Gain", 2.0)], RecordState::Applied);
    retained.previous_revision = Some("active-1".to_owned());
    retained.previous_values = Some(BTreeMap::from([("P1.Gain".to_owned(), 1.0)]));
    let rollback = rollback_review(
        check_status(&binding, retained).expect("the record is well formed"),
        "rollback".to_owned(),
    )
    .expect("a rollback is retained");
    assert!(rollback_request(&binding, &rollback, ParameterSource::Simulation, false).is_ok());
    let apply_for_rollback = apply_request(&binding, &rollback, ParameterSource::Simulation, false)
        .expect_err("a rollback review cannot apply");
    assert_eq!(
        apply_for_rollback.message,
        "Review this parameter set on the bound runtime first."
    );
    let physical_rollback = rollback_request(&binding, &rollback, ParameterSource::Physical, false)
        .expect_err("a physical rollback needs approval");
    assert_eq!(
        physical_rollback.message,
        "Physical approval is required for parameter rollback on a physical target."
    );
}

#[test]
fn a_confirmation_needs_the_reviewed_values_under_its_transaction_and_names_what_failed() {
    let binding = gain_binding();
    let review = idle_review("review-1");

    let waiting = record("active", &[("P1.Gain", 1.0)], RecordState::Pending);
    assert_eq!(
        confirmation_step(&review, &waiting),
        Ok(Confirmation::Waiting)
    );

    let mut changed = record("active", &[("P1.Gain", 1.0)], RecordState::Idle);
    changed.program_revision = "program-2".to_owned();
    let moved = confirmation_step(&review, &changed).expect_err("the program changed");
    assert_eq!(
        moved.message,
        "Runtime program or parameter binding changed during application."
    );
    assert_eq!(moved.next_action, NextAction::Validate);

    let mut rejected = record("active", &[("P1.Gain", 1.0)], RecordState::Rejected);
    rejected.reason =
        Some("controller changed parameter values before confirmation: P1.Gain".to_owned());
    let reason =
        confirmation_step(&review, &rejected).expect_err("the program rewrote the parameter");
    assert_eq!(
        reason.message,
        "controller changed parameter values before confirmation: P1.Gain"
    );

    let idle = record("active", &[("P1.Gain", 1.0)], RecordState::Idle);
    let unconfirmed =
        confirmation_step(&review, &idle).expect_err("an idle record confirms nothing");
    assert_eq!(
        unconfirmed.message,
        "The reviewed values were not confirmed after controller execution."
    );

    let timeout = confirmation_timeout(&review);
    assert_eq!(timeout.next_action, NextAction::ConnectRuntime);
    assert_eq!(
        timeout.message,
        "Timed out waiting for parameter confirmation; inspect runtime status before retrying."
    );

    let mut retained = record("active-2", &[("P1.Gain", 2.0)], RecordState::Applied);
    retained.previous_revision = Some("active-1".to_owned());
    retained.previous_values = Some(BTreeMap::from([("P1.Gain".to_owned(), 1.0)]));
    let rollback = rollback_review(
        check_status(&binding, retained).expect("the record is well formed"),
        "rollback".to_owned(),
    )
    .expect("a rollback is retained");
    let restore = confirmation_step(
        &rollback,
        &record("active-2", &[("P1.Gain", 1.0)], RecordState::Idle),
    )
    .expect_err("the rollback is not confirmed");
    assert_eq!(
        restore.next_action,
        NextAction::Restore,
        "a restore that is not confirmed is restored again"
    );
    assert_eq!(
        confirmation_timeout(&rollback).next_action,
        NextAction::Restore
    );
}

#[test]
fn a_cancel_settles_only_for_a_confirmed_answer_about_its_own_identity() {
    let owner = ParameterOwner {
        id: "apply-1".to_owned(),
        program: "program-1".to_owned(),
        binding: "binding-1".to_owned(),
    };
    let answer = |confirmed: bool, transaction: &str, before_dispatch: bool| CancelResponse {
        confirmed,
        transaction_id: Some(transaction.to_owned()),
        program_revision: Some("program-1".to_owned()),
        binding_revision: Some("binding-1".to_owned()),
        before_dispatch,
    };
    assert_eq!(
        cancel_acknowledgement(&owner, Some(&answer(true, "apply-1", true))),
        Some(CancelOutcome::BeforeDispatch)
    );
    assert_eq!(
        cancel_acknowledgement(&owner, Some(&answer(true, "apply-1", false))),
        Some(CancelOutcome::Terminal)
    );
    assert_eq!(
        cancel_acknowledgement(&owner, Some(&answer(false, "apply-1", true))),
        None
    );
    assert_eq!(
        cancel_acknowledgement(&owner, Some(&answer(true, "other", true))),
        None
    );
    assert_eq!(
        cancel_acknowledgement(&owner, None),
        None,
        "no answer settles nothing"
    );

    let mut pending = record("active-1", &[("P1.Gain", 1.0)], RecordState::Pending);
    pending.transaction_id = Some("apply-1".to_owned());
    assert_eq!(cancel_step(&owner, &pending), Ok(CancelProgress::Waiting));
    let mut ended = pending.clone();
    ended.status = RecordState::Applied;
    assert_eq!(cancel_step(&owner, &ended), Ok(CancelProgress::Terminal));
    let mut other = pending;
    other.transaction_id = Some("another".to_owned());
    let unresolved =
        cancel_step(&owner, &other).expect_err("the identity no longer belongs to the owner");
    assert_eq!(
        unresolved.message,
        "The parameter-operation identity remains unresolved."
    );
    assert_eq!(unresolved.next_action, NextAction::ConnectRuntime);
    assert_eq!(
        cancel_timeout().message,
        "Parameter cancellation remains unconfirmed; retry the retained operation after reconnecting."
    );
}
