//! The program store's behaviour (specification 34 §5.1, §11.2, DEPLOY-03): installs, boot selection
//! and the faults between them.
//!
//! The flash is the C6's: 4-byte program units, 4 KiB erase units and two slots of 3,056 KiB, the
//! halves of the 6,112 KiB `application` region of `chips/esp32c6.toml`. The installed program is
//! the board probe's compiled image padded to 6,412 bytes (`tests/program_store.rs`), which the
//! kernel's checks before a commit admit and the store reads only as bytes. Its record is 342 bytes,
//! and the image starts at the first multiple of 64 after the 60-byte header (specification 44
//! §10.1), so the container is 6,818 bytes (64 + 6,412 + 342) and takes two 4 KiB erase units.

use alloc::boxed::Box;
use alloc::rc::Rc;
use alloc::string::ToString;
use alloc::vec;
use alloc::vec::Vec;
use core::cell::RefCell;

use trust_runtime_core::deployment::{
    sha256, AdmissionRefusal, ApplicationIdentity, Capabilities, Configuration, Contracts,
    DeploymentRecord, DeviceDescriptor, NumericContract, ProcessImageSummary, ProgramVersion,
    RequestedLimits, SchedulingProfile, TimingEvidence,
};
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::native::program::Refusal;
use trust_runtime_core::native::signature::SignatureError;
use trust_runtime_core::ports::{ProgramFlash, ProgramStore};
use trust_runtime_core::retain::checkpoint::{FlashError, FlashGeometry, Slot};

use super::install::{begin, check, start, Committed, Install, InstallError, Progress};
use super::select::{active, order, previous, ScanError, SelectError};
use crate::installation::{encode_image, AdmissionError, InstallationError};
pub(super) use crate::tests::program_store::program;

/// The C6's application slot: 3,056 KiB, half of the 6,112 KiB `application` region.
const SLOT_BYTES: u32 = 3056 * 1024;
/// The C6's program unit, in bytes (`chips/esp32c6.toml` programs a word at a time).
const PROGRAM_UNIT: u32 = 4;
/// The C6's erase unit: 4 KiB (`[memory.spi_flash] erase_kib = 4`).
const ERASE_UNIT: u32 = 4096;
/// The most bytes one install step programs: one 256-byte flash page, as a checkpoint step does.
const BUDGET: usize = 256;

/// The two slots' bytes, and the writes the store has made to them.
struct Flash {
    bytes: [Vec<u8>; 2],
    /// The programs and erases made so far.
    writes: u32,
    /// The write with this index fails with a flash error.
    fail_write: Option<u32>,
    /// The program with this index stores its first byte with one bit changed.
    corrupt_write: Option<u32>,
}

impl Flash {
    fn with_bytes(bytes: [Vec<u8>; 2]) -> Self {
        Self {
            bytes,
            writes: 0,
            fail_write: None,
            corrupt_write: None,
        }
    }

    /// Counts one write and returns its index; the write named by `fail_write` fails instead.
    fn count_write(&mut self, error: FlashError) -> Result<u32, FlashError> {
        let index = self.writes;
        self.writes += 1;
        if self.fail_write == Some(index) {
            return Err(error);
        }
        Ok(index)
    }
}

/// The flash the test holds, shared with the store that owns it.
type Shared = Rc<RefCell<Flash>>;

/// The C6's flash as a fake that checks the store's access rules: whole program units, erase units
/// from their first byte.
struct RamFlash {
    flash: Shared,
}

fn slot_index(slot: Slot) -> usize {
    match slot {
        Slot::A => 0,
        Slot::B => 1,
    }
}

fn geometry() -> FlashGeometry {
    FlashGeometry::new(PROGRAM_UNIT, ERASE_UNIT, SLOT_BYTES)
        .expect("the C6's program store geometry")
}

impl ProgramFlash for RamFlash {
    fn geometry(&self) -> FlashGeometry {
        geometry()
    }

    fn read(&mut self, slot: Slot, offset: u32, bytes: &mut [u8]) -> Result<(), FlashError> {
        assert_eq!(offset % PROGRAM_UNIT, 0, "a read starts at a program unit");
        assert_eq!(
            bytes.len() % PROGRAM_UNIT as usize,
            0,
            "a read covers whole program units"
        );
        let flash = self.flash.borrow();
        let start = offset as usize;
        bytes.copy_from_slice(&flash.bytes[slot_index(slot)][start..start + bytes.len()]);
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
        let mut flash = self.flash.borrow_mut();
        let index = flash.count_write(FlashError::Program)?;
        let corrupt = flash.corrupt_write == Some(index);
        let start = offset as usize;
        let cells = &mut flash.bytes[slot_index(slot)][start..start + bytes.len()];
        for (position, (cell, value)) in cells.iter_mut().zip(bytes).enumerate() {
            // A program clears bits and never sets one, as flash does.
            let value = if corrupt && position == 0 {
                *value ^ 0x01
            } else {
                *value
            };
            *cell &= value;
        }
        Ok(())
    }

