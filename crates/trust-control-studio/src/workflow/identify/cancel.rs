//! The engineer's Cancel of an identification whose run has ended but left work owned (`cancel` of `ConnectedIdentification`;
//! specification 35, A6.2, A8.5 and B3.4; runtime protocol annex E11.4 to E11.6). The run's in-memory state ends with the run,
//! so the journaled campaign is what the identification owns: its recording, and the experiment of its step. A run that is
//! still running is cancelled through its operation instead, and settles by itself.

use parking_lot::Mutex;
use trust_control::failure::Failure;
use trust_control::identification::campaign::journal::IdentificationCampaignJournal;
use trust_control::identification::connected_run::{
    cancel_step, restoration_unconfirmed, CancelStep,
};
use trust_control::loop_definition::LoopDefinition;

use super::run::{clear_entries, confirm_restored, retain_recording};
use super::{IdentifyRuntime, RecordingRetention};
use crate::journal::{Journal, JournalEntry};
use crate::workflow::identify_campaign::{CampaignClock, ExperimentClient};
use crate::workflow::reconcile::read_experiment_identity;

/// Cancels the identification journaled in `journal`: ends its experiment as its identity says, keeps its recording, then clears
/// the journal (`cancel`). Without a journaled campaign there is nothing to cancel.
///
/// # Errors
/// The experiment's restoration cannot be confirmed, or the recording cannot be recovered, stopped, kept or released, or the
/// journal cannot be cleared. The journal then stays.
pub(crate) fn cancel_journaled<T, K, C>(
    runtime: &T,
    clock: &C,
    journal: &Mutex<Journal>,
    loop_definition: &LoopDefinition,
    retention: &mut K,
) -> Result<(), Failure>
where
    T: IdentifyRuntime,
    K: RecordingRetention,
    C: CampaignClock,
{
    let Some(campaign) = journaled_campaign(&journal.lock()) else {
        return Ok(());
    };
    let experiment_id = campaign.plan.as_ref().map(|plan| plan.id.clone());
    if let Some(id) = &experiment_id {
        end_experiment(runtime, clock, id)?;
    }
    // Mode transfer: moves with V4 (runtimeTransfer.ts). The loop's mode returns here, once no experiment is left running.
    let mut recording = runtime.recover_recording(loop_definition, &campaign.recording)?;
    retain_recording(&mut recording, retention)?;
    let mut identities = vec![campaign.recording.recording_id.clone()];
    identities.extend(experiment_id);
    clear_entries(journal, &identities)
}

/// Ends the journaled experiment as its identity says: a running one is aborted through the runtime's restoration path, and a
/// running or ended one has its restoration confirmed. One the runtime does not hold never ran.
pub(crate) fn end_experiment<T: IdentifyRuntime, C: CampaignClock>(
    runtime: &T,
    clock: &C,
    experiment_id: &str,
) -> Result<(), Failure> {
    let identity = read_experiment_identity(|id| runtime.experiment_status(id), experiment_id);
    let mut experiment = runtime.experiment();
    match cancel_step(&identity) {
        CancelStep::Nothing => return Ok(()),
        CancelStep::Unconfirmed(failure) => return Err(failure),
        CancelStep::Abort => experiment.abort(experiment_id)?,
        CancelStep::Confirm => {}
    }
    if confirm_restored(&mut experiment, clock, experiment_id)? {
        Ok(())
    } else {
        Err(restoration_unconfirmed())
    }
}

/// The identification campaign the journal holds, when it holds one.
fn journaled_campaign(journal: &Journal) -> Option<IdentificationCampaignJournal> {
    journal.entries().iter().find_map(|entry| {
        if let JournalEntry::IdentificationCampaign { campaign } = entry {
            Some(campaign.as_ref().clone())
        } else {
            None
        }
    })
}
