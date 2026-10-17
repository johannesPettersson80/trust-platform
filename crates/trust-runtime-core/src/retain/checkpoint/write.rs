//! The order of a checkpoint write (specification 34 §10.1).
//!
//! A write appends to the slot with the highest epoch when the record fits in the erased space
//! after that slot's log. Otherwise it erases the slot that does not hold the newest committed
//! record, unit by unit, programs that slot's header with the next epoch, and appends there. Either
//! way it programs the record header, then the payload, reads back everything it programmed, and
//! programs the commit field last. Before each program it checks that the flash reads as erased.

use super::format::{self, RecordHeader, RecordHeaderBytes, Unit, ERASED};
use super::scan::{self, Committed, Scan, SlotState};
use super::{
    CheckpointError, CheckpointFlash, CheckpointIdentity, FlashGeometry, RecordInfo, Slot,
    RECORD_HEADER_LEN, SLOT_HEADER_LEN,
};
use crate::crc32;

/// The bytes the log reads at a time when it compares or checks flash.
const CHUNK: usize = 256;

/// A read buffer, word-aligned so that a driver which moves whole words can fill it directly.
#[repr(C, align(4))]
struct Chunk([u8; CHUNK]);

/// The part of the write that the next program step writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Part {
    SlotHeader,
    RecordHeader,
    Payload,
    PayloadTail,
    Commit,
}

/// A write that has begun: where the record goes and how far the write has come.
#[derive(Debug, Clone, Copy)]
pub(super) struct PendingWrite {
    slot: Slot,
    /// The next erase unit to erase; equal to `erase_units` once no erase is left.
    erase_next: u32,
    erase_units: u32,
    /// The epoch of the slot header to program after the erase, when the write formats the slot.
    epoch: Option<u32>,
    /// Where the record starts in the slot.
    offset: u32,
    /// The bytes the record takes.
    record_len: u32,
    header: RecordHeader,
    header_bytes: RecordHeaderBytes,
    part: Part,
    /// The bytes of `part` already programmed.
    done: u32,
    verified: bool,
}

impl PendingWrite {
    /// Choose where the record goes, from what the last reading of both slots found.
    pub(super) fn plan<F: CheckpointFlash + ?Sized>(
        flash: &mut F,
        geometry: FlashGeometry,
        scan: &Scan,
        identity: CheckpointIdentity,
        payload: &[u8],
    ) -> Result<Self, CheckpointError> {
        let too_large = CheckpointError::TooLarge {
            len: payload.len(),
            max: geometry.max_payload_len(),
        };
        let payload_len = u32::try_from(payload.len()).map_err(|_| too_large)?;
        let record_len = geometry.record_len(payload_len).ok_or(too_large)?;
        let sequence = scan
            .top_sequence()
            .checked_add(1)
            .ok_or(CheckpointError::Exhausted)?;
        let header = RecordHeader {
            sequence,
            identity,
            payload_len,
            payload_crc: crc32::checksum(payload),
        };

        if let Some(slot) = scan.active()
            && let Some(end) = scan.slot(slot).end
            && u64::from(end) + u64::from(record_len) <= u64::from(geometry.slot_len())
            && is_erased(flash, slot, end, record_len)?
        {
            return Ok(Self::new(slot, None, 0, end, header, record_len));
        }

        // The slot that holds the newest committed record, valid or damaged, is never erased.
        let target = match scan.newest() {
            Some((slot, _)) => slot.other(),
            None => scan.active().map_or(Slot::A, Slot::other),
        };
        let epoch = scan
            .top_epoch()
            .checked_add(1)
            .ok_or(CheckpointError::Exhausted)?;
        Ok(Self::new(
            target,
            Some(epoch),
            geometry.erase_units(),
            geometry.first_record_offset(),
            header,
            record_len,
        ))
    }

    fn new(
        slot: Slot,
        epoch: Option<u32>,
        erase_units: u32,
        offset: u32,
        header: RecordHeader,
        record_len: u32,
    ) -> Self {
        Self {
            slot,
            erase_next: 0,
            erase_units,
            epoch,
            offset,
            record_len,
            header,
            header_bytes: header.encode(),
            part: if epoch.is_some() {
                Part::SlotHeader
            } else {
                Part::RecordHeader
            },
            done: 0,
            verified: false,
        }
    }

