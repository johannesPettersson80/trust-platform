//! The stepped install on the resource loop (specification 34 §5.1, §11.2, PORT-05 and STATE-08): a
//! `program.install` commits to the program store one step in each pass of the loop, its `installed`
//! reply waits for the last step, and the loop reads no request meanwhile. The loop is the Stopped loop
//! of a device without a program, over a transfer session and a program store whose flash is the fake
//! flash of `tests/program_store.rs`, which has the C6's units; the checkpoint flash is a fake with the
//! C6's units too. Every expected value is written out here from the specification, the container
//! layout of §5.1 and the requests below, not taken from the code under test.

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use core::fmt::Write as _;

use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use serde_json::{json, Value};
use smol_str::SmolStr;
use trust_runtime_core::crc32;
use trust_runtime_core::deployment::sha256;
use trust_runtime_core::retain::checkpoint::Slot;
use trust_runtime_core::retain::RetainSnapshot;
use trust_runtime_core::value::Value as Retained;
use trust_runtime_core::vm::PreparationLimits;
use trust_runtime_core::watchdog::RetainMode;

use super::{check_retained_values, Runner};
use crate::events::EVENT_STORE_CAPACITY;
use crate::loading::select::boot;
use crate::tests::program_store::{
    committed_image, device, transfer_of, transfer_with, CheckpointProbe, FlashOp, FlashProbe,
    PROGRAM,
};
use crate::tests::runner_ports::{Fixture, TestPlatform, TestRunner};
use crate::transfer::{TransferSession, PART_BYTES};

/// Why the loop has no program, as a chip that booted from an erased flash says.
const REASON: &str = "no TRSTA001 or TRSTA002 container in either slot";
/// The boot number of the transfer session.
const BOOT: u32 = 3;
/// Memory enough for the boot of any container of these tests.
const ROOM: usize = 1024 * 1024;
/// The passes after which a test gives up waiting for the loop to write a reply.
const MAX_PASSES: usize = 200;
/// The number of the `program.install` request of the tests.
const INSTALL_ID: u64 = 41;

/// The lowercase hexadecimal digits of `bytes`.
fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::new(), |mut text, byte| {
        let _ = write!(text, "{byte:02x}");
        text
    })
}

/// The `program.install` request `id` for the transfer `transfer`.
fn install_request(id: u64, transfer: &str) -> Value {
    json!({"id": id, "type": "program.install", "params": {"transfer": transfer}})
}

/// The flash operations of the commit of a container of `len` bytes into slot `slot`, in the order of
/// specification 34 §5.1 with 4 KiB erase units, 4-byte program units and a step of 256 bytes:
///
/// 1. erase the units the container needs, starting with the unit that holds the magic;
/// 2. program the payloads, which start after the 60-byte header, 256 bytes at a time, then the header
///    fields after the magic (bytes 8 to 60), then the first four bytes of the magic;
/// 3. read back every byte that was programmed: the first four bytes of the magic, then bytes 8 to the
///    end in steps of 256 bytes; and
/// 4. program the last four bytes of the magic, the commit point.
///
/// A step that ends inside a program unit is padded to whole units.
fn commit_ops(slot: Slot, len: usize) -> Vec<FlashOp> {
    let at = |position: usize| u32::try_from(position).expect("a short container");
    let mut ops = Vec::new();
    for unit in 0..len.div_ceil(4096) {
        ops.push(FlashOp::Erase {
            slot,
            offset: at(unit * 4096),
        });
    }
    for start in (60..len).step_by(256) {
        let bytes = (start + 256).min(len) - start;
        ops.push(FlashOp::Program {
            slot,
            offset: at(start),
            len: bytes.next_multiple_of(4),
        });
    }
    ops.push(FlashOp::Program {
        slot,
        offset: 8,
        len: 52,
    });
    ops.push(FlashOp::Program {
        slot,
        offset: 0,
        len: 4,
    });
    ops.push(FlashOp::Read {
        slot,
        offset: 0,
        len: 4,
    });
    for start in (8..len).step_by(256) {
        let bytes = (start + 256).min(len) - start;
        ops.push(FlashOp::Read {
            slot,
            offset: at(start),
            len: bytes.next_multiple_of(4),
        });
    }
    ops.push(FlashOp::Program {
        slot,
        offset: 4,
        len: 4,
    });
    ops
}

