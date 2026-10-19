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
//!
//! A chip's heap is small, and the shared dry run of an install prepares the program with the limits
//! its record requested, which are the hosted maxima. [`check_install`] therefore prepares the program
//! first, with the limits the heap allows now (specification 34 §11.2 and §11.3), and a program that
//! the heap cannot hold is refused before anything is allocated that could reset the chip.

use alloc::vec::Vec;

use smol_str::SmolStr;
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::retain::checkpoint::{
    CheckpointFlash, CheckpointLog, FlashError, FlashGeometry, Slot,
};
use trust_runtime_core::retain::codec::decode_snapshot_payload_v3;
use trust_runtime_core::retain::{RestartMode, RetainSnapshot};
use trust_runtime_core::vm::PreparedModule;
use trust_runtime_core::watchdog::RetainMode;

use crate::installation::Admitted;
use crate::runner::engine::{DeviceMemory, RESOURCE_INDEX};
use crate::runner::online_change::{is_memory_limit, ChangeMemory};

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
    let Some(snapshot) = newest_checkpoint(checkpoints)? else {
        return Ok(());
    };
    let prepared = PreparedModule::from_bytes(admitted.program, admitted.limits)?;
    warm_start(&prepared, &snapshot)
}

/// The check of an install on a chip (specification 34 §11.2, decision G13): the program `admitted`
/// is prepared with the limits that the heap allows now, and then [`check_retained_values`] runs on the
/// module it prepared.
///
/// The heap's room is `room` bytes, less `reserve` bytes kept for the machine, and `memory` is the
/// device's memory, as for an online change's candidate (specification 34 §11.3): the limits are
/// [`ChangeMemory::limits`]. A program that needs more than those limits is refused with
/// `insufficient memory` and its numbers, as the error of this check (the shared install check puts
/// `the retained-value check could not run: ` before it), and the shared dry run of the install, which
/// prepares the program with the limits its record requested, does not run. A refusal changes
/// nothing: the module and the candidate this check builds are dropped before it returns.
///
/// # Errors
///
/// The `insufficient memory` refusal above, or the error of [`check_retained_values`].
pub fn check_install<F: CheckpointFlash + ?Sized>(
    checkpoints: &mut F,
    admitted: &Admitted<'_>,
    memory: DeviceMemory,
    room: usize,
    reserve: usize,
) -> Result<(), RuntimeError> {
    let budget = ChangeMemory::for_install(admitted.program.len(), room, reserve);
    let limits = budget
        .limits(memory)
        .map_err(|text| RuntimeError::ControlError(SmolStr::new(text)))?;
    let prepared = PreparedModule::from_bytes(admitted.program, limits)
        .map_err(|error| shortage_or(budget, error))?;
    if admitted.record.configuration.retain_mode == RetainMode::None {
        return Ok(());
    }
    let Some(snapshot) = newest_checkpoint(checkpoints)? else {
        return Ok(());
    };
    warm_start(&prepared, &snapshot).map_err(|error| shortage_or(budget, error))
}

/// `error` as the `insufficient memory` refusal when the memory limits refused it, and as it is
/// otherwise.
fn shortage_or(budget: ChangeMemory, error: RuntimeError) -> RuntimeError {
    if is_memory_limit(&error) {
        RuntimeError::ControlError(SmolStr::new(budget.refusal(&error)))
    } else {
        error
    }
}

/// The warm start of `prepared` with the retained values of `snapshot`, tried on a fresh
/// construction that is dropped afterwards.
fn warm_start(prepared: &PreparedModule, snapshot: &RetainSnapshot) -> Result<(), RuntimeError> {
    let mut candidate = prepared.instantiate(RESOURCE_INDEX)?;
    candidate.restart_with_retain_snapshot(RestartMode::Warm, snapshot)?;
    Ok(())
}

/// The retained values of the newest committed checkpoint in `checkpoints`, decoded as a warm start
/// decodes them, or `None` when the log holds no committed checkpoint. The slots are only read.
///
/// A chip's online change reads its second step through this (specification 11 §6.7.1): the newest
/// checkpoint is the saved snapshot that a computer reads from its retain store.
///
/// # Errors
///
/// [`RuntimeError::RetainCorruption`] when the newest checkpoint fails its checks,
/// [`RuntimeError::RetainStore`] when the flash cannot be read, and [`RuntimeError::Overflow`] when
/// the heap cannot hold the checkpoint's payload.
pub fn newest_checkpoint<F: CheckpointFlash + ?Sized>(
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

/// The length of the payload that the newest committed checkpoint in `checkpoints` holds: the bytes
/// that [`newest_checkpoint`] reads whole into the heap. It is read from the header of the record, and
/// nothing of the payload is read. It is 0 when the log holds no committed checkpoint, which has
/// nothing to read. The slots are only read.
///
/// A device that keeps the heap free for a retained step asks this before it takes the room, so
/// that the room it keeps is the checkpoint's own payload and not the largest one a slot takes
/// (specification 34 §11.3).
///
/// # Errors
///
/// [`RuntimeError::RetainCorruption`] when the header of the newest checkpoint fails its check,
/// [`RuntimeError::RetainStore`] when the flash cannot be read: the header cannot say how many bytes a
/// read would take.
pub fn newest_checkpoint_payload_len<F: CheckpointFlash + ?Sized>(
    checkpoints: &mut F,
) -> Result<usize, RuntimeError> {
    let mut log = CheckpointLog::open(ReadOnly(checkpoints))?;
    let Some(newest) = log.newest()? else {
        return Ok(0);
    };
    usize::try_from(newest.payload_len).map_err(|_| {
        RuntimeError::RetainCorruption("retain payload length exceeds platform size".into())
    })
}

#[cfg(test)]
mod contract_tests;
