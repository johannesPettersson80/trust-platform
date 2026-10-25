//! Test support for the program store (specification 34 §5.1 and §11.2; specification 44 §10.1): a
//! program flash that has the C6's units, records every operation that reaches it and can fail one,
//! a checkpoint flash that holds committed checkpoints, and the containers of the install tests.
//!
//! The slots of the fake flash are 16 KiB each, four erase units, so that a test can keep a copy of the
//! flash after every step of an install; the units are the C6's: 4-byte program units and 4 KiB erase
//! units.

use alloc::boxed::Box;
use alloc::rc::Rc;
use alloc::vec;
use alloc::vec::Vec;
use core::cell::RefCell;

use trust_runtime_core::crc32;
use trust_runtime_core::deployment::{
    sha256, ApplicationIdentity, Capabilities, Configuration, Contracts, DeploymentRecord,
    DeviceDescriptor, NumericContract, ProcessImageSummary, ProgramVersion, RequestedLimits,
    SchedulingProfile, TimingEvidence,
};
use trust_runtime_core::native::image::{
    type_kind, AreaRecord, CodeImage, EntryKind, EntryRecord, FileRecord, ImageWriter,
    IoBindRecord, MemoryFigure, RetainRecord, SectionId, StmtRecord, StringPool, SymbolRecord,
    Target, TaskRecord, TypeRecord, AREA_RETAINED, HEADER_BYTES, NONE, SECTION_ENTRY_BYTES,
    SYMBOL_PROGRAM, SYMBOL_RETAINED,
};
use trust_runtime_core::ports::{ProgramFlash, ProgramStore};
use trust_runtime_core::retain::checkpoint::{
    CheckpointFlash, CheckpointIdentity, CheckpointLog, FlashError, FlashGeometry, Slot,
};
use trust_runtime_core::retain::RetainSnapshot;
use trust_runtime_core::watchdog::RetainMode;

use crate::installation::{encode, encode_image};
use crate::tests::runner_ports::board_probe_image;

/// The length of [`program`]: 6,412 bytes, so that its container, with the record and the 64 bytes
/// before the image, takes two of the C6's 4 KiB erase units.
pub(crate) const PROGRAM_BYTES: usize = 6_412;

/// The installed program of the tests: the board probe's compiled image for this computer's CPU
/// (`tests/runner_ports.rs`), with zeros after its last section up to [`PROGRAM_BYTES`]. Its header
/// declares that length, and its header CRC, over bytes 12 to the end of the section table, is
/// written again for it (specification 44 §5.2), so that the kernel's checks before a commit admit
/// it as they admit the probe (specification 44 §10.2). The store and the boot read a program only
/// as bytes, which its record names by their SHA-256 and length.
pub(crate) fn program() -> &'static [u8] {
    let mut image = board_probe_image();
    assert!(
        image.len() <= PROGRAM_BYTES,
        "the probe's image is no longer than the program"
    );
    image.resize(PROGRAM_BYTES, 0);
    let length = u32::try_from(PROGRAM_BYTES).expect("a short image");
    image[12..16].copy_from_slice(&length.to_le_bytes());
    let sections = u32::from_le_bytes([image[112], image[113], image[114], image[115]]);
    let table_end = HEADER_BYTES + sections as usize * SECTION_ENTRY_BYTES;
    let crc = crc32::checksum(&image[12..table_end]);
    image[8..12].copy_from_slice(&crc.to_le_bytes());
    Box::leak(image.into_boxed_slice())
}

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
    /// Whether the probe maps its slots for reading in place (specification 34 MEM-14).
    maps: bool,
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
            maps: false,
        })))
    }

    /// This probe, mapping its slots for reading in place as a chip's flash does (specification 34
    /// MEM-14): a mapping is a snapshot of the slot's bytes, which a test may keep for good.
    pub(crate) fn mapping(self) -> Self {
        self.0.borrow_mut().maps = true;
        self
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

    fn map(&mut self, slot: Slot, offset: u32, len: u32) -> Option<&'static [u8]> {
        let state = self.0.borrow();
        if !state.maps {
            return None;
        }
        let start = offset as usize;
        let bytes = state.slots[slot_index(slot)].get(start..start + len as usize)?;
        Some(Box::leak(bytes.to_vec().into_boxed_slice()))
    }
}