/// The two reads of the slot headers with which an install begins: it orders the committed
/// containers before it chooses its slot.
fn header_reads() -> Vec<FlashOp> {
    vec![
        FlashOp::Read {
            slot: Slot::A,
            offset: 0,
            len: 60,
        },
        FlashOp::Read {
            slot: Slot::B,
            offset: 0,
            len: 60,
        },
    ]
}

/// A checkpoint whose `@program/Plant/activations` is `value`: the program fixture declares
/// `activations` as a retained `DINT` in its program `Plant`.
fn checkpoint_of(value: Retained) -> CheckpointProbe {
    let mut snapshot = RetainSnapshot::default();
    snapshot.insert("@program/Plant/activations", value);
    CheckpointProbe::with_checkpoint(&snapshot)
}

/// One device: the ports of the loop, the program flash and the checkpoint flash it is given.
struct Rig {
    ports: Fixture,
    flash: FlashProbe,
    checkpoints: CheckpointProbe,
}

impl Rig {
    /// A device over `flash` and `checkpoints`. Its link moves frames of up to 8,192 bytes, as the
    /// chips' USB links do, so that a full part of 4,096 bytes (5,464 base64 characters) fits a frame.
    fn new(flash: FlashProbe, checkpoints: CheckpointProbe) -> Self {
        Self {
            ports: Fixture::with_frames_of(8_192),
            flash,
            checkpoints,
        }
    }

    /// The Stopped loop of a device that has no program, given a transfer session and a program store.
    fn runner(&self) -> TestRunner<'_> {
        self.runner_with(TestPlatform::default())
    }

    /// As [`Self::runner`], over `platform`.
    fn runner_with(&self, platform: TestPlatform) -> TestRunner<'_> {
        let mut checkpoints = self.checkpoints.clone();
        self.stopped(platform)
            .with_transfers(TransferSession::new(BOOT, 65_536, 8_192))
            .with_program_store(self.flash.store(), move |admitted| {
                check_retained_values(&mut checkpoints, admitted)
            })
    }

    /// The Stopped loop of a device that has no program, over `platform`, without a session or a store.
    fn stopped(&self, platform: TestPlatform) -> TestRunner<'_> {
        Runner::stopped(
            self.ports.link.clone(),
            self.ports.wait.clone(),
            platform,
            self.ports.clock.clone(),
            SmolStr::new_static(REASON),
            EVENT_STORE_CAPACITY,
        )
    }

    /// Uploads `container` as one transfer, in parts, and returns its id as the requests name it.
    fn upload(&self, runner: &mut TestRunner<'_>, container: &[u8]) -> String {
        let begin = self.ports.call(
            runner,
            &json!({
                "id": 1,
                "type": "transfer.begin",
                "params": {"bytes": container.len(), "sha256": hex(&sha256(container))},
            }),
        );
        let transfer = begin["result"]["transfer"]
            .as_str()
            .expect("begin names the transfer")
            .to_string();
        for (index, part) in container.chunks(PART_BYTES).enumerate() {
            let reply = self.ports.call(
                runner,
                &json!({
                    "id": 2 + index,
                    "type": "transfer.part",
                    "params": {
                        "transfer": transfer,
                        "offset": index * PART_BYTES,
                        "data": STANDARD.encode(part),
                        "crc32": format!("{:08x}", crc32::checksum(part)),
                    },
                }),
            );
            assert_eq!(reply["ok"], json!(true), "part {index} is stored: {reply}");
        }
        transfer
    }

    /// Sends `program.install` for `transfer` and runs the pass that reads it.
    fn send_install(&self, runner: &mut TestRunner<'_>, id: u64, transfer: &str) {
        self.ports.link.send(&install_request(id, transfer));
        runner.run_pass().expect("the test wait does not fail");
    }

    /// Uploads `container`, asks for its install and runs passes until the loop answers, and returns
    /// the one answer.
    fn install(&self, runner: &mut TestRunner<'_>, container: &[u8], id: u64) -> Value {
        let transfer = self.upload(runner, container);
        self.send_install(runner, id, &transfer);
        for _ in 0..=MAX_PASSES {
            let mut replies = self.ports.link.take_replies();
            if let Some(reply) = replies.pop() {
                assert!(replies.is_empty(), "one answer to the install: {replies:?}");
                return reply;
            }
            runner.run_pass().expect("the test wait does not fail");
        }
        panic!("the install was not answered within {MAX_PASSES} passes");
    }
}

