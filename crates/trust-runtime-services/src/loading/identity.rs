//! The identities `program.active` reports (specification 34 §11.2), read from the program store in
//! bounded reads.
//!
//! A chip's containers can be far larger than its memory, and the container that is not running holds
//! nothing the chip needs but its identity. So the identity of the selected container and of the one
//! before it is taken from the container's header and its record alone: the header names the install
//! sequence, the program's length and the record's place, and the record names the SHA-256 of the
//! program ([`DeploymentRecord`]). Every read is at most the store's step budget, and no read touches
//! the program. The program is checked when it starts (specification 34 §5.1), not when its identity
//! is asked for.

use alloc::vec;
use alloc::vec::Vec;
use core::fmt;
use core::ops::Range;

use trust_runtime_core::crc32;
use trust_runtime_core::deployment::{DeploymentRecord, RECORD_MAX_BYTES};
use trust_runtime_core::ports::ProgramStore;
use trust_runtime_core::retain::checkpoint::{FlashError, Slot};

use super::install::step_bytes;
use super::read_padded;
use super::select::{self, ScanError, SlotHeader};
use crate::installation::{self, SEQUENCED_HEADER_BYTES};
use crate::ports::{ProgramIdentities, ProgramIdentity};

/// Why the identities of the program store cannot be read (specification 34 §11.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdentityError {
    /// The committed containers cannot be ordered: two have one sequence, or a header cannot be read.
    Scan(ScanError),
    /// A read of a container's header or record failed in the flash.
    Flash(FlashError),
    /// The memory has no room for the record of a container.
    NoRoom {
        /// The record's length in bytes.
        needed: usize,
    },
}

impl fmt::Display for IdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Scan(error) => fmt::Display::fmt(error, formatter),
            Self::Flash(error) => write!(formatter, "the program store's flash {error} failed"),
            Self::NoRoom { needed } => {
                write!(formatter, "no room to read a record of {needed} bytes")
            }
        }
    }
}

impl core::error::Error for IdentityError {}

/// The selected program and the program before it, as `program.active` reports them: the newest
/// committed container and the other committed container, each by its identity.
///
/// A member is `None` when the store holds no such container, or when the entry table or the record
/// of the container fails its check (the record's CRC-32 and its decoding). Nothing but the two
/// headers, the two entry tables and the two records is read, in reads of at most the store's step
/// budget.
///
/// # Errors
///
/// [`IdentityError`] when the containers cannot be ordered, a read fails or a record does not fit
/// the memory.
pub fn identities(store: &mut ProgramStore) -> Result<ProgramIdentities, IdentityError> {
    let order = select::order(store).map_err(IdentityError::Scan)?;
    let active = order
        .newest
        .map(|container| identity_of(store, container))
        .transpose()?
        .flatten();
    let previous = order
        .older
        .map(|container| identity_of(store, container))
        .transpose()?
        .flatten();
    Ok(ProgramIdentities { active, previous })
}

/// The identity of the committed container `container`, or `None` when its entry table or its record
/// fails its check.
fn identity_of(
    store: &mut ProgramStore,
    container: SlotHeader,
) -> Result<Option<ProgramIdentity>, IdentityError> {
    let header = read_padded(store, container.slot, 0, SEQUENCED_HEADER_BYTES)
        .map_err(IdentityError::Flash)?;
    let Ok(layout) = installation::sequenced_layout(&header, container.header.total_len) else {
        return Ok(None);
    };
    // A record longer than the longest one is refused by its decoder; it is not read to find out.
    if layout.record.len() > RECORD_MAX_BYTES {
        return Ok(None);
    }
    let bytes = read_chunked(store, container.slot, &layout.record)?;
    if crc32::checksum(&bytes) != layout.record_crc {
        return Ok(None);
    }
    let Ok(record) = DeploymentRecord::decode(&bytes) else {
        return Ok(None);
    };
    Ok(Some(ProgramIdentity {
        sequence: container.header.sequence,
        program_sha256: record.application.program_sha256,
        bytes: layout.program_bytes,
    }))
}

/// Reads the bytes in `range` of `slot` in reads of at most the store's step budget, rounded down to
/// whole program units and at least one unit. Each read starts at a program unit and covers whole
/// units, which is all the flash takes; the bytes outside `range` that a read covers are dropped.
fn read_chunked(
    store: &mut ProgramStore,
    slot: Slot,
    range: &Range<usize>,
) -> Result<Vec<u8>, IdentityError> {
    let unit_bytes = store.flash.geometry().program_unit();
    let unit = unit_bytes as usize;
    let chunk = step_bytes(store.program_budget, unit_bytes) as usize;
    let needed = range.len();
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(needed)
        .map_err(|_| IdentityError::NoRoom { needed })?;
    let mut buffer = vec![0xff_u8; chunk];
    let mut at = range.start - range.start % unit;
    let stop = range.end.next_multiple_of(unit);
    while at < stop {
        let length = chunk.min(stop - at);
        let offset = u32::try_from(at).map_err(|_| IdentityError::Flash(FlashError::Read))?;
        store
            .flash
            .read(slot, offset, &mut buffer[..length])
            .map_err(IdentityError::Flash)?;
        let from = range.start.max(at);
        let to = range.end.min(at + length);
        bytes.extend_from_slice(&buffer[from - at..to - at]);
        at += length;
    }
    Ok(bytes)
}
