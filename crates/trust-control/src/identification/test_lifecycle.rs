//! The decisions of Control Studio's Test lifecycle (`testExperimentLifecycle.ts`; specification 35, A8.5 and A8.6; annex
//! E11.4 and E11.5). Each function decides what a Test does with a runtime answer, and which sentence it posts. None does
//! I/O or calls the runtime: the flow that carries each decision out is in `trust-lsp`
//! (`control_studio::workflow::test_experiment_lifecycle`).

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::experiment::{
    classify_control_experiment_status, ControlExperimentPlan, ControlExperimentRuntimePhase,
    RecordingRecoveryOwner,
};
use crate::failure::Failure;
use crate::identification::reconcile::ExperimentIdentity;

/// The phases a Test posts to its page: the `phase` of `post(phase, payload)` in `testExperimentLifecycle.ts`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PostedPhase {
    /// The experiment runs, and the Test follows it.
    Running,
    /// The experiment has ended and its restoration is pending.
    Restoring,
    /// The experiment has ended and its restoration is settled. The payload is its conclusion.
    Terminal,
    /// The Test failed. The payload is `{ "reason": text }`.
    Failed,
}

/// The reason a Test posts when the runtime's status answer is outside its vocabulary.
pub const INVALID_STATE_TEXT: &str = "The runtime returned an invalid experiment state.";

/// The reason a Test posts when its recording owner is journalled but its reviewed plan is not.
pub const PLAN_MISSING_TEXT: &str =
    "Test recording recovery is unavailable because the reviewed plan is missing.";

/// What a Test does with a status answer, in the order of `refreshTestExperiment`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusStep {
    /// The answer is outside the runtime's vocabulary: the Test fails with [`INVALID_STATE_TEXT`].
    Invalid,
    /// The experiment runs or awaits its restoration: the follow is scheduled, and the Test posts `phase` with the answer.
    Follow {
        /// `running`, or `restoring` for an experiment awaiting its restoration.
        phase: PostedPhase,
    },
    /// The experiment has ended: the answer is observed and concluded, and the Test posts `terminal` with the conclusion.
    /// The journal is cleared when `clears_journal` is set (no recording is active); otherwise the follow is scheduled.
    Conclude {
        /// Whether the journal is cleared rather than followed again.
        clears_journal: bool,
    },
}

/// What a Test does with an experiment that its identifier reads (`reconcileByIdentity`; annex E11.4 and E11.5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdentityStep {
    /// The runtime still owns the experiment: the follow is scheduled, and the Test posts `phase` with the record.
    Attach {
        /// `running`, or `restoring` for an experiment awaiting its restoration.
        phase: PostedPhase,
        /// The status record the runtime answered with.
        state: Value,
    },
    /// The experiment has ended: its record is observed and concluded, the journal is cleared, and the Test posts `terminal`.
    Conclude {
        /// The status record the runtime answered with.
        state: Value,
    },
    /// The start is still landing and not yet answered, so the journal stays and nothing is posted (A8.5).
    KeepJournal,
    /// The runtime holds no experiment with this identifier, so nothing was applied: the journal is cleared and the Test fails.
    ClearAndFail {
        /// The sentence the Test posts, from [`absent_text`].
        reason: String,
    },
    /// No status can be read: the Test fails, and the journal stays for the next reconciliation.
    Fail {
        /// The sentence the Test posts, from [`unresolved_text`].
        reason: String,
    },
}

/// What a Test does before its status read when no recording is active (`refreshTestExperiment`).
#[derive(Debug, Clone, Copy)]
pub enum RecoveryStep<'a> {
    /// Journalled with its identity only, before any recording owner exists: the runtime's status decides (A8.5, E11.4).
    ByIdentity,
    /// The recording owner is journalled but not the reviewed plan: the Test fails with [`PLAN_MISSING_TEXT`].
    PlanMissing,
    /// The recording owner is recovered, and then the plan is recorded as recovered.
    Recover {
        /// The recording owner to recover.
        owner: &'a RecordingRecoveryOwner,
        /// The reviewed plan of the experiment the recording runs with.
        plan: &'a ControlExperimentPlan,
    },
}

