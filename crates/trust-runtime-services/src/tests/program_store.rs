//! Test support for the program store (specification 34 §5.1 and §11.2): a program flash that has
//! the C6's units, records every operation that reaches it and can fail one, a checkpoint flash that
//! holds committed checkpoints, and the containers of the install tests.
//!
//! The slots of the fake flash are 16 KiB each, four erase units, so that a test can keep a copy of the
//! flash after every step of an install; the units are the C6's: 4-byte program units and 4 KiB erase
//! units.

use alloc::boxed::Box;
use alloc::rc::Rc;
use alloc::vec;
use alloc::vec::Vec;
use core::cell::RefCell;

use trust_runtime_core::bytecode::BytecodeVersion;
use trust_runtime_core::deployment::{
    sha256, ApplicationIdentity, Capabilities, Configuration, Contracts, DeploymentRecord,
    DeviceDescriptor, NumericContract, ProcessImageSummary, RequestedLimits, SchedulingProfile,
    TimingEvidence,
};
use trust_runtime_core::ports::{ProgramFlash, ProgramStore};
use trust_runtime_core::retain::checkpoint::{
    CheckpointFlash, CheckpointIdentity, CheckpointLog, FlashError, FlashGeometry, Slot,
};
use trust_runtime_core::retain::RetainSnapshot;
use trust_runtime_core::vm::PreparationLimits;
use trust_runtime_core::watchdog::RetainMode;

use crate::installation::{encode, encode_sequenced};

/// The installed program of the tests: the fixture `program-v2.stbc`, which has the retained
/// `activations` of its program `Plant`.
pub(crate) const PROGRAM: &[u8] =
    include_bytes!("../../../trust-runtime/tests/fixtures/portability/stbc-2.0/program-v2.stbc");

/// The bytes of each slot of the fake program flash: 16 KiB.
pub(crate) const SLOT_BYTES: u32 = 16 * 1024;
/// The bytes one install step programs or reads back: one 256-byte flash page, as the C6's.
pub(crate) const STEP_BYTES: usize = 256;
/// The C6's program unit.
const PROGRAM_UNIT: u32 = 4;
/// The C6's erase unit.
const ERASE_UNIT: u32 = 4_096;

/// One operation that reached the fake program flash.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FlashOp {
    /// `len` bytes read from `offset` of `slot`.
    Read { slot: Slot, offset: u32, len: usize },
    /// `len` bytes programmed at `offset` of `slot`.
    Program { slot: Slot, offset: u32, len: usize },
    /// The erase unit at `offset` of `slot`.
    Erase { slot: Slot, offset: u32 },
}

struct ProbeState {
    slots: [Vec<u8>; 2],
    ops: Vec<FlashOp>,
    /// The operation with this index, counted from zero over every operation, fails.
    fail_at: Option<usize>,
}

/// The fake program flash, shared with the store that owns a handle on it.
#[derive(Clone)]
pub(crate) struct FlashProbe(Rc<RefCell<ProbeState>>);

fn slot_index(slot: Slot) -> usize {
    match slot {
        Slot::A => 0,
        Slot::B => 1,
    }
}

impl FlashProbe {
    /// A flash whose slots hold `slots`.
    pub(crate) fn from_slots(slots: [Vec<u8>; 2]) -> Self {
        Self(Rc::new(RefCell::new(ProbeState {
            slots,
            ops: Vec::new(),
            fail_at: None,
        })))
    }

    /// A flash whose slots are erased.
    pub(crate) fn erased() -> Self {
        let slot = || vec![0xff_u8; SLOT_BYTES as usize];
        Self::from_slots([slot(), slot()])
    }

    /// A store over this flash with the C6's step budget.
    pub(crate) fn store(&self) -> ProgramStore {
        ProgramStore {
            flash: Box::new(self.clone()),
            program_budget: STEP_BYTES,
        }
    }

    /// The bytes of both slots now.
    pub(crate) fn slots(&self) -> [Vec<u8>; 2] {
        self.0.borrow().slots.clone()
    }

    /// Every operation that reached the flash, in order.
    pub(crate) fn ops(&self) -> Vec<FlashOp> {
        self.0.borrow().ops.clone()
    }

    /// The programs and erases that reached the flash.
    pub(crate) fn writes(&self) -> usize {
        self.0
            .borrow()
            .ops
            .iter()
            .filter(|op| !matches!(op, FlashOp::Read { .. }))
            .count()
    }

    /// Makes the operation with index `index` fail, counting every operation from the first.
    pub(crate) fn fail_at(&self, index: usize) {
        self.0.borrow_mut().fail_at = Some(index);
    }