#[test]
fn an_install_commits_one_step_per_pass_and_replies_installed_after_the_last() {
    // Specification 34 §5.1 (the program store: erase, program, read back, commit point, in steps of
    // at most one erase unit or one step budget), §11.2 (the reply is `{"status":"installed",
    // "sequence","program_sha256","bytes"}`), PORT-05 (one reply in flight) and STATE-08 (the loop
    // serves the link and feeds the watchdog between steps). The pass that reads the request writes no
    // byte and answers nothing; each later pass takes at most one flash operation and feeds the
    // watchdog once; the reply is written in the pass of the last operation, which programs the
    // second half of the magic, and in no pass before it. Wrong behaviour caught: a commit that runs
    // in one pass, or that takes two erases in one; a reply written at the request, before the
    // commit point; a reply that waits for a pass after the commit point; a loop that stops feeding
    // the watchdog while it commits; a slot written with the transferred sequence 0 instead of the
    // device's sequence 1.
    let rig = Rig::new(FlashProbe::erased(), CheckpointProbe::erased());
    let platform = TestPlatform::default();
    let feeds = platform.feeds.clone();
    let idle = platform.idle_passes.clone();
    let mut runner = rig.runner_with(platform);
    let container = transfer_of(RetainMode::None);
    let transfer = rig.upload(&mut runner, &container);
    // The requests so far: `transfer.begin`, the parts, and the install, which is audited when read.
    let requests = 2 + container.len().div_ceil(PART_BYTES);

    rig.send_install(&mut runner, INSTALL_ID, &transfer);
    assert_eq!(
        rig.ports.link.take_replies(),
        Vec::<Value>::new(),
        "the install is answered when its commit ends"
    );
    assert_eq!(
        rig.flash.ops(),
        header_reads(),
        "the pass that reads the request only orders the committed containers"
    );
    assert_eq!(rig.flash.writes(), 0, "and writes no byte");

    let (feeds_before, idle_before) = (feeds.get(), idle.get());
    let mut seen = rig.flash.ops().len();
    let mut passes = 0;
    let replies = loop {
        passes += 1;
        assert!(passes <= MAX_PASSES, "the commit did not end");
        runner.run_pass().expect("the test wait does not fail");
        let ops = rig.flash.ops();
        assert!(
            ops.len() - seen <= 1,
            "pass {passes} took more than one step: {:?}",
            &ops[seen..]
        );
        seen = ops.len();
        let replies = rig.ports.link.take_replies();
        if !replies.is_empty() {
            break replies;
        }
    };
    let passes = u32::try_from(passes).expect("a short commit");
    assert_eq!(
        feeds.get() - feeds_before,
        passes,
        "the watchdog is fed once in every pass of the commit"
    );
    assert_eq!(
        idle.get() - idle_before,
        passes,
        "and the platform's idle work runs once in every pass"
    );

    let ops = rig.flash.ops();
    assert_eq!(
        &ops[2..],
        commit_ops(Slot::A, container.len()).as_slice(),
        "the steps are those of §5.1, in its order, into slot A"
    );
    assert_eq!(
        ops.last(),
        Some(&FlashOp::Program {
            slot: Slot::A,
            offset: 4,
            len: 4
        }),
        "the reply is written in the pass that programs the second half of the magic"
    );
    assert_eq!(
        replies,
        vec![json!({
            "id": INSTALL_ID,
            "ok": true,
            "result": {
                "status": "installed",
                "sequence": 1,
                "program_sha256": hex(&sha256(PROGRAM)),
                "bytes": PROGRAM.len(),
            },
            "audit_id": format!("audit-{requests}"),
        })]
    );

    let image = committed_image(1, RetainMode::None);
    let slots = rig.flash.slots();
    assert_eq!(
        &slots[0][..image.len()],
        image.as_slice(),
        "slot A holds the container with the install sequence the device gave it"
    );
    assert!(
        slots[0][image.len()..].iter().all(|byte| *byte == 0xff),
        "and nothing after it"
    );
    assert!(
        slots[1].iter().all(|byte| *byte == 0xff),
        "slot B is untouched"
    );

    // The link takes requests again.
    let status = rig
        .ports
        .call(&mut runner, &json!({"id": 99, "type": "status"}));
    assert_eq!(status["result"]["state"], json!("stopped"));
}

