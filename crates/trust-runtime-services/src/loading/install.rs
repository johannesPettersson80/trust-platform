//! Installing a program into the program store (specification 34 §5.1 and §11.2; DEPLOY-03,
//! DEPLOY-04).
//!
//! [`check`] runs the checks that read the container alone, before any write: the container and its
//! record, their admission against the device, the retained values, and the kernel's checks of the
//! program image. [`begin`] runs them for a chip's store, and [`start`] begins the commit of a
//! container that [`check`] has passed. The transfer hands over only a complete container.
//! [`Install::step`] then commits the container one step per call, so the loop serves its link
//! between steps (STATE-08). The steps erase the units the container needs, program the payloads,
//! the header and the first half of the magic, read back what was programmed, and program the second
//! half of the magic last: that is the commit point. An install that stops at any step leaves the
//! previous program selected, because the target slot is never the newest committed one.
//!
//! An upload that a chip wrote into its store as it arrived (specification 34 §5.1;
//! [`super::upload`]) has its payloads in the slot already: [`check_bytes`] checks it there, with the
//! header the transfer session kept, and [`start_stored`] begins a commit that programs only the
//! header and the magic.

use alloc::vec::Vec;
use core::fmt;
use core::ops::Range;

use trust_runtime_core::deployment::{
    AdmissionRefusal, DeploymentRecord, DeviceDescriptor, ProgramVersion,
};
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::native::program as native;
use trust_runtime_core::native::program::Refusal;
use trust_runtime_core::ports::ProgramStore;
use trust_runtime_core::retain::checkpoint::{FlashError, FlashGeometry, Slot};

use super::read_padded;
use super::select::{self, ScanError};
use crate::installation::{
    self, AdmissionError, Admitted, ContainerBytes, SequencedAdmitted, SEQUENCED_HEADER_BYTES,
};

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
    /// §6.7.1): the warm start refuses them, or the checkpoint they come from is damaged. The check
    /// that refused is the hook the caller passed.
    Retain(RuntimeError),
    /// The retained-value check could not be carried out, for example because the new program could
    /// not be built to try its warm start (specification 11 §6.7.1; decision G13). The text is the
    /// check's own error: it does not claim that retained values cannot be restored.
    RetainCheck(RuntimeError),
    /// The kernel refuses the compiled program image (specification 44 §10.2): the first of its
    /// checks that failed, with its numbers.
    Image(Refusal),
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
            Self::RetainCheck(error) => {
                write!(formatter, "the retained-value check could not run: {error}")
            }
            Self::Image(refusal) => write!(formatter, "the program image is refused: {refusal}"),
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
    /// The container's bytes from its start, up to `written_end`.
    container: Vec<u8>,
    /// The end of the bytes this commit programs and reads back: the container's length, or the end
    /// of its header when its payloads were written into the slot as they arrived.
    written_end: u32,
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

/// The slot an install writes (specification 34 §5.1): the slot that does not hold the container
/// the program the runtime holds came from, `held`, whose image the runtime reads in place
/// (MEM-14); when the runtime holds none of the store's containers, the slot that does not hold
/// the newest committed container, or slot A when neither slot holds one. A container installed
/// after the held program, which has not run, is therefore replaced by the next install, and the
/// held program's slot is never written.
#[must_use]
pub fn install_slot(order: &select::Order, held: Option<u64>) -> Slot {
    let holding = [order.newest, order.older]
        .into_iter()
        .flatten()
        .find(|container| Some(container.header.sequence) == held);
    match (holding, order.newest) {
        (Some(holding), _) => holding.slot.other(),
        (None, Some(newest)) => newest.slot.other(),
        (None, None) => Slot::A,
    }
}

