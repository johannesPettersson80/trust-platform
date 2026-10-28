//! The identity of one experiment, read by its identifier alone (`identificationReconcile.ts`; specification 35, E11.4 and
//! E11.5; annex E5.6). A read never writes: a start whose outcome is unknown is reconciled here, and is never sent again.
//! The runtime calls that make the read are in `trust-lsp`; this module decides what the read means.

use serde_json::Value;

use crate::experiment::{classify_control_experiment_status, ControlExperimentRuntimePhase};

/// The refusal code of a status read for an identifier the runtime does not hold (annex E5.6).
pub const EXPERIMENT_NOT_FOUND: &str = "experiment_not_found";

/// What a read of an experiment's status gave: the runtime's status record, its refusal with the code and text it
/// carries (E0.4), or no answer that can be read at all (a transport failure or an answer out of protocol).
#[derive(Debug, Clone, PartialEq)]
pub enum StatusRead {
    /// The runtime answered with a status record, which is checked by [`experiment_identity`].
    Answer(Value),
    /// The runtime refused the request; `code` names the refusal when the runtime gave one.
    Refused {
        /// The refusal code of annex E0.4, when the runtime gave one.
        code: Option<String>,
        /// The runtime's text, which names the failed check.
        text: String,
    },
    /// No answer could be read: the connection failed, or the answer is not a control response.
    Unreadable(String),
}

/// One experiment, as its identifier alone shows it (`ExperimentIdentity`).
#[derive(Debug, Clone, PartialEq)]
pub enum ExperimentIdentity {
    /// Running, or ended with its restoration still pending: the runtime still owns the experiment.
    Running {
        /// The status record the runtime answered with.
        state: Value,
    },
    /// Ended, with its restoration confirmed or failed: nothing is left running.
    Terminal {
        /// The status record the runtime answered with.
        state: Value,
    },
    /// The runtime holds no experiment with this identifier, so it never started.
    Absent {
        /// The runtime's text for the refusal.
        reason: String,
    },
    /// No terminal status can be read, so the operation stays unresolved (E11.5).
    Unresolved {
        /// The sentence the engineer reads.
        reason: String,
    },
}

/// Decides what a status read of `experiment_id` means (`readExperimentIdentity`, E11.4, E11.5). An identifier the runtime
/// does not hold is absent. Any other refusal, or an unreadable answer, is unresolved: the status could not be read. An
/// answer outside the runtime's vocabulary is unresolved too. Otherwise the answer is running or terminal by its phase.
#[must_use]
pub fn experiment_identity(experiment_id: &str, read: StatusRead) -> ExperimentIdentity {
    match read {
        StatusRead::Refused {
            code: Some(code),
            text,
        } if code == EXPERIMENT_NOT_FOUND => ExperimentIdentity::Absent { reason: text },
        StatusRead::Refused { text, .. } | StatusRead::Unreadable(text) => {
            ExperimentIdentity::Unresolved {
                reason: format!(
                    "The status of experiment {experiment_id} could not be read: {text}."
                ),
            }
        }
        StatusRead::Answer(state) => match classify_control_experiment_status(&state) {
            ControlExperimentRuntimePhase::Invalid => ExperimentIdentity::Unresolved {
                reason: "The runtime returned an invalid experiment status.".to_string(),
            },
            ControlExperimentRuntimePhase::Terminal => ExperimentIdentity::Terminal { state },
            ControlExperimentRuntimePhase::Running
            | ControlExperimentRuntimePhase::AwaitingRestoration => {
                ExperimentIdentity::Running { state }
            }
        },
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{experiment_identity, ExperimentIdentity, StatusRead};

    #[test]
    fn a_running_or_restoring_experiment_is_running_and_an_ended_one_is_terminal() {
        let running =
            json!({ "experiment_id": "e-1", "status": "running", "restoration": "pending" });
        assert_eq!(
            experiment_identity("e-1", StatusRead::Answer(running.clone())),
            ExperimentIdentity::Running { state: running }
        );
        let restoring = json!({ "status": "completed", "restoration": "pending" });
        assert_eq!(
            experiment_identity("e-1", StatusRead::Answer(restoring.clone())),
            ExperimentIdentity::Running { state: restoring }
        );
        let ended = json!({ "status": "completed", "restoration": "restored", "stop_reason": "duration_complete" });
        assert_eq!(
            experiment_identity("e-1", StatusRead::Answer(ended.clone())),
            ExperimentIdentity::Terminal { state: ended }
        );
    }

    #[test]
    fn an_answer_outside_the_vocabulary_is_unresolved_never_terminal() {
        let identity = experiment_identity(
            "e-1",
            StatusRead::Answer(json!({ "status": "paused", "restoration": "pending" })),
        );
        assert_eq!(
            identity,
            ExperimentIdentity::Unresolved {
                reason: "The runtime returned an invalid experiment status.".to_string()
            }
        );
    }

    #[test]
    fn an_unknown_identifier_is_absent_and_keeps_the_runtime_text() {
        let identity = experiment_identity(
            "e-1",
            StatusRead::Refused {
                code: Some("experiment_not_found".to_string()),
                text: "No experiment holds this identifier.".to_string(),
            },
        );
        assert_eq!(
            identity,
            ExperimentIdentity::Absent {
                reason: "No experiment holds this identifier.".to_string()
            }
        );
    }

    #[test]
    fn any_other_refusal_or_an_unreadable_answer_is_unresolved_with_the_reason() {
        let refused = experiment_identity(
            "e-1",
            StatusRead::Refused {
                code: Some("stale_scan".to_string()),
                text: "The program has not finished its first scan.".to_string(),
            },
        );
        let unreadable =
            experiment_identity("e-1", StatusRead::Unreadable("transport lost".to_string()));
        for identity in [refused, unreadable] {
            let ExperimentIdentity::Unresolved { reason } = identity else {
                panic!("an unreadable status stays unresolved");
            };
            assert!(
                reason.starts_with("The status of experiment e-1 could not be read: "),
                "{reason}"
            );
        }
        let without_code = experiment_identity(
            "e-1",
            StatusRead::Refused {
                code: None,
                text: "No code.".to_string(),
            },
        );
        assert!(matches!(
            without_code,
            ExperimentIdentity::Unresolved { .. }
        ));
    }
}
