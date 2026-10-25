//! An upload written into the program store on the resource loop (specification 34 §5.1, PORT-05,
//! §11.2): while the resource is Stopped, `transfer.begin` erases the first erase unit of the slot the
//! upload goes into, each part is written as it arrives, one flash step in each pass, and its reply
//! waits for the last step; `program.install` checks the container where it lies in the slot and
//! commits its header and its magic, the second half of the magic last. A `restart` that starts the
//! resource ends such an upload, and an upload begun while the program runs is held in memory and
//! writes no flash. The program flash is the fake of `tests/program_store.rs` with the C6's units,
//! which maps its slots for reading in place (MEM-14). Every expected operation and reply is worked
//! out here from the specification, not taken from the code under test.

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
use trust_runtime_core::scheduler::ResourceState;
use trust_runtime_core::watchdog::RetainMode;

use super::{check_image_retained_values, Runner};
use crate::events::EVENT_STORE_CAPACITY;
use crate::installation::set_sequence;
use crate::loading::select::boot;
use crate::tests::program_store::{
    device, program, transfer_of, CheckpointProbe, FlashOp, FlashProbe,
};
use crate::tests::runner_ports::{Fixture, TestPlatform, TestRunner};
use crate::transfer::{TransferSession, PART_BYTES};

/// Why the loop has no program, as a chip that booted from an erased flash says.
const REASON: &str = "no TRSTA002 container in either slot";
/// The boot number of the transfer session.
const BOOT: u32 = 3;
/// The passes after which a test gives up waiting for a reply.
const MAX_PASSES: usize = 200;

/// The lowercase hexadecimal digits of `bytes`.
fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::new(), |mut text, byte| {
        let _ = write!(text, "{byte:02x}");
        text
    })
}

/// The two reads of the slot headers with which the choice of a slot begins: the committed
/// containers are ordered first (§5.1).
fn header_reads() -> Vec<FlashOp> {
    [Slot::A, Slot::B]
        .into_iter()
        .map(|slot| FlashOp::Read {
            slot,
            offset: 0,
            len: 60,
        })
        .collect()
}

/// The steps that write the part of `len` bytes at `offset` into `slot` (§5.1): the erase of its
/// unit unless it is the first part, whose unit `transfer.begin` erased; the program of its bytes after
/// the 60-byte header in steps of 256 bytes, each padded to whole 4-byte units; and their read-back.
fn part_ops(slot: Slot, offset: u32, len: u32) -> Vec<FlashOp> {
    let mut ops = Vec::new();
    if offset > 0 {
        ops.push(FlashOp::Erase { slot, offset });
    }
    let start = offset.max(60);
    let chunks: Vec<(u32, usize)> = (start..offset + len)
        .step_by(256)
        .map(|at| (at, ((offset + len).min(at + 256) - at) as usize))
        .collect();
    ops.extend(chunks.iter().map(|&(offset, len)| FlashOp::Program {
        slot,
        offset,
        len: len.next_multiple_of(4),
    }));
    ops.extend(chunks.iter().map(|&(offset, len)| FlashOp::Read {
        slot,
        offset,
        len: len.next_multiple_of(4),
    }));
    ops
}

/// The commit of an upload in the store (§5.1): the header after the magic, the first half of the
/// magic, their read-back, and the second half of the magic, the commit point.
fn header_commit_ops(slot: Slot) -> Vec<FlashOp> {
    vec![
        FlashOp::Program {
            slot,
            offset: 8,
            len: 52,
        },
        FlashOp::Program {
            slot,
            offset: 0,
            len: 4,
        },
        FlashOp::Read {
            slot,
            offset: 0,
            len: 4,
        },
        FlashOp::Read {
            slot,
            offset: 8,
            len: 52,
        },
        FlashOp::Program {
            slot,
            offset: 4,
            len: 4,
        },
    ]
}

/// The request `transfer.part` for the part at `index` of `container`, as request `id`.
fn part_request(id: u64, transfer: &str, container: &[u8], index: usize) -> Value {
    let part = container
        .chunks(PART_BYTES)
        .nth(index)
        .expect("the part exists");
    json!({
        "id": id,
        "type": "transfer.part",
        "params": {
            "transfer": transfer,
            "offset": index * PART_BYTES,
            "data": STANDARD.encode(part),
            "crc32": format!("{:08x}", crc32::checksum(part)),
        },
    })
}

/// One device whose program store maps its slots.
struct Rig {
    ports: Fixture,
    flash: FlashProbe,
}