/// The step a status answer takes (`refreshTestExperiment`). An answer outside the runtime's vocabulary fails. A running
/// experiment is followed and posted as `running`, and one awaiting its restoration as `restoring`. An ended experiment is
/// concluded, and its journal is cleared unless a recording is still active.
#[must_use]
pub fn status_step(value: &Value, recording_active: bool) -> StatusStep {
    match classify_control_experiment_status(value) {
        ControlExperimentRuntimePhase::Invalid => StatusStep::Invalid,
        ControlExperimentRuntimePhase::Running => StatusStep::Follow {
            phase: PostedPhase::Running,
        },
        ControlExperimentRuntimePhase::AwaitingRestoration => StatusStep::Follow {
            phase: PostedPhase::Restoring,
        },
        ControlExperimentRuntimePhase::Terminal => StatusStep::Conclude {
            clears_journal: !recording_active,
        },
    }
}

/// The step an experiment read by its identifier takes (`reconcileByIdentity`; annex E11.4 and E11.5). `start_landing` says
/// whether the identifier's start is still landing: journalled, and not yet answered or failed (A8.5). An absent identifier
/// then keeps its journal, because an unanswered start does not yet mean that it never started.
#[must_use]
pub fn identity_step(identity: ExperimentIdentity, start_landing: bool) -> IdentityStep {
    match identity {
        ExperimentIdentity::Running { state } => {
            let phase = running_phase(&state);
            IdentityStep::Attach { phase, state }
        }
        ExperimentIdentity::Terminal { state } => IdentityStep::Conclude { state },
        ExperimentIdentity::Absent { .. } if start_landing => IdentityStep::KeepJournal,
        ExperimentIdentity::Absent { reason } => IdentityStep::ClearAndFail {
            reason: absent_text(&reason),
        },
        ExperimentIdentity::Unresolved { reason } => IdentityStep::Fail {
            reason: unresolved_text(&reason),
        },
    }
}

/// The step a Test takes before its status read when no recording is active (`refreshTestExperiment`). A Test journalled
/// with its identity only, before any recording owner exists, is reconciled by its status. A recording owner needs its
/// reviewed plan before it is recovered.
#[must_use]
pub fn recovery_step<'a>(
    owner: Option<&'a RecordingRecoveryOwner>,
    plan: Option<&'a ControlExperimentPlan>,
) -> RecoveryStep<'a> {
    match (owner, plan) {
        (None, _) => RecoveryStep::ByIdentity,
        (Some(_), None) => RecoveryStep::PlanMissing,
        (Some(owner), Some(plan)) => RecoveryStep::Recover { owner, plan },
    }
}

/// The sentence a Test posts for an identity the runtime does not hold. `reason` is the runtime's text for the refusal.
#[must_use]
pub fn absent_text(reason: &str) -> String {
    format!(
        "The runtime holds no experiment with this identity, so the Test never started and nothing was applied. {reason}"
    )
}

/// The sentence a Test posts for an identity whose status cannot be read. `reason` is the reading that failed.
#[must_use]
pub fn unresolved_text(reason: &str) -> String {
    format!("{reason} The start is not sent again; reopening reconciles it.")
}

/// The sentence a Test posts when its recording owner cannot be recovered. `failure` is the failure of the recovery.
#[must_use]
pub fn recovery_unconfirmed_text(failure: &Failure) -> String {
    let text = failure_text(failure);
    format!("Test recording recovery remains unconfirmed: {text}")
}

/// The whole text of a failure: its detail when the text says more than its message, otherwise its message.
#[must_use]
pub fn failure_text(failure: &Failure) -> String {
    failure
        .detail
        .as_deref()
        .unwrap_or(failure.message.as_str())
        .to_string()
}

