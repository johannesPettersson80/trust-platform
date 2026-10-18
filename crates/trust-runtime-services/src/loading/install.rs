//! Installing a program into the program store (specification 34 §5.1 and §11.2; DEPLOY-03,
//! DEPLOY-04).
//!
//! [`begin`] runs the checks that read the container alone, before any flash write: the container
//! and its record, their admission against the device, the retained values, and a dry-run
//! preparation with the admitted limits. The transfer hands over only a complete container.
//! [`Install::step`] then commits the container one step per call, so the loop serves its link
//! between steps (STATE-08). The steps erase the units the container needs, program the payloads,
//! the header and the first half of the magic, read back what was programmed, and program the second
//! half of the magic last: that is the commit point. An install that stops at any step leaves the
//! previous program selected, because the target slot is never the newest committed one.

use alloc::vec::Vec;
use core::fmt;
use core::ops::Range;

use trust_runtime_core::deployment::{DeploymentRecord, DeviceDescriptor};
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::ports::ProgramStore;
use trust_runtime_core::retain::checkpoint::{FlashError, FlashGeometry, Slot};
use trust_runtime_core::vm::{PreparationLimits, PreparedModule};

use super::read_padded;
use super::select::{self, ScanError};
use crate::installation::{self, AdmissionError, SequencedAdmitted, SEQUENCED_HEADER_BYTES};

/// The bytes of a `TRSTA002` header: the payloads start here (specification 34 §5.1).
const HEADER_END: u32 = 60;
const _: () = assert!(HEADER_END as usize == SEQUENCED_HEADER_BYTES);
/// The first half of the magic, programmed before the read-back.
const MAGIC_FIRST: Range<u32> = 0..4;
/// The second half of the magic: the commit point, programmed last.
const MAGIC_SECOND: Range<u32> = 4..8;
/// The header after the magic: the header CRC-32, the total length, the install sequence, the entry
/// count and the entries.
const HEADER_AFTER_MAGIC: Range<u32> = 8..HEADER_END;

/// Why an install is refused or stopped (specification 34 §11.2; DEPLOY-03, DEPLOY-04). A refusal
/// before any flash write leaves the store as it was.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstallError {
    /// The container, its record or its admission is refused (specification 34 §5.1, §5.1.1).
    Admission(AdmissionError),
    /// The new program cannot restore the retained values of the running one (specification 11
    /// §6.7.1). The check that refused is the hook the caller passed.
    Retain(RuntimeError),
    /// The dry-run preparation exceeds the admitted limits: `insufficient memory`, with its numbers
    /// (DEPLOY-04).
    InsufficientMemory {
        /// The program's length in bytes.
        program_bytes: usize,
        /// The preparation limits admission granted.
        limits: PreparationLimits,
    },
    /// The dry-run preparation refused the program for another reason.
    Preparation(RuntimeError),
    /// The container is larger than an application slot.
    SlotTooSmall {
        /// The container's length in bytes.
        needed: usize,
        /// The slot's length in bytes.
        available: usize,
    },
    /// The committed containers cannot be ordered (see [`ScanError`]).
    Scan(ScanError),
    /// The newest committed install sequence is the largest a `u64` holds.
    SequenceExhausted,
    /// A flash operation failed; the previous program stays selected.
    Flash(FlashError),
    /// Programmed bytes did not read back as written; the previous program stays selected.
    Verify,
}

impl fmt::Display for InstallError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Admission(error) => fmt::Display::fmt(error, formatter),
            Self::Retain(error) => {
                write!(formatter, "the retained values cannot be restored: {error}")
            }
            Self::InsufficientMemory {
                program_bytes,
                limits,
            } => write!(
                formatter,
                "insufficient memory: a program of {program_bytes} bytes needs more than \
                 max_preparation_bytes {} or max_preparation_work {}",
                limits.max_preparation_bytes, limits.max_preparation_work
            ),
            Self::Preparation(error) => write!(formatter, "the program does not prepare: {error}"),
            Self::SlotTooSmall { needed, available } => write!(
                formatter,
                "the container needs {needed} bytes, an application slot holds {available}"
            ),
            Self::Scan(error) => fmt::Display::fmt(error, formatter),
            Self::SequenceExhausted => {
                formatter.write_str("the install sequence cannot count higher")
            }
            Self::Flash(error) => write!(formatter, "the program store's flash {error} failed"),
            Self::Verify => formatter.write_str("the programmed bytes do not read back as written"),
        }
    }
}

