//! The parameter session of one loop (specification 35, B6.4; annex E7.3, E7.5): the one review it keeps for a single use, the
//! binding it holds, and the request generation that decides which replies are still current. From `parameterSession.ts`; the
//! client's rules are in `parameter_application`, and the messages' scope from `messages.ts`.
//!
//! The session decides a request before any runtime call. The connection, the runtime calls, the posting of each reply and the
//! choice of the selected analysis values are transport: the caller performs them in the order that each decision states.

use std::collections::BTreeMap;

use serde_json::{Map, Value};

use crate::failure::{Cause, Failure, NextAction};
use crate::json;

use super::parameter_application::{
    review_input, ParameterReview, ParameterSource, RuntimeBinding,
};
use super::texts;

/// The longest review text of a session, in UTF-16 units (`payload.json.length`; 64 kB).
const MAXIMUM_REVIEW_UNITS: usize = 65_536;

/// The loop message scope that every session message is checked against (`LoopMessageScope` of `messages.ts`; moves with V4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageScope {
    /// The project.
    pub project_id: String,
    /// The editor session.
    pub editor_session_id: String,
    /// The loop.
    pub loop_id: String,
    /// The loop revision that the editor shows.
    pub loop_revision: String,
}

/// What a session message asks for (`control-studio.parameters.<action>`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionAction {
    /// Read the record of the runtime.
    Inspect,
    /// Copy the values of the selected model candidate into a draft.
    SelectedAnalysis,
    /// Review the values that the JSON names.
    Review,
    /// Review a rollback to the retained previous values.
    ReviewRollback,
    /// Apply the review that the message names.
    Apply,
    /// Drop the review, because the values were edited.
    Invalidate,
}

impl SessionAction {
    /// The action named by the suffix of a `control-studio.parameters.` message kind.
    #[must_use]
    pub fn from_word(word: &str) -> Option<Self> {
        match word {
            "inspect" => Some(Self::Inspect),
            "selected-analysis" => Some(Self::SelectedAnalysis),
            "review" => Some(Self::Review),
            "review-rollback" => Some(Self::ReviewRollback),
            "apply" => Some(Self::Apply),
            "invalidate" => Some(Self::Invalidate),
            _ => None,
        }
    }
}

/// The session's view of a message (`ParameterSession.handle`, first check): the envelope names the scope's project, editor
/// session, loop and loop revision, its payload is an object, and its kind is one of the six actions. A message for another loop
/// revision is not this session's, so it is ignored and never answered.
#[must_use]
pub fn session_action(message: &Value, scope: &MessageScope) -> Option<SessionAction> {
    let object = message.as_object()?;
    let names_scope = field(object, "projectId") == Some(scope.project_id.as_str())
        && field(object, "editorSessionId") == Some(scope.editor_session_id.as_str())
        && field(object, "loopId") == Some(scope.loop_id.as_str())
        && field(object, "expectedLoopRevision") == Some(scope.loop_revision.as_str());
    let well_formed = object.get("schemaVersion").and_then(Value::as_u64) == Some(1)
        && field(object, "messageId").is_some()
        && object.get("payload").is_some_and(Value::is_object);
    if !names_scope || !well_formed {
        return None;
    }
    let kind = field(object, "kind")?;
    SessionAction::from_word(kind.strip_prefix("control-studio.parameters.")?)
}

/// A string field of a message object.
fn field<'a>(object: &'a Map<String, Value>, key: &str) -> Option<&'a str> {
    object.get(key).and_then(Value::as_str)
}

/// The review that a session keeps for one use: the identifier that the webview quotes, the endpoint that the review was made
/// on, and the review (`Review` of `parameterSession.ts`).
#[derive(Debug, PartialEq)]
pub struct RetainedReview {
    /// The identifier that the webview quotes to apply the review.
    pub id: String,
    /// The endpoint that the review was made on; its write goes there and nowhere else.
    pub source: ParameterSource,
    /// The review.
    pub review: ParameterReview,
}

/// What a session does with a request before any runtime call (`ParameterSession.handle`).
#[derive(Debug)]
pub enum SessionStep {
    /// Invalidate: the review is dropped, and the webview is told that the values changed.
    Edited,
    /// An Apply or a rollback may start with this review, which is no longer retained. The session is applying until
    /// `finish_apply`.
    Apply(Box<RetainedReview>),
    /// The request continues after its connection: inspect, review, review a rollback, or copy the selected analysis.
    Proceed(SessionAction),
}

/// The parameter session of one loop (`ParameterSession`): the binding it holds, the one review it retains for a single use,
/// and the request generation that decides which replies are still current.
#[derive(Debug)]
pub struct ParameterSession {
    binding: RuntimeBinding,
    review: Option<RetainedReview>,
    generation: u64,
    applying: bool,
}

