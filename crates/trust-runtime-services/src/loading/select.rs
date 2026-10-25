//! Boot selection of the program store (specification 34 §5.1 and §11.2; DEPLOY-03).
//!
//! After a reset the store reads the header of each slot. The committed container with the highest
//! install sequence is the installed program. Its payloads and its admission are then checked, and
//! a failed check refuses the boot with `no installed program: <reason>`. The boot never falls back
//! to the other slot: a rollback is an explicit activation with its own check.
//!
//! Only a `TRSTA002` container is committed. A slot that holds a `TRSTA001` container, which has no
//! install sequence, is not committed: it takes no part in the order, and a store with no committed
//! container names the slot that holds one in its refusal ([`SelectError::VersionOne`]).

use alloc::borrow::Cow;
use alloc::vec::Vec;
use core::fmt;

use trust_runtime_core::deployment::{DeploymentRecord, DeviceDescriptor};
use trust_runtime_core::ports::ProgramStore;
use trust_runtime_core::retain::checkpoint::{FlashError, Slot};

use super::read_padded;
use crate::installation::{
    self, AdmissionError, ProgramFormat, SequencedAdmitted, SequencedHeader, SEQUENCED_HEADER_BYTES,
};

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

/// The installed program the store selects, read whole into memory with the identity of its record
/// (specification 34 §11.2). A chip answers `program.active` from [`identities`](super::identity::identities)
/// instead, which reads no program; this is for a host that has the memory, and for the tests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stored {
    /// The container's install sequence.
    pub sequence: u64,
    /// The SHA-256 of the program, as the record names it.
    pub program_sha256: [u8; 32],
    /// The program's bytes.
    pub bytes: Vec<u8>,
}

/// The installed program a device starts at boot: the program's bytes and the decoded record
/// (specification 34 §5.1).
#[derive(Debug, Clone, PartialEq)]
pub struct Booted {
    /// The container's install sequence.
    pub sequence: u64,
    /// The slot that holds the container.
    pub slot: Slot,
    /// What the program is: a compiled program image (specification 44 §10.1); admission refuses
    /// any other.
    pub format: ProgramFormat,
    /// The bytes of the program: where they lie in the slot when the platform reads its flash in
    /// place (specification 34 MEM-14, the program image in place), else read into memory with the
    /// rest of the container released.
    pub program: Cow<'static, [u8]>,
    /// The deployment record the container holds.
    pub record: DeploymentRecord,
}

/// Why the store selects no installed program at boot (specification 34 §5.1, DEPLOY-03). Its text
/// is `no installed program: <reason>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelectError {
    /// Neither slot holds a container.
    NoContainer,
    /// Neither slot holds a committed container, and this slot, the first that does, holds a
    /// `TRSTA001` container: version 1 has no install sequence and is not committed (specification 34
    /// §5.1).
    VersionOne(Slot),
    /// The two slots cannot be ordered (see [`ScanError`]).
    Scan(ScanError),
    /// The selected container fails its payload check or its admission. The other slot is not
    /// selected in its place.
    Refused(AdmissionError),
    /// The selected container is longer than the memory the boot of a platform that does not read
    /// its flash in place has for it. Nothing was read into memory, and the other slot is not
    /// selected in its place.
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
            Self::NoContainer => formatter.write_str("no TRSTA002 container in either slot"),
            Self::VersionOne(slot) => write!(
                formatter,
                "slot {} holds a TRSTA001 container, which is not committed (only TRSTA002 is read)",
                slot_name(*slot)
            ),
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
/// `TRSTA002` container whose header CRC-32 and total length are good (specification 34 §5.1).
fn committed_header(store: &mut ProgramStore, slot: Slot) -> Result<Option<SlotHeader>, ScanError> {
    let slot_len = store.flash.geometry().slot_len();
    let bytes = read_padded(store, slot, 0, SEQUENCED_HEADER_BYTES).map_err(ScanError::Flash)?;
    let header = installation::committed_header(&bytes, slot_len);
    Ok(header.map(|header| SlotHeader { slot, header }))
}

/// The newest committed container, or why the store has none (specification 34 §5.1).
fn newest_committed(store: &mut ProgramStore) -> Result<SlotHeader, SelectError> {
    let ordered = order(store).map_err(SelectError::Scan)?;
    ordered.newest.ok_or_else(|| no_container(store))
}

