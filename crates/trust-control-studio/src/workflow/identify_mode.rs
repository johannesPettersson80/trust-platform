//! Identify's mode change (`ConnectedIdentificationMode` of `connectedIdentificationMode.ts`; specification 35, A6.2, A8.5, A8.6
//! and B3.4 step 1; runtime protocol annex E6, E11.2, E11.4 and E11.9). A method that needs the other controller mode enters it
//! through a transfer that holds the present output, and the loop returns to the mode it was found in when the campaign ends,
//! fails or is cancelled, and after a restart of the engine from the journal. The mode found is journaled as the mode checkpoint
//! before the first transfer is dispatched, and the owner of each transfer is retained in that checkpoint before its apply is
//! sent, so that an interrupted transfer is reconciled by its identity and never applied again (E11.2, E11.4).
//!
//! Every rule is in `trust-control` (`identification::mode_transfer`): the binding, the states, the tracking, the allowance, the
//! references and commands written back, and the checks of the loop and the program. This module reads the runtime, writes the
//! journal and dispatches the transfers in their order. It connects to the runtime for each entry and each return, as the
//! campaign's other clients do, so a long campaign holds no connection open.

use std::collections::BTreeMap;

use parking_lot::Mutex;
use trust_control::commissioning::application::ApplySource;
use trust_control::commissioning::checkpoint::{IdentificationModeCheckpoint, ModeTransferOwner};
use trust_control::commissioning::commissioning_result::{TransferOwner, TransferStatus};
use trust_control::commissioning::transfer::{TransferBinding, TransferValue};
use trust_control::commissioning::transfer_protocol::check_transfer_status;
use trust_control::commissioning::transfer_tracking::TransferTracking;
use trust_control::failure::Failure;
use trust_control::identification::mode_transfer::{
    check_mode_loop, check_mode_program, command_return, found_in_auto, in_mode, mode_binding,
    mode_output_change, mode_states, mode_tracking, reference_return,
};
use trust_control::loop_definition::LoopDefinition;
use trust_control::runtime::RuntimeConnection;

use crate::journal::{Journal, JournalEntry};
use crate::runtime_client::transfers::{TransferOwnerSink, TransferSession, TransferWrite};
use crate::runtime_client::{new_transaction_id, RuntimeClient};

/// The mode change of one identification (`ConnectedIdentificationMode`): the loop, the runtime it transfers on, the loop's
/// journal, the mode binding, the mode checkpoint it holds, and the measured noise that a return to Auto allows for.
#[derive(Debug)]
pub struct IdentificationMode<'a> {
    loop_definition: &'a LoopDefinition,
    connection: RuntimeConnection,
    journal: &'a Mutex<Journal>,
    binding: TransferBinding,
    held: Option<HeldCheckpoint>,
    process_noise: Option<f64>,
}

impl<'a> IdentificationMode<'a> {
    /// The mode change of `loop_definition` on the runtime of `connection`, journaled in `journal` (the constructor of
    /// `ConnectedIdentificationMode`). It holds the mode checkpoint that the journal already holds for the loop, when an earlier
    /// campaign or a restart of the engine left one (A8.6), and it reads and writes nothing on the runtime until it enters or
    /// restores a mode.
    ///
    /// # Errors
    /// The loop's controller has no adapter, or the adapter's binding does not fit the loop, so the loop's mode cannot be
    /// transferred (see `mode_binding`).
    pub fn new(
        loop_definition: &'a LoopDefinition,
        connection: RuntimeConnection,
        journal: &'a Mutex<Journal>,
    ) -> Result<Self, Failure> {
        let binding = mode_binding(loop_definition)?;
        let held = journaled_checkpoint(&journal.lock());
        Ok(Self {
            loop_definition,
            connection,
            journal,
            binding,
            held,
            process_noise: None,
        })
    }

    /// Keeps the measured output noise of the identification's steady baseline, its half range (`processNoise`), which the
    /// first output of a return to Auto is allowed to move by (A6.4).
    pub fn set_process_noise(&mut self, half_range: f64) {
        self.process_noise = Some(half_range);
    }

    /// Moves the loop to the mode a method needs (`enter`, B3.4 step 1): Manual when `manual`, Auto otherwise. A checkpoint that
    /// is already held is restored first. A loop already in that mode is left as it is, and nothing is journaled or written.
    /// Otherwise the mode found is journaled as the checkpoint before anything is dispatched, and one transfer writes the mode
    /// with the controller's states as they are; its owner is retained in the checkpoint before the apply is sent (E11.2) and
    /// let go once the runtime has confirmed the outcome.
    ///
    /// # Errors
    /// The held checkpoint cannot be restored; the runtime cannot be reached or its status is out of protocol; a state does not
    /// fit its member; the journal cannot be written; or the runtime refuses or does not confirm the transfer. A checkpoint
    /// journaled before the failure stays held and journaled, so that [`Self::restore`] returns the mode.
    pub fn enter(&mut self, manual: bool) -> Result<(), Failure> {
        if self.held.is_some() {
            self.restore()?;
        }
        let mut client = RuntimeClient::connect(&self.connection)?;
        let status = read_status(&mut client, &self.binding)?;
        if in_mode(&self.binding, &status, manual) {
            return Ok(());
        }
        let write = mode_write(
            self.loop_definition,
            &self.binding,
            &status,
            manual,
            Some(&status),
            self.process_noise,
        )?;
        let checkpoint = HeldCheckpoint {
            operation_id: new_operation_id(),
            checkpoint: IdentificationModeCheckpoint {
                loop_revision: self.loop_definition.revision.clone(),
                original: status,
                transfer: None,
            },
        };
        checkpoint.persist(self.journal)?;
        let held = self.held.insert(checkpoint);
        transfer(&mut client, self.journal, held, &self.binding, &write)
    }