#[test]
fn no_request_is_read_while_the_install_reply_is_pending() {
    // Specification 34 PORT-05 (one reply in flight: while a reply is being sent the link gives the
    // control loop no further request): the install's reply is the reply in flight from the moment its
    // checks have passed. A `status` request that arrives during the commit is not polled from the link
    // in any pass of the commit, and is answered after the `installed` reply, in the order of the
    // requests. Wrong behaviour caught: a loop that reads and answers requests while it commits, which
    // would answer the `status` before the `installed` reply and let a second install or a `restart`
    // run in the middle of a commit.
    let rig = Rig::new(FlashProbe::erased(), CheckpointProbe::erased());
    let mut runner = rig.runner();
    let container = transfer_of(RetainMode::None);
    let transfer = rig.upload(&mut runner, &container);
    rig.send_install(&mut runner, INSTALL_ID, &transfer);
    rig.ports.link.send(&json!({"id": 99, "type": "status"}));

    let mut passes = 0;
    let replies = loop {
        passes += 1;
        assert!(passes <= MAX_PASSES, "the commit did not end");
        let polls = rig.ports.link.polls();
        runner.run_pass().expect("the test wait does not fail");
        let replies = rig.ports.link.take_replies();
        if replies.is_empty() {
            assert_eq!(
                rig.ports.link.polls(),
                polls,
                "pass {passes} polled the link while the install's reply was pending"
            );
        } else {
            break replies;
        }
    };
    assert_eq!(replies.len(), 2, "the installed reply, then the status");
    assert_eq!(replies[0]["id"], json!(INSTALL_ID));
    assert_eq!(replies[0]["result"]["status"], json!("installed"));
    assert_eq!(replies[1]["id"], json!(99));
    assert_eq!(replies[1]["result"]["state"], json!("stopped"));
}