impl core::error::Error for InstallError {}

/// A committed install: the identity the `installed` reply carries (specification 34 §11.2 step 8).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Committed {
    /// The install sequence the commit gave the container.
    pub sequence: u64,
    /// The program's SHA-256, as the record names it.
    pub program_sha256: [u8; 32],
    /// The program's length in bytes.
    pub program_bytes: usize,
}

/// What one step of a commit did.
#[derive(Debug)]
pub enum Progress {
    /// The commit needs more steps; step the install again with this value.
    Pending(Install),
    /// The commit point is written: the container is the newest committed one.
    Committed(Committed),
}

/// An install in progress: its target slot, its container and the step it is on.
#[derive(Debug)]
pub struct Install {
    slot: Slot,
    geometry: FlashGeometry,
    /// The bytes a program or a read-back step covers: whole program units, at most one budget.
    chunk: u32,
    container: Vec<u8>,
    container_len: u32,
    sequence: u64,
    program_sha256: [u8; 32],
    program_bytes: usize,
    /// The erase units the container takes, counted from the start of the target slot.
    erase_units: u32,
    phase: Phase,
}

/// Where a commit is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    /// Erase the unit that starts at `unit` erase units into the target.
    Erase { unit: u32 },
    /// Program the next bytes of the payloads, the header and the first half of the magic.
    Program(Cursor),
    /// Read back the next bytes that the program steps wrote.
    Verify(Cursor),
    /// Program the second half of the magic: the commit point.
    Commit,
}

/// A position in an ordered list of byte ranges of the container: the range, and the next byte in it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Cursor {
    range: usize,
    next: u32,
}

impl Cursor {
    /// The position of the first byte of the first range.
    fn start(ranges: &[Range<u32>]) -> Self {
        Self {
            range: 0,
            next: ranges.first().map_or(0, |range| range.start),
        }
    }
}

/// Starts the install of `container`, a complete `TRSTA002` container, and returns the commit that
/// writes it. The container is the upload that `TransferSession::take_complete` returns, once every
/// part has arrived and matched the digest (specification 34 §5.1, PORT-05).
///
/// The checks come first and write nothing: the container and its record, their admission against
/// `device` (specification 34 §5.1.1, DEPLOY-04), the retained values, a dry-run preparation with
/// the admitted limits, and the fit in an application slot. Then the store's committed containers
/// are ordered. The container takes the next install sequence, and the slot that does not hold the
/// newest committed container, or slot A when none is committed.
///
/// `retain_check` is the check of specification 11 §6.7.1 against the newest checkpoint: the caller
/// passes it as a hook, and the S4-3 wiring supplies the real check. The sequence the transfer
/// carries is not used: the device writes its own into the header before the commit.
///
/// # Errors
///
/// The [`InstallError`] of the first check that refuses. Nothing has been written when it returns.
pub fn begin(
    store: &mut ProgramStore,
    mut container: Vec<u8>,
    device: &DeviceDescriptor,
    retain_check: impl FnOnce(&DeploymentRecord) -> Result<(), RuntimeError>,
) -> Result<Install, InstallError> {
    let (program_bytes, program_sha256) = check_program(&container, device, retain_check)?;
    let geometry = store.flash.geometry();
    let slot_len = geometry.slot_len();
    let Some(container_len) = u32::try_from(container.len())
        .ok()
        .filter(|len| *len <= slot_len)
    else {
        return Err(InstallError::SlotTooSmall {
            needed: container.len(),
            available: slot_len as usize,
        });
    };
    let order = select::order(store).map_err(InstallError::Scan)?;
    let (slot, sequence) = match order.newest {
        Some(newest) => (
            newest.slot.other(),
            newest
                .header
                .sequence
                .checked_add(1)
                .ok_or(InstallError::SequenceExhausted)?,
        ),
        None => (Slot::A, 1),
    };
    installation::set_sequence(&mut container, sequence)
        .map_err(|error| InstallError::Admission(AdmissionError::Container(error)))?;
    Ok(Install {
        slot,
        geometry,
        chunk: step_bytes(store.program_budget, geometry.program_unit()),
        container,
        container_len,
        sequence,
        program_sha256,
        program_bytes,
        erase_units: container_len.div_ceil(geometry.erase_unit()),
        phase: Phase::Erase { unit: 0 },
    })
}