    /// Run the next step: erase one unit, or program at most `max_program_bytes` (rounded down to
    /// whole program units, at least one unit). The step that programs the commit field first
    /// reads back everything the write programmed. Returns the record once it is committed.
    pub(super) fn step<F: CheckpointFlash + ?Sized>(
        &mut self,
        flash: &mut F,
        geometry: FlashGeometry,
        payload: &[u8],
        max_program_bytes: usize,
    ) -> Result<Option<RecordInfo>, CheckpointError> {
        if payload.len() != self.header.payload_len as usize {
            return Err(CheckpointError::PayloadChanged);
        }
        if self.erase_next < self.erase_units {
            flash
                .erase(self.slot, self.erase_next * geometry.erase_unit())
                .map_err(CheckpointError::Flash)?;
            self.erase_next += 1;
            return Ok(None);
        }

        let unit = geometry.program_unit();
        let budget = u32::try_from(max_program_bytes)
            .unwrap_or(u32::MAX)
            .max(unit)
            & !(unit - 1);
        let payload_start = geometry.payload_offset(self.offset);
        let full = self.header.payload_len & !(unit - 1);
        loop {
            match self.part {
                Part::SlotHeader => {
                    let field = self.slot_header_field();
                    let len = geometry.first_record_offset() as usize;
                    self.program_part(flash, 0, &field[..len], budget, Part::RecordHeader)?;
                    return Ok(None);
                }
                Part::RecordHeader => {
                    let bytes = self.header_bytes;
                    self.program_part(flash, self.offset, &bytes, budget, Part::Payload)?;
                    return Ok(None);
                }
                Part::Payload if full == 0 => self.part = Part::PayloadTail,
                Part::Payload => {
                    let bytes = &payload[..full as usize];
                    self.program_part(flash, payload_start, bytes, budget, Part::PayloadTail)?;
                    return Ok(None);
                }
                Part::PayloadTail if full == self.header.payload_len => self.part = Part::Commit,
                Part::PayloadTail => {
                    let tail = tail_unit(payload, full);
                    let bytes = &tail[..unit as usize];
                    self.program_part(flash, payload_start + full, bytes, budget, Part::Commit)?;
                    return Ok(None);
                }
                Part::Commit => {
                    if !self.verified {
                        self.verify(flash, geometry, payload)?;
                        self.verified = true;
                    }
                    let field = format::commit_field();
                    let bytes = &field[..geometry.commit_len() as usize];
                    let at = self.offset + RECORD_HEADER_LEN;
                    if self.program_part(flash, at, bytes, budget, Part::Commit)? {
                        return Ok(Some(RecordInfo {
                            sequence: self.header.sequence,
                            identity: self.header.identity,
                            payload_len: self.header.payload_len,
                        }));
                    }
                    return Ok(None);
                }
            }
        }
    }

    /// Record the committed write in `scan`, as reading both slots again would find it.
    pub(super) fn apply(&self, scan: &mut Scan) {
        let state = scan.slot_mut(self.slot);
        if let Some(epoch) = self.epoch {
            *state = SlotState {
                epoch: Some(epoch),
                ..SlotState::default()
            };
        }
        state.end = Some(self.offset + self.record_len);
        state.last = Some(Committed::Record {
            offset: self.offset,
            header: self.header,
        });
        state.top_committed = state.top_committed.max(Some(self.header.sequence));
        state.top_sequence = state.top_sequence.max(Some(self.header.sequence));
    }

    /// Program the next chunk of `bytes`, which go at `start`: at most `budget` bytes from where
    /// the part stands. Moves on to `next` and returns `true` once `bytes` are complete.
    fn program_part<F: CheckpointFlash + ?Sized>(
        &mut self,
        flash: &mut F,
        start: u32,
        bytes: &[u8],
        budget: u32,
        next: Part,
    ) -> Result<bool, CheckpointError> {
        let done = self.done as usize;
        let len = (bytes.len() - done).min(budget as usize);
        program_erased(
            flash,
            self.slot,
            start + self.done,
            &bytes[done..done + len],
        )?;
        self.done += len as u32;
        if self.done as usize == bytes.len() {
            self.done = 0;
            self.part = next;
            return Ok(true);
        }
        Ok(false)
    }

