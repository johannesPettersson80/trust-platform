//! The ending of a Control.PID run and the result it records (`pidCommissioning.ts`: `execute`'s catch, its finally block and its return;
//! specification 35, B6.3, B6.7, A6.3, A8.5): how the run ends after its attempt, whether the previous settings are confirmed back,
//! whether a Restore is awaited, the journal stage the outcome is written at, and the stored result. The attempt is decided in
//! `pid_commissioning` (the judgement of the response and the steps of the run), and the recovery in `pid_recovery`.
//!
//! The sentences of a Control.PID ending are its own (`pid_commission_*`), except those that the proportional run words identically, which
//! are shared (`prop_commission_*`, `recovery_*`). Left out, as transport: the journal save of the stage, the recording stop itself (its
//! error is passed in), and the panel's detach flag.

use crate::failure::Failure;
use crate::recording::RecordedRun;

use super::commissioning_result::{CommissioningResult, CommissioningStatus, TransferStatus};
use super::observation_assessment::JournalStage;
use super::parameter_status::ParameterStatus;
use super::pid_commissioning::PidCampaign;
use super::proportional_commissioning::{restore_again, Recovery, RunAttempt, RunEnding};
use super::texts;

impl PidCampaign<'_> {
    /// Ends the run (`execute`; B6.3, B6.7, A6.3, A8.5): the status, the reason, the failure with its next action, whether the previous
    /// settings are confirmed back, whether a Restore is awaited, and the journal stage. It returns `None` when the panel closed during a
    /// recorded observation: that outcome belongs to the runtime and to the reopened panel.
    ///
    /// `cancelled` is whether the engineer cancelled the run. `recovery` is how the recovery of the previous settings ended. A refusal, a
    /// cancel and a departure always run their recovery, which confirms at once when nothing was applied, so the caller reports
    /// [`Recovery::Confirmed`] or [`Recovery::Unconfirmed`] for them. `recording_stop` is the error of a recording stop that was not
    /// confirmed.
    #[must_use]
    pub fn settle(
        &self,
        attempt: RunAttempt,
        cancelled: bool,
        recovery: Recovery,
        recording_stop: Option<&str>,
    ) -> Option<RunEnding> {
        let mut ending = self.ending_of(attempt, cancelled, recovery)?;
        if let Some(error) = recording_stop {
            ending.recovery_confirmed = false;
            ending.reason = format!(
                "{} {}",
                ending.reason,
                texts::prop_commission_stop_unconfirmed(error)
            );
        }
        ending.stage = stage_of(&ending);
        Some(ending)
    }

    /// The ending of the attempt, before a recording stop that was not confirmed is considered (`execute`'s catch; B6.3, B6.7).
    fn ending_of(
        &self,
        attempt: RunAttempt,
        cancelled: bool,
        recovery: Recovery,
    ) -> Option<RunEnding> {
        let failed = if cancelled {
            CommissioningStatus::Cancelled
        } else {
            CommissioningStatus::Failed
        };
        let ending = match attempt {
            RunAttempt::Detached => return None,
            RunAttempt::Passed { maximum_error } => RunEnding {
                status: CommissioningStatus::Passed,
                reason: texts::pid_commission_passed(
                    self.start.profile.label(),
                    maximum_error,
                    &self.start.loop_definition.signals.process_variable.unit,
                ),
                failure: None,
                recovery_confirmed: true,
                awaiting_restore: false,
                stage: JournalStage::Completed,
            },
            RunAttempt::RequirementFailed(failure) => {
                // B6.3: the reviewed settings stay applied, the failed requirement is reported, and Restore is offered.
                let mut ending = failed_ending(failed, full_text(&failure), Some(failure), true);
                if let Recovery::Unconfirmed(error) = recovery {
                    unconfirmed(&mut ending, &texts::pid_commission_settings_unread(&error));
                }
                ending
            }
            RunAttempt::PermittedRangeLeft(failure) => {
                // B6.3, B6.7: the previous settings are restored at once, and the departure is the reason.
                let statement = full_text(&failure);
                let mut ending = failed_ending(failed, statement.clone(), Some(failure), false);
                match recovery {
                    Recovery::Unconfirmed(error) => {
                        unconfirmed(
                            &mut ending,
                            &texts::prop_commission_recovery_unconfirmed(&error),
                        );
                    }
                    Recovery::Confirmed | Recovery::NotAttempted => {
                        ending.reason = texts::prop_commission_range_restored(&statement);
                    }
                }
                ending
            }
            RunAttempt::Refused(failure) => {
                let mut ending = failed_ending(failed, full_text(&failure), Some(failure), false);
                self.recovered(&mut ending, recovery);
                ending
            }
            RunAttempt::Cancelled => {
                let mut ending = failed_ending(
                    CommissioningStatus::Cancelled,
                    texts::pid_commission_cancelled(),
                    None,
                    false,
                );
                self.recovered(&mut ending, recovery);
                ending
            }
        };
        Some(ending)
    }

    /// Adds the outcome of the recovery to the reason of a refusal or a cancel (A6.3, B6.7): the loop is back in the mode it was found in,
    /// or the recovery is unconfirmed and Restore is offered again.
    fn recovered(&self, ending: &mut RunEnding, recovery: Recovery) {
        match recovery {
            Recovery::Confirmed | Recovery::NotAttempted => {
                let mode = if self.found_in_auto() {
                    "auto"
                } else {
                    "manual"
                };
                ending.reason = texts::recovery_with_outcome(
                    &ending.reason,
                    &texts::recovery_confirmed_reason(mode),
                );
            }
            Recovery::Unconfirmed(error) => {
                unconfirmed(ending, &texts::prop_commission_recovery_unconfirmed(&error));
            }
        }
    }

    /// The result the run records (`execute`'s return; B6.3, B6.7, C1): the ending, the recording the run kept, the transfers it made, the
    /// record the runtime last reported, the largest error of a passed response, the reviewed target, the criteria and channels it was
    /// judged by, and the mode the loop was found in.
    ///
    /// # Errors
    /// The target, the criteria or the channels cannot be built.
    pub fn result(
        &self,
        ending: RunEnding,
        outcome: RunOutcome,
    ) -> Result<CommissioningResult, Failure> {
        Ok(CommissioningResult {
            status: ending.status,
            reason: ending.reason,
            recording: outcome.recording,
            transfers: outcome.transfers,
            recovery_confirmed: ending.recovery_confirmed,
            maximum_error: outcome.maximum_error,
            target: self.target()?,
            parameters: outcome.parameters,
            model_revision: self.start.model.identity().revision.clone(),
            channels: self.channels()?,
            criteria: self.criteria()?,
            gain_recovery: None,
            failure: ending.failure,
            found_mode: self.found_mode(),
        })
    }
}