    /// Returns the loop to the mode Identify found it in (`restore`; A6.2, A8.6, E11.9). Without a held checkpoint nothing is
    /// done. The loop must still be the one whose mode was changed. A transfer the checkpoint still owns is reconciled by its
    /// identity, cancelled or read to its outcome, and never applied again (E11.4); then the runtime must still run the
    /// checkpoint's program. A loop that is not in Manual returns to Manual through a transfer that holds its output; the loop's
    /// references and commands are written back in Manual (`command_return`); and a loop found in Auto then enters Auto with the
    /// controller's states as they are and the allowance of its first output (A6.4), and a reference its program overwrote in
    /// Manual returns in Auto (`reference_return`, C1). The checkpoint is cleared from the journal once the runtime has confirmed
    /// every transfer.
    ///
    /// # Errors
    /// The loop's revision or the runtime's program changed; the runtime cannot be reached, or it refuses or does not confirm a
    /// transfer or the cancel of the owned one; a value does not fit its member; or the journal cannot be written. The checkpoint
    /// then stays held and journaled, so that a later return, a Cancel or a reopened panel returns the mode (A8.6).
    pub fn restore(&mut self) -> Result<(), Failure> {
        let Some(held) = self.held.as_mut() else {
            return Ok(());
        };
        check_mode_loop(self.loop_definition, &held.checkpoint)?;
        let mut client = RuntimeClient::connect(&self.connection)?;
        if let Some(owned) = held.checkpoint.transfer.clone() {
            let program = held.checkpoint.original.program_revision.clone();
            let mut session = TransferSession::new(&mut client, owned.binding, program);
            session.restore_owner(owned.owner)?;
            // Reconciled by its identity: the owned transfer is cancelled or read to its outcome, never applied again (E11.4).
            session.cancel()?;
            held.checkpoint.transfer = None;
            held.persist(self.journal)?;
        }
        let current = read_status(&mut client, &self.binding)?;
        check_mode_program(&held.checkpoint, &current)?;
        if !in_mode(&self.binding, &current, true) {
            let write = mode_write(
                self.loop_definition,
                &self.binding,
                &current,
                true,
                None,
                self.process_noise,
            )?;
            transfer(&mut client, self.journal, held, &self.binding, &write)?;
        }
        let commands = command_return(
            self.loop_definition,
            &self.binding,
            &held.checkpoint.original,
        )?;
        // A6.2, E6.9: restoring the found Manual command moves the output back by design, so that change is its allowance.
        let in_manual = read_status(&mut client, &self.binding)?;
        let output_now = in_manual
            .values
            .get(&self.binding.output.symbol)
            .copied()
            .and_then(TransferValue::as_number);
        let base = mode_output_change(self.loop_definition, true, None, self.process_noise);
        let tolerance = commands.allowance(base, output_now);
        let write = transfer_write(commands.values, true, tolerance, None);
        transfer(&mut client, self.journal, held, &commands.binding, &write)?;
        if found_in_auto(&self.binding, &held.checkpoint.original) {
            let current = read_status(&mut client, &self.binding)?;
            let write = mode_write(
                self.loop_definition,
                &self.binding,
                &current,
                false,
                Some(&current),
                self.process_noise,
            )?;
            transfer(&mut client, self.journal, held, &self.binding, &write)?;
            // C1: a program that sets its reference from the measurement in Manual overwrote the reference written back there, so
            // it returns now, in Auto.
            let entered = read_status(&mut client, &self.binding)?;
            if let Some(step) = reference_return(
                self.loop_definition,
                &self.binding,
                &held.checkpoint.original,
                &entered,
            )? {
                let write = transfer_write(step.values, false, step.tolerance, Some(step.tracking));
                transfer(&mut client, self.journal, held, &self.binding, &write)?;
            }
        }
        self.journal.lock().clear(&held.operation_id)?;
        self.held = None;
        Ok(())
    }
}

/// Whether `journal` holds the mode checkpoint of an identification (`ports.mode.read()`): a mode an interrupted
/// identification changed and has not returned yet (A8.6).
#[must_use]
pub fn mode_journaled(journal: &Journal) -> bool {
    journal
        .entries()
        .iter()
        .any(|entry| matches!(entry, JournalEntry::IdentificationMode { .. }))
}