/// Starts the install of `container`, a complete `TRSTA002` container, and returns the commit that
/// writes it. The container is the upload that `TransferSession::take_complete` returns, once every
/// part has arrived and matched the digest (specification 34 §5.1, PORT-05).
///
/// The checks come first and write nothing: the container and its record, their admission against
/// `device` (specification 34 §5.1.1, DEPLOY-04), the retained values, the kernel's checks of the
/// image, and the fit in an application slot. Then the store's committed containers
/// are ordered. The container takes the next install sequence, and the slot that does not hold the
/// newest committed container, or slot A when none is committed.
///
/// `retain_check` is the check of specification 11 §6.7.1 (decision G13), passed in as a hook by the
/// platform, as [`check`] takes it. The sequence the transfer carries is not used: the device writes
/// its own into the header before the commit. The runtime holds no program of the store here; a
/// loop that holds one begins its commit with [`start`].
///
/// # Errors
///
/// The [`InstallError`] of the first check that refuses. Nothing has been written when it returns.
pub fn begin(
    store: &mut ProgramStore,
    container: Vec<u8>,
    device: &DeviceDescriptor,
    retain_check: impl FnOnce(&Admitted<'_>) -> Result<(), RuntimeError>,
) -> Result<Install, InstallError> {
    let checked = check(&container, device, retain_check)?;
    start(store, container, checked, None)
}

/// Begins the commit of `container`, a complete `TRSTA002` container that [`check`] has passed, and
/// returns the commit that writes it. `checked` is what [`check`] found of it, or [`Checked::of`]
/// reads from it again.
///
/// This is [`begin`] without the checks, for a caller that ran them in an earlier step: the loop of a
/// device runs them in the request that carried the container and begins the commit right after
/// (specification 34 §11.2). Nothing has been written when it returns. The store's committed
/// containers are ordered, and the container takes the next install sequence and the slot that
/// [`install_slot`] names for `held`, the install sequence of the container that the program the
/// runtime holds came from.
///
/// # Errors
///
/// [`InstallError::SlotTooSmall`] when the container is longer than a slot,
/// [`InstallError::Scan`] when the committed containers cannot be ordered and
/// [`InstallError::SequenceExhausted`] when the newest sequence cannot count higher.
pub fn start(
    store: &mut ProgramStore,
    mut container: Vec<u8>,
    checked: Checked,
    held: Option<u64>,
) -> Result<Install, InstallError> {
    let Checked {
        program_bytes,
        program_sha256,
    } = checked;
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
    let sequence = next_sequence(&order)?;
    let slot = install_slot(&order, held);
    installation::set_sequence(&mut container, sequence)
        .map_err(|error| InstallError::Admission(AdmissionError::Container(error)))?;
    Ok(Install {
        slot,
        geometry,
        chunk: step_bytes(store.program_budget, geometry.program_unit()),
        container,
        written_end: container_len,
        sequence,
        program_sha256,
        program_bytes,
        erase_units: container_len.div_ceil(geometry.erase_unit()),
        phase: Phase::Erase { unit: 0 },
    })
}

/// Begins the commit of a container of `container_len` bytes that was written into `slot` as its
/// upload arrived, and that [`check_bytes`] has passed: `header` is the container's header, which
/// the transfer session kept (specification 34 §5.1). The container takes the next install sequence,
/// written into `header`, and the commit programs the header after the magic and the first half of
/// the magic, reads them back, and programs the second half of the magic last. It erases nothing:
/// the upload's first part found the slot's units erased.
///
/// # Errors
///
/// [`InstallError::SlotTooSmall`] when the container is longer than a slot,
/// [`InstallError::Scan`] when the committed containers cannot be ordered,
/// [`InstallError::SequenceExhausted`] when the newest sequence cannot count higher, and
/// [`InstallError::Admission`] when `header` is no `TRSTA002` header.
pub fn start_stored(
    store: &mut ProgramStore,
    slot: Slot,
    mut header: Vec<u8>,
    container_len: usize,
    checked: Checked,
) -> Result<Install, InstallError> {
    let geometry = store.flash.geometry();
    let slot_len = geometry.slot_len();
    if u32::try_from(container_len)
        .ok()
        .is_none_or(|len| len > slot_len)
    {
        return Err(InstallError::SlotTooSmall {
            needed: container_len,
            available: slot_len as usize,
        });
    }
    let order = select::order(store).map_err(InstallError::Scan)?;
    let sequence = next_sequence(&order)?;
    installation::set_sequence(&mut header, sequence)
        .map_err(|error| InstallError::Admission(AdmissionError::Container(error)))?;
    Ok(Install {
        slot,
        geometry,
        chunk: step_bytes(store.program_budget, geometry.program_unit()),
        container: header,
        written_end: HEADER_END,
        sequence,
        program_sha256: checked.program_sha256,
        program_bytes: checked.program_bytes,
        erase_units: 0,
        phase: Phase::Program(Cursor::start(&program_ranges(HEADER_END))),
    })
}

/// The install sequence of the next commit: one more than the newest committed container's, or the
/// first sequence when none is committed.
fn next_sequence(order: &select::Order) -> Result<u64, InstallError> {
    match order.newest {
        Some(newest) => newest
            .header
            .sequence
            .checked_add(1)
            .ok_or(InstallError::SequenceExhausted),
        None => Ok(installation::FIRST_SEQUENCE),
    }
}

/// What the checks of [`check`] found of an admitted program.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Checked {
    /// The program's length in bytes.
    pub program_bytes: usize,
    /// The program's SHA-256, as its record names it.
    pub program_sha256: [u8; 32],
}

