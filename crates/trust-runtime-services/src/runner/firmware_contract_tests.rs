//! A firmware update on the resource loop (specification 34 TARGET-04; specification 11, the firmware
//! update contract): while the resource is Stopped, `transfer.begin {"for": "firmware"}` writes the
//! upload into the firmware slot the chip does not run from as its parts arrive, keeping its first
//! bytes; `firmware.install` has the chip check the image, programs the kept bytes, reads them back
//! and selects the slot, one step per pass, and the reply waits for the last step; the chip resets into
//! the new firmware once the link has sent the reply and 100 ms have passed, and answers no request
//! meanwhile. The firmware flash is the program store's fake flash with the C6's units, and the
//! chip-specific parts are a fake that records what the loop asked of it. Every expected value is
//! worked out here from the specification.

use alloc::boxed::Box;
use alloc::format;
use alloc::rc::Rc;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use core::cell::{Cell, RefCell};
use core::fmt::Write as _;

use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use serde_json::{json, Value};
use smol_str::SmolStr;
use trust_runtime_core::crc32;
use trust_runtime_core::deployment::sha256;
use trust_runtime_core::ports::ProgramFlash;
use trust_runtime_core::retain::checkpoint::Slot;
use trust_runtime_core::watchdog::RetainMode;

use super::{check_image_retained_values, Runner};
use crate::events::EVENT_STORE_CAPACITY;
use crate::firmware::{FirmwareDevice, FirmwareImage, FirmwareStore};
use crate::loading::firmware_install::NOT_FIRMWARE;
use crate::loading::program_install::FIRMWARE_UPLOAD;
use crate::tests::program_store::{transfer_of, CheckpointProbe, FlashOp, FlashProbe};
use crate::tests::runner_ports::{ms, Fixture, TestPlatform, TestRunner};
use crate::transfer::{TransferSession, PART_BYTES};

/// The boot number of the transfer session.
const BOOT: u32 = 5;
/// The passes after which a test gives up waiting for a reply.
const MAX_PASSES: usize = 200;
/// The version the fake chip's check reads from an image.
const VERSION: &str = "0.24.76";

fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::new(), |mut text, byte| {
        let _ = write!(text, "{byte:02x}");
        text
    })
}

/// A firmware image of `len` bytes: the ESP image magic, then a pattern.
fn image(len: usize) -> Vec<u8> {
    let mut bytes: Vec<u8> = (0..len)
        .map(|index| u8::try_from(index % 251).expect("a remainder below 251 is a byte"))
        .collect();
    bytes[0] = 0xE9;
    bytes
}

/// What the loop asked of the fake chip.
#[derive(Default)]
struct Asked {
    /// Each check: the slot, the kept first bytes and the upload's length.
    checks: Vec<(Slot, Vec<u8>, usize)>,
    activated: Vec<Slot>,
    /// The refusal the check answers with, when set.
    refuse: Option<String>,
}

/// The chip-specific parts of the update: the chip runs from slot A, and its check takes an image
/// that begins with the ESP magic and whose bytes after the kept ones lie in the slot as uploaded.
struct FakeChip {
    asked: Rc<RefCell<Asked>>,
    expected: Vec<u8>,
}

impl FirmwareDevice for FakeChip {
    fn running(&self) -> Slot {
        Slot::A
    }

    fn check(
        &mut self,
        flash: &mut dyn ProgramFlash,
        slot: Slot,
        head: &[u8],
        bytes: usize,
    ) -> Result<FirmwareImage, String> {
        let mut asked = self.asked.borrow_mut();
        asked.checks.push((slot, head.to_vec(), bytes));
        if let Some(refusal) = asked.refuse.clone() {
            return Err(refusal);
        }
        let mut rest = vec![0u8; bytes - head.len()];
        let at = u32::try_from(head.len()).expect("a short head");
        flash
            .read(slot, at, &mut rest)
            .map_err(|_| String::from("read"))?;
        if head.first() != Some(&0xE9) || rest != self.expected[head.len()..] {
            return Err(String::from("not the uploaded image"));
        }
        Ok(FirmwareImage {
            version: VERSION.into(),
        })
    }