/// The mode checkpoint a mode change holds, with the identifier the journal keeps it under.
#[derive(Debug, Clone)]
struct HeldCheckpoint {
    operation_id: String,
    checkpoint: IdentificationModeCheckpoint,
}

impl HeldCheckpoint {
    /// Journals the checkpoint, in place of its earlier entry (E11.2).
    fn persist(&self, journal: &Mutex<Journal>) -> Result<(), Failure> {
        journal.lock().persist(JournalEntry::IdentificationMode {
            operation_id: self.operation_id.clone(),
            checkpoint: Box::new(self.checkpoint.clone()),
        })
    }
}

/// Retains the owner of a mode transfer in the checkpoint, and the checkpoint in the journal, before the transfer's apply is
/// sent (`setOwnerPersistence`, E11.2). The checkpoint holds the owner only once the journal does.
struct CheckpointSink<'h> {
    journal: &'h Mutex<Journal>,
    held: &'h mut HeldCheckpoint,
    binding: &'h TransferBinding,
}

impl TransferOwnerSink for CheckpointSink<'_> {
    fn retain(&mut self, owner: &TransferOwner) -> Result<(), Failure> {
        let mut retained = self.held.clone();
        retained.checkpoint.transfer = Some(ModeTransferOwner {
            binding: self.binding.clone(),
            owner: owner.clone(),
        });
        retained.persist(self.journal)?;
        *self.held = retained;
        Ok(())
    }
}

/// One transfer of a mode change (`transfer` of `ConnectedIdentificationMode`), on the program of the checkpoint's original
/// status: its owner is retained in the checkpoint before the apply is sent, the runtime confirms and the outcome is verified,
/// and the checkpoint then lets the owner go (E11.2, E11.6).
fn transfer(
    client: &mut RuntimeClient,
    journal: &Mutex<Journal>,
    held: &mut HeldCheckpoint,
    binding: &TransferBinding,
    write: &TransferWrite,
) -> Result<(), Failure> {
    let program = held.checkpoint.original.program_revision.clone();
    let mut session = TransferSession::new(client, binding.clone(), program);
    session.apply(
        write,
        &mut CheckpointSink {
            journal,
            held: &mut *held,
            binding,
        },
    )?;
    held.checkpoint.transfer = None;
    held.persist(journal)
}

/// The write of a transfer of the mode binding at the states `current` reports (`transfer` with `stateValues` and `tracking`):
/// Manual when `manual`, with the tracking of `mode_tracking` and the allowance of `mode_output_change`, which an Auto entry
/// reviewed on `observed` widens by the controller's own response.
fn mode_write(
    loop_definition: &LoopDefinition,
    binding: &TransferBinding,
    current: &TransferStatus,
    manual: bool,
    observed: Option<&TransferStatus>,
    process_noise: Option<f64>,
) -> Result<TransferWrite, Failure> {
    let values = mode_states(binding, current)?;
    let tracking = mode_tracking(loop_definition, binding, current)?;
    let tolerance = mode_output_change(loop_definition, manual, observed, process_noise);
    Ok(transfer_write(values, manual, tolerance, Some(tracking)))
}

/// A transfer write of a mode change. Its target is a simulation, as Identify's live campaign is (a physical target is refused
/// before the run, A2.5), and the session reads the status it applies on, since no reviewed status is given.
fn transfer_write(
    values: BTreeMap<String, f64>,
    manual: bool,
    tolerance: f64,
    tracking: Option<TransferTracking>,
) -> TransferWrite {
    TransferWrite {
        values,
        manual,
        tolerance,
        tracking,
        reviewed: None,
        source: ApplySource::Simulation,
        physical_approved: false,
    }
}

/// The transfer status of `binding`, checked (`status` of `ConnectedIdentificationMode`, annex E6.3).
fn read_status(
    client: &mut RuntimeClient,
    binding: &TransferBinding,
) -> Result<TransferStatus, Failure> {
    let status = client.transfer_status(binding)?;
    check_transfer_status(binding, &status)?;
    Ok(status)
}

/// The mode checkpoint the journal holds, with its identifier (`persistence.read()`).
fn journaled_checkpoint(journal: &Journal) -> Option<HeldCheckpoint> {
    journal.entries().iter().find_map(|entry| {
        if let JournalEntry::IdentificationMode {
            operation_id,
            checkpoint,
        } = entry
        {
            Some(HeldCheckpoint {
                operation_id: operation_id.clone(),
                checkpoint: checkpoint.as_ref().clone(),
            })
        } else {
            None
        }
    })
}

/// A new identifier of one mode change (E0.2), which the journal keeps its checkpoint under: `identification-mode-` and the hex
/// digits of a fresh transaction identifier.
fn new_operation_id() -> String {
    let transaction = new_transaction_id();
    let digits = transaction
        .strip_prefix("transaction-")
        .unwrap_or(&transaction);
    format!("identification-mode-{digits}")
}

#[cfg(test)]
#[path = "identify_mode_test_support.rs"]
pub(crate) mod test_support;

#[cfg(test)]
#[path = "identify_mode_tests.rs"]
mod tests;