impl Checked {
    /// What [`check`] found of `container`, read from the container again without the checks: its
    /// program's length and the SHA-256 its record names. For a caller that ran [`check`] on the
    /// container in an earlier step and begins the commit with [`start`].
    ///
    /// # Errors
    ///
    /// [`InstallError::Admission`] when the container or its record cannot be read, which a container
    /// that passed [`check`] does not do.
    pub fn of(container: &[u8]) -> Result<Self, InstallError> {
        let sequenced = installation::decode_sequenced(container)
            .map_err(|error| InstallError::Admission(AdmissionError::Container(error)))?;
        let record = DeploymentRecord::decode(sequenced.installed.record)
            .map_err(|error| InstallError::Admission(AdmissionError::Record(error)))?;
        Ok(Self {
            program_bytes: sequenced.installed.program.len(),
            program_sha256: record.application.program_sha256,
        })
    }
}

/// The checks that read the container alone, before any write, in the order of specification 34
/// §11.2: the container and its record, their admission against `device` (specification 34
/// §5.1.1, DEPLOY-04), the retained values, and the kernel's checks of the program image
/// (specification 44 §10.2), whose memory figure must fit the memory `device` has for programs.
/// Every runtime runs these checks in this order with the same texts; the platform supplies `retain_check`, the check of specification 11 §6.7.1
/// (decision G13), which is given the admitted program.
///
/// An error of `retain_check` that is a retained-value error (the checkpoint or the retain file is
/// damaged, or the warm start refuses a value) is [`InstallError::Retain`]; any other error means
/// that the check could not be carried out, for example because the new program could not be built,
/// and is [`InstallError::RetainCheck`] with its own text.
///
/// # Errors
///
/// The [`InstallError`] of the first check that refuses. Nothing has been written when it returns.
pub fn check(
    container: &[u8],
    device: &DeviceDescriptor,
    retain_check: impl FnOnce(&Admitted<'_>) -> Result<(), RuntimeError>,
) -> Result<Checked, InstallError> {
    check_bytes(ContainerBytes::whole(container), device, retain_check)
}

/// [`check`] of a container whose header and payloads may lie apart: an upload that a chip wrote
/// into its program store, whose payloads the checks read where they lie in the slot and whose
/// header the transfer session kept (specification 34 §5.1, §11.2). The checks, their order and
/// their texts are those of [`check`].
///
/// # Errors
///
/// The [`InstallError`] of the first check that refuses. Nothing has been written when it returns.
pub fn check_bytes(
    container: ContainerBytes<'_>,
    device: &DeviceDescriptor,
    retain_check: impl FnOnce(&Admitted<'_>) -> Result<(), RuntimeError>,
) -> Result<Checked, InstallError> {
    let SequencedAdmitted { admitted, .. } =
        installation::admit_sequenced_bytes(container, device).map_err(InstallError::Admission)?;
    retain_check(&admitted).map_err(retain_refusal)?;
    let program_bytes = admitted.program.len();
    let program_sha256 = admitted.record.application.program_sha256;
    check_image(admitted.program, device)?;
    Ok(Checked {
        program_bytes,
        program_sha256,
    })
}

/// The kernel's own checks of the compiled program image `image`, as a boot runs them, against the
/// memory `device` has for programs (specification 44 §10.2, steps 2 to 6). Admission has already
/// refused an image on a device that does not run images.
fn check_image(image: &[u8], device: &DeviceDescriptor) -> Result<(), InstallError> {
    let Some(target) = device.native else {
        return Err(runs_no_images());
    };
    native::admit(image, &target.device(device.program_memory_bytes))
        .map(|_| ())
        .map_err(InstallError::Image)
}

/// The refusal of an image on a device that runs none, as admission refuses its record's version.
fn runs_no_images() -> InstallError {
    let version = ProgramVersion::NATIVE_IMAGE;
    InstallError::Admission(AdmissionError::Deployment(RuntimeError::DeploymentRefused(
        AdmissionRefusal::ProgramVersion {
            major: version.major,
            minor: version.minor,
        },
    )))
}

/// The refusal of the retained-value check: a retained-value error is the retained values that cannot
/// be restored, and any other error is a check that could not run (decision G13).
fn retain_refusal(error: RuntimeError) -> InstallError {
    if matches!(
        error,
        RuntimeError::RetainStore(_)
            | RuntimeError::RetainCorruption(_)
            | RuntimeError::RetainMigration(_)
            | RuntimeError::RetainSubrangeViolation { .. }
    ) {
        InstallError::Retain(error)
    } else {
        InstallError::RetainCheck(error)
    }
}

/// The bytes one step of a program or a read-back covers: the budget rounded down to whole program
/// units, and at least one unit. The reads of `program.active` are no longer than a step either.
pub(super) fn step_bytes(budget: usize, program_unit: u32) -> u32 {
    // A budget above what a `u32` holds is far above any container, so it is clamped, not refused.
    let budget = u32::try_from(budget).unwrap_or(u32::MAX);
    (budget / program_unit).max(1) * program_unit
}

/// The ranges the program steps write, in order: the payloads up to `written_end`, the header after
/// the magic, then the first half of the magic. A commit whose payloads are in the slot already has
/// `written_end` at the end of the header, and programs no payload.
fn program_ranges(written_end: u32) -> [Range<u32>; 3] {
    [HEADER_END..written_end, HEADER_AFTER_MAGIC, MAGIC_FIRST]
}

/// The ranges the read-back checks: every byte the program steps wrote before the commit point.
fn verify_ranges(written_end: u32) -> [Range<u32>; 2] {
    [MAGIC_FIRST, HEADER_AFTER_MAGIC.start..written_end]
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
pub(super) fn padded(bytes: &[u8], program_unit: u32) -> Vec<u8> {
    let mut padded = bytes.to_vec();
    padded.resize(bytes.len().next_multiple_of(program_unit as usize), 0xff);
    padded
}

impl Install {
    /// The identity the commit gives the program once it is committed: the install sequence it
    /// took, the program's SHA-256 and its length. It is known when the commit begins, so the reply
    /// of an install can be written before the last step is done (specification 34 §11.2).
    #[must_use]
    pub fn identity(&self) -> Committed {
        Committed {
            sequence: self.sequence,
            program_sha256: self.program_sha256,
            program_bytes: self.program_bytes,
        }
    }

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
            Phase::Program(Cursor::start(&program_ranges(self.written_end)))
        };
        Ok(Progress::Pending(self))
    }

    /// Programs the next chunk of the payloads, the header and the first half of the magic.
    fn program(
        mut self,
        store: &mut ProgramStore,
        mut cursor: Cursor,
    ) -> Result<Progress, InstallError> {
        let ranges = program_ranges(self.written_end);
        if let Some(chunk) = next_chunk(&ranges, &mut cursor, self.chunk) {
            let bytes = padded(self.bytes(&chunk), self.geometry.program_unit());
            store
                .flash
                .program(self.slot, chunk.start, &bytes)
                .map_err(InstallError::Flash)?;
            self.phase = Phase::Program(cursor);
        } else {
            self.phase = Phase::Verify(Cursor::start(&verify_ranges(self.written_end)));
        }
        Ok(Progress::Pending(self))
    }

    /// Reads back the next chunk of what the program steps wrote, and checks it.
    fn verify(
        mut self,
        store: &mut ProgramStore,
        mut cursor: Cursor,
    ) -> Result<Progress, InstallError> {
        let ranges = verify_ranges(self.written_end);
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