impl Rig {
    fn new() -> Self {
        Self {
            ports: Fixture::with_frames_of(8_192),
            flash: FlashProbe::erased().mapping(),
        }
    }

    /// The Stopped loop of a device that has no program, given a transfer session and the store.
    fn stopped(&self) -> TestRunner<'_> {
        let mut checkpoints = CheckpointProbe::erased();
        Runner::stopped(
            self.ports.link.clone(),
            self.ports.wait.clone(),
            TestPlatform::default(),
            self.ports.clock.clone(),
            SmolStr::new_static(REASON),
            EVENT_STORE_CAPACITY,
        )
        .with_transfers(TransferSession::new(BOOT, 65_536, 8_192))
        .with_program_store(self.flash.store(), move |admitted| {
            check_image_retained_values(&mut checkpoints, admitted)
        })
    }

    /// The loop of a device that runs the board probe, given a transfer session and the store.
    fn running(&self) -> TestRunner<'_> {
        let mut checkpoints = CheckpointProbe::erased();
        self.ports
            .runner_with(TestPlatform::default())
            .with_transfers(TransferSession::new(BOOT, 65_536, 8_192))
            .with_program_store(self.flash.store(), move |admitted| {
                check_image_retained_values(&mut checkpoints, admitted)
            })
    }

    /// Sends `request`, runs passes until the loop answers, and returns the one answer with the
    /// flash operations of each pass, the pass that read the request first.
    fn exchange(&self, runner: &mut TestRunner<'_>, request: &Value) -> (Value, Vec<Vec<FlashOp>>) {
        self.ports.link.send(request);
        let mut passes = Vec::new();
        for _ in 0..=MAX_PASSES {
            let before = self.flash.ops().len();
            runner.run_pass().expect("the test wait does not fail");
            passes.push(self.flash.ops()[before..].to_vec());
            let mut replies = self.ports.link.take_replies();
            if let Some(reply) = replies.pop() {
                assert!(replies.is_empty(), "one answer to {request}: {replies:?}");
                return (reply, passes);
            }
        }
        panic!("{request} was not answered within {MAX_PASSES} passes");
    }

    /// `transfer.begin` for `container`, as request 1.
    fn begin(&self, runner: &mut TestRunner<'_>, container: &[u8]) -> (Value, Vec<Vec<FlashOp>>) {
        self.exchange(
            runner,
            &json!({
                "id": 1,
                "type": "transfer.begin",
                "params": {"bytes": container.len(), "sha256": hex(&sha256(container))},
            }),
        )
    }
}

/// The operations of `passes`, one after the other, and a check that no pass took more than one.
fn one_step_per_pass(passes: &[Vec<FlashOp>]) -> Vec<FlashOp> {
    for (index, ops) in passes.iter().enumerate().skip(1) {
        assert!(
            ops.len() <= 1,
            "pass {index} took more than one step: {ops:?}"
        );
    }
    passes.concat()
}