    fn activate(&mut self, slot: Slot) -> Result<(), String> {
        self.asked.borrow_mut().activated.push(slot);
        Ok(())
    }
}

/// One chip with a program store and firmware slots.
struct Rig {
    ports: Fixture,
    program: FlashProbe,
    firmware: FlashProbe,
    asked: Rc<RefCell<Asked>>,
    /// The platform's resets of the chip.
    resets: Rc<Cell<u32>>,
}

impl Rig {
    fn new() -> Self {
        Self {
            ports: Fixture::with_frames_of(8_192),
            program: FlashProbe::erased().mapping(),
            firmware: FlashProbe::erased(),
            asked: Rc::new(RefCell::new(Asked::default())),
            resets: Rc::new(Cell::new(0)),
        }
    }

    /// The test platform, whose resets the rig counts.
    fn platform(&self) -> TestPlatform {
        TestPlatform {
            resets: Rc::clone(&self.resets),
            ..TestPlatform::default()
        }
    }

    fn store(&self, expected: &[u8]) -> FirmwareStore {
        FirmwareStore {
            slots: self.firmware.store(),
            device: Box::new(FakeChip {
                asked: Rc::clone(&self.asked),
                expected: expected.to_vec(),
            }),
        }
    }

    /// The Stopped loop of a chip without a program, with a session, a program store and, unless
    /// `firmware` is `None`, firmware slots expecting that image.
    fn stopped(&self, firmware: Option<&[u8]>) -> TestRunner<'_> {
        let mut checkpoints = CheckpointProbe::erased();
        let runner = Runner::stopped(
            self.ports.link.clone(),
            self.ports.wait.clone(),
            self.platform(),
            self.ports.clock.clone(),
            SmolStr::new_static("no TRSTA002 container in either slot"),
            EVENT_STORE_CAPACITY,
        )
        .with_transfers(TransferSession::new(BOOT, 65_536, 8_192))
        .with_program_store(self.program.store(), move |admitted| {
            check_image_retained_values(&mut checkpoints, admitted)
        });
        match firmware {
            Some(expected) => runner.with_firmware(self.store(expected)),
            None => runner,
        }
    }

    /// Sends `request`, runs passes until the loop answers, and returns the answer with the firmware
    /// flash's operations of each pass.
    fn exchange(&self, runner: &mut TestRunner<'_>, request: &Value) -> (Value, Vec<Vec<FlashOp>>) {
        self.ports.link.send(request);
        let mut passes = Vec::new();
        for _ in 0..=MAX_PASSES {
            let before = self.firmware.ops().len();
            runner.run_pass().expect("the test wait does not fail");
            passes.push(self.firmware.ops()[before..].to_vec());
            let mut replies = self.ports.link.take_replies();
            if let Some(reply) = replies.pop() {
                assert!(replies.is_empty(), "one answer to {request}: {replies:?}");
                return (reply, passes);
            }
        }
        panic!("{request} was not answered within {MAX_PASSES} passes");
    }

    /// Uploads `bytes` with `for` set to `target`, and returns the transfer id.
    fn upload(&self, runner: &mut TestRunner<'_>, bytes: &[u8], target: &str) -> String {
        let (begun, _) = self.exchange(
            runner,
            &json!({"id": 1, "type": "transfer.begin", "params": {
                "bytes": bytes.len(), "sha256": hex(&sha256(bytes)), "for": target}}),
        );
        assert_eq!(begun["ok"], json!(true), "{begun}");
        let transfer = begun["result"]["transfer"]
            .as_str()
            .expect("begin names the transfer")
            .to_string();
        for (index, part) in bytes.chunks(PART_BYTES).enumerate() {
            let (sent, _) = self.exchange(
                runner,
                &json!({"id": 2 + index, "type": "transfer.part", "params": {
                    "transfer": transfer,
                    "offset": index * PART_BYTES,
                    "data": STANDARD.encode(part),
                    "crc32": format!("{:08x}", crc32::checksum(part)),
                }}),
            );
            assert_eq!(sent["ok"], json!(true), "{sent}");
        }
        transfer
    }
}

