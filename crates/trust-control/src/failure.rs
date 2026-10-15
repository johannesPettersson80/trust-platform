//! Failures and their next actions (specification 35, A11). A failure carries its cause and one
//! typed next action, chosen where the failure is made, never read back from its message text.

use schemars::JsonSchema;
use serde::Serialize;

/// Why a step failed (A11.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Cause {
    /// The model or the recorded data does not support the step.
    ModelOrData,
    /// A setting, limit or requirement stops the step.
    Settings,
    /// The selected controller cannot do what is asked.
    ControllerCapability,
    /// The requirements cannot be met with this plant.
    TaskInfeasible,
    /// The project's implementation does not match its loop setup.
    Implementation,
    /// The runtime refused, or could not be reached.
    Runtime,
    /// A physical target needs approval.
    Approval,
    /// No other cause applies.
    Unclassified,
}

/// The one action a failure offers (A11.2). `Wait` and `None` offer no button.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum NextAction {
    /// Connect a runtime.
    ConnectRuntime,
    /// Open the loop setup.
    OpenLoopSetup,
    /// Open the limits.
    OpenLimits,
    /// Open the requirements.
    OpenRequirements,
    /// Open the channels.
    OpenChannels,
    /// Choose another method.
    ChooseMethod,
    /// Identify the plant.
    Identify,
    /// Tune the controller.
    Tune,
    /// Validate the controller.
    Validate,
    /// Approve the physical target.
    Approve,
    /// Restore the previous settings.
    Restore,
    /// Wait: the condition clears by itself.
    Wait,
    /// Nothing to do from here.
    None,
}

impl NextAction {
    /// The words on the action's button; empty for `Wait` and `None`.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::ConnectRuntime => "Connect runtime…",
            Self::OpenLoopSetup => "Loop setup…",
            Self::OpenLimits => "Limits…",
            Self::OpenRequirements => "Requirements…",
            Self::OpenChannels => "Channels…",
            Self::ChooseMethod => "Choose another method",
            Self::Identify => "Identify plant",
            Self::Tune => "Tune controller",
            Self::Validate => "Validate controller",
            Self::Approve => "Approve…",
            Self::Restore => "Restore previous settings",
            Self::Wait | Self::None => "",
        }
    }
}

/// A failed step as the engineer sees it: the message, a longer detail when it says more, the
/// cause, and the next action (A11.1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema, thiserror::Error)]
#[serde(rename_all = "camelCase")]
#[error("{message}")]
pub struct Failure {
    /// The first sentence: what went wrong.
    pub message: String,
    /// The whole text, when it says more than the message.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    /// Why it went wrong.
    pub cause: Cause,
    /// What to do next.
    pub next_action: NextAction,
    /// The words of the next action's button, as [`NextAction::label`] gives them, so that a front end shows the engine's words
    /// (one source of the words); absent for `wait` and `none`.
    #[serde(skip_serializing_if = "Option::is_none")]
    next_action_label: Option<&'static str>,
    /// The key of the form field that the failure refuses, such as `desiredPoles` or `stateWeights` (D-B12), when the refusal
    /// concerns one field of a form; absent otherwise. The page shows the sentence under that field.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
}

impl Failure {
    /// A failure whose message is `text`'s first sentence; the whole text is its detail when longer.
    #[must_use]
    pub fn new(text: &str, cause: Cause, next_action: NextAction) -> Self {
        let full = text.trim();
        let message = first_sentence(full);
        let detail = (full != message).then(|| full.to_string());
        Self {
            message,
            detail,
            cause,
            next_action,
            next_action_label: label_of(next_action),
            field: None,
        }
    }

    /// A failure with exactly these parts, as a stored failure is read back: the message and the detail are kept as they were
    /// written, without splitting the text again.
    #[must_use]
    pub fn from_parts(
        message: String,
        detail: Option<String>,
        cause: Cause,
        next_action: NextAction,
    ) -> Self {
        Self {
            message,
            detail,
            cause,
            next_action,
            next_action_label: label_of(next_action),
            field: None,
        }
    }

    /// The same failure, naming the key of the form field it refuses (D-B12). The page shows its sentence under that field.
    #[must_use]
    pub fn with_field(mut self, field: &str) -> Self {
        self.field = Some(field.to_owned());
        self
    }

    /// The failure for a runtime error `code` (runtime protocol annex, E0.4) with the runtime's own
    /// `text`: the code decides the next action, and the text is kept whole.
    #[must_use]
    pub fn from_runtime(code: &str, text: &str) -> Self {
        let (cause, next_action) = match code {
            "experiment_active"
            | "transaction_pending"
            | "controller_write_pending"
            | "experiment_state_unavailable"
            | "stale_scan" => (Cause::Runtime, NextAction::Wait),
            "physical_approval_required" => (Cause::Approval, NextAction::Approve),
            "invalid_target"
            | "invalid_address"
            | "incompatible_binding"
            | "invalid_experiment"
            | "controller_unavailable" => (Cause::Runtime, NextAction::OpenLoopSetup),
            "outside_limits" => (Cause::Runtime, NextAction::OpenLimits),
            _ => (Cause::Runtime, NextAction::None),
        };
        if next_action == NextAction::Wait {
            return Self {
                message: "Wait for the running operation to finish.".to_string(),
                detail: Some(text.trim().to_string()),
                cause,
                next_action,
                next_action_label: label_of(next_action),
                field: None,
            };
        }
        Self::new(text, cause, next_action)
    }

