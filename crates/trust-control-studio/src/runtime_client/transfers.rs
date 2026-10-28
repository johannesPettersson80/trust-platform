//! The transfer client of Control Studio (runtime protocol annex E6; specification 35, A6.3, E11.2 to E11.4): one owned transfer at a
//! time. The owner is retained before its apply is dispatched, a lost acknowledgement is settled by the same identity and is never
//! sent again, a cancel is answered before dispatch or by the owner's own outcome, and the confirmation is polled and verified. From
//! `runtimeTransfer.ts`; the rules are in `trust_control::commissioning::transfer_protocol`.

use std::collections::BTreeMap;
use std::thread::sleep;
use std::time::{Duration, Instant};

use trust_control::commissioning::application::ApplySource;
use trust_control::commissioning::commissioning_result::{
    TransferOwner, TransferState, TransferStatus,
};
use trust_control::commissioning::texts;
use trust_control::commissioning::transfer::TransferBinding;
use trust_control::commissioning::transfer_protocol::{
    check_transfer_status, require_transfer_approval, transfer_apply_request,
    transfer_cancel_acknowledged, transfer_finish_step, verify_transfer_outcome,
    TransferApplyInput, TransferCancelRequest, TransferStep,
};
use trust_control::commissioning::transfer_tracking::TransferTracking;
use trust_control::failure::{Cause, Failure, NextAction};

use super::{new_transaction_id, RuntimeClient};

/// Retains a transfer's owner before its apply is dispatched (E11.2, A8.5). The journal implements it; a failure keeps the apply from
/// being sent.
pub trait TransferOwnerSink {
    /// Retains `owner` durably.
    ///
    /// # Errors
    /// The owner could not be retained, and nothing may be dispatched.
    fn retain(&mut self, owner: &TransferOwner) -> Result<(), Failure>;
}

/// One transfer write: the states it writes, the mode it leaves, the output change it allows, the tracking of an Auto entry, the status
/// it was reviewed on, and the target it goes to.
#[derive(Debug, Clone, PartialEq)]
pub struct TransferWrite {
    /// The states the transfer writes: exactly the states that its tracking does not compute.
    pub values: BTreeMap<String, f64>,
    /// `true` leaves the loop in Manual, `false` leaves it in Auto.
    pub manual: bool,
    /// The reviewed output change allowed at the first execution (E6.9).
    pub tolerance: f64,
    /// The factor of the error at entry that the runtime adds to `tolerance`, for an Auto entry (E6.9).
    pub entry_error_gain: Option<f64>,
    /// The factor of the measurement's change before an Auto entry that the runtime adds to `tolerance` (E6.9).
    pub entry_measurement_gain: Option<f64>,
    /// The tracking of an Auto entry, when the transfer has one (E6.4).
    pub tracking: Option<TransferTracking>,
    /// The status the write was reviewed on; the session reads one when this is absent.
    pub reviewed: Option<TransferStatus>,
    /// The target the transfer goes to.
    pub source: ApplySource,
    /// Whether the engineer approved a physical write (E10.2).
    pub physical_approved: bool,
}

/// One transfer of one loop on one program: the binding it writes, the program it was reviewed on, and the owner it holds.
pub struct TransferSession<'c> {
    client: &'c mut RuntimeClient,
    binding: TransferBinding,
    program_revision: String,
    active: Option<TransferOwner>,
    outcomes: Vec<TransferStatus>,
}

impl<'c> TransferSession<'c> {
    /// A session for `binding` on the program `program_revision`, with no owner.
    #[must_use]
    pub fn new(
        client: &'c mut RuntimeClient,
        binding: TransferBinding,
        program_revision: String,
    ) -> Self {
        Self {
            client,
            binding,
            program_revision,
            active: None,
            outcomes: Vec::new(),
        }
    }

    /// Takes over an owner that the journal kept, so that a reopened session holds the same identity (E11.5).
    ///
    /// # Errors
    /// The session already holds an owner, or the owner belongs to another program (`restoreOwner` of `runtimeTransfer.ts`).
    pub fn restore_owner(&mut self, owner: TransferOwner) -> Result<(), Failure> {
        if self.active.is_some() || owner.program != self.program_revision {
            return Err(Failure::new(
                &texts::transfer_owner_restore_refused(),
                Cause::Runtime,
                NextAction::None,
            ));
        }
        self.active = Some(owner);
        Ok(())
    }

    /// The transfer status of the loop, checked (`status` of `runtimeTransfer.ts`, E6.3).
    ///
    /// # Errors
    /// The runtime cannot be reached, refuses, or answers out of protocol.
    pub fn status(&mut self) -> Result<TransferStatus, Failure> {
        let status = self.client.transfer_status(&self.binding)?;
        check_transfer_status(&self.binding, &status)?;
        Ok(status)
    }