#[test]
fn a_reset_after_any_commit_step_boots_the_previous_program() {
    // Specification 34 §5.1 and DEPLOY-03: an install that stops at any step leaves the newest
    // committed container as it was, so the previous program stays selected, and only the commit
    // point selects the new one. The flash is copied after the request pass and after every pass of the
    // second install, and a boot reads each copy as a reset at that moment would leave it. Wrong
    // behaviour caught: a commit point written before the read-back or the payloads; a write into the
    // slot of the running program; a step that selects a half-written container.
    let rig = Rig::new(FlashProbe::erased(), CheckpointProbe::erased());
    let mut runner = rig.runner();
    let container = transfer_of(RetainMode::None);
    let first = rig.install(&mut runner, &container, 40);
    assert_eq!(first["result"]["sequence"], json!(1), "the first install");

    let mut copies = vec![rig.flash.slots()];
    let transfer = rig.upload(&mut runner, &container);
    rig.send_install(&mut runner, INSTALL_ID, &transfer);
    copies.push(rig.flash.slots());
    loop {
        assert!(copies.len() <= MAX_PASSES, "the commit did not end");
        runner.run_pass().expect("the test wait does not fail");
        copies.push(rig.flash.slots());
        if !rig.ports.link.take_replies().is_empty() {
            break;
        }
    }

    let (last, earlier) = copies.split_last().expect("copies were taken");
    assert!(earlier.len() > 60, "the commit takes more than 60 steps");
    for (index, slots) in earlier.iter().enumerate() {
        let mut store = FlashProbe::from_slots(slots.clone()).store();
        let booted =
            boot(&mut store, &device(), ROOM).expect("a reset leaves a program that boots");
        assert_eq!(
            booted.sequence, 1,
            "a reset after {index} steps of the second install boots the first program"
        );
        assert_eq!(booted.program, PROGRAM);
    }
    let mut store = FlashProbe::from_slots(last.clone()).store();
    let booted = boot(&mut store, &device(), ROOM).expect("the committed install boots");
    assert_eq!(
        booted.sequence, 2,
        "after its commit point it is the program"
    );
}

#[test]
fn a_step_that_fails_replies_with_its_error_and_keeps_the_previous_program() {
    // Specification 34 §5.1 and DEPLOY-03: a flash operation that fails ends the install, the reply is
    // that failure with the id of the request, and the previous program stays selected. The second
    // install runs the two header reads of its begin and then its 60 operations, and the operation
    // with each index fails in turn. A failure of a header read ends the install in the pass that reads
    // the request, and the others in the pass of their step. Wrong behaviour caught: a failed step that
    // is ignored, so the commit goes on; a failure that is never answered; a reply with another id.
    let first = Rig::new(FlashProbe::erased(), CheckpointProbe::erased());
    let container = transfer_of(RetainMode::None);
    let reply = first.install(&mut first.runner(), &container, 40);
    assert_eq!(reply["ok"], json!(true), "the first install: {reply}");
    let previous = first.flash.slots();

    let mut operations = header_reads();
    operations.extend(commit_ops(Slot::B, container.len()));
    for (index, operation) in operations.iter().enumerate() {
        let rig = Rig::new(
            FlashProbe::from_slots(previous.clone()),
            CheckpointProbe::erased(),
        );
        rig.flash.fail_at(index);
        let reply = rig.install(&mut rig.runner(), &container, INSTALL_ID);
        let kind = match operation {
            FlashOp::Erase { .. } => "erase",
            FlashOp::Program { .. } => "program",
            FlashOp::Read { .. } => "read",
        };
        assert_eq!(reply["id"], json!(INSTALL_ID), "operation {index} fails");
        assert_eq!(reply["ok"], json!(false), "operation {index} fails");
        assert_eq!(
            reply["error"],
            json!(format!("the program store's flash {kind} failed")),
            "operation {index} fails"
        );
        let mut store = FlashProbe::from_slots(rig.flash.slots()).store();
        let booted = boot(&mut store, &device(), ROOM).expect("the previous program boots");
        assert_eq!(
            booted.sequence, 1,
            "the previous program stays selected after operation {index} failed"
        );
    }
}

