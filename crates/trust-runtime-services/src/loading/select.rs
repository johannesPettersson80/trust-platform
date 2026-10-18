//! Boot selection of the program store (specification 34 §5.1 and §11.2; DEPLOY-03).
//!
//! After a reset the store reads the header of each slot. The committed container with the highest
//! install sequence is the installed program. Its payloads and its admission are then checked, and
//! a failed check refuses the boot with `no installed program: <reason>`. The boot never falls back
//! to the other slot: a rollback is an explicit activation with its own check.
//!
//! A container packed on the bench (`TRSTA001`) has no sequence. It counts as committed with install
//! sequence 0 until the bench packer writes version 2 (S4-7), so it boots, and an install made after
//! it takes sequence 1 in the other slot.

use alloc::vec::Vec;
use core::fmt;

use trust_runtime_core::crc32;
use trust_runtime_core::deployment::{DeploymentRecord, DeviceDescriptor};
use trust_runtime_core::ports::ProgramStore;
use trust_runtime_core::retain::checkpoint::{FlashError, Slot};
use trust_runtime_core::vm::PreparationLimits;

use super::read_padded;
use crate::installation::{
    self, AdmissionError, SequencedAdmitted, SequencedHeader, SEQUENCED_HEADER_BYTES,
};

/// The bytes of the install sequence in a `TRSTA002` header, which `TRSTA001` does not have.
const SEQUENCE_BYTES: usize = 8;
/// The bytes of a `TRSTA001` header: a `TRSTA002` header without the install sequence.
const PACKED_HEADER_BYTES: usize = SEQUENCED_HEADER_BYTES - SEQUENCE_BYTES;
/// Offset of the header CRC-32 in both versions.
const HEADER_CRC_OFFSET: usize = 8;
/// The header CRC-32 covers the bytes from here to the end of the header, in both versions.
const HEADER_CRC_START: usize = 12;
/// Offset of the container's total length in both versions.
const TOTAL_LENGTH_OFFSET: usize = 12;

/// A committed container and the slot that holds it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SlotHeader {
    /// The slot that holds the container.
    pub slot: Slot,
    /// The container's total length and install sequence.
    pub header: SequencedHeader,
}

/// The committed containers of the two slots, the newest first (specification 34 §5.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Order {
    /// The committed container with the highest install sequence, when a slot holds one.
    pub newest: Option<SlotHeader>,
    /// The other committed container, when both slots hold one.
    pub older: Option<SlotHeader>,
}

/// Why the two slots cannot be ordered (specification 34 §5.1, DEPLOY-03).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScanError {
    /// Two committed containers have the same install sequence, so neither is the newest.
    SequenceTie(u64),
    /// A slot's header cannot be read from the flash.
    Flash(FlashError),
}

impl fmt::Display for ScanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SequenceTie(sequence) => {
                write!(
                    formatter,
                    "two committed containers have sequence {sequence}"
                )
            }
            Self::Flash(error) => write!(formatter, "the program store's flash {error} failed"),
        }
    }
}

impl core::error::Error for ScanError {}

/// The installed program the store selects, with the identity `program.active` reports
/// (specification 34 §11.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stored {
    /// The container's install sequence.
    pub sequence: u64,
    /// The SHA-256 of the program, as the record names it.
    pub program_sha256: [u8; 32],
    /// The program's bytes.
    pub bytes: Vec<u8>,
}

/// The installed program a device starts at boot: the program's bytes and the decoded record, with
/// the container they came out of released (specification 34 §5.1).
#[derive(Debug, Clone, PartialEq)]
pub struct Booted {
    /// The container's install sequence, 0 for a `TRSTA001` container.
    pub sequence: u64,
    /// The bytes of `program.stbc`.
    pub program: Vec<u8>,
    /// The deployment record the container holds.
    pub record: DeploymentRecord,
    /// The preparation limits admission granted; prepare the program with the device's own limits,
    /// which are at most these.
    pub limits: PreparationLimits,
}

/// Why the store selects no installed program at boot (specification 34 §5.1, DEPLOY-03). Its text
/// is `no installed program: <reason>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelectError {
    /// Neither slot holds a committed container.
    NoContainer,
    /// The two slots cannot be ordered (see [`ScanError`]).
    Scan(ScanError),
    /// The selected container fails its payload check or its admission. The other slot is not
    /// selected in its place.
    Refused(AdmissionError),
    /// The selected container is longer than the memory the boot has for it. Nothing was read into
    /// memory, and the other slot is not selected in its place.
    NoRoom {
        /// The container's length in bytes.
        needed: usize,
        /// The bytes of memory the boot has for it.
        room: usize,
    },
}