/// The checks that read the container alone, before any flash write: its admission, the retained
/// values and a dry-run preparation with the admitted limits. The dry run's result is dropped.
/// It returns the program's length and SHA-256.
fn check_program(
    container: &[u8],
    device: &DeviceDescriptor,
    retain_check: impl FnOnce(&DeploymentRecord) -> Result<(), RuntimeError>,
) -> Result<(usize, [u8; 32]), InstallError> {
    let SequencedAdmitted { admitted, .. } =
        installation::admit_sequenced(container, device).map_err(InstallError::Admission)?;
    retain_check(&admitted.record).map_err(InstallError::Retain)?;
    let program_bytes = admitted.program.len();
    let program_sha256 = admitted.record.application.program_sha256;
    PreparedModule::from_bytes(admitted.program, admitted.limits)
        .map_err(|error| preparation_error(error, program_bytes, admitted.limits))?;
    Ok((program_bytes, program_sha256))
}

/// The dry run's refusal. A program that needs more than the admitted preparation limits is
/// `insufficient memory` with its numbers (DEPLOY-04); any other refusal keeps its own error.
fn preparation_error(
    error: RuntimeError,
    program_bytes: usize,
    limits: PreparationLimits,
) -> InstallError {
    match error {
        RuntimeError::PreparationLimit => InstallError::InsufficientMemory {
            program_bytes,
            limits,
        },
        other => InstallError::Preparation(other),
    }
}

/// The bytes one step of a program or a read-back covers: the budget rounded down to whole program
/// units, and at least one unit.
fn step_bytes(budget: usize, program_unit: u32) -> u32 {
    // A budget above what a `u32` holds is far above any container, so it is clamped, not refused.
    let budget = u32::try_from(budget).unwrap_or(u32::MAX);
    (budget / program_unit).max(1) * program_unit
}

/// The ranges the program steps write, in order: the payloads, the header after the magic, then the
/// first half of the magic.
fn program_ranges(container_len: u32) -> [Range<u32>; 3] {
    [HEADER_END..container_len, HEADER_AFTER_MAGIC, MAGIC_FIRST]
}

/// The ranges the read-back checks: every byte the program steps wrote before the commit point.
fn verify_ranges(container_len: u32) -> [Range<u32>; 2] {
    [MAGIC_FIRST, HEADER_AFTER_MAGIC.start..container_len]
}

/// The next chunk of at most `budget` bytes of `ranges`, which the cursor moves past, or `None`
/// once every range is done.
fn next_chunk(ranges: &[Range<u32>], cursor: &mut Cursor, budget: u32) -> Option<Range<u32>> {
    while let Some(range) = ranges.get(cursor.range) {
        if cursor.next < range.end {
            let end = range.end.min(cursor.next.saturating_add(budget));
            let chunk = cursor.next..end;
            cursor.next = end;
            return Some(chunk);
        }
        cursor.range += 1;
        cursor.next = ranges.get(cursor.range).map_or(0, |next| next.start);
    }
    None
}

