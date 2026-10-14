//! The reviewed Apply and Restore sequence of a page (specification 35, B6.1 and B6.4; annex E7.3 to E7.5). From
//! `reviewedStudioApplication.ts`: the decoding of a command, the admission of a request, the reading of the engineer's replies
//! to its review, and the verdict that the review gives. The engineer workflow, the posting of the replies and the message
//! identifiers' transport are the caller's; this module decides what each step may do.

use std::collections::BTreeSet;

use serde::Deserialize;
use serde_json::{Map, Value};

use crate::failure::{Cause, Failure, NextAction};
use crate::json;

use super::parameter_session::MessageScope;
use super::texts;

/// The longest application command, in UTF-16 units of its JSON text.
const MAXIMUM_COMMAND_UNITS: usize = 16_000;
/// The most application requests one page handles; the next one is refused.
const MAXIMUM_HANDLED_REQUESTS: usize = 4_096;

/// What a reviewed request asks for: an Apply, or a Restore (a rollback).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApplicationAction {
    /// Apply the reviewed values.
    Apply,
    /// Restore the previous settings.
    Restore,
}

impl ApplicationAction {
    /// The word that names the action in the replies (`applicationAction`).
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Self::Apply => "apply",
            Self::Restore => "restore",
        }
    }

    /// The engineer step that reviews the action: an Apply reviews an apply, and a Restore reviews a rollback.
    #[must_use]
    pub fn review_step(self) -> EngineerStep {
        match self {
            Self::Apply => EngineerStep::Apply,
            Self::Restore => EngineerStep::Rollback,
        }
    }
}

/// A step of the engineer workflow that a reviewed request drives (`control-studio.engineer.<step>`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineerStep {
    /// Review an apply.
    Apply,
    /// Review a rollback.
    Rollback,
    /// Confirm the reviewed write.
    Confirm,
}

impl EngineerStep {
    /// The message kind of the step.
    #[must_use]
    pub fn kind(self) -> &'static str {
        match self {
            Self::Apply => "control-studio.engineer.apply",
            Self::Rollback => "control-studio.engineer.rollback",
            Self::Confirm => "control-studio.engineer.confirm",
        }
    }
}

/// The identifier of a request's review message: the request's identifier with the `review` suffix.
#[must_use]
pub fn review_message_id(message_id: &str) -> String {
    format!("{message_id}:review")
}

/// The identifier of a request's confirmation message: the request's identifier with the `confirm` suffix.
#[must_use]
pub fn confirm_message_id(message_id: &str) -> String {
    format!("{message_id}:confirm")
}

/// A decoded application command (`ReviewedStudioApplicationCommand`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplicationCommand {
    /// The action the command asks for.
    pub action: ApplicationAction,
    /// The webview's identifier of the message.
    pub message_id: String,
}

/// The envelope of an application command (`decodeReviewedStudioApplicationCommand`): exactly these fields, and no others.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CommandEnvelope {
    schema_version: u64,
    message_id: String,
    project_id: String,
    editor_session_id: String,
    loop_id: String,
    expected_loop_revision: String,
    kind: String,
    payload: Map<String, Value>,
}

/// Decodes an application command for `scope`. The message is an object of at most 16,000 UTF-16 units, with the envelope's
/// fields and no others; schema version 1; the scope's project, editor session, loop and loop revision; a non-empty message
/// identifier; an empty payload; and an `apply` or `restore` kind. Anything else is not this command.
#[must_use]
pub fn decode_application_command(
    message: Value,
    scope: &MessageScope,
) -> Option<ApplicationCommand> {
    if !message.is_object() {
        return None;
    }
    let text = serde_json::to_string(&message).ok()?;
    if text.encode_utf16().count() > MAXIMUM_COMMAND_UNITS {
        return None;
    }
    let envelope: CommandEnvelope = json::decode_value(message).ok()?;
    if envelope.schema_version != 1
        || envelope.message_id.is_empty()
        || envelope.project_id != scope.project_id
        || envelope.editor_session_id != scope.editor_session_id
        || envelope.loop_id != scope.loop_id
        || envelope.expected_loop_revision != scope.loop_revision
        || !envelope.payload.is_empty()
    {
        return None;
    }
    let action = match envelope.kind.as_str() {
        "control-studio.application.apply" => ApplicationAction::Apply,
        "control-studio.application.restore" => ApplicationAction::Restore,
        _ => return None,
    };
    Some(ApplicationCommand {
        action,
        message_id: envelope.message_id,
    })
}

/// An admitted application request: its message, its action, and the generation that it started in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplicationTicket {
    /// The webview's identifier of the request.
    pub message_id: String,
    /// The action of the request.
    pub action: ApplicationAction,
    generation: u64,
}

/// The verdict on a command before its review (`handle`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Admission {
    /// The page is disposed, or its owner is no longer current: nothing is answered and nothing runs.
    Silent,
    /// The request is refused with this reason, as a `busy` reply.
    Busy(String),
    /// The review may start under this ticket.
    Start(ApplicationTicket),
}

