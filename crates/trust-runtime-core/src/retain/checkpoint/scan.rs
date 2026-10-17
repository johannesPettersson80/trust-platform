//! Reading both slots, and the recovery rule (specification 34 §10.1).

use super::format::{self, RecordHeader, RecordHeaderBytes, SlotHeaderBytes, Unit};
use super::{
    CheckpointError, CheckpointFlash, Corruption, FlashGeometry, Slot, RECORD_HEADER_LEN,
    SLOT_HEADER_LEN,
};

/// A committed record: one whose commit field reads exactly as written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Committed {
    /// Its header is valid; the payload has not been checked.
    Record {
        /// Where the record starts in its slot.
        offset: u32,
        /// Its header.
        header: RecordHeader,
    },
    /// Its header fails its check.
    Damaged,
}

/// What reading one slot found.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct SlotState {
    /// The slot's epoch, when its header is valid.
    pub(super) epoch: Option<u32>,
    /// Where the slot's log ends in erased space; `None` when a failing header ended it.
    pub(super) end: Option<u32>,
    /// The slot's last committed record.
    pub(super) last: Option<Committed>,
    /// The highest sequence of a committed record with a valid header.
    pub(super) top_committed: Option<u64>,
    /// The highest sequence of any valid record header, committed or not.
    pub(super) top_sequence: Option<u64>,
}

/// What reading both slots found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Scan {
    slots: [SlotState; 2],
}

impl Scan {
    /// Read both slots of `flash`.
    pub(super) fn read<F: CheckpointFlash + ?Sized>(
        flash: &mut F,
        geometry: FlashGeometry,
    ) -> Result<Self, CheckpointError> {
        Ok(Self {
            slots: [
                read_slot(flash, geometry, Slot::A)?,
                read_slot(flash, geometry, Slot::B)?,
            ],
        })
    }

    pub(super) fn slot(&self, slot: Slot) -> &SlotState {
        &self.slots[slot.index()]
    }

    pub(super) fn slot_mut(&mut self, slot: Slot) -> &mut SlotState {
        &mut self.slots[slot.index()]
    }

    /// The slot with a valid header and the highest epoch; `A` when both have the same epoch,
    /// which no write produces.
    pub(super) fn active(&self) -> Option<Slot> {
        match (self.slots[0].epoch, self.slots[1].epoch) {
            (Some(a), Some(b)) if b > a => Some(Slot::B),
            (Some(_), _) => Some(Slot::A),
            (None, Some(_)) => Some(Slot::B),
            (None, None) => None,
        }
    }

    /// The newest committed record: the last committed record of the slot with the highest epoch
    /// that has one.
    pub(super) fn newest(&self) -> Option<(Slot, Committed)> {
        let active = self.active()?;
        [active, active.other()]
            .into_iter()
            .filter(|slot| self.slot(*slot).epoch.is_some())
            .find_map(|slot| self.slot(slot).last.map(|record| (slot, record)))
    }

    /// The recovery rule without the payload check: the newest committed record and where it is,
    /// `None` when neither slot holds one, or the damage that fails a warm start.
    pub(super) fn decide(&self) -> Result<Option<(Slot, u32, RecordHeader)>, CheckpointError> {
        let newest = match self.newest() {
            Some((_, Committed::Damaged)) => {
                return Err(CheckpointError::Corrupt(Corruption::RecordHeader));
            }
            Some((slot, Committed::Record { offset, header })) => Some((slot, offset, header)),
            None => None,
        };
        let newest_sequence = newest.map(|(_, _, header)| header.sequence);
        let hidden_newer = self.slots.iter().any(|state| {
            state.epoch.is_none()
                && state
                    .top_committed
                    .is_some_and(|sequence| newest_sequence.is_none_or(|newest| sequence > newest))
        });
        if hidden_newer {
            return Err(CheckpointError::Corrupt(Corruption::SlotHeader));
        }
        Ok(newest)
    }

    /// The highest sequence of any valid record header in either slot, or 0.
    pub(super) fn top_sequence(&self) -> u64 {
        self.slots
            .iter()
            .filter_map(|state| state.top_sequence)
            .max()
            .unwrap_or(0)
    }

    /// The highest epoch of either slot, or 0.
    pub(super) fn top_epoch(&self) -> u32 {
        self.slots
            .iter()
            .filter_map(|state| state.epoch)
            .max()
            .unwrap_or(0)
    }
}

/// Read one slot: its header, then its records in order. A slot whose header fails its check is
/// read too, so that newer records behind a damaged slot header are found.
fn read_slot<F: CheckpointFlash + ?Sized>(
    flash: &mut F,
    geometry: FlashGeometry,
    slot: Slot,
) -> Result<SlotState, CheckpointError> {
    let first = geometry.first_record_offset();
    let commit_len = geometry.commit_len() as usize;
    let slot_len = u64::from(geometry.slot_len());

    let mut unit: Unit = [0; RECORD_HEADER_LEN as usize];
    read(flash, slot, 0, &mut unit[..first as usize])?;
    let mut header: SlotHeaderBytes = [0; SLOT_HEADER_LEN as usize];
    header.copy_from_slice(&unit[..SLOT_HEADER_LEN as usize]);
    let mut state = SlotState {
        epoch: format::decode_slot_header(&header),
        ..SlotState::default()
    };

    // Each pass moves on by a whole record, at least the header and the commit field, so the
    // loop ends within the slot.
    let mut offset = first;
    loop {
        let fixed = u64::from(RECORD_HEADER_LEN) + commit_len as u64;
        if u64::from(offset) + fixed > slot_len {
            state.end = Some(offset);
            break;
        }
        let mut bytes: RecordHeaderBytes = [0; RECORD_HEADER_LEN as usize];
        read(flash, slot, offset, &mut bytes)?;
        let mut commit: Unit = [0; RECORD_HEADER_LEN as usize];
        let commit = &mut commit[..commit_len];
        read(flash, slot, offset + RECORD_HEADER_LEN, commit)?;
        if format::is_erased(&bytes) && format::is_erased(commit) {
            state.end = Some(offset);
            break;
        }
        let committed = format::is_committed(commit);
        let record = RecordHeader::decode(&bytes).and_then(|header| {
            let len = geometry.record_len(header.payload_len)?;
            (u64::from(offset) + u64::from(len) <= slot_len).then_some((header, len))
        });
        let Some((header, len)) = record else {
            if committed {
                state.last = Some(Committed::Damaged);
            }
            break;
        };
        state.top_sequence = state.top_sequence.max(Some(header.sequence));
        if committed {
            state.last = Some(Committed::Record { offset, header });
            state.top_committed = state.top_committed.max(Some(header.sequence));
        }
        offset += len;
    }
    Ok(state)
}

/// Read `bytes.len()` bytes from `offset` in `slot`.
pub(super) fn read<F: CheckpointFlash + ?Sized>(
    flash: &mut F,
    slot: Slot,
    offset: u32,
    bytes: &mut [u8],
) -> Result<(), CheckpointError> {
    flash
        .read(slot, offset, bytes)
        .map_err(CheckpointError::Flash)
}