/// The bytes padded to whole program units with erased (0xFF) bytes. A program never sets a bit to
/// 1, so the padding leaves the erased flash it lands on as it is; it lies inside the erase units
/// the container took.
fn padded(bytes: &[u8], program_unit: u32) -> Vec<u8> {
    let mut padded = bytes.to_vec();
    padded.resize(bytes.len().next_multiple_of(program_unit as usize), 0xff);
    padded
}

impl Install {
    /// Advances the commit by one step: one erase unit, at most one budget of programmed or
    /// read-back bytes, or the commit point. A step that fails ends the install, which the caller
    /// drops, and the previous program stays selected.
    ///
    /// # Errors
    ///
    /// [`InstallError::Flash`] when the flash fails, and [`InstallError::Verify`] when a programmed
    /// byte does not read back as written.
    pub fn step(self, store: &mut ProgramStore) -> Result<Progress, InstallError> {
        let phase = self.phase;
        match phase {
            Phase::Erase { unit } => self.erase(store, unit),
            Phase::Program(cursor) => self.program(store, cursor),
            Phase::Verify(cursor) => self.verify(store, cursor),
            Phase::Commit => self.commit(store),
        }
    }

    /// The container's bytes in `range`.
    fn bytes(&self, range: &Range<u32>) -> &[u8] {
        &self.container[range.start as usize..range.end as usize]
    }

    /// Erases the unit `unit` of the target. The units are erased first, one per step.
    fn erase(mut self, store: &mut ProgramStore, unit: u32) -> Result<Progress, InstallError> {
        store
            .flash
            .erase(self.slot, unit * self.geometry.erase_unit())
            .map_err(InstallError::Flash)?;
        let next = unit + 1;
        self.phase = if next < self.erase_units {
            Phase::Erase { unit: next }
        } else {
            Phase::Program(Cursor::start(&program_ranges(self.container_len)))
        };
        Ok(Progress::Pending(self))
    }

    /// Programs the next chunk of the payloads, the header and the first half of the magic.
    fn program(
        mut self,
        store: &mut ProgramStore,
        mut cursor: Cursor,
    ) -> Result<Progress, InstallError> {
        let ranges = program_ranges(self.container_len);
        if let Some(chunk) = next_chunk(&ranges, &mut cursor, self.chunk) {
            let bytes = padded(self.bytes(&chunk), self.geometry.program_unit());
            store
                .flash
                .program(self.slot, chunk.start, &bytes)
                .map_err(InstallError::Flash)?;
            self.phase = Phase::Program(cursor);
        } else {
            self.phase = Phase::Verify(Cursor::start(&verify_ranges(self.container_len)));
        }
        Ok(Progress::Pending(self))
    }

    /// Reads back the next chunk of what the program steps wrote, and checks it.
    fn verify(
        mut self,
        store: &mut ProgramStore,
        mut cursor: Cursor,
    ) -> Result<Progress, InstallError> {
        let ranges = verify_ranges(self.container_len);
        if let Some(chunk) = next_chunk(&ranges, &mut cursor, self.chunk) {
            let len = (chunk.end - chunk.start) as usize;
            let read =
                read_padded(store, self.slot, chunk.start, len).map_err(InstallError::Flash)?;
            if read != self.bytes(&chunk) {
                return Err(InstallError::Verify);
            }
            self.phase = Phase::Verify(cursor);
        } else {
            self.phase = Phase::Commit;
        }
        Ok(Progress::Pending(self))
    }

    /// Programs the second half of the magic, the commit point. After it the container is the
    /// newest committed one.
    fn commit(self, store: &mut ProgramStore) -> Result<Progress, InstallError> {
        store
            .flash
            .program(self.slot, MAGIC_SECOND.start, self.bytes(&MAGIC_SECOND))
            .map_err(InstallError::Flash)?;
        Ok(Progress::Committed(Committed {
            sequence: self.sequence,
            program_sha256: self.program_sha256,
            program_bytes: self.program_bytes,
        }))
    }
}