    /// The runtime could not be reached or stopped answering; `text` says how.
    #[must_use]
    pub fn unreachable(text: &str) -> Self {
        Self {
            message: "Control Studio cannot reach the runtime.".to_string(),
            detail: Some(text.trim().to_string()),
            cause: Cause::Runtime,
            next_action: NextAction::ConnectRuntime,
            next_action_label: label_of(NextAction::ConnectRuntime),
            field: None,
        }
    }

    /// No newer runtime value arrived within the connection-loss timeout (A3.4).
    #[must_use]
    pub fn stopped_sending() -> Self {
        Self::new(
            "The runtime stopped sending values.",
            Cause::Runtime,
            NextAction::ConnectRuntime,
        )
    }

    /// A request built from the loop setup fails Control Studio's own checks, so it is not sent
    /// (annex E11.8).
    #[must_use]
    pub fn invalid_request(text: &str) -> Self {
        Self::new(text, Cause::Implementation, NextAction::OpenLoopSetup)
    }

    /// The runtime's answer to `operation` does not have the form of the protocol annex. Such an
    /// answer is never evidence and never confirms anything (annex E11.8).
    #[must_use]
    pub fn invalid_response(operation: &str) -> Self {
        Self::new(
            &format!("The runtime's answer to {operation} does not match the protocol."),
            Cause::Runtime,
            NextAction::None,
        )
    }
}

/// The label a failure carries for `next_action`: its button's words, or none for an action without a button.
fn label_of(next_action: NextAction) -> Option<&'static str> {
    Some(next_action.label()).filter(|label| !label.is_empty())
}

/// The text up to the first full stop, question or exclamation mark that ends a word, so a value
/// such as "0.21 °C" stays whole.
fn first_sentence(text: &str) -> String {
    let mut chars = text.char_indices().peekable();
    while let Some((index, character)) = chars.next() {
        if matches!(character, '.' | '!' | '?')
            && chars.peek().is_none_or(|(_, next)| next.is_whitespace())
        {
            return text[..=index].trim().to_string();
        }
    }
    let sentence = text.trim();
    if sentence.is_empty() {
        "Control Studio could not complete this step.".to_string()
    } else {
        sentence.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_message_is_the_first_sentence_and_the_detail_the_whole_text() {
        let failure = Failure::new(
            "The error is 0.21 °C. It must be below 0.1 °C.",
            Cause::Settings,
            NextAction::Tune,
        );
        assert_eq!(failure.message, "The error is 0.21 °C.");
        assert_eq!(
            failure.detail.as_deref(),
            Some("The error is 0.21 °C. It must be below 0.1 °C.")
        );
        let single = Failure::new("No model.", Cause::ModelOrData, NextAction::Identify);
        assert_eq!(single.detail, None, "a detail only when it says more");
        assert_eq!(
            Failure::new("  ", Cause::Unclassified, NextAction::None).message,
            "Control Studio could not complete this step."
        );
    }

    #[test]
    fn a_runtime_code_decides_the_next_action() {
        assert_eq!(
            Failure::from_runtime("stale_scan", "The program has not finished its first scan.")
                .next_action,
            NextAction::Wait
        );
        assert_eq!(
            Failure::from_runtime("outside_limits", "x").next_action,
            NextAction::OpenLimits
        );
        assert_eq!(
            Failure::from_runtime("invalid_binding_code_not_listed", "x").next_action,
            NextAction::None
        );
        let approval = Failure::from_runtime("physical_approval_required", "Approve the target.");
        assert_eq!(
            (approval.cause, approval.next_action),
            (Cause::Approval, NextAction::Approve)
        );
        let wait = Failure::from_runtime("experiment_active", "Experiment e1 is running.");
        assert_eq!(wait.message, "Wait for the running operation to finish.");
        assert_eq!(wait.detail.as_deref(), Some("Experiment e1 is running."));
    }

    #[test]
    fn the_runtime_codes_of_the_annex_map_to_their_typed_actions_by_code_and_never_by_text() {
        // The TypeScript table of `control-studio-lifecycle.test.ts`. Every code gets the same text, so only the code decides.
        let text = "A runtime message.";
        let action = |code: &str| Failure::from_runtime(code, text).next_action;
        for code in [
            "experiment_active",
            "transaction_pending",
            "controller_write_pending",
            "experiment_state_unavailable",
        ] {
            assert_eq!(action(code), NextAction::Wait, "{code}");
        }
        for code in [
            "invalid_target",
            "invalid_address",
            "incompatible_binding",
            "invalid_experiment",
            "controller_unavailable",
        ] {
            assert_eq!(action(code), NextAction::OpenLoopSetup, "{code}");
        }
        for code in ["job_active", "out_of_limits", "io_unavailable"] {
            assert_eq!(action(code), NextAction::None, "{code}");
        }
    }

    #[test]
    fn every_action_with_a_button_has_a_label() {
        assert_eq!(NextAction::Restore.label(), "Restore previous settings");
        assert_eq!(NextAction::Wait.label(), "");
        assert_eq!(NextAction::None.label(), "");
    }

    #[test]
    fn a_failure_serializes_with_its_typed_action() {
        let json = serde_json::to_value(Failure::unreachable("connection refused")).expect("json");
        assert_eq!(json["nextAction"], "connect-runtime");
        assert_eq!(json["nextActionLabel"], "Connect runtime…");
        assert_eq!(json["cause"], "runtime");
        assert_eq!(json["detail"], "connection refused");
        let waiting =
            serde_json::to_value(Failure::from_runtime("stale_scan", "busy")).expect("json");
        assert!(
            waiting.get("nextActionLabel").is_none(),
            "an action without a button carries no label: {waiting}"
        );
    }
}