#[test]
fn an_upload_into_the_store_is_written_as_it_arrives_and_its_install_commits_the_header_last() {
    // Specification 34 §5.1, PORT-05 and §11.2. The container (the fixture program and its record,
    // with the transferred sequence 0) takes two parts. In the Stopped loop of a chip without a
    // program the upload goes into slot A, the slot an install writes when no slot is committed.
    // `transfer.begin` orders the committed containers (the two header reads) in the pass that reads
    // it and erases slot A's first unit in the next pass, which writes its reply. Part 0 programs its
    // bytes after the header and reads them back, and part 1 erases its unit first; each part's reply
    // comes in the pass of its last read-back, and every pass after the one that reads a request
    // takes one flash operation. `program.install` orders the containers again and commits the
    // header (bytes 8 to 60), the first half of the magic, their read-back and the second half of the
    // magic; the reply carries the device's sequence 1, the program's SHA-256 and length. The slot
    // then holds the container with sequence 1, which the boot selects. Wrong behaviour caught: a
    // begin that erases the whole slot or none; a part whose reply comes before its bytes are
    // written or read back; a header written with part 0 (with the transferred sequence 0); an
    // install that copies the container again; a commit point programmed before the read-back.
    let rig = Rig::new();
    let mut runner = rig.stopped();
    let container = transfer_of(RetainMode::None);
    let len = u32::try_from(container.len()).expect("a short container");
    assert!(container.len() > PART_BYTES && container.len() <= 2 * PART_BYTES);

    let (begun, passes) = rig.begin(&mut runner, &container);
    assert_eq!(begun["ok"], json!(true), "{begun}");
    let transfer = begun["result"]["transfer"]
        .as_str()
        .expect("begin names the transfer")
        .to_string();
    assert!(
        transfer.starts_with("00000003"),
        "the boot number is high: {transfer}"
    );
    assert_eq!(begun["result"]["part_bytes"], json!(4_096));
    assert_eq!(begun["result"]["next_offset"], json!(0));
    assert_eq!(passes.len(), 2, "the reply comes in the pass of the erase");
    assert_eq!(passes[0], header_reads());
    assert_eq!(
        passes[1],
        vec![FlashOp::Erase {
            slot: Slot::A,
            offset: 0
        }]
    );

    let (first, passes) = rig.exchange(&mut runner, &part_request(2, &transfer, &container, 0));
    assert_eq!(first["result"], json!({"next_offset": 4_096}), "{first}");
    assert_eq!(
        passes[0],
        Vec::new(),
        "the pass that reads a part writes nothing"
    );
    assert_eq!(one_step_per_pass(&passes), part_ops(Slot::A, 0, 4_096));

    let (second, passes) = rig.exchange(&mut runner, &part_request(3, &transfer, &container, 1));
    assert_eq!(
        second["result"],
        json!({"next_offset": container.len()}),
        "{second}"
    );
    assert_eq!(passes[0], Vec::new());
    assert_eq!(
        one_step_per_pass(&passes),
        part_ops(Slot::A, 4_096, len - 4_096)
    );

    let (installed, passes) = rig.exchange(
        &mut runner,
        &json!({"id": 41, "type": "program.install", "params": {"transfer": transfer}}),
    );
    assert_eq!(
        installed,
        json!({
            "id": 41,
            "ok": true,
            "result": {
                "status": "installed",
                "sequence": 1,
                "program_sha256": hex(&sha256(program())),
                "bytes": program().len(),
            },
            "audit_id": "audit-4",
        }),
        "{installed}"
    );
    assert_eq!(
        passes[0],
        header_reads(),
        "the checks read the slot in place"
    );
    assert_eq!(one_step_per_pass(&passes[1..]), header_commit_ops(Slot::A));

    let mut committed = container.clone();
    set_sequence(&mut committed, 1).expect("a TRSTA002 container");
    assert_eq!(
        &rig.flash.slots()[0][..container.len()],
        committed.as_slice()
    );
    let booted = boot(&mut rig.flash.store(), &device(), 1024 * 1024)
        .expect("the boot selects the installed container");
    assert_eq!(booted.sequence, 1);
}

#[test]
fn a_part_whose_write_fails_is_refused_and_written_when_sent_again() {
    // Specification 34 PORT-05, the transfer part bullet: a step that fails refuses the part with
    // the store's failure and leaves the next offset where it was, so the client sends the part
    // again. The first program of part 1 fails: operation 36, after the two header reads, the begin's
    // erase (2), part 0's 32 operations (3 to 34) and part 1's erase (35). The part sent again erases
    // its unit again and is written. Catches a failed part counted as received, which would leave the
    // slot without its bytes under a complete upload.
    let rig = Rig::new();
    let mut runner = rig.stopped();
    let container = transfer_of(RetainMode::None);
    let len = u32::try_from(container.len()).expect("a short container");
    let (begun, _) = rig.begin(&mut runner, &container);
    let transfer = begun["result"]["transfer"].as_str().unwrap().to_string();
    let (first, _) = rig.exchange(&mut runner, &part_request(2, &transfer, &container, 0));
    assert_eq!(first["ok"], json!(true));
    assert_eq!(rig.flash.ops().len(), 35);
    rig.flash.fail_at(36);

    let (failed, _) = rig.exchange(&mut runner, &part_request(3, &transfer, &container, 1));
    assert_eq!(
        failed,
        json!({"id": 3, "ok": false, "error": "the program store's flash program failed"})
    );
    let (status, _) = rig.exchange(
        &mut runner,
        &json!({"id": 4, "type": "transfer.status", "params": {"transfer": transfer}}),
    );
    assert_eq!(
        status["result"],
        json!({"state": "receiving", "next_offset": 4_096, "bytes": container.len()})
    );

    let before = rig.flash.ops().len();
    let (again, _) = rig.exchange(&mut runner, &part_request(5, &transfer, &container, 1));
    assert_eq!(
        again["result"],
        json!({"next_offset": container.len()}),
        "{again}"
    );
    assert_eq!(
        rig.flash.ops()[before..],
        part_ops(Slot::A, 4_096, len - 4_096)[..]
    );
}