/// What a Control.PID run records besides its ending (`execute`'s return; B6.3): the recording it kept, the transfers it made, the
/// largest error of a passed response, and the parameter record the runtime last reported.
#[derive(Debug, Clone, PartialEq)]
pub struct RunOutcome {
    /// The recording the run kept, when it recorded one (E4.7).
    pub recording: Option<RecordedRun>,
    /// The transfers the run made, oldest first.
    pub transfers: Vec<TransferStatus>,
    /// The largest error of the settled samples, once the response was judged.
    pub maximum_error: Option<f64>,
    /// The parameter record the runtime last reported.
    pub parameters: Option<ParameterStatus>,
}

/// The ending of a run that did not pass: cancelled or failed, with the recovery still confirmed until a recovery says otherwise.
fn failed_ending(
    status: CommissioningStatus,
    reason: String,
    failure: Option<Failure>,
    awaiting_restore: bool,
) -> RunEnding {
    RunEnding {
        status,
        reason,
        failure,
        recovery_confirmed: true,
        awaiting_restore,
        stage: JournalStage::Transfer,
    }
}

/// Records that the previous settings are not confirmed: the reason carries the outcome, and the failure offers Restore again (B6.7).
fn unconfirmed(ending: &mut RunEnding, outcome: &str) {
    ending.recovery_confirmed = false;
    ending.reason = texts::recovery_with_outcome(&ending.reason, outcome);
    let failure = ending.failure.take();
    ending.failure = Some(restore_again(failure, &ending.reason));
}

/// The whole text of a failure: its detail when it says more, otherwise its message (`MainWorkflowError.message`).
fn full_text(failure: &Failure) -> String {
    failure
        .detail
        .clone()
        .unwrap_or_else(|| failure.message.clone())
}

/// The journal stage an ending is written at (`execute`): completed for a pass or an awaited Restore, recovered when the previous
/// settings are confirmed back, and transfer otherwise.
fn stage_of(ending: &RunEnding) -> JournalStage {
    if ending.status == CommissioningStatus::Passed || ending.awaiting_restore {
        JournalStage::Completed
    } else if ending.recovery_confirmed {
        JournalStage::Recovered
    } else {
        JournalStage::Transfer
    }
}