    /// Records `op`, and says whether it fails.
    fn record(&self, op: FlashOp) -> bool {
        let mut state = self.0.borrow_mut();
        let index = state.ops.len();
        state.ops.push(op);
        state.fail_at == Some(index)
    }
}

impl ProgramFlash for FlashProbe {
    fn geometry(&self) -> FlashGeometry {
        FlashGeometry::new(PROGRAM_UNIT, ERASE_UNIT, SLOT_BYTES)
            .expect("the C6's program store geometry")
    }

    fn read(&mut self, slot: Slot, offset: u32, bytes: &mut [u8]) -> Result<(), FlashError> {
        assert_eq!(offset % PROGRAM_UNIT, 0, "a read starts at a program unit");
        assert_eq!(
            bytes.len() % PROGRAM_UNIT as usize,
            0,
            "a read covers whole program units"
        );
        let len = bytes.len();
        if self.record(FlashOp::Read { slot, offset, len }) {
            return Err(FlashError::Read);
        }
        let state = self.0.borrow();
        let start = offset as usize;
        bytes.copy_from_slice(&state.slots[slot_index(slot)][start..start + len]);
        Ok(())
    }

    fn program(&mut self, slot: Slot, offset: u32, bytes: &[u8]) -> Result<(), FlashError> {
        assert_eq!(
            offset % PROGRAM_UNIT,
            0,
            "a program starts at a program unit"
        );
        assert_eq!(
            bytes.len() % PROGRAM_UNIT as usize,
            0,
            "a program covers whole program units"
        );
        let len = bytes.len();
        if self.record(FlashOp::Program { slot, offset, len }) {
            return Err(FlashError::Program);
        }
        let mut state = self.0.borrow_mut();
        let start = offset as usize;
        let cells = &mut state.slots[slot_index(slot)][start..start + len];
        for (cell, byte) in cells.iter_mut().zip(bytes) {
            // A program clears bits and never sets one, as flash does.
            *cell &= *byte;
        }
        Ok(())
    }

    fn erase(&mut self, slot: Slot, offset: u32) -> Result<(), FlashError> {
        assert_eq!(offset % ERASE_UNIT, 0, "an erase starts at an erase unit");
        if self.record(FlashOp::Erase { slot, offset }) {
            return Err(FlashError::Erase);
        }
        let mut state = self.0.borrow_mut();
        let start = offset as usize;
        state.slots[slot_index(slot)][start..start + ERASE_UNIT as usize].fill(0xff);
        Ok(())
    }
}

/// The device of the install tests: a computer's maxima, with the fake flash's slot.
pub(crate) fn device() -> DeviceDescriptor {
    DeviceDescriptor {
        application_slot_bytes: Some(u64::from(SLOT_BYTES)),
        ..DeviceDescriptor::hosted()
    }
}

/// The deployment record of [`PROGRAM`]: the program's SHA-256 and length, the preparation limits
/// `limits` as the limits it requests, `retain_mode` as its `[runtime.retain] mode`, and no
/// capability, import or task (specification 34 §5.1.1). `bytecode_flags` is the CRC-32 flag of the
/// fixture's STBC 2.0 header.
pub(crate) fn record_bytes(retain_mode: RetainMode, limits: &PreparationLimits) -> Vec<u8> {
    DeploymentRecord {
        application: ApplicationIdentity {
            program_sha256: sha256(PROGRAM),
            program_bytes: u32::try_from(PROGRAM.len()).expect("a short program"),
            bytecode_version: BytecodeVersion::SOURCE_FREE_V2,
            bytecode_flags: 1,
            configuration_sha256: [0; 32],
            retain_schema_sha256: [0; 32],
        },
        contracts: Contracts {
            scheduling_profile: SchedulingProfile::COOPERATIVE_RESOURCE_V1,
            numeric_contract: NumericContract::V1,
            timing_evidence: TimingEvidence::UNQUALIFIED,
            capabilities: Capabilities::NONE,
        },
        limits: RequestedLimits::from_preparation(limits),
        process_image: ProcessImageSummary::default(),
        tasks: Vec::new(),
        imports: Vec::new(),
        configuration: Configuration {
            retain_mode,
            ..Configuration::default()
        },
    }
    .encode()
    .expect("the record encodes")
}

/// The `TRSTA002` container a transfer of [`PROGRAM`] carries, with the default preparation limits.
/// Its sequence is zero: the device writes its own before the commit.
pub(crate) fn transfer_of(retain_mode: RetainMode) -> Vec<u8> {
    transfer_with(retain_mode, &PreparationLimits::default())
}

