//! The retained-value check of an install on a chip (specification 34 §11.2, decision G13;
//! specification 11 §6.7.1).
//!
//! The check runs the new program's warm start with the retained values of the newest committed
//! checkpoint, before any byte of the install is written, as the warm start of the boot will run it:
//! the checkpoint is read and decoded as a warm start reads it, and applied by name to a fresh
//! construction of the new program, which is dropped afterwards. A checkpoint that is damaged, or a
//! value the new program refuses, is the check's refusal. A program that keeps no retained values
//! (`[runtime.retain] mode = "none"`) restores nothing at its start, and a log with no committed
//! checkpoint has nothing to restore.
//!
//! The device builds its check from this function and gives it to the loop with its program store
//! ([`Runner::with_program_store`](crate::runner::Runner::with_program_store)). The function is
//! generic, so its code is instantiated in the device's own crate, next to the engine's, and a chip
//! image holds one copy of the code that the check and the engine share. The checkpoint slots are
//! read through `ReadOnly`, so that the check can never write them.

use alloc::vec::Vec;

use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::retain::checkpoint::{
    CheckpointFlash, CheckpointLog, FlashError, FlashGeometry, Slot,
};
use trust_runtime_core::retain::codec::decode_snapshot_payload_v3;
use trust_runtime_core::retain::{RestartMode, RetainSnapshot};
use trust_runtime_core::vm::PreparedModule;
use trust_runtime_core::watchdog::RetainMode;

use crate::installation::Admitted;
use crate::runner::engine::RESOURCE_INDEX;

/// A checkpoint flash that is read and never written: its program and erase fail.
struct ReadOnly<'a, F: ?Sized>(&'a mut F);

impl<F: CheckpointFlash + ?Sized> CheckpointFlash for ReadOnly<'_, F> {
    fn geometry(&self) -> FlashGeometry {
        self.0.geometry()
    }

    fn read(&mut self, slot: Slot, offset: u32, bytes: &mut [u8]) -> Result<(), FlashError> {
        self.0.read(slot, offset, bytes)
    }

    fn program(&mut self, _slot: Slot, _offset: u32, _bytes: &[u8]) -> Result<(), FlashError> {
        Err(FlashError::Program)
    }

    fn erase(&mut self, _slot: Slot, _offset: u32) -> Result<(), FlashError> {
        Err(FlashError::Erase)
    }
}

/// Checks that the retained values of the newest checkpoint in `checkpoints` can be restored into
/// the program `admitted`, as its warm start restores them (specification 11 §6.7.1). The checkpoint
/// slots are only read.
///
/// # Errors
///
/// A retained-value error when the newest checkpoint is damaged or its flash cannot be read, and the
/// warm start's own error, which names the retained value and its reason, when the program refuses
/// a value. Any other error means that the program could not be built to try its warm start.
pub fn check_retained_values<F: CheckpointFlash + ?Sized>(
    checkpoints: &mut F,
    admitted: &Admitted<'_>,
) -> Result<(), RuntimeError> {
    if admitted.record.configuration.retain_mode == RetainMode::None {
        return Ok(());
    }
    let Some(snapshot) = newest_snapshot(checkpoints)? else {
        return Ok(());
    };
    let prepared = PreparedModule::from_bytes(admitted.program, admitted.limits)?;
    let mut candidate = prepared.instantiate(RESOURCE_INDEX)?;
    candidate.restart_with_retain_snapshot(RestartMode::Warm, &snapshot)?;
    Ok(())
}

/// The retained values of the newest committed checkpoint, decoded as a warm start decodes them, or
/// `None` when the log holds no committed checkpoint.
fn newest_snapshot<F: CheckpointFlash + ?Sized>(
    checkpoints: &mut F,
) -> Result<Option<RetainSnapshot>, RuntimeError> {
    let mut log = CheckpointLog::open(ReadOnly(checkpoints))?;
    let Some(newest) = log.newest()? else {
        return Ok(None);
    };
    let length = usize::try_from(newest.payload_len).map_err(|_| {
        RuntimeError::RetainCorruption("retain payload length exceeds platform size".into())
    })?;
    let mut payload = Vec::new();
    payload
        .try_reserve_exact(length)
        .map_err(|_| RuntimeError::Overflow)?;
    payload.resize(length, 0);
    log.recover(&mut payload)?;
    decode_snapshot_payload_v3(&payload).map(Some)
}