/// The refusal of a store that has no committed container. A slot whose first eight bytes read
/// `TRSTA001` holds a version-1 container, which is not committed; the first such slot is named, so
/// that a device flashed with an old container says why it has no program.
fn no_container(store: &mut ProgramStore) -> SelectError {
    for slot in [Slot::A, Slot::B] {
        match read_padded(store, slot, 0, installation::MAGIC.len()) {
            Ok(magic) if magic == installation::MAGIC => return SelectError::VersionOne(slot),
            Ok(_) => {}
            Err(error) => return SelectError::Scan(ScanError::Flash(error)),
        }
    }
    SelectError::NoContainer
}

/// The name of `slot` in a reason: the letter of its key in the chip file.
fn slot_name(slot: Slot) -> &'static str {
    match slot {
        Slot::A => "A",
        Slot::B => "B",
    }
}

/// The installed program the store selects at boot: the newest committed container, with its
/// payloads checked and its admission granted by `device` (specification 34 §5.1.1, DEPLOY-04). The
/// whole container is read into memory.
///
/// # Errors
///
/// [`SelectError`], whose text is `no installed program: <reason>`. A refused container refuses the
/// boot, and the other slot is never selected in its place (DEPLOY-03).
pub fn active(store: &mut ProgramStore, device: &DeviceDescriptor) -> Result<Stored, SelectError> {
    let newest = newest_committed(store)?;
    load(store, newest, device)
}

/// The other committed container, when both slots hold one: the program the store kept before the
/// newest. It is checked as [`active`] checks the selected one, and is read whole into memory.
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
/// starting.
///
/// A platform that reads its flash in place checks the container where it lies in its slot, and
/// the program's bytes stay there: the boot copies no part of the container to the heap but the
/// decoded record (specification 34 MEM-14, the program image in place). Any other platform reads
/// the container once into memory, which `room` bounds: a longer container refuses the boot before
/// anything is read. It keeps the program's bytes, which it moves to the front of the memory it
/// read, and the decoded record, and releases the rest of the container.
///
/// # Errors
///
/// [`SelectError`], whose text is `no installed program: <reason>`. A refused container, or one
/// longer than `room` on a platform that reads it into memory, refuses the boot, and the other slot
/// is never selected in its place (DEPLOY-03).
pub fn boot(
    store: &mut ProgramStore,
    device: &DeviceDescriptor,
    room: usize,
) -> Result<Booted, SelectError> {
    let newest = newest_committed(store)?;
    if let Some(container) = store.flash.map(newest.slot, 0, newest.header.total_len) {
        let SequencedAdmitted { sequence, admitted } =
            installation::admit_sequenced(container, device).map_err(SelectError::Refused)?;
        return Ok(Booted {
            sequence,
            slot: newest.slot,
            format: admitted.format,
            program: Cow::Borrowed(admitted.program),
            record: admitted.record,
        });
    }
    let needed = newest.header.total_len as usize;
    if needed > room {
        return Err(SelectError::NoRoom { needed, room });
    }
    let mut bytes = read_container(store, newest)?;
    let SequencedAdmitted { sequence, admitted } =
        installation::admit_sequenced(&bytes, device).map_err(SelectError::Refused)?;
    let (format, record, program) = (admitted.format, admitted.record, admitted.program);
    let start = offset_in(&bytes, program);
    let end = start + program.len();
    bytes.truncate(end);
    bytes.drain(..start);
    Ok(Booted {
        sequence,
        slot: newest.slot,
        format,
        program: Cow::Owned(bytes),
        record,
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

/// Reads the whole container in its slot, checks its payloads and admits it.
fn load(
    store: &mut ProgramStore,
    container: SlotHeader,
    device: &DeviceDescriptor,
) -> Result<Stored, SelectError> {
    let bytes = read_container(store, container)?;
    let admitted = installation::admit_sequenced(&bytes, device).map_err(SelectError::Refused)?;
    Ok(Stored {
        sequence: admitted.sequence,
        program_sha256: admitted.admitted.record.application.program_sha256,
        bytes: admitted.admitted.program.to_vec(),
    })
}