impl SelectError {
    /// The reason the boot is refused, without the `no installed program: ` that [`Display`](fmt::Display)
    /// puts before it. A device that serves its link without a program names this reason when it
    /// refuses a `restart` (specification 34, STATE-08).
    #[must_use]
    pub fn reason(&self) -> Reason<'_> {
        Reason(self)
    }

    fn write_reason(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoContainer => {
                formatter.write_str("no TRSTA001 or TRSTA002 container in either slot")
            }
            Self::Scan(error) => fmt::Display::fmt(error, formatter),
            // The container's own error, without the admission's "no installed application"
            // prefix, which would repeat this one.
            Self::Refused(AdmissionError::Container(error)) => fmt::Display::fmt(error, formatter),
            Self::Refused(error) => fmt::Display::fmt(error, formatter),
            Self::NoRoom { needed, room } => write!(
                formatter,
                "the installed container needs {needed} bytes, more than the heap's {room} bytes"
            ),
        }
    }
}

/// The reason a boot is refused, as [`SelectError::reason`] writes it.
#[derive(Debug, Clone, Copy)]
pub struct Reason<'a>(&'a SelectError);

impl fmt::Display for Reason<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.write_reason(formatter)
    }
}

impl fmt::Display for SelectError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("no installed program: ")?;
        self.write_reason(formatter)
    }
}

impl core::error::Error for SelectError {}

/// Reads the headers of both slots and orders their committed containers by install sequence. Two
/// committed containers with one sequence are refused, since neither is the newest (DEPLOY-03).
///
/// # Errors
///
/// [`ScanError::Flash`] when a slot cannot be read, and [`ScanError::SequenceTie`] when two
/// committed containers share a sequence.
pub fn order(store: &mut ProgramStore) -> Result<Order, ScanError> {
    let first = committed_header(store, Slot::A)?;
    let second = committed_header(store, Slot::B)?;
    match (first, second) {
        (Some(left), Some(right)) if left.header.sequence == right.header.sequence => {
            Err(ScanError::SequenceTie(left.header.sequence))
        }
        (Some(left), Some(right)) if left.header.sequence > right.header.sequence => Ok(Order {
            newest: Some(left),
            older: Some(right),
        }),
        (Some(left), Some(right)) => Ok(Order {
            newest: Some(right),
            older: Some(left),
        }),
        (Some(only), None) | (None, Some(only)) => Ok(Order {
            newest: Some(only),
            older: None,
        }),
        (None, None) => Ok(Order {
            newest: None,
            older: None,
        }),
    }
}

/// Reads the header of `slot`, and returns it when the slot holds a committed container: a
/// `TRSTA002` container, or a `TRSTA001` container, which counts as sequence 0.
fn committed_header(store: &mut ProgramStore, slot: Slot) -> Result<Option<SlotHeader>, ScanError> {
    let slot_len = store.flash.geometry().slot_len();
    let bytes = read_padded(store, slot, 0, SEQUENCED_HEADER_BYTES).map_err(ScanError::Flash)?;
    let header = installation::committed_header(&bytes, slot_len)
        .or_else(|| packed_header(&bytes, slot_len));
    Ok(header.map(|header| SlotHeader { slot, header }))
}

/// The header of a committed `TRSTA001` container, with install sequence 0, read from the first
/// bytes of a slot of `slot_len` bytes. As for `TRSTA002`, the container is committed when its magic,
/// its header CRC-32 and its total length are good (specification 34 §5.1).
fn packed_header(bytes: &[u8], slot_len: u32) -> Option<SequencedHeader> {
    if bytes.get(..installation::MAGIC.len())? != installation::MAGIC.as_slice() {
        return None;
    }
    let header = bytes.get(..PACKED_HEADER_BYTES)?;
    let word = |offset: usize| {
        u32::from_le_bytes([
            header[offset],
            header[offset + 1],
            header[offset + 2],
            header[offset + 3],
        ])
    };
    if crc32::checksum(&header[HEADER_CRC_START..]) != word(HEADER_CRC_OFFSET) {
        return None;
    }
    let total_len = word(TOTAL_LENGTH_OFFSET);
    let total = usize::try_from(total_len).ok()?;
    if total < PACKED_HEADER_BYTES || total_len > slot_len {
        return None;
    }
    Some(SequencedHeader {
        total_len,
        sequence: 0,
    })
}

