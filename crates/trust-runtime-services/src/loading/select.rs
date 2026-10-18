//! Boot selection of the program store (specification 34 §5.1 and §11.2; DEPLOY-03).
//!
//! After a reset the store reads the header of each slot. The committed container with the highest
//! install sequence is the installed program. Its payloads and its admission are then checked, and
//! a failed check refuses the boot with `no installed program: <reason>`. The boot never falls back
//! to the other slot: a rollback is an explicit activation with its own check.

use alloc::vec::Vec;
use core::fmt;

use trust_runtime_core::deployment::DeviceDescriptor;
use trust_runtime_core::ports::ProgramStore;
use trust_runtime_core::retain::checkpoint::{FlashError, Slot};

use super::read_padded;
use crate::installation::{self, AdmissionError, SequencedHeader, SEQUENCED_HEADER_BYTES};

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
}

impl fmt::Display for SelectError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("no installed program: ")?;
        match self {
            Self::NoContainer => formatter.write_str("no TRSTA002 container in either slot"),
            Self::Scan(error) => fmt::Display::fmt(error, formatter),
            // The container's own error, without the admission's "no installed application"
            // prefix, which would repeat this one.
            Self::Refused(AdmissionError::Container(error)) => fmt::Display::fmt(error, formatter),
            Self::Refused(error) => fmt::Display::fmt(error, formatter),
        }
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

/// Reads the header of `slot`, and returns it when the slot holds a committed container.
fn committed_header(store: &mut ProgramStore, slot: Slot) -> Result<Option<SlotHeader>, ScanError> {
    let slot_len = store.flash.geometry().slot_len();
    let bytes = read_padded(store, slot, 0, SEQUENCED_HEADER_BYTES).map_err(ScanError::Flash)?;
    Ok(installation::committed_header(&bytes, slot_len).map(|header| SlotHeader { slot, header }))
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

/// Reads the whole container in its slot, checks its payloads and admits it.
fn load(
    store: &mut ProgramStore,
    container: SlotHeader,
    device: &DeviceDescriptor,
) -> Result<Stored, SelectError> {
    let total_len = container.header.total_len as usize;
    let bytes = read_padded(store, container.slot, 0, total_len)
        .map_err(|error| SelectError::Scan(ScanError::Flash(error)))?;
    let admitted = installation::admit_sequenced(&bytes, device).map_err(SelectError::Refused)?;
    Ok(Stored {
        sequence: admitted.sequence,
        program_sha256: admitted.admitted.record.application.program_sha256,
        bytes: admitted.admitted.program.to_vec(),
    })
}
