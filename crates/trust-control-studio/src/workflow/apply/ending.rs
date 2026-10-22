//! How an Apply run and a reattached observation end (specification 35, B6.3, B6.4, B6.5, B6.7 and A8.5; annex E4.7, E11.4 and E11.6):
//! an unfinished recording is stopped and its samples kept in the checkpoint before anything else, a failure after the first write runs
//! the one Restore at once (B6.4, V4 decision 4), and the outcome says whether Restore and Keep are offered. The rules of the ending are
//! trust-control's (`settle`, `settle_run`); this module performs the calls they need and reads the journal.

use trust_control::commissioning::application_offers::{apply_offered_after, primary_action};
use trust_control::commissioning::application_records::{
    ApplyOutcome, RestoreOutcome, ReviewedAction,
};
use trust_control::commissioning::checkpoint::CheckpointStage;
use trust_control::commissioning::commissioning_result::{
    CommissioningResult, CommissioningStatus,
};
use trust_control::commissioning::parameter_status::ParameterStatus;
use trust_control::commissioning::proportional_commissioning::Recovery;
use trust_control::failure::Failure;
use trust_control::loop_definition::LoopDefinition;
use trust_control::messages::AnswerKind;
use trust_control::recording::RecordedRun;
use trust_control::runtime::RuntimeConnection;

use super::journaling::CheckpointWriter;
use crate::runtime_client::runtime_recording::RuntimeRecordingSession;
use crate::runtime_client::RuntimeClient;
use crate::workflow::restore::restore_with_approval;

/// How the one Restore that a failed Apply runs ended (B6.4, B6.7): the recovery the ending reports, whether the runtime confirmed the
/// previous settings, and the parameter record the Restore last read.
pub(crate) struct Restoration {
    /// The recovery the rules of the ending read.
    pub(crate) recovery: Recovery,
    /// Whether the runtime confirmed the previous settings (Restored, B6.4).
    pub(crate) restored: bool,
    /// The parameter record the Restore last read, when it read one.
    pub(crate) parameters: Option<ParameterStatus>,
}

/// Runs the one Restore from the checkpoint that `writer` journaled (B6.4, V4 decision 4): a failure after the first write, a departure
/// from the permitted range and a cancel all restore through it. `physical_approved` is the approval of the reviewed Apply, which covers
/// its restoration (A2.5: the approval shows the restoration, and one approval covers one reviewed operation shown in full); `progress`
/// shows each step that writes. A confirmed Restore journals the checkpoint as recovered and clears it itself (A8.5); one that is refused
/// or not confirmed leaves it journaled with every identity it dispatched, so Restore is offered again (B6.7).
pub(crate) fn restore_at_once(
    connection: &RuntimeConnection,
    loop_definition: &LoopDefinition,
    writer: &CheckpointWriter<'_>,
    physical_approved: bool,
    progress: &dyn Fn(&str),
) -> Restoration {
    let restored: Result<RestoreOutcome, Failure> =
        RuntimeClient::connect(connection).and_then(|mut client| {
            restore_with_approval(
                &mut client,
                loop_definition,
                writer.journal(),
                writer.checkpoint(),
                physical_approved,
                progress,
            )
        });
    match restored {
        Ok(outcome) if outcome.restored => Restoration {
            recovery: Recovery::Confirmed,
            restored: true,
            parameters: Some(outcome.parameters),
        },
        Ok(outcome) => Restoration {
            recovery: Recovery::Unconfirmed(outcome.reason),
            restored: false,
            parameters: Some(outcome.parameters),
        },
        Err(failure) => Restoration {
            recovery: Recovery::Unconfirmed(whole_text(&failure)),
            restored: false,
            parameters: None,
        },
    }
}

/// Stops a recording that is still running and keeps what it returns (E11.6): the samples up to the end of the run are evidence. Returns
/// the stop's error, in words, when the stop was not confirmed; the owner then stays in the checkpoint, so that Restore stops it.
pub(crate) fn stop_unfinished(
    session: Option<&mut RuntimeRecordingSession>,
    recording: &mut Option<RecordedRun>,
) -> Option<String> {
    let session = session?;
    if recording.is_some() {
        return None;
    }
    match session.stop() {
        Ok(run) => {
            *recording = Some(run);
            None
        }
        Err(failure) => Some(whole_text(&failure)),
    }
}

/// Keeps a stopped recording in the checkpoint (E4.7, E11.6): its samples are retained there, and its owner is cleared, because the stop
/// returned the terminal record. Nothing is written when no recording was stopped.
///
/// # Errors
/// The journal cannot be written.
pub(crate) fn keep_recording(
    writer: &CheckpointWriter<'_>,
    recording: Option<&RecordedRun>,
) -> Result<(), Failure> {
    let Some(run) = recording else {
        return Ok(());
    };
    writer.save_with(None, |checkpoint| {
        checkpoint.recording_result = Some(run.clone());
        checkpoint.recording = None;
    })
}

/// Frees the runtime's buffer of a recording whose samples are kept (E4.7): only once the run is retained. A release that the runtime
/// does not answer keeps the buffer pending for the next start of the endpoint (`runtime_recording`).
pub(crate) fn release_kept(
    session: Option<&mut RuntimeRecordingSession>,
    recording: Option<&RecordedRun>,
) {
    if let (Some(session), Some(_)) = (session, recording) {
        // The session refuses a release only before its stop, and the stop returned the kept run, so nothing is lost here.
        let _ = session.release();
    }
}

/// The outcome of an Apply (B6.3, B6.5, B6.7): the result, whether the one Restore confirmed the previous settings, what is offered
/// next and the action that holds the position then (B7), and the checkpoint and validation that the Apply's receipt names (A8.1,
/// A8.5), taken from the checkpoint this Apply journaled, also when a confirmed Restore has cleared it since. Restore is offered while
/// the checkpoint stays journaled and not retired: the settings are applied (stage `completed`), or a Restore was not confirmed and is
/// offered again. Keep in project is offered only for applied settings whose run was not cancelled.
#[must_use]
pub(crate) fn outcome_of(
    result: CommissioningResult,
    writer: &CheckpointWriter<'_>,
    restored: bool,
) -> ApplyOutcome {
    let written = writer.checkpoint();
    let journaled = writer
        .journal()
        .checkpoint()
        .filter(|checkpoint| checkpoint.id == written.id);
    let completed = journaled
        .as_ref()
        .is_some_and(|checkpoint| checkpoint.stage == CheckpointStage::Completed);
    let restore_available = journaled
        .as_ref()
        .is_some_and(|checkpoint| checkpoint.stage != CheckpointStage::Retired);
    let keep_available = completed && result.status != CommissioningStatus::Cancelled;
    let apply_available = apply_offered_after(ReviewedAction::Apply);
    ApplyOutcome {
        kind: AnswerKind::new(),
        keep_available,
        restore_available,
        apply_available,
        primary_action: primary_action(apply_available, restore_available, keep_available),
        // The stages read the study and the journal once the run has ended: `apply_action` adds them to its answer.
        stages: None,
        restored,
        result,
        checkpoint_id: written.id,
        validation_identity: written.validation_identity,
    }
}

/// The whole text of a failure: its detail when it says more, otherwise its message (A11.1).
#[must_use]
pub(crate) fn whole_text(failure: &Failure) -> String {
    failure
        .detail
        .clone()
        .unwrap_or_else(|| failure.message.clone())
}