impl ParameterSession {
    /// A session for `binding`, with no review and nothing applying.
    #[must_use]
    pub fn new(binding: RuntimeBinding) -> Self {
        Self {
            binding,
            review: None,
            generation: 0,
            applying: false,
        }
    }

    /// The binding the session holds: its parameters are the only ones a review may write.
    #[must_use]
    pub fn binding(&self) -> &RuntimeBinding {
        &self.binding
    }

    /// Starts a request and returns its generation (`++this.generation`). A reply is posted only while its generation is the
    /// newest, so an answer to an older request is discarded.
    #[must_use]
    pub fn begin_request(&mut self) -> u64 {
        self.generation += 1;
        self.generation
    }

    /// Whether a request still owns the newest generation.
    #[must_use]
    pub fn is_current(&self, generation: u64) -> bool {
        self.generation == generation
    }

    /// Decides one request before any runtime call (`handle`). An invalidation drops the review. A transaction that awaits its
    /// confirmation refuses every other request. An Apply takes the retained review, which must be the one that the message names.
    /// Every other request drops the retained review, because a review is valid for one use only.
    ///
    /// # Errors
    /// A transaction is awaiting its confirmation, or the named review is no longer retained (the values changed or it expired).
    pub fn admit(
        &mut self,
        action: SessionAction,
        review_id: Option<&str>,
    ) -> Result<SessionStep, Failure> {
        if action == SessionAction::Invalidate {
            self.review = None;
            return Ok(SessionStep::Edited);
        }
        if self.applying {
            return Err(Failure::new(
                &texts::parameter_applying(),
                Cause::Runtime,
                NextAction::Wait,
            ));
        }
        if action != SessionAction::Apply {
            self.review = None;
            return Ok(SessionStep::Proceed(action));
        }
        match self.review.take() {
            Some(retained) if review_id == Some(retained.id.as_str()) => {
                self.applying = true;
                Ok(SessionStep::Apply(Box::new(retained)))
            }
            _ => Err(Failure::new(
                &texts::parameter_review_expired(),
                Cause::Settings,
                NextAction::None,
            )),
        }
    }

    /// The values that a `review` message names, after its connection (`review`): the JSON text of at most 64 kB, parsed as an
    /// object of numbers, and checked and rounded against the held binding (`review_input`).
    ///
    /// # Errors
    /// The text is missing or longer than 64 kB, is not JSON, is not an object of numbers, or is outside the binding.
    pub fn review_values(
        &self,
        review_json: Option<&str>,
    ) -> Result<BTreeMap<String, f64>, Failure> {
        let text = review_json
            .filter(|text| text.encode_utf16().count() <= MAXIMUM_REVIEW_UNITS)
            .ok_or_else(|| {
                Failure::new(
                    &texts::parameter_json_too_long(),
                    Cause::Settings,
                    NextAction::None,
                )
            })?;
        let parsed = json::parse_text(text).map_err(|_| {
            Failure::new(
                &texts::parameter_json_invalid(),
                Cause::Settings,
                NextAction::None,
            )
        })?;
        let input: BTreeMap<String, f64> = json::decode_value(parsed).map_err(|_| {
            Failure::new(
                &texts::parameter_review_values_invalid(),
                Cause::Settings,
                NextAction::None,
            )
        })?;
        review_input(&self.binding, &input)
    }

    /// Keeps `retained` for a later Apply, unless its request is no longer current. A later review replaces it. Returns whether
    /// the review was kept.
    #[must_use]
    pub fn retain_if_current(&mut self, generation: u64, retained: RetainedReview) -> bool {
        let current = self.is_current(generation);
        if current {
            self.review = Some(retained);
        }
        current
    }

    /// Whether a transaction is awaiting its confirmation.
    #[must_use]
    pub fn is_applying(&self) -> bool {
        self.applying
    }

    /// Ends the applying state of an Apply that `admit` started (`finally`).
    pub fn finish_apply(&mut self) {
        self.applying = false;
    }

    /// Ends the session: its replies stop, and its review is dropped (`dispose`).
    pub fn dispose(&mut self) {
        self.generation += 1;
        self.review = None;
    }
}

/// The failure of a selected analysis that has no values to copy: a calculated candidate with an explicit ST parameter mapping
/// comes first.
#[must_use]
pub fn selected_analysis_missing() -> Failure {
    Failure::new(
        &texts::parameter_selected_analysis_missing(),
        Cause::Settings,
        NextAction::Tune,
    )
}

/// The reason of an applied reply: a rollback reports the restored values, and an Apply reports the reviewed values.
#[must_use]
pub fn applied_reason(rollback: bool) -> String {
    if rollback {
        texts::parameter_applied_rollback()
    } else {
        texts::parameter_applied_apply()
    }
}

#[cfg(test)]
#[path = "parameter_session_tests.rs"]
mod tests;