/// The one reviewed Apply or Restore sequence of a page (`ReviewedStudioApplicationCoordinator`): a message is handled once, one
/// request awaits its confirmation at a time, at most 4,096 requests are handled, and no reply follows the page's disposal.
#[derive(Debug, Default)]
pub struct ReviewedApplication {
    handled: BTreeSet<String>,
    active: Option<String>,
    generation: u64,
    disposed: bool,
}

impl ReviewedApplication {
    /// A page with no request handled and none awaiting its confirmation.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Disposes the page (`dispose`): no reply is posted afterwards, and no request is admitted.
    pub fn dispose(&mut self) {
        self.disposed = true;
        self.generation += 1;
    }

    /// Whether a reply may be posted to the owner now (`forward`).
    #[must_use]
    pub fn may_forward(&self, owner_current: bool) -> bool {
        !self.disposed && owner_current
    }

    /// Whether a request's replies still belong to the page: the page is not disposed, its owner is current, and the request started
    /// in the page's current generation. Every step checks it before it posts.
    #[must_use]
    pub fn is_live(&self, ticket: &ApplicationTicket, owner_current: bool) -> bool {
        ticket.generation == self.generation && self.may_forward(owner_current)
    }

    /// Admits a decoded command (`handle`, before the review). A message is handled only once; a request waits behind the one that
    /// awaits its confirmation; and a page handles at most 4,096 requests.
    #[must_use]
    pub fn admit(&mut self, command: &ApplicationCommand, owner_current: bool) -> Admission {
        if !self.may_forward(owner_current) {
            return Admission::Silent;
        }
        if self.handled.contains(&command.message_id) {
            return Admission::Busy(texts::parameter_request_already_handled());
        }
        if self.active.is_some() {
            return Admission::Busy(texts::parameter_request_busy());
        }
        if self.handled.len() >= MAXIMUM_HANDLED_REQUESTS {
            return Admission::Busy(texts::parameter_request_limit());
        }
        self.handled.insert(command.message_id.clone());
        self.active = Some(command.message_id.clone());
        Admission::Start(ApplicationTicket {
            message_id: command.message_id.clone(),
            action: command.action,
            generation: self.generation,
        })
    }

    /// Ends an admitted request (`finally`): the next request may start.
    pub fn finish(&mut self, ticket: &ApplicationTicket) {
        if self.active.as_deref() == Some(ticket.message_id.as_str()) {
            self.active = None;
        }
    }
}

/// What one engineer reply means for the review of a request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewReply {
    /// The review of this request's action: the status is `review`, and its rollback flag is true for a Restore and false for an
    /// Apply.
    Reviewed,
    /// A reply that ends the review (`failed`, `recovery-required` or `busy`); it is forwarded as it is.
    Terminal,
    /// Any other reply: it is forwarded, and it is not a review.
    Other,
}

/// Classifies one engineer reply to a review step (`reviewed` and `terminalReviewPayload`). `status` and `rollback` are the reply's
/// fields as the engineer sends them, and `rollback` is `None` when the reply has no such field.
#[must_use]
pub fn classify_review_reply(
    action: ApplicationAction,
    status: &str,
    rollback: Option<bool>,
) -> ReviewReply {
    if status == "review" && rollback == Some(action == ApplicationAction::Restore) {
        ReviewReply::Reviewed
    } else if matches!(status, "failed" | "recovery-required" | "busy") {
        ReviewReply::Terminal
    } else {
        ReviewReply::Other
    }
}

/// What the replies to a review step said, gathered as they are forwarded (`reviewed` and `terminalReviewPayload`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ReviewReplies {
    reviewed: bool,
    terminal: bool,
}

impl ReviewReplies {
    /// Records one reply of the review step.
    pub fn record(&mut self, reply: ReviewReply) {
        match reply {
            ReviewReply::Reviewed => self.reviewed = true,
            ReviewReply::Terminal => self.terminal = true,
            ReviewReply::Other => {}
        }
    }
}

/// What a request does after its review step (`handle`, after the review).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReviewOutcome {
    /// The review was made for this action and the request is live: the confirmation step runs.
    Confirm,
    /// No review came, and nothing ended the review: the webview is told that the review was not available.
    Unavailable(Failure),
    /// The request ends here: its replies were forwarded, or it was superseded.
    Stop,
}

/// The outcome of a review step (`handle`, after the review). `engineer_handled` is what the engineer returned, `replies` says what
/// its replies were, and `live` is `is_live` of the request's ticket. The confirmation runs only for a review that was made for the
/// request's action, while the request is still live.
#[must_use]
pub fn review_outcome(
    action: ApplicationAction,
    engineer_handled: bool,
    replies: ReviewReplies,
    live: bool,
) -> ReviewOutcome {
    if engineer_handled && replies.reviewed && live {
        ReviewOutcome::Confirm
    } else if !replies.reviewed && !replies.terminal && live {
        let text = texts::parameter_review_unavailable(action.word());
        ReviewOutcome::Unavailable(Failure::new(&text, Cause::Runtime, NextAction::None))
    } else {
        ReviewOutcome::Stop
    }
}

#[cfg(test)]
#[path = "reviewed_application_tests.rs"]
mod tests;
