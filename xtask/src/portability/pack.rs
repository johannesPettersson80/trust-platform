//! `xtask portability pack-project` (specification 34 §5.1): the bench packer that builds the
//! `TRSTA002` container of a first flash from a compiled program image (specification 44 §10.1)
//! and its `deployment-record.bin`.
//!
//! Each wire format has one owner: the chip's own `trust_runtime_services::installation` writes
//! the container, and the core's `DeploymentRecord` reads the record. The container carries the
//! install sequence of a first commit, so a packed container that is written into slot A is a
//! committed container, and it has the bytes that the program store commits for the same program and
//! record. Before packing, the packer checks that the record names exactly the program it packs, so
//! a container never pairs a record with another program.

use anyhow::{ensure, Context, Result};
use trust_runtime_core::deployment::{sha256, DeploymentRecord};
use trust_runtime_core::native::image::MAGIC as IMAGE_MAGIC;
use trust_runtime_services::installation::{self, FIRST_SEQUENCE};

/// Build the `TRSTA002` container of a first flash for the program image `program` and `record`: the
/// image as entry kind 3, at the first 64-byte boundary after the header.
///
/// The record is read by the core's own decoder, which refuses a record it cannot read
/// (specification 34 §5.1.1). The container is built by the writer of every image container,
/// `installation::encode_image`, with the install sequence of a first commit, so the bench
/// writes exactly the bytes a chip checks and the program store commits (specification 34 §5.1, "A
/// first flash").
///
/// # Errors
///
/// The program is not a program image (it does not start with the image magic `TRSTI001`), the
/// record does not read, or it describes a program other than the one being packed, by length or
/// by SHA-256.
pub(super) fn pack_project(program: &[u8], record: &[u8]) -> Result<Vec<u8>> {
    ensure!(
        program.starts_with(&IMAGE_MAGIC),
        "the program is not a program image: build the project with trust-runtime build"
    );
    let named = DeploymentRecord::decode(record).context("read the deployment record")?;
    ensure!(
        usize::try_from(named.application.program_bytes)? == program.len(),
        "the record describes a program of {} bytes, this program has {}",
        named.application.program_bytes,
        program.len()
    );
    ensure!(
        named.application.program_sha256 == sha256(program),
        "the record describes another program, not this program.tpi"
    );
    Ok(installation::encode_image(FIRST_SEQUENCE, program, record)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;
    use trust_runtime_core::deployment::{
        ApplicationIdentity, Capabilities, Configuration, Contracts, DeviceDescriptor,
        NumericContract, ProcessImageSummary, ProgramVersion, RequestedLimits, SchedulingProfile,
        TimingEvidence,
    };
    use trust_runtime_core::ports::{ProgramFlash, ProgramStore};
    use trust_runtime_core::retain::checkpoint::{FlashError, FlashGeometry, Slot};
    use trust_runtime_services::loading::install::{start, Checked, Progress};
    use trust_runtime_services::loading::select::boot;

    /// The program the tests pack: 33 bytes that start with the image magic, which is all the packer
    /// and the store read of an image.
    const IMAGE: &[u8] = b"TRSTI001 and the rest of an image";

    /// A valid version-1 record that names the image of `sha256` and `bytes`, written by the core's
    /// own record writer (specification 34 §5.1.1), with the requests of an image: no limit, since an
    /// image is not prepared (specification 44 §10.1).
    fn record_of(program_sha256: [u8; 32], program_bytes: u32) -> Vec<u8> {
        DeploymentRecord {
            application: ApplicationIdentity {
                program_sha256,
                program_bytes,
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
        .encode()
        .expect("a small record encodes")
    }

    /// The record of [`IMAGE`].
    fn image_record() -> Vec<u8> {
        record_of(sha256(IMAGE), 33)
    }

    /// The C6's program unit, in bytes.
    const PROGRAM_UNIT: u32 = 4;
    /// The C6's erase unit, in bytes.
    const ERASE_UNIT: u32 = 4_096;
    /// The bytes of each slot of the fake flash: four erase units.
    const SLOT_LEN: u32 = 16 * 1024;
    /// The most bytes one step of an install programs or reads back: one flash page of the C6.
    const STEP_BYTES: usize = 256;
    /// Memory enough for the boot of any container of these tests.
    const ROOM: usize = 1024 * 1024;

    /// The two slots of a program flash kept in memory, with the C6's units and shared with the store
    /// that drives them, so that a test reads what was written.
    #[derive(Clone)]
    struct Flash(Rc<RefCell<[Vec<u8>; 2]>>);

    fn index(slot: Slot) -> usize {
        match slot {
            Slot::A => 0,
            Slot::B => 1,
        }
    }

    impl Flash {
        /// A flash whose slots are erased.
        fn erased() -> Self {
            Self(Rc::new(RefCell::new([
                vec![0xff; SLOT_LEN as usize],
                vec![0xff; SLOT_LEN as usize],
            ])))
        }

        /// A store over this flash, with the C6's step budget.
        fn store(&self) -> ProgramStore {
            ProgramStore {
                flash: Box::new(self.clone()),
                program_budget: STEP_BYTES,
            }
        }

        /// The bytes of `slot` now.
        fn slot(&self, slot: Slot) -> Vec<u8> {
            self.0.borrow()[index(slot)].clone()
        }

        /// Writes `bytes` at the start of the erased `slot`, as a flasher does.
        fn flash(&self, slot: Slot, bytes: &[u8]) {
            self.0.borrow_mut()[index(slot)][..bytes.len()].copy_from_slice(bytes);
        }
    }

    impl ProgramFlash for Flash {
        fn geometry(&self) -> FlashGeometry {
            FlashGeometry::new(PROGRAM_UNIT, ERASE_UNIT, SLOT_LEN)
                .expect("the C6's program store geometry")
        }

        fn read(&mut self, slot: Slot, offset: u32, bytes: &mut [u8]) -> Result<(), FlashError> {
            let first = offset as usize;
            bytes.copy_from_slice(&self.0.borrow()[index(slot)][first..first + bytes.len()]);
            Ok(())
        }

        fn program(&mut self, slot: Slot, offset: u32, bytes: &[u8]) -> Result<(), FlashError> {
            let first = offset as usize;
            let mut slots = self.0.borrow_mut();
            let cells = &mut slots[index(slot)][first..first + bytes.len()];
            for (cell, byte) in cells.iter_mut().zip(bytes) {
                // A program clears bits and never sets one, as flash does.
                *cell &= *byte;
            }
            Ok(())
        }

        fn erase(&mut self, slot: Slot, offset: u32) -> Result<(), FlashError> {
            let first = offset as usize;
            self.0.borrow_mut()[index(slot)][first..first + ERASE_UNIT as usize].fill(0xff);
            Ok(())
        }
    }

    /// Commits `container` into the store of `flash` as `program.install` commits it once its checks
    /// have passed, with no program held: `start` chooses the slot and the install sequence, and
    /// `step` then erases, programs, reads back and commits, one step at a time. Returns the sequence
    /// the commit gave.
    fn commit(flash: &Flash, container: Vec<u8>) -> u64 {
        let mut store = flash.store();
        let checked = Checked::of(&container).expect("the container reads");
        let mut install = start(&mut store, container, checked, None).expect("the install starts");
        loop {
            match install.step(&mut store).expect("the step runs") {
                Progress::Pending(next) => install = next,
                Progress::Committed(committed) => return committed.sequence,
            }
        }
    }

    /// Specification 44 §10.1: an image is packed as an image, entry kind 3, at byte 64, the first
    /// 64-byte boundary after the 60-byte header. Catches an image packed with another entry kind or
    /// at another offset, which every device would refuse or read wrongly.
    #[test]
    fn an_image_is_packed_as_an_image() {
        let record = image_record();
        let packed = pack_project(IMAGE, &record).expect("the record describes this image");
        assert_eq!(packed[28..32], 3_u32.to_le_bytes());
        assert_eq!(packed[32..36], 64_u32.to_le_bytes());
        let decoded =
            installation::decode_sequenced(&packed).expect("the packed container decodes");
        assert_eq!(decoded.installed.format, installation::ProgramFormat::Image);
        assert_eq!(decoded.installed.program, IMAGE);
    }

    /// Specification 44 §15: a device runs program images only, so the packer refuses any other
    /// program before it reads the record. Catches a packer that packs a program no device admits.
    #[test]
    fn pack_project_refuses_a_program_that_is_not_an_image() {
        let program = b"abc";
        let error = pack_project(program, &record_of(sha256(program), 3))
            .unwrap_err()
            .to_string();
        assert!(error.contains("not a program image"), "{error}");
    }

    #[test]
    fn pack_project_refuses_a_record_of_another_program() {
        let matching = image_record();
        let container = pack_project(IMAGE, &matching).expect("the record describes this program");
        let decoded =
            installation::decode_sequenced(&container).expect("the packed container decodes");
        assert_eq!(decoded.installed.program, IMAGE);
        assert_eq!(decoded.installed.record, matching.as_slice());

        // The record of another program, by SHA-256 or by length, is refused.
        for other in [record_of([0x22; 32], 33), record_of(sha256(IMAGE), 34)] {
            assert!(
                pack_project(IMAGE, &other).is_err(),
                "a record of another program must be refused"
            );
        }
        // A damaged record is refused before it is packed.
        let mut damaged = matching;
        let middle = damaged.len() / 2;
        damaged[middle] ^= 1;
        assert!(pack_project(IMAGE, &damaged).is_err());
    }

    /// Specification 34 §5.1, "A first flash": for one image and record, the container that the
    /// packer writes is, byte for byte, the container that `program.install` commits into an empty
    /// store. A client's upload carries any install sequence (77 here), and the device replaces it with
    /// the sequence of a first commit, 1. Worked out by hand: the container is the 60-byte `TRSTA002`
    /// header, four zero bytes up to the 64-byte boundary where the image starts, the 33-byte image
    /// and the record, so `97 + record.len()` bytes, with the magic `TRSTA002` at byte 0 and the
    /// sequence 1 as eight little-endian bytes at byte 16. Catches a packer that writes a sequence
    /// other than the first commit's (0, or 2), a packer whose layout or version differs from the
    /// writer the store commits, and a store that keeps the sequence of the upload.
    #[test]
    fn a_packed_container_is_the_container_the_store_commits() {
        let record = image_record();
        let packed = pack_project(IMAGE, &record).expect("the record describes this program");
        assert_eq!(&packed[..8], b"TRSTA002");
        assert_eq!(packed[16..24], 1_u64.to_le_bytes());
        assert_eq!(packed.len(), 97 + record.len());

        let flash = Flash::erased();
        let upload = installation::encode_image(77, IMAGE, &record).expect("the upload encodes");
        assert_eq!(
            commit(&flash, upload),
            1,
            "the first commit into an empty store has sequence 1"
        );
        let slot_a = flash.slot(Slot::A);
        assert_eq!(
            &slot_a[..packed.len()],
            packed.as_slice(),
            "slot A holds the packed container byte for byte"
        );
        assert!(
            slot_a[packed.len()..].iter().all(|byte| *byte == 0xff),
            "and nothing else"
        );
        assert!(
            flash.slot(Slot::B).iter().all(|byte| *byte == 0xff),
            "the first commit left slot B erased"
        );
    }

    /// Specification 34 §5.1, "A first flash": a packed container written into slot A is a committed
    /// container. The boot starts it with install sequence 1, and the first `program.install` after it
    /// writes slot B with sequence 2 and leaves slot A as it was flashed, so the install never touches
    /// the program it replaces. Catches a packer whose container the store does not count as
    /// committed (version 1, or a sequence the boot cannot read), after which the first install would
    /// erase slot A under the running program, and a packer that writes sequence 2 or more, so that
    /// the first install would not outrank the flashed program. The boot admits the image on this
    /// computer, whose CPU runs program images.
    #[test]
    fn a_packed_container_boots_as_a_committed_first_flash() {
        let record = image_record();
        let packed = pack_project(IMAGE, &record).expect("the record describes this program");
        let flash = Flash::erased();
        flash.flash(Slot::A, &packed);

        let booted = boot(&mut flash.store(), &DeviceDescriptor::hosted(), ROOM)
            .expect("a packed container boots");
        assert_eq!(booted.sequence, 1, "a first flash has sequence 1");
        assert_eq!(&*booted.program, IMAGE, "and hands over its program");

        let upload = installation::encode_image(0, IMAGE, &record).expect("the upload encodes");
        assert_eq!(
            commit(&flash, upload),
            2,
            "the install after a first flash takes sequence 2"
        );
        assert_eq!(
            &flash.slot(Slot::A)[..packed.len()],
            packed.as_slice(),
            "slot A is as it was flashed"
        );
        let slot_b = flash.slot(Slot::B);
        let committed = installation::committed_header(&slot_b, SLOT_LEN)
            .expect("slot B holds a committed container");
        assert_eq!(committed.sequence, 2, "slot B holds sequence 2");
        let booted =
            boot(&mut flash.store(), &DeviceDescriptor::hosted(), ROOM).expect("the install boots");
        assert_eq!(
            booted.sequence, 2,
            "and the install outranks the flashed program"
        );
    }
}