/// As [`transfer_of`], with the preparation limits `limits` in its record.
pub(crate) fn transfer_with(retain_mode: RetainMode, limits: &PreparationLimits) -> Vec<u8> {
    encode_sequenced(0, PROGRAM, &record_bytes(retain_mode, limits)).expect("the container encodes")
}

/// The bytes a commit of [`transfer_of`] with install sequence `sequence` leaves at the start of its
/// slot, written by the container writer, not by the store.
pub(crate) fn committed_image(sequence: u64, retain_mode: RetainMode) -> Vec<u8> {
    encode_sequenced(
        sequence,
        PROGRAM,
        &record_bytes(retain_mode, &PreparationLimits::default()),
    )
    .expect("the container encodes")
}

/// A `TRSTA001` container of [`PROGRAM`], as the bench packer wrote it before S4-7: no install
/// sequence, and the boot no longer reads it (specification 34 §5.1, the program store).
pub(crate) fn packed_container() -> Vec<u8> {
    encode(
        PROGRAM,
        &record_bytes(RetainMode::None, &PreparationLimits::default()),
    )
    .expect("the container encodes")
}

/// The identity every checkpoint of these tests is written for.
const IDENTITY: CheckpointIdentity = CheckpointIdentity {
    application: [0x11; 16],
    schema: [0x22; 16],
};

/// The bytes of each slot of the fake checkpoint flash: 16 KiB, as the C6's.
const CHECKPOINT_SLOT_BYTES: u32 = 16 * 1024;

struct CheckpointState {
    /// The two slots' bytes; a blank flash reads 0xFF.
    bytes: Vec<u8>,
    /// The programs and erases that reached the flash.
    writes: usize,
}

/// The fake checkpoint flash, with the C6's units, shared with the handle a loop owns.
#[derive(Clone)]
pub(crate) struct CheckpointProbe(Rc<RefCell<CheckpointState>>);

impl CheckpointProbe {
    /// A flash without a checkpoint.
    pub(crate) fn erased() -> Self {
        Self(Rc::new(RefCell::new(CheckpointState {
            bytes: vec![0xff; 2 * CHECKPOINT_SLOT_BYTES as usize],
            writes: 0,
        })))
    }

    /// A flash whose newest committed checkpoint holds `snapshot`, written by the checkpoint log. The
    /// writes of that checkpoint are not counted.
    pub(crate) fn with_checkpoint(snapshot: &RetainSnapshot) -> Self {
        let probe = Self::erased();
        let mut payload = Vec::new();
        snapshot
            .encode_snapshot_payload_into(&mut payload)
            .expect("the snapshot encodes");
        CheckpointLog::open(probe.clone())
            .expect("the checkpoint log opens")
            .append(IDENTITY, &payload)
            .expect("the checkpoint is written");
        probe.0.borrow_mut().writes = 0;
        probe
    }

    /// The programs and erases that reached the flash since the checkpoint was written.
    pub(crate) fn writes(&self) -> usize {
        self.0.borrow().writes
    }

    /// Flips the lowest bit of the byte at `offset` of the first slot, as damage does.
    pub(crate) fn damage(&self, offset: usize) {
        self.0.borrow_mut().bytes[offset] ^= 0x01;
    }

    fn slot_base(slot: Slot) -> usize {
        match slot {
            Slot::A => 0,
            Slot::B => CHECKPOINT_SLOT_BYTES as usize,
        }
    }
}

impl CheckpointFlash for CheckpointProbe {
    fn geometry(&self) -> FlashGeometry {
        FlashGeometry::new(PROGRAM_UNIT, ERASE_UNIT, CHECKPOINT_SLOT_BYTES)
            .expect("the C6's checkpoint geometry")
    }

    fn read(&mut self, slot: Slot, offset: u32, bytes: &mut [u8]) -> Result<(), FlashError> {
        let state = self.0.borrow();
        let base = Self::slot_base(slot) + offset as usize;
        bytes.copy_from_slice(&state.bytes[base..base + bytes.len()]);
        Ok(())
    }

    fn program(&mut self, slot: Slot, offset: u32, bytes: &[u8]) -> Result<(), FlashError> {
        let mut state = self.0.borrow_mut();
        state.writes += 1;
        let base = Self::slot_base(slot) + offset as usize;
        for (index, byte) in bytes.iter().enumerate() {
            state.bytes[base + index] &= *byte;
        }
        Ok(())
    }

    fn erase(&mut self, slot: Slot, offset: u32) -> Result<(), FlashError> {
        let mut state = self.0.borrow_mut();
        state.writes += 1;
        let base = Self::slot_base(slot) + offset as usize;
        state.bytes[base..base + ERASE_UNIT as usize].fill(0xff);
        Ok(())
    }
}