#[test]
fn the_chip_refuses_an_install_whose_retained_values_cannot_be_restored_before_any_write() {
    // Specification 11 §6.7.1 and 34 §11.2 (decision G13): the install runs the new program's warm start
    // with the retained values of the newest checkpoint before any byte is written, and a value the
    // warm start refuses refuses the install with the warm start's own error. The checkpoint holds
    // `@program/Plant/activations` as a BOOL, which a DINT destination cannot take (§6.7: only a safe
    // numeric widening such as INT to DINT is migrated). The transfer is used up by the refusal (§11.2).
    // Wrong behaviour caught: a commit that starts before the check, so the program is installed and
    // then fails its warm start at the next boot; a refusal that is dropped; a check that reads the
    // checkpoint through a writable handle.
    let checkpoints = checkpoint_of(Retained::Bool(true));
    let rig = Rig::new(FlashProbe::erased(), checkpoints.clone());
    let mut runner = rig.runner();
    let container = transfer_of(RetainMode::File);
    let transfer = rig.upload(&mut runner, &container);
    let refused = rig
        .ports
        .call(&mut runner, &install_request(INSTALL_ID, &transfer));
    assert_eq!(refused["id"], json!(INSTALL_ID));
    assert_eq!(refused["ok"], json!(false));
    assert_eq!(
        refused["error"],
        json!("the retained values cannot be restored: retain migration error 'invalid retained value for '@program/Plant/activations': type mismatch'")
    );
    assert_eq!(
        rig.flash.ops(),
        Vec::<FlashOp>::new(),
        "a refused check touches no flash of the program store"
    );
    assert_eq!(checkpoints.writes(), 0, "and writes no checkpoint");
    let status = rig.ports.call(
        &mut runner,
        &json!({"id": 5, "type": "transfer.status", "params": {"transfer": transfer}}),
    );
    assert_eq!(
        status["error"],
        json!("unknown transfer"),
        "the refusal used up the transfer"
    );
}

#[test]
fn the_chip_installs_when_its_retained_values_restore() {
    // Specification 34 §11.2 and 11 §6.7: the warm start by name migrates an INT to the DINT
    // `activations`, so the same install as above is accepted: the check is not a refusal of every
    // install with retained values. Wrong behaviour caught: a check that refuses whatever the
    // checkpoint holds.
    let rig = Rig::new(FlashProbe::erased(), checkpoint_of(Retained::Int(7)));
    let mut runner = rig.runner();
    let reply = rig.install(&mut runner, &transfer_of(RetainMode::File), INSTALL_ID);
    assert_eq!(reply["ok"], json!(true), "the install commits: {reply}");
    assert_eq!(reply["result"]["status"], json!("installed"));
    assert_eq!(rig.flash.slots()[0][..8], *b"TRSTA002");
}

#[test]
fn a_program_that_keeps_no_retained_values_installs_whatever_the_checkpoint_holds() {
    // Specification 11 §6.7 and 34 §10: with `[runtime.retain] mode = "none"` the program starts cold
    // and restores nothing, so a checkpoint it could not restore does not refuse its install. Wrong
    // behaviour caught: a check that ignores the retain mode of the new program.
    let rig = Rig::new(FlashProbe::erased(), checkpoint_of(Retained::Bool(true)));
    let mut runner = rig.runner();
    let reply = rig.install(&mut runner, &transfer_of(RetainMode::None), INSTALL_ID);
    assert_eq!(reply["ok"], json!(true), "the install commits: {reply}");
}

#[test]
fn a_damaged_checkpoint_refuses_the_install_as_retained_values_that_cannot_be_restored() {
    // Specification 34 §10.1: when the newest checkpoint fails its check the warm start fails, so an
    // install that would warm start from it is refused before any write. The first record of slot A
    // starts after the 16-byte slot header; its header takes 64 bytes and its commit marker 4 bytes, so
    // its payload starts at byte 84. Wrong behaviour caught: a damaged checkpoint that reads as empty
    // and lets the install through.
    let checkpoints = checkpoint_of(Retained::Int(7));
    checkpoints.damage(84);
    let rig = Rig::new(FlashProbe::erased(), checkpoints);
    let mut runner = rig.runner();
    let container = transfer_of(RetainMode::File);
    let transfer = rig.upload(&mut runner, &container);
    let refused = rig
        .ports
        .call(&mut runner, &install_request(INSTALL_ID, &transfer));
    assert_eq!(refused["ok"], json!(false));
    assert_eq!(
        refused["error"],
        json!("the retained values cannot be restored: retain corruption 'newest checkpoint payload fails its check'")
    );
    assert_eq!(rig.flash.ops(), Vec::<FlashOp>::new());
}