#[test]
fn a_repeat_of_a_part_is_compared_with_the_slot_and_writes_nothing() {
    // Specification 34 PORT-05, the transfer part bullet: a repeat of a received part with the same
    // bytes is acknowledged again with the next offset, and one with other bytes is refused, the
    // received bytes staying; for an upload in the store the comparison is with what the store and
    // the session hold. Catches a repeat written again (which would program over programmed flash)
    // and one accepted with other bytes.
    let rig = Rig::new();
    let mut runner = rig.stopped();
    let container = transfer_of(RetainMode::None);
    let (begun, _) = rig.begin(&mut runner, &container);
    let transfer = begun["result"]["transfer"].as_str().unwrap().to_string();
    rig.exchange(&mut runner, &part_request(2, &transfer, &container, 0));
    let writes = rig.flash.writes();

    let (same, _) = rig.exchange(&mut runner, &part_request(3, &transfer, &container, 0));
    assert_eq!(same["result"], json!({"next_offset": 4_096}), "{same}");
    let mut other = container.clone();
    other[100] ^= 0x01;
    let (changed, _) = rig.exchange(&mut runner, &part_request(4, &transfer, &other, 0));
    assert_eq!(
        changed,
        json!({
            "id": 4,
            "ok": false,
            "error": "part at offset 0 was received with other bytes",
            "audit_id": "audit-4",
        })
    );
    assert_eq!(rig.flash.writes(), writes, "no repeat writes the flash");
}

#[test]
fn an_erase_that_fails_at_begin_leaves_no_upload_open() {
    // Specification 34 PORT-05, the transfer begin bullet: an erase that fails refuses begin with
    // `the program store's flash erase failed`, and no upload stays open. The erase is operation 2,
    // after the two header reads. Catches a refused begin that keeps its upload, which a second begin
    // would then find busy.
    let rig = Rig::new();
    let mut runner = rig.stopped();
    let container = transfer_of(RetainMode::None);
    rig.flash.fail_at(2);
    let (refused, _) = rig.begin(&mut runner, &container);
    assert_eq!(
        refused,
        json!({"id": 1, "ok": false, "error": "the program store's flash erase failed"})
    );
    let (again, _) = rig.begin(&mut runner, &container);
    assert_eq!(again["ok"], json!(true), "{again}");
}

#[test]
fn a_restart_ends_an_upload_in_the_store_and_a_running_program_keeps_its_upload_in_memory() {
    // Specification 34 PORT-05 and §5.1: a `restart` that starts the resource ends an upload written
    // into the store, whose id is then unknown; an upload begun while the program runs is held in
    // memory and writes no flash (Appendix D.2: no flash write while a program runs). The board probe
    // runs, stops by `shutdown`, takes the begin and part 0 of an upload in the store, and starts
    // again by `restart`. Catches a store upload that outlives the start, whose next part would erase
    // flash while the program runs, and a begin while running that erases.
    let rig = Rig::new();
    let mut runner = rig.running();
    rig.ports.pass_at(&mut runner, 0);
    rig.ports.pass_at(&mut runner, 10);
    let _ = rig
        .ports
        .call_at(&mut runner, 15, &json!({"id": 90, "type": "shutdown"}));
    rig.ports.pass_at(&mut runner, 20);
    assert_eq!(runner.resource_state(), ResourceState::Stopped);

    let container = transfer_of(RetainMode::None);
    let (begun, passes) = rig.begin(&mut runner, &container);
    assert_eq!(
        passes.concat().last(),
        Some(&FlashOp::Erase {
            slot: Slot::A,
            offset: 0
        })
    );
    let transfer = begun["result"]["transfer"].as_str().unwrap().to_string();
    rig.exchange(&mut runner, &part_request(2, &transfer, &container, 0));

    let restarted = rig.ports.call(
        &mut runner,
        &json!({"id": 3, "type": "restart", "params": {"mode": "cold"}}),
    );
    assert_eq!(restarted["ok"], json!(true), "{restarted}");
    assert_eq!(runner.resource_state(), ResourceState::Running);
    let status = rig.ports.call(
        &mut runner,
        &json!({"id": 4, "type": "transfer.status", "params": {"transfer": transfer}}),
    );
    assert_eq!(
        status,
        json!({"id": 4, "ok": false, "error": "unknown transfer", "audit_id": "audit-5"})
    );

    let writes = rig.flash.writes();
    let (in_memory, passes) = rig.begin(&mut runner, &container);
    assert_eq!(in_memory["ok"], json!(true), "{in_memory}");
    assert_eq!(
        passes.len(),
        1,
        "an upload in memory is answered in the pass that reads it"
    );
    assert_eq!(rig.flash.writes(), writes, "and writes no flash");
}
