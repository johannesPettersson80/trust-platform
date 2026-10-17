//! The checkpoint log over a chip's flash (specification 34 §10.1).

use super::format::{RecordHeader, Unit};
use super::scan::{self, Scan};
use super::write::PendingWrite;
use super::{
    CheckpointError, CheckpointFlash, CheckpointIdentity, Corruption, FlashGeometry, Progress,
    RecordInfo, Slot, RECORD_HEADER_LEN,
};
use crate::crc32;

/// The checkpoint log in a chip's two checkpoint slots (specification 34 §10.1).
///
/// [`open`](Self::open) reads both slots; [`recover`](Self::recover) applies the recovery rule;
/// [`append`](Self::append), or [`begin`](Self::begin) followed by [`step`](Self::step), writes
/// a checkpoint. A write that fails leaves the log as an interruption would; the next call reads
/// both slots again before it acts.
pub struct CheckpointLog<F> {
    flash: F,
    geometry: FlashGeometry,
    scan: Scan,
    /// Set when a write failed, so the slots must be read again.
    stale: bool,
    write: Option<PendingWrite>,
}

impl<F: CheckpointFlash> CheckpointLog<F> {
    /// Read both slots of `flash`.
    ///
    /// # Errors
    ///
    /// [`CheckpointError::Flash`] when the flash cannot be read.
    pub fn open(mut flash: F) -> Result<Self, CheckpointError> {
        let geometry = flash.geometry();
        let scan = Scan::read(&mut flash, geometry)?;
        Ok(Self {
            flash,
            geometry,
            scan,
            stale: false,
            write: None,
        })
    }

    /// The flash's geometry.
    #[must_use]
    pub fn geometry(&self) -> FlashGeometry {
        self.geometry
    }

    /// The newest committed record by the recovery rule, without checking its payload; `None`
    /// when neither slot holds a committed record, as for a missing retain file.
    ///
    /// # Errors
    ///
    /// [`CheckpointError::Corrupt`] when the newest committed record's header fails its check, or
    /// a slot whose header fails its check holds a newer record; [`CheckpointError::Flash`] when
    /// the slots must be read again and cannot be.
    pub fn newest(&mut self) -> Result<Option<RecordInfo>, CheckpointError> {
        self.refresh()?;
        Ok(self.scan.decide()?.map(|(_, _, header)| info(&header)))
    }

    /// The recovery rule: read the newest committed record's payload into the front of `payload`
    /// and return the record, or `None` when neither slot holds a committed record. A record that
    /// fails its checks is never replaced by an older one.
    ///
    /// # Errors
    ///
    /// [`CheckpointError::Corrupt`] when the newest committed record fails its checks, which fails
    /// a warm start; [`CheckpointError::BufferTooSmall`] when `payload` is shorter than its
    /// payload; [`CheckpointError::Flash`] when the flash cannot be read.
    pub fn recover(&mut self, payload: &mut [u8]) -> Result<Option<RecordInfo>, CheckpointError> {
        self.refresh()?;
        let Some((slot, offset, header)) = self.scan.decide()? else {
            return Ok(None);
        };
        let Some(target) = payload.get_mut(..header.payload_len as usize) else {
            return Err(CheckpointError::BufferTooSmall {
                needed: header.payload_len,
            });
        };
        let start = self.geometry.payload_offset(offset);
        read_payload(&mut self.flash, self.geometry, slot, start, target)?;
        if crc32::checksum(target) != header.payload_crc {
            return Err(CheckpointError::Corrupt(Corruption::Payload));
        }
        Ok(Some(info(&header)))
    }

    /// Begin writing a checkpoint of `payload` for `identity`. No flash operation runs until
    /// [`step`](Self::step), and every step must be given the same payload.
    ///
    /// # Errors
    ///
    /// [`CheckpointError::WriteInProgress`] while another write is pending;
    /// [`CheckpointError::TooLarge`] when the payload does not fit in an empty slot;
    /// [`CheckpointError::Exhausted`] when the sequence or the slot epoch cannot count further;
    /// [`CheckpointError::Flash`] when the flash cannot be read.
    pub fn begin(
        &mut self,
        identity: CheckpointIdentity,
        payload: &[u8],
    ) -> Result<(), CheckpointError> {
        if self.write.is_some() {
            return Err(CheckpointError::WriteInProgress);
        }
        self.refresh()?;
        let write = PendingWrite::plan(
            &mut self.flash,
            self.geometry,
            &self.scan,
            identity,
            payload,
        )?;
        self.write = Some(write);
        Ok(())
    }

    /// Run the next step of the pending write: erase one unit, or program at most
    /// `max_program_bytes`, rounded down to whole program units and at least one unit. The step
    /// that programs the commit field first reads back everything the write programmed.
    ///
    /// # Errors
    ///
    /// [`CheckpointError::NoWrite`] when no write is pending; [`CheckpointError::PayloadChanged`]
    /// when `payload` is not the payload the write began with; [`CheckpointError::Flash`],
    /// [`CheckpointError::NotErased`] or [`CheckpointError::Verify`] when the flash fails. A failed
    /// step ends the write without committing it.
    pub fn step(
        &mut self,
        payload: &[u8],
        max_program_bytes: usize,
    ) -> Result<Progress, CheckpointError> {
        let Some(write) = self.write.as_mut() else {
            return Err(CheckpointError::NoWrite);
        };
        match write.step(&mut self.flash, self.geometry, payload, max_program_bytes) {
            Ok(None) => Ok(Progress::Pending),
            Ok(Some(record)) => {
                write.apply(&mut self.scan);
                self.write = None;
                Ok(Progress::Committed(record))
            }
            Err(error) => {
                self.write = None;
                self.stale = true;
                Err(error)
            }
        }
    }

    /// Write a checkpoint of `payload` for `identity`, all steps at once.
    ///
    /// # Errors
    ///
    /// The errors of [`begin`](Self::begin) and [`step`](Self::step).
    pub fn append(
        &mut self,
        identity: CheckpointIdentity,
        payload: &[u8],
    ) -> Result<RecordInfo, CheckpointError> {
        self.begin(identity, payload)?;
        loop {
            if let Progress::Committed(record) = self.step(payload, usize::MAX)? {
                return Ok(record);
            }
        }
    }

    /// Read both slots again after a failed write.
    fn refresh(&mut self) -> Result<(), CheckpointError> {
        if self.stale {
            self.scan = Scan::read(&mut self.flash, self.geometry)?;
            self.stale = false;
        }
        Ok(())
    }
}

fn info(header: &RecordHeader) -> RecordInfo {
    RecordInfo {
        sequence: header.sequence,
        identity: header.identity,
        payload_len: header.payload_len,
    }
}

/// Read a payload into `target`: its whole program units directly, its last partial unit through
/// a unit buffer.
fn read_payload<F: CheckpointFlash + ?Sized>(
    flash: &mut F,
    geometry: FlashGeometry,
    slot: Slot,
    start: u32,
    target: &mut [u8],
) -> Result<(), CheckpointError> {
    let unit = geometry.program_unit() as usize;
    let full = target.len() & !(unit - 1);
    let (whole, rest) = target.split_at_mut(full);
    if !whole.is_empty() {
        scan::read(flash, slot, start, whole)?;
    }
    if !rest.is_empty() {
        let mut tail: Unit = [0; RECORD_HEADER_LEN as usize];
        scan::read(flash, slot, start + full as u32, &mut tail[..unit])?;
        rest.copy_from_slice(&tail[..rest.len()]);
    }
    Ok(())
}