/// The device of the install tests: a computer's maxima, with the fake flash's slot.
pub(crate) fn device() -> DeviceDescriptor {
    DeviceDescriptor {
        application_slot_bytes: Some(u64::from(SLOT_BYTES)),
        ..DeviceDescriptor::hosted()
    }
}

/// The deployment record of [`program`] as an image's record (specification 44 §10.1): the
/// program's SHA-256 and length, the version 44.1, no preparation limit, `retain_mode` as its
/// `[runtime.retain] mode`, and no capability, import or task (specification 34 §5.1.1).
pub(crate) fn record_bytes(retain_mode: RetainMode) -> Vec<u8> {
    DeploymentRecord {
        application: ApplicationIdentity {
            program_sha256: sha256(program()),
            program_bytes: u32::try_from(PROGRAM_BYTES).expect("a short program"),
            program_version: ProgramVersion::NATIVE_IMAGE,
            program_flags: 0,
            configuration_sha256: [0; 32],
            retain_schema_sha256: [0; 32],
        },
        contracts: Contracts {
            scheduling_profile: SchedulingProfile::COOPERATIVE_RESOURCE_V1,
            numeric_contract: NumericContract::V1,
            timing_evidence: TimingEvidence::UNQUALIFIED,
            capabilities: Capabilities::NONE,
        },
        limits: RequestedLimits {
            max_artifact_bytes: 0,
            max_preparation_bytes: 0,
            max_preparation_work: 0,
            max_validation_scratch_bytes: 0,
            max_validation_work: 0,
            max_construction_values: 0,
            max_construction_bytes: 0,
            max_process_image_bytes: 0,
            max_work: 0,
            max_construction_work: 0,
            max_call_depth: 0,
        },
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

/// The `TRSTA002` container a transfer of [`program`] carries (specification 44 §10.1). Its sequence
/// is zero: the device writes its own before the commit.
pub(crate) fn transfer_of(retain_mode: RetainMode) -> Vec<u8> {
    encode_image(0, program(), &record_bytes(retain_mode)).expect("the container encodes")
}

/// The bytes a commit of [`transfer_of`] with install sequence `sequence` leaves at the start of its
/// slot, written by the container writer, not by the store.
pub(crate) fn committed_image(sequence: u64, retain_mode: RetainMode) -> Vec<u8> {
    encode_image(sequence, program(), &record_bytes(retain_mode)).expect("the container encodes")
}

/// A `TRSTA001` container of [`program`], as the bench packer wrote it before S4-7: no install
/// sequence, and the boot no longer reads it (specification 34 §5.1, the program store).
pub(crate) fn packed_container() -> Vec<u8> {
    encode(program(), &record_bytes(RetainMode::None)).expect("the container encodes")
}

/// A compiled program image for this computer's CPU whose program instance `Plant` keeps the
/// retained `activations : DINT`, named `@program/Plant/activations` in its retain map
/// (specification 44 §5.10; specification 11 §6.7). Its module never runs in these tests: the
/// install's retained-value check reads its tables alone.
pub(crate) fn retaining_image() -> Vec<u8> {
    let mut strings = StringPool::new();
    let task = strings.add("Main");
    let program_type = strings.add("Counter");
    let key = strings.add("@program/Plant/activations");
    let label = strings.add("main.st");
    let mut writer = ImageWriter::new(
        Target::this_cpu().unwrap_or(Target::X86_64),
        u16::try_from(trust_plc_abi::INTERFACE_VERSION).expect("the interface version is a u16"),
    );
    layout_tables(&mut writer, task);
    retained_symbols(&mut writer, &mut strings, program_type);
    writer.table::<IoBindRecord>(&[], &[]);
    writer.table::<StmtRecord>(&[], &[]);
    writer.table(
        &[RetainRecord {
            key,
            area: 5,
            offset: 0,
            type_index: 0,
        }],
        &[],
    );
    writer.table(
        &[FileRecord {
            label,
            source_offset: NONE,
            source_length: 0,
        }],
        &[],
    );
    writer.section(
        SectionId::MemFig,
        MemoryFigure {
            areas: 16,
            stack: 64,
            bookkeeping: 24,
            retain_capture: 64,
            total: 168,
            ..MemoryFigure::default()
        }
        .bytes()
        .to_vec(),
    );
    // No area starts from memory of its own; the section is there, as in every image.
    writer.section(SectionId::Init, Vec::new());
    writer.section(SectionId::Strings, strings.into_bytes());
    writer.finish().expect("the image builds")
}

/// Writes the module of [`retaining_image`] and its entry, area and task tables: one program body,
/// the retained area 5 and the program area 6 of 8 bytes each, and the task `task`, every 25 ms.
fn layout_tables(writer: &mut ImageWriter, task: u32) {
    writer.module(CodeImage {
        code: vec![0; 16],
        rodata: Vec::new(),
        rodata_distance: 16,
    });
    writer.table(
        &[EntryRecord {
            kind: EntryKind::ProgramBody as u32,
            index: 0,
            code_offset: 0,
        }],
        &[],
    );
    let area = |id: u8, length: u64, flags: u32| AreaRecord {
        id: u32::from(id),
        length,
        alignment: 8,
        flags,
        init_offset: 0,
    };
    writer.table(
        &[
            area(1, 0, 0),
            area(2, 0, 0),
            area(3, 0, 0),
            area(4, 0, 0),
            area(5, 8, AREA_RETAINED),
            area(6, 8, 0),
        ],
        &[],
    );
    writer.table(
        &[TaskRecord {
            name: task,
            interval_ns: 25_000_000,
            priority: 1,
            single: NONE,
            program_count: 1,
            program_first: 0,
            flags: 0,
        }],
        &0u32.to_le_bytes(),
    );
}

/// Writes the type and symbol tables of [`retaining_image`]: the program instance `Plant` of type
/// `program_type` and its retained `activations : DINT`.
fn retained_symbols(writer: &mut ImageWriter, strings: &mut StringPool, program_type: u32) {
    writer.table(
        &[
            TypeRecord {
                kind: type_kind::DINT,
                name: 0,
                size: 4,
                alignment: 4,
                detail: 0,
                count: 0,
            },
            TypeRecord {
                kind: type_kind::PROGRAM,
                name: program_type,
                size: 8,
                alignment: 8,
                detail: 0,
                count: 0,
            },
        ],
        &[],
    );
    let plant = SymbolRecord {
        canonical: strings.add("PLANT"),
        display: strings.add("Plant"),
        type_index: 1,
        area: 6,
        flags: SYMBOL_PROGRAM,
        offset: 0,
        size: 8,
        parent: NONE,
        // The program root carries the instance's number, 0 (specification 44 §5.7).
        pou: 0,
    };
    let activations = SymbolRecord {
        canonical: strings.add("PLANT.ACTIVATIONS"),
        display: strings.add("Plant.activations"),
        type_index: 0,
        area: 5,
        flags: SYMBOL_RETAINED,
        offset: 0,
        size: 4,
        parent: 0,
        pou: NONE,
    };
    writer.table(&[plant, activations], &[]);
}

/// The `TRSTA002` container a transfer of [`retaining_image`] carries, with the record the build
/// writes for it (`DeploymentRecord::for_image`) and `retain_mode` as its `[runtime.retain] mode`.
pub(crate) fn retaining_transfer(retain_mode: RetainMode) -> Vec<u8> {
    let image = retaining_image();
    let mut record = DeploymentRecord::for_image(&image, [0; 32]).expect("the image has a record");
    record.configuration.retain_mode = retain_mode;
    let record = record.encode().expect("the record encodes");
    encode_image(0, &image, &record).expect("the container encodes")
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
        let mut state = probe.0.borrow_mut();
        state.writes = 0;
        drop(state);
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