    /// Applies a transfer and confirms it (`apply` of `runtimeTransfer.ts`, E6.5 to E6.7). The owner is retained through `sink` before
    /// the request is sent, so that a lost acknowledgement is read by the same identity and is never sent again (E11.2, E11.4).
    ///
    /// # Errors
    /// The approval is missing, another transfer is owned or pending, the program changed, the runtime refuses the request, the
    /// confirmation is unresolved, or the outcome does not match what the apply asked for. A refusal that the runtime gave before it
    /// accepted the transaction releases the owner.
    pub fn apply(
        &mut self,
        input: &TransferWrite,
        sink: &mut dyn TransferOwnerSink,
    ) -> Result<TransferStatus, Failure> {
        require_transfer_approval(input.source, input.physical_approved)?;
        if self.active.is_some() {
            return Err(Failure::new(
                &texts::transfer_owned_pending(),
                Cause::Runtime,
                NextAction::None,
            ));
        }
        let reviewed = match &input.reviewed {
            Some(reviewed) => reviewed.clone(),
            None => self.status()?,
        };
        if reviewed.program_revision != self.program_revision
            || reviewed.status == TransferState::Pending
        {
            return Err(Failure::new(
                &texts::transfer_program_changed(),
                Cause::Runtime,
                NextAction::Validate,
            ));
        }
        let owner = TransferOwner {
            id: new_transaction_id(),
            program: self.program_revision.clone(),
            binding: reviewed.binding_revision.clone(),
        };
        let request = transfer_apply_request(
            &self.binding,
            &reviewed,
            &TransferApplyInput {
                transaction_id: &owner.id,
                values: &input.values,
                manual: input.manual,
                maximum_output_change: input.tolerance,
                entry_error_gain: input.entry_error_gain,
                entry_measurement_gain: input.entry_measurement_gain,
                tracking: input.tracking.as_ref(),
                source: input.source,
                physical_approved: input.physical_approved,
            },
        )?;
        sink.retain(&owner)?;
        self.active = Some(owner.clone());
        if let Err(refusal) = self.client.transfer_apply_coded(&request) {
            // A refusal of the request means the transaction was never accepted, so nothing is owned (`apply` of `runtimeTransfer.ts`).
            if refusal.refusal_code.as_deref() == Some("invalid_transfer_request") {
                self.active = None;
                return Err(refusal.failure);
            }
            // Any other unanswered apply is settled by the same identity, and it is never sent again (E11.4).
            if !self.observes_owner(&owner) {
                return Err(refusal.failure);
            }
        }
        let outcome = self.finish(&owner)?;
        verify_transfer_outcome(
            &self.binding,
            &input.values,
            input.manual,
            input.tracking.as_ref(),
            &outcome,
        )?;
        Ok(outcome)
    }

    /// Cancels the owned transfer (`cancel` of `runtimeTransfer.ts`, E6.10). A cancel acknowledged before dispatch releases the owner;
    /// any other answer is settled by the transaction's own outcome, never by another apply.
    ///
    /// # Errors
    /// The confirmation of the owned transfer is unresolved, or the runtime cannot be read.
    pub fn cancel(&mut self) -> Result<(), Failure> {
        let Some(owner) = self.active.clone() else {
            return Ok(());
        };
        let request = TransferCancelRequest {
            loop_id: self.binding.loop_id.clone(),
            transaction_id: owner.id.clone(),
            expected_program_revision: owner.program.clone(),
        };
        let acknowledged = self
            .client
            .transfer_cancel(&request)
            .is_ok_and(|answer| transfer_cancel_acknowledged(&owner, &answer));
        if acknowledged {
            self.active = None;
            return Ok(());
        }
        self.finish(&owner).map(|_| ())
    }

    /// The outcomes this session has confirmed, oldest first (`outcomes` of `runtimeTransfer.ts`).
    #[must_use]
    pub fn outcomes(&self) -> &[TransferStatus] {
        &self.outcomes
    }

    /// The owner this session holds, when a transfer is unresolved (`owned` of `runtimeTransfer.ts`).
    #[must_use]
    pub fn owned(&self) -> Option<&TransferOwner> {
        self.active.as_ref()
    }

    /// Whether this session holds an owner or has confirmed an outcome (`hasOwnedOperation` of `runtimeTransfer.ts`).
    #[must_use]
    pub fn has_owned_operation(&self) -> bool {
        self.active.is_some() || !self.outcomes.is_empty()
    }

    /// Whether the last transaction of the runtime is the owner's, after a lost acknowledgement (E11.4).
    fn observes_owner(&mut self, owner: &TransferOwner) -> bool {
        self.status()
            .ok()
            .and_then(|status| status.transaction)
            .is_some_and(|transaction| {
                transaction.id == owner.id
                    && transaction.program_revision == owner.program
                    && transaction.binding_revision == owner.binding
            })
    }

    /// Polls the owned transfer until the runtime ends it, within the confirmation window (`finish` of `runtimeTransfer.ts`, A7, E11.4).
    /// The owner is released only when its terminal outcome is in hand.
    fn finish(&mut self, owner: &TransferOwner) -> Result<TransferStatus, Failure> {
        let timing = self.client.confirmation_timing(&self.binding.task_name);
        let deadline = Instant::now() + duration_ms(timing.window_ms);
        while Instant::now() < deadline {
            let status = self.status()?;
            match transfer_finish_step(owner, &status)? {
                TransferStep::Wait => sleep(duration_ms(timing.poll_ms)),
                TransferStep::Terminal(outcome) => {
                    self.active = None;
                    let recorded = self.outcomes.iter().any(|value| {
                        value
                            .transaction
                            .as_ref()
                            .is_some_and(|transaction| transaction.id == owner.id)
                    });
                    if !recorded {
                        self.outcomes.push(outcome.clone());
                    }
                    return Ok(outcome);
                }
            }
        }
        Err(Failure::new(
            &texts::transfer_unresolved(),
            Cause::Runtime,
            NextAction::None,
        ))
    }
}

/// A duration of `milliseconds`: the intervals of the confirmation timing, which are finite and positive.
fn duration_ms(milliseconds: f64) -> Duration {
    Duration::from_secs_f64(milliseconds / 1000.0)
}

#[cfg(test)]
#[path = "transfers_tests.rs"]
mod tests;