    /// The slot header followed by erased bytes, the length of the largest program unit.
    fn slot_header_field(&self) -> Unit {
        let mut field = [ERASED; RECORD_HEADER_LEN as usize];
        let header = format::encode_slot_header(self.epoch.unwrap_or(0));
        field[..SLOT_HEADER_LEN as usize].copy_from_slice(&header);
        field
    }

    /// Check that the payload is the one the write began with and that the flash reads back
    /// everything the write programmed.
    fn verify<F: CheckpointFlash + ?Sized>(
        &self,
        flash: &mut F,
        geometry: FlashGeometry,
        payload: &[u8],
    ) -> Result<(), CheckpointError> {
        if crc32::checksum(payload) != self.header.payload_crc {
            return Err(CheckpointError::PayloadChanged);
        }
        let unit = geometry.program_unit();
        if self.epoch.is_some() {
            let field = self.slot_header_field();
            let len = geometry.first_record_offset() as usize;
            reads_as(flash, self.slot, 0, &field[..len])?;
        }
        reads_as(flash, self.slot, self.offset, &self.header_bytes)?;
        let payload_start = geometry.payload_offset(self.offset);
        let full = self.header.payload_len & !(unit - 1);
        reads_as(flash, self.slot, payload_start, &payload[..full as usize])?;
        if full < self.header.payload_len {
            let tail = tail_unit(payload, full);
            reads_as(
                flash,
                self.slot,
                payload_start + full,
                &tail[..unit as usize],
            )?;
        }
        Ok(())
    }
}

/// The last, partial program unit of a payload: its bytes from `full`, then erased bytes.
fn tail_unit(payload: &[u8], full: u32) -> Unit {
    let mut unit = [ERASED; RECORD_HEADER_LEN as usize];
    let rest = &payload[full as usize..];
    unit[..rest.len()].copy_from_slice(rest);
    unit
}

/// Program `bytes` at `offset` in `slot`, after checking that the flash there reads as erased.
fn program_erased<F: CheckpointFlash + ?Sized>(
    flash: &mut F,
    slot: Slot,
    offset: u32,
    bytes: &[u8],
) -> Result<(), CheckpointError> {
    if !is_erased(flash, slot, offset, bytes.len() as u32)? {
        return Err(CheckpointError::NotErased);
    }
    flash
        .program(slot, offset, bytes)
        .map_err(CheckpointError::Flash)
}

/// Whether `len` bytes from `offset` in `slot` read as erased.
fn is_erased<F: CheckpointFlash + ?Sized>(
    flash: &mut F,
    slot: Slot,
    offset: u32,
    len: u32,
) -> Result<bool, CheckpointError> {
    let mut chunk = Chunk([0; CHUNK]);
    let mut at = 0;
    while at < len {
        let take = (len - at).min(CHUNK as u32);
        let bytes = &mut chunk.0[..take as usize];
        scan::read(flash, slot, offset + at, bytes)?;
        if !format::is_erased(bytes) {
            return Ok(false);
        }
        at += take;
    }
    Ok(true)
}

/// Check that the flash from `offset` in `slot` reads as `expected`.
fn reads_as<F: CheckpointFlash + ?Sized>(
    flash: &mut F,
    slot: Slot,
    offset: u32,
    expected: &[u8],
) -> Result<(), CheckpointError> {
    let mut chunk = Chunk([0; CHUNK]);
    for (index, part) in expected.chunks(CHUNK).enumerate() {
        let bytes = &mut chunk.0[..part.len()];
        scan::read(flash, slot, offset + (index * CHUNK) as u32, bytes)?;
        if bytes != part {
            return Err(CheckpointError::Verify);
        }
    }
    Ok(())
}
