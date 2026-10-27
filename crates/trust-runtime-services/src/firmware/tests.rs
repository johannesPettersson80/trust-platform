use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec;

use super::*;
use crate::tests::program_store::{FlashOp, FlashProbe};

/// A chip that selects what it is told to and refuses nothing.
struct Chip;

impl FirmwareDevice for Chip {
    fn running(&self) -> Slot {
        Slot::A
    }

    fn check(
        &mut self,
        _flash: &mut dyn ProgramFlash,
        _slot: Slot,
        _head: &[u8],
        _bytes: usize,
    ) -> Result<FirmwareImage, String> {
        Ok(FirmwareImage {
            version: "1.0.0".into(),
        })
    }

    fn activate(&mut self, _slot: Slot) -> Result<(), String> {
        Ok(())
    }
}

/// The kept first bytes are padded to whole 4-byte units with erased bytes and programmed, then read
/// back; a read-back that differs ends the commit before the slot is selected. 7 kept bytes are
/// programmed as 8. Catches a commit that selects a slot whose first bytes did not read back, which
/// the boot loader would boot.
#[test]
fn a_commit_programs_the_kept_bytes_reads_them_back_and_then_selects() {
    let flash = FlashProbe::erased();
    let mut store = FirmwareStore {
        slots: flash.store(),
        device: Box::new(Chip),
    };
    let commit = FirmwareCommit::new(&store, Slot::B, &[0xE9, 1, 2, 3, 4, 5, 6]);
    let CommitStep::Pending(commit) = commit.step(&mut store).expect("the program") else {
        panic!("the program is not the last step");
    };
    let CommitStep::Pending(commit) = commit.step(&mut store).expect("the read-back") else {
        panic!("the read-back is not the last step");
    };
    let CommitStep::Done = commit.step(&mut store).expect("the selection") else {
        panic!("the selection ends the commit");
    };
    assert_eq!(
        flash.ops(),
        vec![
            FlashOp::Program {
                slot: Slot::B,
                offset: 0,
                len: 8
            },
            FlashOp::Read {
                slot: Slot::B,
                offset: 0,
                len: 8
            },
        ]
    );
    assert_eq!(&flash.slots()[1][..8], &[0xE9, 1, 2, 3, 4, 5, 6, 0xFF]);

    let failing = FlashProbe::erased();
    failing.fail_at(1);
    let mut store = FirmwareStore {
        slots: failing.store(),
        device: Box::new(Chip),
    };
    let commit = FirmwareCommit::new(&store, Slot::B, &[0xE9; 8]);
    let CommitStep::Pending(commit) = commit.step(&mut store).expect("the program") else {
        panic!("the program is not the last step");
    };
    assert_eq!(
        commit.step(&mut store).err(),
        Some(String::from("the firmware slot's flash read failed"))
    );
}

/// `capabilities` names the slot `a` or `b`, the version, whether it is confirmed and the version
/// rolled back. Catches a slot named by its partition (`ota_1`) or a missing member.
#[test]
fn the_facts_are_reported_by_their_contract_names() {
    let facts = FirmwareFacts {
        slot: Slot::A,
        version: "0.24.76".into(),
        confirmed: false,
        rolled_back: None,
    };
    assert_eq!(
        facts.to_json(),
        json!({"slot": "a", "version": "0.24.76", "confirmed": false, "rolled_back": null})
    );
}