    fn erase(&mut self, slot: Slot, offset: u32) -> Result<(), FlashError> {
        assert_eq!(offset % ERASE_UNIT, 0, "an erase starts at an erase unit");
        let mut flash = self.flash.borrow_mut();
        flash.count_write(FlashError::Erase)?;
        let start = offset as usize;
        flash.bytes[slot_index(slot)][start..start + ERASE_UNIT as usize].fill(0xff);
        Ok(())
    }
}

/// A store over the fake flash `flash`, with the test's step budget.
fn store_over(flash: &Shared) -> ProgramStore {
    ProgramStore {
        flash: Box::new(RamFlash {
            flash: Rc::clone(flash),
        }),
        program_budget: BUDGET,
    }
}

/// A store over two slots with the given bytes.
fn store_from(bytes: [Vec<u8>; 2]) -> (Shared, ProgramStore) {
    let flash = Rc::new(RefCell::new(Flash::with_bytes(bytes)));
    let store = store_over(&flash);
    (flash, store)
}

/// A store over two erased slots.
fn erased_store() -> (Shared, ProgramStore) {
    let erased = || vec![0xff_u8; SLOT_BYTES as usize];
    store_from([erased(), erased()])
}

/// The deployment record of `program` as an image's record (specification 44 §10.1): the program's
/// SHA-256 and length, the version 44.1, no limit, and no capability, import or task
/// (specification 34 §5.1.1).
fn record_of(program: &[u8]) -> DeploymentRecord {
    DeploymentRecord {
        application: ApplicationIdentity {
            program_sha256: sha256(program),
            program_bytes: u32::try_from(program.len()).expect("a short program"),
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
        configuration: Configuration::default(),
    }
}

/// The complete `TRSTA002` container a transfer of `program` carries. Its sequence is zero: the
/// device writes its own sequence before the commit.
pub(super) fn transfer_of(program: &[u8]) -> Vec<u8> {
    let record = record_of(program).encode().expect("the record encodes");
    encode_image(0, program, &record).expect("the container encodes")
}

/// The device of these tests: a computer's maxima, with the C6's application slot, running compiled
/// program images.
fn device() -> DeviceDescriptor {
    DeviceDescriptor {
        program_versions: &[ProgramVersion::NATIVE_IMAGE],
        application_slot_bytes: Some(u64::from(SLOT_BYTES)),
        ..DeviceDescriptor::hosted()
    }
}

/// Installs `program` to its commit point, one step at a time.
fn install_program(store: &mut ProgramStore, program: &[u8]) -> Result<Committed, InstallError> {
    let mut pending = begin(store, transfer_of(program), &device(), |_| Ok(()))?;
    loop {
        match pending.step(store)? {
            Progress::Pending(next) => pending = next,
            Progress::Committed(committed) => return Ok(committed),
        }
    }
}

/// How many steps the commit of `pending` takes to its commit point, counting that step.
fn steps_to_commit(store: &mut ProgramStore, mut pending: Install) -> u32 {
    let mut steps = 0;
    loop {
        steps += 1;
        match pending.step(store).expect("the step runs") {
            Progress::Pending(next) => pending = next,
            Progress::Committed(_) => return steps,
        }
    }
}

#[test]
fn a_write_stopped_at_any_step_keeps_the_previous_program_selected() {
    // Specification 34 §5.1 and DEPLOY-03: a write stopped at any step leaves the previous
    // container the newest committed one, so the previous program stays selected, and only the
    // commit point selects the new one. Catches a commit point written before the payloads, and a
    // write into the newest slot.
    let (flash, mut store) = erased_store();
    install_program(&mut store, program()).expect("the first install commits");
    install_program(&mut store, program()).expect("the second install commits");
    let committed_bytes = flash.borrow().bytes.clone();

    let (_, mut counting) = store_from(committed_bytes.clone());
    let third = begin(&mut counting, transfer_of(program()), &device(), |_| Ok(()))
        .expect("the third install begins");
    let total = steps_to_commit(&mut counting, third);
    assert!(total > 4, "the commit takes several steps, not {total}");

    for stopped_after in 0..=total {
        let (_, mut store) = store_from(committed_bytes.clone());
        let mut pending = begin(&mut store, transfer_of(program()), &device(), |_| Ok(()))
            .expect("the third install begins");
        let mut committed = None;
        for _ in 0..stopped_after {
            match pending.step(&mut store).expect("the step runs") {
                Progress::Pending(next) => pending = next,
                Progress::Committed(done) => {
                    committed = Some(done);
                    break;
                }
            }
        }
        let selected = active(&mut store, &device()).expect("a program is selected");
        if stopped_after < total {
            assert!(
                committed.is_none(),
                "stopped after {stopped_after} steps, before the commit point"
            );
            assert_eq!(
                selected.sequence, 2,
                "stopped after {stopped_after} steps: the previous program stays selected"
            );
        } else {
            assert_eq!(committed.map(|done| done.sequence), Some(3));
            assert_eq!(
                selected.sequence, 3,
                "after the commit point the new program is selected"
            );
        }
        assert_eq!(selected.bytes, program());
    }
}

#[test]
fn the_write_target_is_never_the_newest_committed_slot() {
    // Specification 34 §5.1 and DEPLOY-03: each install writes the slot that does not hold the
    // newest committed container, and slot A when none does. The newest container's bytes stay as
    // they were. Catches an install that overwrites the running program.
    let (flash, mut store) = erased_store();
    install_program(&mut store, program()).expect("the first install commits");
    assert!(
        flash.borrow().bytes[1].iter().all(|byte| *byte == 0xff),
        "the first install writes slot A, and slot B stays erased"
    );
    let first_slot = flash.borrow().bytes[0].clone();

    install_program(&mut store, program()).expect("the second install commits");
    assert!(
        flash.borrow().bytes[0] == first_slot,
        "the second install writes slot B, and the newest committed slot A keeps its bytes"
    );
    let second_slot = flash.borrow().bytes[1].clone();

    install_program(&mut store, program()).expect("the third install commits");
    assert!(
        flash.borrow().bytes[1] == second_slot,
        "the third install writes slot A, and the newest committed slot B keeps its bytes"
    );
}

/// Installs `program` while the runtime holds the program of the container with install sequence
/// `held`, and returns the commit.
fn install_held(
    store: &mut ProgramStore,
    program: &[u8],
    held: Option<u64>,
) -> Result<Committed, InstallError> {
    let container = transfer_of(program);
    let checked = check(&container, &device(), |_| Ok(()))?;
    let mut pending = start(store, container, checked, held)?;
    loop {
        match pending.step(store)? {
            Progress::Pending(next) => pending = next,
            Progress::Committed(committed) => return Ok(committed),
        }
    }
}

/// The slot that holds the newest committed container.
fn newest_slot(store: &mut ProgramStore) -> Slot {
    order(store)
        .expect("the slots are ordered")
        .newest
        .expect("a committed container")
        .slot
}

#[test]
fn an_install_never_writes_the_slot_of_the_program_the_runtime_holds() {
    // Specification 34 §5.1 and MEM-14: an install writes the slot that does not hold the container
    // the runtime's program came from, whose image the runtime reads in place. Worked by hand: the
    // runtime holds sequence 1 from slot A; an install writes sequence 2 to slot B; a second install
    // while the runtime still holds sequence 1 writes slot B again, sequence 3, replacing the
    // container that never ran, and slot A keeps its bytes; once the runtime holds sequence 3 from
    // slot B, the next install, sequence 4, writes slot A. Catches the rule without the held program,
    // which writes sequence 3 to slot A, over the program the runtime reads.
    let (flash, mut store) = erased_store();
    install_program(&mut store, program()).expect("sequence 1 commits to slot A");
    let held = flash.borrow().bytes[0].clone();

    let second = install_held(&mut store, program(), Some(1)).expect("sequence 2 commits");
    assert_eq!((second.sequence, newest_slot(&mut store)), (2, Slot::B));
    let third = install_held(&mut store, program(), Some(1)).expect("sequence 3 commits");
    assert_eq!(
        (third.sequence, newest_slot(&mut store)),
        (3, Slot::B),
        "the container that never ran is replaced"
    );
    assert!(
        flash.borrow().bytes[0] == held,
        "the held program's slot A keeps its bytes"
    );

    let fourth = install_held(&mut store, program(), Some(3)).expect("sequence 4 commits");
    assert_eq!(
        (fourth.sequence, newest_slot(&mut store)),
        (4, Slot::A),
        "the program held from slot B keeps it"
    );
}

#[test]
fn the_highest_committed_sequence_is_selected() {
    // Specification 34 §5.1 and DEPLOY-03: the committed container with the highest install sequence
    // is the installed program, whichever slot holds it, and the other one is the previous program.
    // Catches a boot that takes the first slot, or the slot written last.
    let (_, mut store) = erased_store();
    install_program(&mut store, program()).expect("commit 1 is written to slot A");
    install_program(&mut store, program()).expect("commit 2 is written to slot B");
    let selected = active(&mut store, &device()).expect("a program is selected");
    assert_eq!(selected.sequence, 2);
    assert_eq!(selected.program_sha256, sha256(program()));
    assert_eq!(selected.bytes, program());

    install_program(&mut store, program()).expect("commit 3 is written to slot A");
    assert_eq!(
        active(&mut store, &device())
            .expect("a program is selected")
            .sequence,
        3
    );
    let older = previous(&mut store, &device()).expect("the slots are ordered");
    assert_eq!(older.map(|stored| stored.sequence), Some(2));
}

#[test]
fn the_sequence_rises_by_one_per_commit() {
    // Specification 34 §5.1 (the device's install sequence): the first commit takes sequence 1, and
    // each later commit takes one more than the newest committed sequence. Catches a repeated or a
    // skipped sequence number.
    let (_, mut store) = erased_store();
    let sequences: Vec<u64> = (0..3)
        .map(|_| {
            install_program(&mut store, program())
                .expect("the install commits")
                .sequence
        })
        .collect();
    assert_eq!(sequences, [1, 2, 3]);
}

#[test]
fn a_damaged_newest_committed_container_refuses_the_boot() {
    // Specification 34 §5.1 and DEPLOY-03: a committed container whose payload fails its CRC-32
    // refuses the boot, and the older container is not selected in its place. Catches a boot that
    // falls back to the other slot.
    let (flash, mut store) = erased_store();
    install_program(&mut store, program()).expect("commit 1 is written to slot A");
    install_program(&mut store, program()).expect("commit 2 is written to slot B");
    // The image of slot B starts at byte 64, so this flips a byte of the program, entry kind 3.
    flash.borrow_mut().bytes[1][64] ^= 0x01;
    assert_eq!(
        active(&mut store, &device()),
        Err(SelectError::Refused(AdmissionError::Container(
            InstallationError::PayloadChecksum(3)
        )))
    );
    let older = previous(&mut store, &device()).expect("the slots are ordered");
    assert_eq!(older.map(|stored| stored.sequence), Some(1));
}

#[test]
fn two_committed_containers_of_one_sequence_refuse_the_boot_and_name_it() {
    // Specification 34 §5.1 and DEPLOY-03: neither of two committed containers with one install
    // sequence is the newest, so the boot refuses, and the reason names the sequence. Catches a boot
    // that picks one of them.
    let (flash, mut store) = erased_store();
    install_program(&mut store, program()).expect("commit 1 is written to slot A");
    let copy = flash.borrow().bytes[0].clone();
    flash.borrow_mut().bytes[1] = copy;
    let error = active(&mut store, &device()).expect_err("the boot refuses");
    assert_eq!(error, SelectError::Scan(ScanError::SequenceTie(1)));
    assert_eq!(
        error.to_string(),
        "no installed program: two committed containers have sequence 1"
    );
}

#[test]
fn an_erased_store_selects_no_program() {
    // Specification 34 §5.1 and §11.2: an erased store holds no installed program, and the boot
    // refuses with its reason. Catches a boot that reads erased flash as a container.
    let (_, mut store) = erased_store();
    assert_eq!(active(&mut store, &device()), Err(SelectError::NoContainer));
    assert_eq!(previous(&mut store, &device()), Ok(None));
}

#[test]
fn an_install_refused_by_admission_writes_no_byte() {
    // Specification 34 §5.1.1 and DEPLOY-04: a program the device's application slot cannot hold is
    // refused before any write. The program and its record need 6,754 bytes (6,412 + 342) and the
    // device has 100. Catches a write that starts before admission.
    let (flash, mut store) = erased_store();
    let small = DeviceDescriptor {
        application_slot_bytes: Some(100),
        ..device()
    };
    let refused = begin(&mut store, transfer_of(program()), &small, |_| Ok(()));
    assert_eq!(
        refused.err(),
        Some(InstallError::Admission(AdmissionError::Deployment(
            RuntimeError::DeploymentRefused(AdmissionRefusal::ApplicationSlot {
                needed: 6754,
                available: 100,
            })
        )))
    );
    assert_eq!(flash.borrow().writes, 0);
}

#[test]
fn an_install_on_a_device_that_trusts_keys_refuses_an_unsigned_program_before_any_write() {
    // Specification 44 §5.11 and §10.2 step 4: a device that trusts a key admits only an image that
    // key signed. The board probe's image is unsigned (algorithm 0), so the device that trusts one key
    // refuses it as untrusted, before the store writes a byte; the device that trusts none admits it.
    // Catches an install check that ignores the device's keys, which a chip only found at the next
    // load, after the install had replaced the program it would boot.
    static KEYS: [[u8; 32]; 1] = [[0x5A; 32]];
    let (flash, mut store) = erased_store();
    let trusting = DeviceDescriptor {
        trusted_keys: &KEYS,
        ..device()
    };
    let refused = begin(&mut store, transfer_of(program()), &trusting, |_| Ok(()));
    assert_eq!(
        refused.err(),
        Some(InstallError::Image(Refusal::Signature(
            SignatureError::Untrusted
        )))
    );
    assert_eq!(flash.borrow().writes, 0);
    assert!(begin(&mut store, transfer_of(program()), &device(), |_| Ok(())).is_ok());
}

#[test]
fn an_install_that_cannot_restore_its_retained_values_is_refused_before_any_write() {
    // Specification 11 §6.7.1 (decision G13): the retained-value check runs before any write, and
    // its refusal ends the install. Catches a commit that starts before the check.
    let (flash, mut store) = erased_store();
    let refused = begin(&mut store, transfer_of(program()), &device(), |_| {
        Err(RuntimeError::RetainCorruption(
            "the retained values differ".into(),
        ))
    });
    assert_eq!(
        refused.err(),
        Some(InstallError::Retain(RuntimeError::RetainCorruption(
            "the retained values differ".into()
        )))
    );
    assert_eq!(flash.borrow().writes, 0);
}

#[test]
fn a_flash_error_ends_the_install_and_keeps_the_previous_program() {
    // Specification 34 §5.1 and DEPLOY-03: a flash error ends the install with its error, and the
    // previous program stays selected. The container takes two erase units, so the second write of
    // the install is an erase. Catches a failed write that is ignored, or one that is selected.
    let (flash, mut store) = erased_store();
    install_program(&mut store, program()).expect("commit 1 is written to slot A");
    install_program(&mut store, program()).expect("commit 2 is written to slot B");
    let writes = flash.borrow().writes;
    flash.borrow_mut().fail_write = Some(writes + 1);
    let mut pending = begin(&mut store, transfer_of(program()), &device(), |_| Ok(()))
        .expect("the third install begins");
    let error = loop {
        match pending.step(&mut store) {
            Ok(Progress::Pending(next)) => pending = next,
            Ok(Progress::Committed(_)) => panic!("the failed write must end the install"),
            Err(error) => break error,
        }
    };
    assert_eq!(error, InstallError::Flash(FlashError::Erase));
    assert_eq!(
        active(&mut store, &device())
            .expect("the previous program is selected")
            .sequence,
        2
    );
}

#[test]
fn a_read_back_that_differs_from_the_container_ends_the_install() {
    // Specification 34 §5.1 and DEPLOY-03: programmed bytes must read back as written before the
    // commit point. The first program write follows the two erase units, so it is write number two
    // of the install. Catches a commit point written over payload bytes the flash did not keep.
    let (flash, mut store) = erased_store();
    install_program(&mut store, program()).expect("commit 1 is written to slot A");
    install_program(&mut store, program()).expect("commit 2 is written to slot B");
    let writes = flash.borrow().writes;
    flash.borrow_mut().corrupt_write = Some(writes + 2);
    let mut pending = begin(&mut store, transfer_of(program()), &device(), |_| Ok(()))
        .expect("the third install begins");
    let error = loop {
        match pending.step(&mut store) {
            Ok(Progress::Pending(next)) => pending = next,
            Ok(Progress::Committed(_)) => panic!("the read-back must end the install"),
            Err(error) => break error,
        }
    };
    assert_eq!(error, InstallError::Verify);
    assert_eq!(
        active(&mut store, &device())
            .expect("the previous program is selected")
            .sequence,
        2
    );
}