#[test]
fn a_firmware_upload_goes_into_the_other_slot_and_its_install_selects_it_and_resets() {
    // The chip runs from slot A, so the upload of 5,000 bytes goes into slot B: `transfer.begin`
    // erases B's first unit in the pass after the one that reads it, and the program store is not
    // touched. `firmware.install` has the chip check the image in slot B with the 60 kept bytes and
    // the upload's 5,000 in the pass that reads the request, then programs the kept bytes at offset
    // 0, reads them back and selects B, one step per pass; the reply names the version the check read, slot b and the restart. The chip
    // resets only once the link has sent the reply and 100 ms have passed, and answers no request in
    // between. Wrong behaviour caught: an upload written into the running slot or the program store;
    // the image's first bytes written with its first part (a bootable half image); a selection before
    // the read-back; a reset before the reply is out, which loses the reply; a request answered after
    // the reply, which a reset would lose.
    let rig = Rig::new();
    let firmware = image(5_000);
    let mut runner = rig.stopped(Some(&firmware));
    let program_ops = rig.program.ops().len();
    let transfer = rig.upload(&mut runner, &firmware, "firmware");
    assert_eq!(
        rig.program.ops().len(),
        program_ops,
        "the program store is not touched"
    );
    let ops = rig.firmware.ops();
    assert_eq!(
        ops.first(),
        Some(&FlashOp::Erase {
            slot: Slot::B,
            offset: 0
        })
    );
    assert!(
        ops.iter()
            .all(|op| !matches!(op, FlashOp::Program { offset, .. } if *offset < 60)),
        "the first 60 bytes are kept: {ops:?}"
    );

    let (installed, passes) = rig.exchange(
        &mut runner,
        &json!({"id": 9, "type": "firmware.install", "params": {"transfer": transfer}}),
    );
    assert_eq!(installed["ok"], json!(true), "{installed}");
    assert_eq!(
        installed["result"],
        json!({"status": "installed", "version": VERSION, "slot": "b", "restarting": true})
    );
    {
        let asked = rig.asked.borrow();
        assert_eq!(
            asked.checks,
            vec![(Slot::B, firmware[..60].to_vec(), 5_000)]
        );
        assert_eq!(asked.activated, vec![Slot::B]);
        assert_eq!(rig.resets.get(), 0, "no reset before the reply is out");
    }
    // The pass that reads the request runs the chip's check, which reads the slot after the kept
    // bytes; the commit's steps follow, one per pass.
    let commit: Vec<FlashOp> = passes.concat();
    assert_eq!(
        commit,
        vec![
            FlashOp::Read {
                slot: Slot::B,
                offset: 60,
                len: 4_940
            },
            FlashOp::Program {
                slot: Slot::B,
                offset: 0,
                len: 60
            },
            FlashOp::Read {
                slot: Slot::B,
                offset: 0,
                len: 60
            },
        ]
    );
    assert_eq!(&rig.firmware.slots()[1][..5_000], firmware.as_slice());

    // A request now is not answered: the chip is about to reset.
    rig.ports.link.send(&json!({"id": 10, "type": "status"}));
    runner.run_pass().expect("the test wait does not fail");
    assert_eq!(rig.ports.link.take_replies(), Vec::<Value>::new());
    assert_eq!(rig.resets.get(), 0, "not before 100 ms");
    rig.ports.clock.advance(ms(100));
    runner.run_pass().expect("the test wait does not fail");
    assert_eq!(rig.resets.get(), 1, "the reset after 100 ms");
}

