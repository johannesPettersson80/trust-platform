//! Tests of the reviewed Apply and Restore sequence (`reviewedStudioApplication.ts`). The first six tests port the assertions of
//! `control-studio-reviewed-application.test.ts`; its last test, which routes the boundary through the session owner
//! (`studioAnalysisSessions.ts`), belongs to that owner and is not ported here.

use serde_json::json;

use super::*;

fn scope() -> MessageScope {
    MessageScope {
        project_id: "project".to_owned(),
        editor_session_id: "editor".to_owned(),
        loop_id: "loop".to_owned(),
        loop_revision: "revision".to_owned(),
    }
}

/// An application command for the scope, with the given kind, message identifier and payload.
#[allow(clippy::needless_pass_by_value)] // A test fixture: it takes the JSON literals its callers write in place.
fn command(kind: &str, message_id: &str, payload: Value) -> Value {
    json!({
        "schemaVersion": 1,
        "projectId": "project",
        "editorSessionId": "editor",
        "loopId": "loop",
        "expectedLoopRevision": "revision",
        "kind": kind,
        "messageId": message_id,
        "payload": payload,
    })
}

#[test]
fn a_command_decodes_only_the_exact_scoped_apply_and_restore() {
    // Ports "strictly decodes scoped single-click Apply and Restore commands".
    let scope = scope();
    assert_eq!(
        decode_application_command(
            command("control-studio.application.apply", "apply", json!({})),
            &scope
        ),
        Some(ApplicationCommand {
            action: ApplicationAction::Apply,
            message_id: "apply".to_owned(),
        })
    );
    assert_eq!(
        decode_application_command(
            command("control-studio.application.restore", "restore", json!({})),
            &scope
        ),
        Some(ApplicationCommand {
            action: ApplicationAction::Restore,
            message_id: "restore".to_owned(),
        })
    );
    assert_eq!(
        decode_application_command(
            command(
                "control-studio.application.apply",
                "wrong",
                json!({ "extra": true })
            ),
            &scope
        ),
        None,
        "a payload must be empty"
    );
    let mut other_project = command("control-studio.application.apply", "wrong", json!({}));
    other_project["projectId"] = json!("other");
    assert_eq!(decode_application_command(other_project, &scope), None);
}

#[test]
fn a_command_has_exactly_the_envelope_fields_and_is_an_object_of_at_most_16_000_units() {
    let scope = scope();
    let mut extra_field = command("control-studio.application.apply", "apply", json!({}));
    extra_field["unexpected"] = json!(1);
    assert_eq!(
        decode_application_command(extra_field, &scope),
        None,
        "an unknown field is refused"
    );
    assert_eq!(
        decode_application_command(json!([1, 2, 3, 4, 5, 6, 7, 8]), &scope),
        None,
        "an array is never decoded as the envelope"
    );
    let long_identifier = "a".repeat(16_000);
    let oversized = command(
        "control-studio.application.apply",
        &long_identifier,
        json!({}),
    );
    assert_eq!(
        decode_application_command(oversized, &scope),
        None,
        "a command over 16,000 units is refused"
    );
}

#[test]
fn a_reviewed_apply_runs_one_review_and_then_one_confirmation() {
    // Ports "reviews and confirms exactly one Apply request, marking both normal payloads".
    let mut application = ReviewedApplication::new();
    let request = ApplicationCommand {
        action: ApplicationAction::Apply,
        message_id: "request".to_owned(),
    };
    let Admission::Start(ticket) = application.admit(&request, true) else {
        panic!("the first request starts");
    };
    assert_eq!(review_message_id(&ticket.message_id), "request:review");
    assert_eq!(
        ticket.action.review_step().kind(),
        "control-studio.engineer.apply"
    );
    let mut replies = ReviewReplies::default();
    replies.record(classify_review_reply(ticket.action, "review", Some(false)));
    let live = application.is_live(&ticket, true);
    assert_eq!(
        review_outcome(ticket.action, true, replies, live),
        ReviewOutcome::Confirm
    );
    assert_eq!(confirm_message_id(&ticket.message_id), "request:confirm");
    assert_eq!(
        EngineerStep::Confirm.kind(),
        "control-studio.engineer.confirm"
    );
    application.finish(&ticket);
}

#[test]
fn a_restore_does_not_confirm_a_review_made_for_an_apply() {
    // Ports "does not confirm a Restore request when the returned review is for Apply".
    let mut application = ReviewedApplication::new();
    let request = ApplicationCommand {
        action: ApplicationAction::Restore,
        message_id: "restore".to_owned(),
    };
    let Admission::Start(ticket) = application.admit(&request, true) else {
        panic!("the restore request starts");
    };
    assert_eq!(
        ticket.action.review_step().kind(),
        "control-studio.engineer.rollback"
    );
    let reply = classify_review_reply(ticket.action, "review", Some(false));
    assert_eq!(
        reply,
        ReviewReply::Other,
        "an apply review is not a restore review"
    );
    let mut replies = ReviewReplies::default();
    replies.record(reply);
    let live = application.is_live(&ticket, true);
    assert_eq!(
        review_outcome(ticket.action, true, replies, live),
        ReviewOutcome::Unavailable(Failure::new(
            "The restore review was not available.",
            Cause::Runtime,
            NextAction::None
        ))
    );
}

