//! The stepwise persister of a chip's retained values (specification 34 §10; C5-3).
//!
//! [`CheckpointPersister`] joins the shared capture
//! ([`RetainCapture`](crate::retain::capture::RetainCapture)) to the checkpoint log
//! ([`CheckpointLog`](crate::retain::checkpoint::CheckpointLog)). When the shared save rule says a
//! save is due and the staged image differs from the image last written, the persister begins a
//! checkpoint; [`step`](CheckpointPersister::step) then advances it by one log step at a time so a
//! save never delays a scan, and [`finish`](CheckpointPersister::finish) completes it at a stop.
//! A failed step latches a retain-store failure, which the caller takes once and fails its next
//! scan with before physical outputs (specification 11 §6.7).
//!
//! The persister reserves its payload buffer once, for the admitted retain image, and copies the
//! staged image into it when a checkpoint begins. The copy keeps the checkpoint being written stable
//! while the capture refills its staging buffer in place with a newer image, and it lets the
//! unchanged-image skip compare the newest staged image with the one last written, without touching
//! the capture's buffers. Starting, advancing and finishing a checkpoint therefore allocate nothing.
//! [`recover`](CheckpointPersister::recover) leaves the payload of a recovered checkpoint written
//! for the persister's identity in that buffer, so a warm start does not write the checkpoint it
//! has just restored (specification 11 §6.7).

use alloc::vec::Vec;

use crate::error::RuntimeError;
use crate::retain::capture::RetainCapture;
use crate::value::Duration;

use super::{
    CheckpointError, CheckpointFlash, CheckpointIdentity, CheckpointLog, Progress, RecordInfo,
};

/// What one [`step`](CheckpointPersister::step) did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// No checkpoint is being written.
    Idle,
    /// The pending checkpoint needs more steps.
    Pending,
    /// The pending checkpoint committed this record.
    Committed(RecordInfo),
}

/// Saves retained values into a chip's checkpoint log between scans (specification 34 §10).
///
/// A persister owns the log, the application and retain-schema
/// [`CheckpointIdentity`](crate::retain::checkpoint::CheckpointIdentity) every checkpoint is written
/// for, and the payload of the checkpoint being written. [`offer`](Self::offer) starts a checkpoint
/// from a capture's staged image when the shared save rule says it is due and the image changed;
/// [`step`](Self::step) writes at most one log step per call; [`finish`](Self::finish) completes
/// the pending checkpoint and the newest image at a stop; and
/// [`take_failure`](Self::take_failure) hands out a latched failure once.
pub struct CheckpointPersister<F> {
    log: CheckpointLog<F>,
    identity: CheckpointIdentity,
    /// The payload of the checkpoint being written, or, when no write is in progress, the payload
    /// last written. Reserved once for the admitted retain image.
    payload: Vec<u8>,
    /// Whether the log holds a checkpoint that has begun and not committed.
    writing: bool,
    /// The failure of a write that did not commit, handed out once by
    /// [`take_failure`](Self::take_failure).
    failure: Option<RuntimeError>,
}

impl<F: CheckpointFlash> CheckpointPersister<F> {
    /// A persister over `log` that writes checkpoints for `identity`, with its payload buffer
    /// reserved for an admitted retain image of `image_bytes` bytes.
    #[must_use]
    pub fn new(log: CheckpointLog<F>, identity: CheckpointIdentity, image_bytes: usize) -> Self {
        Self {
            log,
            identity,
            payload: Vec::with_capacity(image_bytes),
            writing: false,
            failure: None,
        }
    }

    /// Whether a checkpoint has begun and not committed.
    #[must_use]
    pub fn is_writing(&self) -> bool {
        self.writing
    }

    /// The recovery rule at boot: read the newest committed checkpoint's payload into the front of
    /// `payload` and return the record, which carries its identity and payload length, or `None`
    /// for an empty log (specification 34 §10.1).
    ///
    /// A record written for this persister's identity becomes the image last written: the payload
    /// it holds is kept as that image, so a due save of an unchanged image writes nothing, also on
    /// the first due save after a warm start (specification 11 §6.7). A record of another identity
    /// is not kept, so the first due save writes the image once under this persister's identity.
    ///
    /// # Errors
    ///
    /// [`RuntimeError::RetainCorruption`] when the newest committed checkpoint fails its checks,
    /// which fails the warm start as a damaged retain file does (specification 11 §6.7);
    /// [`RuntimeError::RetainStore`] when the flash cannot be read or `payload` is too short.
    pub fn recover(&mut self, payload: &mut [u8]) -> Result<Option<RecordInfo>, RuntimeError> {
        let record = self.log.recover(payload).map_err(RuntimeError::from)?;
        let Some(record) = record else {
            return Ok(None);
        };
        if record.identity == self.identity {
            let len = record.payload_len as usize;
            let Some(image) = payload.get(..len) else {
                return Err(CheckpointError::BufferTooSmall {
                    needed: record.payload_len,
                }
                .into());
            };
            self.payload.clear();
            self.payload.extend_from_slice(image);
        }
        Ok(Some(record))
    }