#[test]
fn a_firmware_update_is_refused_where_it_cannot_run() {
    // Specification 11, the firmware update contract. Each refusal leaves nothing selected and
    // nothing reset: a `for` that names neither target; a runtime without firmware slots; a program
    // upload given to `firmware.install`; a firmware upload given to `program.install`, which stays
    // open for `firmware.install`; and an image the chip's check refuses, whose reply is the check's
    // text. Wrong behaviour caught: a firmware upload installed as a program or the reverse, a
    // refused image selected, and an unknown `for` taken as a program upload.
    let rig = Rig::new();
    let firmware = image(300);
    let mut runner = rig.stopped(Some(&firmware));
    let (wrong_for, _) = rig.exchange(
        &mut runner,
        &json!({"id": 1, "type": "transfer.begin", "params": {
            "bytes": 3, "sha256": hex(&sha256(b"abc")), "for": "kernel"}}),
    );
    assert_eq!(
        wrong_for["error"],
        json!("invalid params: for must be \"program\" or \"firmware\""),
        "{wrong_for}"
    );

    let transfer = rig.upload(&mut runner, &firmware, "firmware");
    let (as_program, _) = rig.exchange(
        &mut runner,
        &json!({"id": 5, "type": "program.install", "params": {"transfer": transfer}}),
    );
    assert_eq!(as_program["error"], json!(FIRMWARE_UPLOAD), "{as_program}");

    rig.asked.borrow_mut().refuse = Some("the firmware image's SHA-256 is wrong".into());
    let (refused, _) = rig.exchange(
        &mut runner,
        &json!({"id": 6, "type": "firmware.install", "params": {"transfer": transfer}}),
    );
    assert_eq!(
        refused["error"],
        json!("the firmware image's SHA-256 is wrong"),
        "the upload was still open for firmware.install: {refused}"
    );
    rig.asked.borrow_mut().refuse = None;

    let program = transfer_of(RetainMode::None);
    let program_transfer = rig.upload(&mut runner, &program, "program");
    let (as_firmware, _) = rig.exchange(
        &mut runner,
        &json!({"id": 7, "type": "firmware.install", "params": {"transfer": program_transfer}}),
    );
    assert_eq!(as_firmware["error"], json!(NOT_FIRMWARE), "{as_firmware}");
    assert_eq!(rig.asked.borrow().activated, Vec::<Slot>::new());
    assert_eq!(rig.resets.get(), 0);

    let plain = Rig::new();
    let mut without = plain.stopped(None);
    let (unsupported, _) = plain.exchange(
        &mut without,
        &json!({"id": 1, "type": "transfer.begin", "params": {
            "bytes": 3, "sha256": hex(&sha256(b"abc")), "for": "firmware"}}),
    );
    assert_eq!(unsupported["error"], json!("unsupported request"));
}

#[test]
fn a_firmware_upload_is_refused_while_the_program_runs() {
    // A chip writes its firmware only while the resource is Stopped (TARGET-04; no flash is erased
    // while a program runs, Appendix D.2): `transfer.begin {"for": "firmware"}` on a running resource
    // is refused as `resource_not_running`, and nothing is erased. Wrong behaviour caught: an erase
    // of 0.4 s with the interrupts masked in the middle of a running control program.
    let rig = Rig::new();
    let firmware = image(300);
    let mut checkpoints = CheckpointProbe::erased();
    let mut runner = rig
        .ports
        .runner_with(rig.platform())
        .with_transfers(TransferSession::new(BOOT, 65_536, 8_192))
        .with_program_store(rig.program.store(), move |admitted| {
            check_image_retained_values(&mut checkpoints, admitted)
        })
        .with_firmware(rig.store(&firmware));
    rig.ports.pass_at(&mut runner, 0);
    let (refused, _) = rig.exchange(
        &mut runner,
        &json!({"id": 1, "type": "transfer.begin", "params": {
            "bytes": firmware.len(), "sha256": hex(&sha256(&firmware)), "for": "firmware"}}),
    );
    assert_eq!(refused["ok"], json!(false), "{refused}");
    assert_eq!(
        refused["error_code"],
        json!("resource_not_running"),
        "{refused}"
    );
    assert!(rig.firmware.ops().is_empty(), "nothing erased");
}
