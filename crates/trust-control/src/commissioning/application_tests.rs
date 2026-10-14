use super::*;
use serde_json::json;

/// The candidate of `control-studio-validate-apply.test.ts`, "uses revision-guarded existing runtime requests and separate
/// physical approval".
fn candidate() -> ControlPidParameters {
    ControlPidParameters {
        proportional_gain: 1.0,
        integral_gain: 0.25,
        derivative_gain: 0.1,
        proportional_setpoint_weight: 0.8,
        derivative_setpoint_weight: 0.0,
        bias: 2.0,
        output_minimum: 0.0,
        output_maximum: 100.0,
        rate_minimum: -20.0,
        rate_maximum: 20.0,
        tracking_time: 4.0,
        anti_windup: AntiWindup::Combined,
    }
}

fn parameters_json() -> Value {
    json!({
        "ProportionalGain": 1.0, "IntegralGain": 0.25, "DerivativeGain": 0.1,
        "ProportionalSetpointWeight": 0.8, "DerivativeSetpointWeight": 0.0, "Bias": 2.0,
        "OutputMinimum": 0.0, "OutputMaximum": 100.0, "RateMinimum": -20.0, "RateMaximum": 20.0,
        "TrackingTime": 4.0, "AntiWindup": "Combined"
    })
}

/// A status answer as the runtime gives it (E7.7).
fn status_answer(active: &str, pending: Option<&str>, confirmed: bool) -> Value {
    let mut answer = json!({ "active_revision": active, "confirmed": confirmed, "parameters": parameters_json(), "reason": null });
    if let Some(pending) = pending {
        answer["pending_revision"] = json!(pending);
    }
    answer
}

#[test]
fn a_simulated_apply_writes_the_reviewed_record_under_the_runtime_names() {
    // control-studio-validate-apply.test.ts, "uses revision-guarded existing runtime requests…": the request of the first apply.
    let request = ControllerApplyRequest::new(
        "Main.Controller",
        ApplySource::Simulation,
        "controller-1",
        &candidate(),
        false,
    )
    .expect("the candidate is valid");
    assert_eq!(
        serde_json::to_value(&request).expect("the request serializes"),
        json!({
            "instance_path": "Main.Controller",
            "source": "simulation",
            "expected_revision": "controller-1",
            "parameters": parameters_json(),
            "physical_approved": false
        })
    );
}

#[test]
fn a_physical_apply_without_its_own_approval_is_refused_before_anything_is_sent() {
    // The same TypeScript test: the physical apply is rejected with the approval named.
    let refusal = ControllerApplyRequest::new(
        "Main.Controller",
        ApplySource::Physical,
        "controller-1",
        &candidate(),
        false,
    )
    .expect_err("a physical write needs its approval");
    assert!(refusal.message.contains("approval"), "{}", refusal.message);
    assert_eq!(refusal.cause, Cause::Approval);
    assert_eq!(refusal.next_action, NextAction::Approve);
}

#[test]
fn a_physical_apply_with_its_approval_names_the_physical_source() {
    let request = ControllerApplyRequest::new(
        "Main.Controller",
        ApplySource::Physical,
        "controller-1",
        &candidate(),
        true,
    )
    .expect("approved");
    assert_eq!(request.source, "physical");
    assert!(request.physical_approved);
}

#[test]
fn a_blank_instance_path_or_reviewed_revision_is_refused_by_name() {
    let blank_path = ControllerApplyRequest::new(
        " ",
        ApplySource::Simulation,
        "controller-1",
        &candidate(),
        false,
    )
    .expect_err("blank path");
    assert_eq!(
        blank_path.message,
        "The controller request needs a non-blank instance path."
    );
    assert_eq!(blank_path.next_action, NextAction::OpenLoopSetup);
    let blank_revision = ControllerApplyRequest::new(
        "Main.Controller",
        ApplySource::Simulation,
        "",
        &candidate(),
        false,
    )
    .expect_err("blank revision");
    assert_eq!(
        blank_revision.message,
        "The controller request needs a non-blank reviewed revision."
    );
    assert_eq!(blank_revision.next_action, NextAction::Validate);
}

#[test]
fn an_invalid_candidate_keeps_the_refusal_of_its_record() {
    let mut reversed = candidate();
    reversed.output_minimum = 100.0;
    let refusal = ControllerApplyRequest::new(
        "Main.Controller",
        ApplySource::Simulation,
        "controller-1",
        &reversed,
        false,
    )
    .expect_err("output limits in reverse order");
    assert_eq!(
        refusal.message,
        "Control.PID output minimum must be below output maximum."
    );
    assert_eq!(refusal.cause, Cause::Settings);
}

