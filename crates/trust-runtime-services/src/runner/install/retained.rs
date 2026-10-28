//! The retained-value check of an install on a chip (specification 34 §11.2, decision G13;
//! specification 11 §6.7.1; specification 44 §11).
//!
//! The check restores the retained values of the newest committed checkpoint into the new image,
//! before any byte of the install is written, as the warm start of the boot will restore them: the
//! checkpoint is read and decoded as a warm start reads it, and applied by name against the image's
//! type table. A checkpoint that is damaged, or a value the new image refuses, is the check's
//! refusal. A program that keeps no retained values (`[runtime.retain] mode = "none"`) restores
//! nothing at its start, and a log with no committed checkpoint has nothing to restore.
//!
//! The device builds its check from this function and gives it to the loop with its program store
//! ([`Runner::with_program_store`](crate::runner::Runner::with_program_store)). The checkpoint slots
//! are read through `ReadOnly`, so that the check can never write them.

use alloc::vec::Vec;

use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::retain::checkpoint::{
    CheckpointFlash, CheckpointLog, FlashError, FlashGeometry, Slot,
};
use trust_runtime_core::retain::codec::decode_snapshot_payload_v3;
use trust_runtime_core::retain::RetainSnapshot;
use trust_runtime_core::watchdog::RetainMode;

use crate::installation::Admitted;

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

/// Checks that the retained values of the newest checkpoint in `checkpoints` restore into the
/// compiled program image `admitted`, as its warm start after a power-on restores them, by name and
/// against its type table (specification 44 §11; specification 11 §6.7.1). The checkpoint slots
/// are only read. An image whose tables do not read is left to the kernel's checks that follow in
/// the install, which refuse it with their own reason (specification 44 §10.2).
///
/// # Errors
///
/// A retained-value error when the newest checkpoint is damaged or its flash cannot be read, and the
/// restore's own error, which names the retained value and its reason, when the image refuses a
/// value.
pub fn check_image_retained_values<F: CheckpointFlash + ?Sized>(
    checkpoints: &mut F,
    admitted: &Admitted<'_>,
) -> Result<(), RuntimeError> {
    use trust_runtime_core::native::image::Image;

    if admitted.record.configuration.retain_mode == RetainMode::None {
        return Ok(());
    }
    let Some(image) = Image::parse(admitted.program)
        .ok()
        .filter(|image| image.check_tables().is_ok())
    else {
        return Ok(());
    };
    let Some(snapshot) = newest_checkpoint(checkpoints)? else {
        return Ok(());
    };
    trust_runtime_core::native::retain::check(&image, &snapshot).map(drop)
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
