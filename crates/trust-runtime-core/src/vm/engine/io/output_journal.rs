//! The journal of a publication to output hooks (specification 34 MEM-09 and ARCH-05; checklist
//! C1-19f).
//!
//! Output hooks may refuse a publication after the output sample, before the drivers have taken
//! the image, and the image then goes back to the bytes it had. The publication copies the whole
//! output image into the copy that construction reserves with the staging, before it applies a
//! patch, and a refusal copies it back: every window that the patches and the hooks' writes
//! touched returns to the bytes it had, and every other byte has them already. The journal keeps
//! the charges of the journal of saved windows it replaces (TIME-08): an entry for each output
//! patch, charged when the publication opens the journal; an entry for each window a hook writes,
//! charged when the entries outgrow the room charged so far; and the bytes of every window.

use super::*;

/// The charge of one entry: the size of an entry of the journal of saved windows, the window's
/// byte offset and a vector of its bytes.
const ENTRY_BYTES: usize = core::mem::size_of::<usize>() + core::mem::size_of::<Vec<u8>>();

/// The entries one publication's journal has charged and the room charged for them.
#[derive(Debug)]
pub(in crate::vm::engine) struct OutputJournal {
    entries: usize,
    room: usize,
}

impl EngineState<'_> {
    /// Open the journal of a publication whose patches hold `windows` output windows, charging
    /// an entry for each.
    pub(in crate::vm::engine) fn open_output_journal(
        &self,
        windows: usize,
    ) -> Result<OutputJournal, RuntimeError> {
        self.charge_allocation_bytes(
            windows
                .checked_mul(ENTRY_BYTES)
                .ok_or(RuntimeError::Overflow)?,
        )?;
        Ok(OutputJournal {
            entries: 0,
            room: windows,
        })
    }

    /// Journal the output window of `width` bytes at byte `start`, which must lie in the image:
    /// its bytes are charged, and the room for one more entry when the entries have filled the
    /// room charged so far.
    pub(in crate::vm::engine) fn journal_output_window(
        &self,
        journal: &mut OutputJournal,
        start: usize,
        width: usize,
    ) -> Result<(), RuntimeError> {
        let end = start.checked_add(width).ok_or(RuntimeError::Overflow)?;
        if self.data.images.outputs.get(start..end).is_none() {
            return Err(RuntimeError::TypeMismatch);
        }
        self.charge_allocation_bytes(width)?;
        if journal.entries == journal.room {
            let room = journal
                .entries
                .checked_add(1)
                .ok_or(RuntimeError::Overflow)?;
            self.charge_allocation_bytes(
                room.checked_mul(ENTRY_BYTES)
                    .ok_or(RuntimeError::Overflow)?,
            )?;
            journal.room = room;
        }
        journal.entries += 1;
        Ok(())
    }
}