/// The installed program the store selects at boot: the newest committed container, with its
/// payloads checked and its admission granted by `device` (specification 34 §5.1.1, DEPLOY-04).
///
/// # Errors
///
/// [`SelectError`], whose text is `no installed program: <reason>`. A refused container refuses the
/// boot, and the other slot is never selected in its place (DEPLOY-03).
pub fn active(store: &mut ProgramStore, device: &DeviceDescriptor) -> Result<Stored, SelectError> {
    let newest = order(store)
        .map_err(SelectError::Scan)?
        .newest
        .ok_or(SelectError::NoContainer)?;
    load(store, newest, device)
}

/// The other committed container, when both slots hold one: the program the store kept before the
/// newest. It is checked as [`active`] checks the selected one.
///
/// # Errors
///
/// [`SelectError`] when the slots cannot be ordered or the other container fails its check.
pub fn previous(
    store: &mut ProgramStore,
    device: &DeviceDescriptor,
) -> Result<Option<Stored>, SelectError> {
    let older = order(store).map_err(SelectError::Scan)?.older;
    older.map(|older| load(store, older, device)).transpose()
}

/// The installed program a device starts at boot: the newest committed container, with its payloads
/// checked and its admission granted by `device`, as [`active`] selects it, but handed over for
/// starting. `room` is the bytes of memory the boot has for the container; a longer container
/// refuses the boot before anything is read into memory.
///
/// The boot reads the container once. It keeps the program's bytes, which it moves to the front of
/// the memory it read, and the decoded record, and releases the rest of the container.
///
/// # Errors
///
/// [`SelectError`], whose text is `no installed program: <reason>`. A refused container, or one
/// longer than `room`, refuses the boot, and the other slot is never selected in its place
/// (DEPLOY-03).
pub fn boot(
    store: &mut ProgramStore,
    device: &DeviceDescriptor,
    room: usize,
) -> Result<Booted, SelectError> {
    let newest = order(store)
        .map_err(SelectError::Scan)?
        .newest
        .ok_or(SelectError::NoContainer)?;
    let needed = newest.header.total_len as usize;
    if needed > room {
        return Err(SelectError::NoRoom { needed, room });
    }
    let mut bytes = read_container(store, newest)?;
    let SequencedAdmitted { sequence, admitted } =
        admit_container(&bytes, device).map_err(SelectError::Refused)?;
    let (record, limits, program) = (admitted.record, admitted.limits, admitted.program);
    let start = offset_in(&bytes, program);
    let end = start + program.len();
    bytes.truncate(end);
    bytes.drain(..start);
    Ok(Booted {
        sequence,
        program: bytes,
        record,
        limits,
    })
}

/// Where `part` lies in `whole`, which it is a slice of.
fn offset_in(whole: &[u8], part: &[u8]) -> usize {
    (part.as_ptr() as usize).saturating_sub(whole.as_ptr() as usize)
}

/// Reads the whole container in its slot.
fn read_container(store: &mut ProgramStore, container: SlotHeader) -> Result<Vec<u8>, SelectError> {
    let total_len = container.header.total_len as usize;
    read_padded(store, container.slot, 0, total_len)
        .map_err(|error| SelectError::Scan(ScanError::Flash(error)))
}

/// Admits the container `bytes` against `device`: a `TRSTA001` container, which has no sequence,
/// as sequence 0, and any other as a `TRSTA002` container.
fn admit_container<'a>(
    bytes: &'a [u8],
    device: &DeviceDescriptor,
) -> Result<SequencedAdmitted<'a>, AdmissionError> {
    if bytes.starts_with(&installation::MAGIC) {
        installation::admit(bytes, device).map(|admitted| SequencedAdmitted {
            sequence: 0,
            admitted,
        })
    } else {
        installation::admit_sequenced(bytes, device)
    }
}

/// Reads the whole container in its slot, checks its payloads and admits it.
fn load(
    store: &mut ProgramStore,
    container: SlotHeader,
    device: &DeviceDescriptor,
) -> Result<Stored, SelectError> {
    let bytes = read_container(store, container)?;
    let admitted = admit_container(&bytes, device).map_err(SelectError::Refused)?;
    Ok(Stored {
        sequence: admitted.sequence,
        program_sha256: admitted.admitted.record.application.program_sha256,
        bytes: admitted.admitted.program.to_vec(),
    })
}