#[test]
fn a_disposed_page_answers_no_late_confirmation_and_repeats_no_operation() {
    // Ports "deduplicates a completed request and suppresses late confirmation after disposal".
    let mut application = ReviewedApplication::new();
    let request = ApplicationCommand {
        action: ApplicationAction::Apply,
        message_id: "late".to_owned(),
    };
    let Admission::Start(ticket) = application.admit(&request, true) else {
        panic!("the request starts");
    };
    let mut replies = ReviewReplies::default();
    replies.record(classify_review_reply(
        ApplicationAction::Apply,
        "review",
        Some(false),
    ));
    assert_eq!(
        review_outcome(
            ApplicationAction::Apply,
            true,
            replies,
            application.is_live(&ticket, true)
        ),
        ReviewOutcome::Confirm
    );
    application.dispose();
    assert!(
        !application.is_live(&ticket, true),
        "a late confirmation is not posted"
    );
    assert_eq!(
        review_outcome(
            ApplicationAction::Apply,
            true,
            replies,
            application.is_live(&ticket, true)
        ),
        ReviewOutcome::Stop
    );
    assert_eq!(
        application.admit(&request, true),
        Admission::Silent,
        "a disposed request cannot repeat the runtime operation"
    );
}

#[test]
fn a_distinct_request_is_busy_while_the_first_awaits_its_confirmation() {
    // Ports "blocks a distinct request while the first request is awaiting confirmation".
    let mut application = ReviewedApplication::new();
    let first = ApplicationCommand {
        action: ApplicationAction::Apply,
        message_id: "first".to_owned(),
    };
    let second = ApplicationCommand {
        action: ApplicationAction::Restore,
        message_id: "second".to_owned(),
    };
    let Admission::Start(ticket) = application.admit(&first, true) else {
        panic!("the first request starts");
    };
    assert_eq!(review_message_id(&ticket.message_id), "first:review");
    assert_eq!(confirm_message_id(&ticket.message_id), "first:confirm");
    assert_eq!(
        application.admit(&second, true),
        Admission::Busy("Another application request is awaiting confirmation.".to_owned())
    );
    application.finish(&ticket);
    assert!(
        matches!(application.admit(&second, true), Admission::Start(_)),
        "once the first request has finished, the next one starts"
    );
}

#[test]
fn an_engineer_recovery_outcome_is_kept_and_no_second_refusal_follows() {
    // Ports "preserves an underlying recovery-required review outcome".
    let mut application = ReviewedApplication::new();
    let request = ApplicationCommand {
        action: ApplicationAction::Apply,
        message_id: "recovery".to_owned(),
    };
    let Admission::Start(ticket) = application.admit(&request, true) else {
        panic!("the request starts");
    };
    let reply = classify_review_reply(ApplicationAction::Apply, "recovery-required", None);
    assert_eq!(reply, ReviewReply::Terminal);
    let mut replies = ReviewReplies::default();
    replies.record(reply);
    assert_eq!(
        review_outcome(
            ApplicationAction::Apply,
            true,
            replies,
            application.is_live(&ticket, true)
        ),
        ReviewOutcome::Stop,
        "the engineer's own outcome ends the request without a 'not available' reply"
    );
    application.finish(&ticket);
}

#[test]
fn a_page_handles_each_message_once_and_at_most_4096_requests() {
    let mut application = ReviewedApplication::new();
    for index in 0..MAXIMUM_HANDLED_REQUESTS {
        let request = ApplicationCommand {
            action: ApplicationAction::Apply,
            message_id: format!("request-{index}"),
        };
        let Admission::Start(ticket) = application.admit(&request, true) else {
            panic!("request {index} starts");
        };
        application.finish(&ticket);
    }
    let repeated = ApplicationCommand {
        action: ApplicationAction::Apply,
        message_id: "request-0".to_owned(),
    };
    assert_eq!(
        application.admit(&repeated, true),
        Admission::Busy(texts::parameter_request_already_handled())
    );
    let next = ApplicationCommand {
        action: ApplicationAction::Apply,
        message_id: "request-4096".to_owned(),
    };
    assert_eq!(
        application.admit(&next, true),
        Admission::Busy(texts::parameter_request_limit())
    );
    assert_eq!(
        application.admit(&next, false),
        Admission::Silent,
        "an owner that is no longer current receives nothing"
    );
}

#[test]
fn an_engineer_reply_is_a_review_only_for_the_requests_own_action() {
    assert_eq!(
        classify_review_reply(ApplicationAction::Apply, "review", Some(false)),
        ReviewReply::Reviewed
    );
    assert_eq!(
        classify_review_reply(ApplicationAction::Restore, "review", Some(true)),
        ReviewReply::Reviewed
    );
    assert_eq!(
        classify_review_reply(ApplicationAction::Apply, "review", None),
        ReviewReply::Other
    );
    assert_eq!(
        classify_review_reply(ApplicationAction::Apply, "busy", None),
        ReviewReply::Terminal
    );
    assert_eq!(
        classify_review_reply(ApplicationAction::Restore, "failed", None),
        ReviewReply::Terminal
    );
}

#[test]
fn a_command_and_a_review_name_their_messages_by_the_engineer_workflow() {
    assert_eq!(EngineerStep::Apply.kind(), "control-studio.engineer.apply");
    assert_eq!(
        EngineerStep::Rollback.kind(),
        "control-studio.engineer.rollback"
    );
    assert_eq!(
        ApplicationAction::Restore.review_step(),
        EngineerStep::Rollback
    );
    assert_eq!(ApplicationAction::Apply.word(), "apply");
    assert_eq!(ApplicationAction::Restore.word(), "restore");
    assert_eq!(review_message_id("first"), "first:review");
    assert_eq!(confirm_message_id("first"), "first:confirm");
}