#[test]
fn the_status_answer_decodes_the_pending_write_and_its_confirmation() {
    // The three status answers of the TypeScript test, in order: the write is queued, read back, and then restored.
    let queued =
        decode_status(&status_answer("controller-1", Some("controller-2"), false)).expect("status");
    assert_eq!(queued.active_revision, "controller-1");
    assert_eq!(queued.pending_revision.as_deref(), Some("controller-2"));
    assert_eq!(
        confirmation_step(&queued, "controller-2"),
        Ok(Confirmation::Pending)
    );

    let read_back = decode_status(&status_answer("controller-2", None, true)).expect("status");
    assert_eq!(
        confirmation_step(&read_back, "controller-2"),
        Ok(Confirmation::Confirmed)
    );

    let restored = decode_status(&status_answer("controller-1", None, true)).expect("status");
    assert_eq!(
        confirmation_step(&restored, "controller-1"),
        Ok(Confirmation::Confirmed)
    );
}

#[test]
fn a_queued_write_is_not_confirmed_until_the_complete_record_is_read_back() {
    // C1: the revision stays pending until a later scan reads the structure back, so a status that is not yet confirmed waits.
    let active_but_unconfirmed =
        decode_status(&status_answer("controller-2", None, false)).expect("status");
    assert_eq!(
        confirmation_step(&active_but_unconfirmed, "controller-2"),
        Ok(Confirmation::Pending)
    );
}

#[test]
fn another_write_that_is_pending_replaces_the_reviewed_candidate() {
    let replaced =
        decode_status(&status_answer("controller-1", Some("controller-9"), false)).expect("status");
    let refusal =
        confirmation_step(&replaced, "controller-2").expect_err("another write is pending");
    assert_eq!(
        refusal.message,
        "A different controller write replaced the reviewed candidate: revision controller-9 is pending, and the reviewed candidate is controller-2."
    );
    assert_eq!(refusal.next_action, NextAction::Validate);
}

#[test]
fn a_record_that_changed_without_a_pending_write_is_not_confirmed() {
    let changed = decode_status(&status_answer("controller-1", None, false)).expect("status");
    let refusal =
        confirmation_step(&changed, "controller-2").expect_err("the record did not take the write");
    assert_eq!(
        refusal.message,
        "The controller write was not confirmed by the runtime: the active revision is controller-1, and the reviewed candidate is controller-2."
    );
}

#[test]
fn an_empty_pending_revision_is_no_pending_write() {
    let mut answer = status_answer("controller-1", None, false);
    answer["pending_revision"] = json!("");
    let status = decode_status(&answer).expect("status");
    assert_eq!(status.pending_revision, None);
    assert!(
        confirmation_step(&status, "controller-2").is_err(),
        "no pending write and not the reviewed revision"
    );
}

#[test]
fn the_status_parameters_must_be_an_object() {
    let refusal = decode_status(
        &json!({ "active_revision": "controller-1", "confirmed": true, "parameters": [1] }),
    )
    .expect_err("an array is not a record");
    assert_eq!(
        refusal.message,
        "Runtime returned invalid controller parameters."
    );
}

#[test]
fn an_apply_answer_names_both_revisions_and_the_applied_result_keeps_the_previous_one() {
    let answer = json!({ "previous_revision": "controller-1", "pending_revision": "controller-2" });
    let accepted = decode_apply_answer(&answer).expect("both revisions");
    assert_eq!(
        AppliedRevisions::confirmed(&accepted),
        AppliedRevisions {
            previous_revision: "controller-1".to_owned(),
            active_revision: "controller-2".to_owned(),
        }
    );
    let missing = decode_apply_answer(&json!({ "previous_revision": "controller-1" }))
        .expect_err("no pending revision");
    assert_eq!(
        missing.message,
        "Runtime returned an invalid controller revision."
    );
}

#[test]
fn a_rollback_request_carries_its_three_revisions_and_the_approval() {
    let request =
        ControllerRollbackRequest::new("Main.Controller", "controller-2", "controller-1", false)
            .expect("valid");
    assert_eq!(
        serde_json::to_value(&request).expect("serializes"),
        json!({
            "instance_path": "Main.Controller",
            "expected_active_revision": "controller-2",
            "previous_revision": "controller-1",
            "physical_approved": false
        })
    );
    assert_eq!(
        decode_rollback_answer(&json!({ "pending_revision": "controller-1" })),
        Ok("controller-1".to_owned())
    );
}

#[test]
fn the_status_request_refuses_a_blank_instance_path() {
    assert!(ControllerStatusRequest::new("Main.Controller").is_ok());
    assert!(ControllerStatusRequest::new("").is_err());
}

#[test]
fn an_unconfirmed_window_is_a_wait_with_its_length_named() {
    let timed_out = confirmation_timed_out(10_000.0);
    assert_eq!(timed_out.next_action, NextAction::Wait);
    assert!(
        timed_out
            .message
            .starts_with("Timed out waiting for scan-boundary controller confirmation"),
        "{}",
        timed_out.message
    );
    assert!(
        timed_out.message.contains("after 10 s"),
        "{}",
        timed_out.message
    );
}