#[test]
fn a_program_that_cannot_be_built_for_the_check_has_its_own_refusal() {
    // Specification 11 §6.7.1 and 34 §11.2 (decision G13): a new program that cannot be built to try
    // its warm start is refused with the check's own error, not as retained values that cannot be
    // restored. The record asks for an artifact limit of one byte, which admission grants (it is below
    // the device's maximum) and preparation exceeds. Wrong behaviour caught: the build failure under
    // the prefix `the retained values cannot be restored`.
    let rig = Rig::new(FlashProbe::erased(), checkpoint_of(Retained::Int(7)));
    let mut runner = rig.runner();
    let limits = PreparationLimits {
        max_artifact_bytes: 1,
        ..PreparationLimits::default()
    };
    let container = transfer_with(RetainMode::File, &limits);
    let transfer = rig.upload(&mut runner, &container);
    let refused = rig
        .ports
        .call(&mut runner, &install_request(INSTALL_ID, &transfer));
    assert_eq!(refused["ok"], json!(false));
    assert_eq!(
        refused["error"],
        json!("the retained-value check could not run: preparation resource limit exceeded")
    );
    assert_eq!(rig.flash.ops(), Vec::<FlashOp>::new());
}

#[test]
fn a_running_device_refuses_the_install_and_writes_nothing() {
    // Specification 34 §11.2 (decision Q11): a program is installed only while the resource is Stopped.
    // A running program refuses `program.install` with `resource_not_running` and its state, and the
    // flash is not touched. Wrong behaviour caught: a commit that starts during RUN, which would erase
    // flash while the scans run.
    let rig = Rig::new(FlashProbe::erased(), CheckpointProbe::erased());
    let mut runner = rig
        .ports
        .runner()
        .with_transfers(TransferSession::new(BOOT, 65_536, 8_192))
        .with_program_store(rig.flash.store(), |_| Ok(()));
    rig.ports.pass_at(&mut runner, 0);
    let container = transfer_of(RetainMode::None);
    let transfer = rig.upload(&mut runner, &container);
    let refused = rig
        .ports
        .call(&mut runner, &install_request(INSTALL_ID, &transfer));
    assert_eq!(refused["ok"], json!(false));
    assert_eq!(
        refused["error"],
        json!("resource is running and its scheduler does not accept commands")
    );
    assert_eq!(refused["error_code"], json!("resource_not_running"));
    assert_eq!(rig.flash.ops(), Vec::<FlashOp>::new());
}

#[test]
fn a_loop_without_a_program_store_answers_unsupported_request_and_keeps_the_upload() {
    // Specification 34 §11.2: a platform without a program store answers `unsupported request` once its
    // state check has passed, and takes nothing: the complete upload is still there. Wrong behaviour
    // caught: an install that takes the upload and then has nowhere to put it.
    let rig = Rig::new(FlashProbe::erased(), CheckpointProbe::erased());
    let mut runner = rig
        .stopped(TestPlatform::default())
        .with_transfers(TransferSession::new(BOOT, 65_536, 8_192));
    let container = transfer_of(RetainMode::None);
    let transfer = rig.upload(&mut runner, &container);
    let refused = rig
        .ports
        .call(&mut runner, &install_request(INSTALL_ID, &transfer));
    assert_eq!(refused["error"], json!("unsupported request"));
    let status = rig.ports.call(
        &mut runner,
        &json!({"id": 6, "type": "transfer.status", "params": {"transfer": transfer}}),
    );
    assert_eq!(status["result"]["state"], json!("complete"));
}