/// The phase a running record posts: `restoring` once the experiment has ended and awaits its restoration.
fn running_phase(state: &Value) -> PostedPhase {
    let phase = classify_control_experiment_status(state);
    if phase == ControlExperimentRuntimePhase::AwaitingRestoration {
        PostedPhase::Restoring
    } else {
        PostedPhase::Running
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::experiment::{ControlEvidenceSource, RecordingRecoveryOwner};
    use crate::failure::{Cause, Failure, NextAction};
    use crate::identification::reconcile::ExperimentIdentity;

    use super::{
        absent_text, failure_text, identity_step, recovery_step, recovery_unconfirmed_text,
        status_step, unresolved_text, IdentityStep, PostedPhase, RecoveryStep, StatusStep,
        INVALID_STATE_TEXT, PLAN_MISSING_TEXT,
    };

    #[test]
    fn a_status_is_followed_while_it_runs_and_concluded_once_it_has_ended() {
        let running = json!({ "status": "running", "restoration": "pending" });
        assert_eq!(
            status_step(&running, true),
            StatusStep::Follow {
                phase: PostedPhase::Running
            }
        );
        let restoring = json!({ "status": "completed", "restoration": "pending" });
        assert_eq!(
            status_step(&restoring, true),
            StatusStep::Follow {
                phase: PostedPhase::Restoring
            }
        );
        let ended = json!({ "status": "completed", "restoration": "restored" });
        assert_eq!(
            status_step(&ended, true),
            StatusStep::Conclude {
                clears_journal: false
            },
            "an active recording keeps the follow"
        );
        assert_eq!(
            status_step(&ended, false),
            StatusStep::Conclude {
                clears_journal: true
            },
            "without an active recording the journal clears"
        );
    }

    #[test]
    fn an_answer_outside_the_vocabulary_is_invalid() {
        let paused = json!({ "status": "paused", "restoration": "pending" });
        assert_eq!(status_step(&paused, false), StatusStep::Invalid);
        assert_eq!(
            INVALID_STATE_TEXT,
            "The runtime returned an invalid experiment state."
        );
    }

    #[test]
    fn an_identity_is_attached_concluded_kept_or_failed_by_what_it_reads() {
        let running = json!({ "status": "running", "restoration": "pending" });
        assert_eq!(
            identity_step(
                ExperimentIdentity::Running {
                    state: running.clone()
                },
                false
            ),
            IdentityStep::Attach {
                phase: PostedPhase::Running,
                state: running
            }
        );
        let restoring = json!({ "status": "failed", "restoration": "pending" });
        assert_eq!(
            identity_step(
                ExperimentIdentity::Running {
                    state: restoring.clone()
                },
                false
            ),
            IdentityStep::Attach {
                phase: PostedPhase::Restoring,
                state: restoring
            }
        );
        let ended = json!({ "status": "aborted", "restoration": "restored" });
        assert_eq!(
            identity_step(
                ExperimentIdentity::Terminal {
                    state: ended.clone()
                },
                false
            ),
            IdentityStep::Conclude { state: ended }
        );
        let absent = || ExperimentIdentity::Absent {
            reason: "No experiment holds this identifier.".to_string(),
        };
        assert_eq!(identity_step(absent(), true), IdentityStep::KeepJournal);
        assert_eq!(
            identity_step(absent(), false),
            IdentityStep::ClearAndFail {
                reason: absent_text("No experiment holds this identifier.")
            }
        );
        let unresolved = ExperimentIdentity::Unresolved {
            reason: "The status of experiment e-1 could not be read: transport lost.".to_string(),
        };
        assert_eq!(
            identity_step(unresolved, false),
            IdentityStep::Fail {
                reason: "The status of experiment e-1 could not be read: transport lost. The start is not sent again; reopening reconciles it.".to_string()
            }
        );
    }

    #[test]
    fn the_sentences_of_the_lifecycle_are_exact() {
        assert_eq!(
            PLAN_MISSING_TEXT,
            "Test recording recovery is unavailable because the reviewed plan is missing."
        );
        assert_eq!(
            absent_text("No experiment holds this identifier."),
            "The runtime holds no experiment with this identity, so the Test never started and nothing was applied. No experiment holds this identifier."
        );
        assert_eq!(
            unresolved_text("x."),
            "x. The start is not sent again; reopening reconciles it."
        );
        let failure = Failure::new(
            "Recording owner is busy. Retry later.",
            Cause::Runtime,
            NextAction::Wait,
        );
        assert_eq!(
            failure_text(&failure),
            "Recording owner is busy. Retry later."
        );
        assert_eq!(
            recovery_unconfirmed_text(&failure),
            "Test recording recovery remains unconfirmed: Recording owner is busy. Retry later."
        );
    }

    #[test]
    fn a_test_without_a_recording_owner_is_reconciled_by_identity_and_an_owner_needs_its_plan() {
        let owner = RecordingRecoveryOwner {
            schema_version: 1,
            recording_id: "recording-1".to_string(),
            project_id: "project".to_string(),
            runtime_id: "runtime".to_string(),
            loop_id: "tank".to_string(),
            loop_revision: "rev-1".to_string(),
            source: ControlEvidenceSource::Simulation,
            program_revision: None,
            start_acknowledgement_unconfirmed: None,
            bindings: Vec::new(),
        };
        assert!(matches!(
            recovery_step(None, None),
            RecoveryStep::ByIdentity
        ));
        assert!(matches!(
            recovery_step(Some(&owner), None),
            RecoveryStep::PlanMissing
        ));
    }
}