    /// Start a checkpoint of the capture's staged image when the shared save rule says a save is due
    /// and that image differs from the one last written (specification 11 §6.7).
    ///
    /// While a checkpoint is being written, a newer due image is left in the capture: this call
    /// starts nothing, so the pending checkpoint is not disturbed and the newer image is still
    /// there to be offered after the pending one commits. An unchanged image takes the save
    /// cadence without a write. A capture with no staged image starts nothing.
    ///
    /// # Errors
    ///
    /// [`RuntimeError::RetainStore`] when the log refuses the checkpoint (a payload larger than a
    /// slot, an exhausted sequence, or flash that cannot be read); the failure is latched.
    pub fn offer(
        &mut self,
        capture: &mut RetainCapture,
        now: Duration,
    ) -> Result<(), RuntimeError> {
        if self.writing || self.failure.is_some() {
            return Ok(());
        }
        if !capture.should_save(now) {
            return Ok(());
        }
        let staged = capture.staged_payload();
        if staged.is_empty() {
            return Ok(());
        }
        if staged == self.payload.as_slice() {
            capture.unchanged(now);
            return Ok(());
        }
        self.payload.clear();
        self.payload.extend_from_slice(staged);
        match self.log.begin(self.identity, &self.payload) {
            Ok(()) => {
                self.writing = true;
                capture.accept(now);
                Ok(())
            }
            Err(error) => {
                self.payload.clear();
                Err(self.latch(error))
            }
        }
    }

    /// Advance the pending checkpoint by one log step: one erase unit, or at most
    /// `max_program_bytes` bytes programmed (rounded down to whole program units, at least one
    /// unit). The caller chooses the budget from its idle time; this never runs more than one step.
    ///
    /// A step that fails ends the checkpoint without committing it and latches the failure, which
    /// [`take_failure`](Self::take_failure) hands out once.
    ///
    /// # Errors
    ///
    /// [`RuntimeError::RetainStore`] when the flash fails, does not read as erased where it must be
    /// programmed, or does not read back as programmed; the failure is latched and the newest
    /// committed checkpoint stays readable.
    pub fn step(&mut self, max_program_bytes: usize) -> Result<Step, RuntimeError> {
        if !self.writing {
            return Ok(Step::Idle);
        }
        match self.log.step(&self.payload, max_program_bytes) {
            Ok(Progress::Pending) => Ok(Step::Pending),
            Ok(Progress::Committed(record)) => {
                self.writing = false;
                Ok(Step::Committed(record))
            }
            Err(error) => {
                self.writing = false;
                self.payload.clear();
                Err(self.latch(error))
            }
        }
    }

    /// Complete the pending checkpoint, then write the newest staged image if it was not written,
    /// all steps at once (specification 11 §6.7: the save completes before Stopped).
    ///
    /// # Errors
    ///
    /// [`RuntimeError::RetainStore`] when a step of either checkpoint fails, or when an earlier step
    /// already latched a failure (specification 11 §6.7: a requested-stop save failure is exposed
    /// rather than reported as a successful stop); the newest committed checkpoint stays readable.
    pub fn finish(
        &mut self,
        capture: &mut RetainCapture,
        now: Duration,
    ) -> Result<(), RuntimeError> {
        if let Some(error) = &self.failure {
            return Err(error.clone());
        }
        if self.writing {
            loop {
                match self.log.step(&self.payload, usize::MAX) {
                    Ok(Progress::Pending) => {}
                    Ok(Progress::Committed(_)) => {
                        self.writing = false;
                        break;
                    }
                    Err(error) => {
                        self.writing = false;
                        self.payload.clear();
                        return Err(self.latch(error));
                    }
                }
            }
        }
        let staged = capture.staged_payload();
        if staged.is_empty() || staged == self.payload.as_slice() {
            return Ok(());
        }
        self.payload.clear();
        self.payload.extend_from_slice(staged);
        match self.log.append(self.identity, &self.payload) {
            Ok(_record) => {
                capture.accept(now);
                Ok(())
            }
            Err(error) => {
                self.payload.clear();
                Err(self.latch(error))
            }
        }
    }

    /// The failure of a write that did not commit, handed out once (specification 11 §6.7: a retain
    /// failure fails the scan before physical outputs are committed).
    pub fn take_failure(&mut self) -> Option<RuntimeError> {
        self.failure.take()
    }

    /// Latch the error a write step or start reported, and return it for the caller that saw it.
    fn latch(&mut self, error: CheckpointError) -> RuntimeError {
        let error = RuntimeError::from(error);
        self.failure = Some(error.clone());
        error
    }
}
