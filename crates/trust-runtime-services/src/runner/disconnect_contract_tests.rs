//! The host of a control link goes away in the middle of a run (specification 34, ARCH-07, TARGET-08,
//! PORT-05 and STATE-08). The program keeps its cycles, its outputs, its state and its watchdog feeds
//! as they are without a host, and a host that comes back is answered by the link's rules.
//!
//! The link is the shared line link of a chip's USB port ([`crate::link::LineLink`]) behind a fake of
//! the adapter's hardware, so the rules the tests check are those of the code a chip runs. The board
//! probe runs on the real engine. Every expected value is written out here from the specification and
//! the schedule of the test, not taken from the code under test.

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
use trust_runtime_core::crc32;
use trust_runtime_core::deployment::sha256;
use trust_runtime_core::ports::{ControlLink, LinkFrame, MonotonicClock, PeerClass, PortError};
use trust_runtime_core::scheduler::ResourceState;

use crate::link::{DeviceLineLink, LinkCounters, DEVICE_FRAME_LEN};
use crate::runner::Runner;
use crate::tests::runner_ports::{
    ms, CycleCall, Fixture, IoStep, ScriptedCycle, TestClock, TestPlatform, TestWait,
};
use crate::transfer::{TransferSession, PART_BYTES};

/// The boot number of the transfer session, which every transfer id carries (PORT-05).
const BOOT: u32 = 3;
/// The largest upload the session takes, in bytes.
const MAX_UPLOAD: usize = 65_536;
/// The reply capacity of the session, in bytes: a reply longer than the frame is held up to this.
const REPLY_CAPACITY: usize = 1_048_576;
/// The length of every run, in milliseconds: 320 scans of the 10 ms cycle, the last at 3,190 ms.
const END_MS: u64 = 3_200;
/// The bytes the host takes from the link on one poll: one USB-Serial/JTAG FIFO, as the chip's link
/// writes.
const FIFO: usize = 64;
/// The passes after which a test stops waiting for a reply or a condition, and fails.
const MAX_PASSES: u32 = 20_000;
/// The upload of the second test: three parts of [`PART_BYTES`].
const UPLOAD_BYTES: usize = 3 * PART_BYTES;
/// The button of the board probe, which `led := button` copies to the output in the 10 ms task: each
/// change takes effect for the scans from its time on, in milliseconds.
const BUTTON: [(u64, u8); 8] = [
    (35, 1),
    (205, 0),
    (455, 1),
    (905, 0),
    (1_405, 1),
    (1_905, 0),
    (2_405, 1),
    (2_805, 0),
];

/// The host's end of the control link, and the adapter's hardware that moves bytes for it. A chip's
/// adapter does the same: its receive interrupt hands the host's bytes to the line link, its transmit
/// interrupt takes the reply's bytes from the line link for the host, and a new host session, such as
/// a USB bus reset, is a disconnect here (PORT-05). The host is attached and reading from the start.
#[derive(Clone)]
struct HostLink {
    state: Rc<RefCell<HostState>>,
    clock: TestClock,
}

struct HostState {
    /// The line link of the chip's USB port, for frames of up to `DEVICE_FRAME_LEN` bytes.
    line: Box<DeviceLineLink>,
    /// The bytes the host wrote that the line link has not received yet.
    written: Vec<u8>,
    /// Whether the host is attached.
    attached: bool,
    /// Whether the host takes the reply's bytes from the hardware.
    reading: bool,
    /// Every byte the host has read.
    read: Vec<u8>,
    /// How many bytes of `read` have been taken as complete lines.
    taken: usize,
}

impl HostState {
    /// The receive interrupt: the line link takes what the host wrote, as much as its ring holds.
    fn receive(&mut self) {
        let taken = self.line.receive(&self.written);
        self.written.drain(..taken);
    }

    /// The transmit interrupt: while the host reads, one FIFO of the reply's bytes goes to it.
    fn transmit(&mut self) {
        if !self.reading {
            return;
        }
        let chunk: Vec<u8> = self.line.unsent().iter().take(FIFO).copied().collect();
        self.line.mark_sent(chunk.len());
        self.read.extend_from_slice(&chunk);
    }
}

impl HostLink {
    /// A host that is attached and reading, with a line link of its own.
    fn new(clock: TestClock) -> Self {
        let state = HostState {
            line: Box::new(DeviceLineLink::new(PeerClass::Local)),
            written: Vec::new(),
            attached: true,
            reading: true,
            read: Vec::new(),
            taken: 0,
        };
        Self {
            state: Rc::new(RefCell::new(state)),
            clock,
        }
    }

    /// The host writes one request line.
    fn write(&self, request: &Value) {
        let mut state = self.state.borrow_mut();
        assert!(state.attached, "a host that is away writes nothing");
        state
            .written
            .extend_from_slice(request.to_string().as_bytes());
        state.written.push(b'\n');
    }

    /// The host takes no more of its reply. What the hardware holds stays there, and the link's stall
    /// rule drops the reply a second after its last progress.
    fn stop_reading(&self) {
        self.state.borrow_mut().reading = false;
    }

    /// A new host session, as a USB disconnect is: the host is away, the bytes it wrote that the line
    /// link has not received are gone, and the link drops the reply in flight and the request part-way.
    fn disconnect(&self) {
        let mut state = self.state.borrow_mut();
        state.attached = false;
        state.reading = false;
        state.written.clear();
        state.line.reset_session();
    }

    /// The host comes back and reads its replies.
    fn connect(&self) {
        let mut state = self.state.borrow_mut();
        state.attached = true;
        state.reading = true;
    }

    /// How many bytes the host has read so far.
    fn read_len(&self) -> usize {
        self.state.borrow().read.len()
    }

    /// The next complete line the host has read, without its line feed.
    fn next_line(&self) -> Option<Vec<u8>> {
        let mut state = self.state.borrow_mut();
        let start = state.taken;
        let end = start + state.read[start..].iter().position(|byte| *byte == b'\n')?;
        state.taken = end + 1;
        Some(state.read[start..end].to_vec())
    }

    /// What the line link has done since it started.
    fn counters(&self) -> LinkCounters {
        self.state.borrow().line.counters()
    }
}

impl ControlLink for HostLink {
    fn max_frame_len(&self) -> usize {
        DEVICE_FRAME_LEN
    }

    /// The adapter's receive and transmit interrupts run, then the loop polls the line link.
    fn poll_frame(&mut self, buffer: &mut [u8]) -> Option<LinkFrame> {
        let now = self.clock.now();
        let mut state = self.state.borrow_mut();
        state.receive();
        state.transmit();
        state.line.poll_frame(buffer, now)
    }

    fn send_frame(&mut self, frame: &[u8]) -> Result<(), PortError> {
        let now = self.clock.now();
        let sent = self.state.borrow_mut().line.send_frame(frame, now);
        // The loop sends one reply at a time, and only to the request it polled last, so the line
        // link takes every reply it is given.
        sent.map_err(|_| PortError { code: 1, device: 0 })
    }

    fn sending(&self) -> bool {
        self.state.borrow().line.reply_in_flight()
    }
}

/// The loop of the board probe on the host's link, and what the test has counted: the passes of the
/// loop, and the lines the host read that were not replies.
struct Device<'a> {
    fixture: &'a Fixture,
    runner: Runner<HostLink, TestWait, ScriptedCycle<'a>, TestPlatform, TestClock>,
    link: HostLink,
    feeds: Rc<Cell<u32>>,
    idle_passes: Rc<Cell<u32>>,
    passes: u32,
    /// Lines the host read that are not JSON: the stale part of a reply that a host session dropped.
    fragments: Vec<Vec<u8>>,
    /// The number of the next request.
    next_id: u64,
}

impl<'a> Device<'a> {
    /// The loop of the board probe with the host attached, on the fixture's clock. The power-on scan
    /// runs at the first pass, as at a chip's start.
    fn new(fixture: &'a Fixture) -> Self {
        fixture.wait.advance_time();
        let link = HostLink::new(fixture.clock.clone());
        let platform = TestPlatform::default();
        let feeds = Rc::clone(&platform.feeds);
        let idle_passes = Rc::clone(&platform.idle_passes);
        let runner = fixture
            .runner_with_link_and_platform(link.clone(), platform)
            .with_transfers(TransferSession::new(BOOT, MAX_UPLOAD, REPLY_CAPACITY));
        let _ = fixture.io.take_steps();
        Self {
            fixture,
            runner,
            link,
            feeds,
            idle_passes,
            passes: 0,
            fragments: Vec::new(),
            next_id: 1,
        }
    }

    /// The clock in whole milliseconds.
    fn now_ms(&self) -> u64 {
        u64::try_from(self.fixture.clock.now().since_origin().as_millis())
            .expect("test times are small")
    }

    /// One pass of the loop.
    fn pass(&mut self) {
        self.runner.run_pass().expect("the test wait does not fail");
        self.passes += 1;
    }

    /// Runs passes until the clock reaches `at` milliseconds.
    fn run_until(&mut self, at: u64) {
        while self.now_ms() < at {
            self.pass();
        }
    }

    /// Runs passes until `done` holds, which is checked before each pass.
    fn pass_until(&mut self, done: impl Fn(&Self) -> bool) {
        for _ in 0..MAX_PASSES {
            if done(self) {
                return;
            }
            self.pass();
        }
        panic!("the condition did not hold within {MAX_PASSES} passes");
    }

    /// Writes a request of `kind` with its `params`, and returns its number.
    fn send(&mut self, kind: &str, params: Option<Value>) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        let mut request = json!({"id": id, "type": kind});
        if let Some(params) = params {
            request["params"] = params;
        }
        self.link.write(&request);
        id
    }

    /// Sends a request and runs passes until the host has read its reply, which must answer it. A line
    /// that is not JSON is the stale part of a reply that a session dropped: it is counted, not read.
    fn ask(&mut self, kind: &str, params: Option<Value>) -> Value {
        let id = self.send(kind, params);
        for _ in 0..MAX_PASSES {
            while let Some(line) = self.link.next_line() {
                match serde_json::from_slice::<Value>(&line) {
                    Ok(reply) => {
                        assert_eq!(
                            reply["id"],
                            json!(id),
                            "the reply answers its request, in order"
                        );
                        return reply;
                    }
                    Err(_) => self.fragments.push(line),
                }
            }
            self.pass();
        }
        panic!("request {id} got no reply within {MAX_PASSES} passes");
    }

    /// Sends a request whose reply is longer than `bytes`, and runs passes until the host has read
    /// `bytes` bytes of that reply.
    fn ask_partly(&mut self, kind: &str, params: Option<Value>, bytes: usize) {
        let before = self.link.read_len();
        self.send(kind, params);
        self.pass_until(|device| device.link.read_len() >= before + bytes);
    }
}

/// Schedules the button's changes on the fixture's clock. Each one sets the input image when its time
/// comes, and the scan at or after that time reads it.
fn schedule_button(fixture: &Fixture) {
    for (at, value) in BUTTON {
        let inputs = Rc::clone(&fixture.io.state);
        fixture
            .wait
            .at(ms(at), move || inputs.borrow_mut().inputs = vec![value]);
    }
}

/// The button's value at `at` milliseconds: the value of the last change at or before that time, and
/// 0 before the first change.
fn button_at(at: u64) -> u8 {
    BUTTON
        .iter()
        .rev()
        .find(|(when, _)| *when <= at)
        .map_or(0, |&(_, value)| value)
}

/// The times at which the 10 ms cycle is due, from the power-on at 0 to the end of the run.
fn due_times_ms() -> impl Iterator<Item = u64> {
    (0..END_MS).step_by(10)
}

/// The logical time of each scan that the cycle is due at, in milliseconds.
fn due_scans() -> Vec<u128> {
    due_times_ms().map(u128::from).collect()
}

/// The output image of each scan. The first scan has no task due and publishes the initial 0; every
/// later scan publishes the button's value that the task read, which is the value at the scan's time.
fn expected_outputs() -> Vec<Vec<u8>> {
    due_times_ms().map(|at| vec![button_at(at)]).collect()
}

/// What the program did over a run: the logical time of each scan, the output image each scan
/// published, and how many safe-state writes the run made.
#[derive(Debug, PartialEq, Eq)]
struct Trace {
    scans: Vec<u128>,
    outputs: Vec<Vec<u8>>,
    safe_writes: usize,
}

/// What the program did since the last call, from the fixture's records.
fn take_trace(fixture: &Fixture) -> Trace {
    let calls = fixture.take_calls();
    let steps = fixture.io.take_steps();
    Trace {
        scans: calls
            .iter()
            .filter_map(|call| match call {
                CycleCall::Scan { now, .. } => Some(now.as_millis()),
                _ => None,
            })
            .collect(),
        outputs: steps
            .iter()
            .filter_map(|step| match step {
                IoStep::WriteOutputs(image) => Some(image.clone()),
                _ => None,
            })
            .collect(),
        safe_writes: calls
            .iter()
            .filter(|call| **call == CycleCall::SafeOutputs)
            .count()
            + steps
                .iter()
                .filter(|step| matches!(step, IoStep::WriteSafeOutputs(_)))
                .count(),
    }
}

/// The trace of the same program with the same inputs over the same run, and no host requests at all.
fn reference_trace() -> Trace {
    let fixture = Fixture::board_probe();
    schedule_button(&fixture);
    let mut device = Device::new(&fixture);
    device.run_until(END_MS);
    take_trace(&fixture)
}

/// The program's side of a run in which the host went away and came back. The host asks for the status
/// first, which must show no fault and no overrun. Then the loop runs to the end of the run, and the
/// cycles, outputs, state and watchdog feeds are checked against the schedule and the run without a
/// host.
fn assert_program_kept_running(device: &mut Device<'_>, reference: &Trace) {
    let status = device.ask("status", None);
    assert_eq!(
        status["result"]["metrics"]["faults"],
        json!(0),
        "the program faulted on no scan"
    );
    assert_eq!(
        status["result"]["metrics"]["overruns"],
        json!(0),
        "no scan overran its interval"
    );
    device.run_until(END_MS);
    assert_eq!(
        device.runner.resource_state(),
        ResourceState::Running,
        "the program still runs"
    );
    let trace = take_trace(device.fixture);
    assert_eq!(
        trace.scans,
        due_scans(),
        "every cycle that was due ran, at its due time"
    );
    assert_eq!(
        trace.outputs,
        expected_outputs(),
        "each cycle published the output that `led := button` computes from the input it read"
    );
    assert_eq!(trace.safe_writes, 0, "no safe state was applied");
    assert_eq!(
        trace, *reference,
        "the cycles and the state at the end are those of the same cycles without a host"
    );
    assert_eq!(
        device.feeds.get(),
        device.passes,
        "the watchdog was fed in every pass"
    );
    assert_eq!(
        device.idle_passes.get(),
        device.passes,
        "the platform's idle work ran in every pass"
    );
    assert_eq!(
        device.link.counters().refused_replies,
        0,
        "the link refused no reply"
    );
}

/// The lowercase hexadecimal digits of `bytes`, as the protocol writes a SHA-256.
fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::new(), |mut text, byte| {
        let _ = write!(text, "{byte:02x}");
        text
    })
}

/// The `params` of `transfer.get`: `len` bytes at `offset` of the held reply `transfer`.
fn get_params(transfer: &str, offset: usize, len: usize) -> Value {
    json!({"transfer": transfer, "offset": offset, "len": len})
}

/// The `params` of `transfer.part` that carry `data` at `offset` of the upload `transfer`.
fn part_params(transfer: &str, offset: usize, data: &[u8]) -> Value {
    json!({
        "transfer": transfer,
        "offset": offset,
        "data": STANDARD.encode(data),
        "crc32": format!("{:08x}", crc32::checksum(data)),
    })
}

/// Reads the part of the held reply `transfer` at `offset`: a part of `PART_BYTES`, or the bytes left
/// of a reply of `bytes` bytes. The reply must be whole and answer the request.
fn read_part(device: &mut Device<'_>, transfer: &str, offset: usize, bytes: usize) -> Vec<u8> {
    let reply = device.ask(
        "transfer.get",
        Some(get_params(transfer, offset, PART_BYTES)),
    );
    assert_eq!(reply["ok"], json!(true), "the part at {offset}: {reply}");
    let data = STANDARD
        .decode(reply["result"]["data"].as_str().expect("the part is text"))
        .expect("the part is base64");
    let expected = PART_BYTES.min(bytes - offset);
    assert_eq!(
        data.len(),
        expected,
        "the part at {offset} holds {expected} bytes"
    );
    data
}

/// Reads the held reply `transfer` of `bytes` bytes from `from` to its end, part by part.
fn read_rest(device: &mut Device<'_>, transfer: &str, bytes: usize, from: usize) -> Vec<u8> {
    let mut read = Vec::new();
    for offset in (from..bytes).step_by(PART_BYTES) {
        read.extend(read_part(device, transfer, offset, bytes));
    }
    read
}

#[test]
fn a_host_that_goes_away_mid_run_changes_nothing_for_the_program() {
    // Specification 34, ARCH-07 and TARGET-08: a disconnected computer changes nothing, and the runtime
    // keeps running. PORT-05: a host that stops reading loses its reply a second after its last
    // progress; a new host session drops the reply being sent; a held reply of the old session stays
    // held until it is read to its end, and then it is freed. Wrong behaviour caught: a scan that waits
    // for the link, so that a cycle is late or missing; a disconnect that drives an output to a safe
    // state or faults the program; a watchdog that is not fed while a reply is in flight; a session end
    // that frees the held reply, so the returning host cannot finish it; and a returning host whose first
    // reply is glued to the stale part of the dropped reply (no repair line feed), so no whole reply
    // arrives.
    let reference = reference_trace();
    let fixture = Fixture::board_probe();
    schedule_button(&fixture);
    let mut device = Device::new(&fixture);

    // 120 writes that the scans refuse (`probe_depth` is an INT) fill the event store with
    // `write_rejected` events, about 11 KB of `events.tail`, and change nothing in the program; the
    // store keeps no event that every scan makes (specification 11, the event store).
    for _ in 0..120 {
        let queued = device.ask(
            "set",
            Some(json!({"target": "global:probe_depth", "value": "TRUE"})),
        );
        assert_eq!(queued["result"], json!({"status": "queued"}));
    }
    device.run_until(1_000);
    let head = device.ask("events.tail", Some(json!({"limit": 1_000})));
    let tail = head["id"].clone();
    assert_eq!(
        head["ok"],
        json!(true),
        "the head of the held reply: {head}"
    );
    let transfer = head["result"]["reply_transfer"]
        .as_str()
        .expect("a reply transfer id")
        .to_string();
    let digest = head["result"]["sha256"]
        .as_str()
        .expect("the reply's SHA-256")
        .to_string();
    let bytes = usize::try_from(
        head["result"]["bytes"]
            .as_u64()
            .expect("the reply's length"),
    )
    .expect("a length fits");
    assert!(
        bytes > DEVICE_FRAME_LEN,
        "{bytes} bytes is longer than the {DEVICE_FRAME_LEN}-byte frame, so the reply is held"
    );

    // The host reads the first part in full, then 1,000 bytes of the second, and stops in the middle.
    let mut whole = read_part(&mut device, &transfer, 0, bytes);
    device.ask_partly(
        "transfer.get",
        Some(get_params(&transfer, PART_BYTES, PART_BYTES)),
        1_000,
    );
    device.link.stop_reading();
    // The link drops that reply a second after its last progress (PORT-05, a host that stops reading).
    device.pass_until(|device| device.link.counters().stalled_replies == 1);

    // The host is unplugged: a new host session, which drops nothing, as the stall has dropped the
    // reply. The host stays away for 50 cycle intervals.
    let unplugged = device.now_ms();
    device.link.disconnect();
    device.run_until(unplugged + 500);
    device.link.connect();

    // The host comes back. Its first request is the part it had not read, and it is answered whole.
    whole.extend(read_rest(&mut device, &transfer, bytes, PART_BYTES));
    assert_eq!(whole.len(), bytes, "the parts make the whole reply");
    assert_eq!(
        hex(&sha256(&whole)),
        digest,
        "the reply read to its end is the one the head named"
    );
    let reply: Value = serde_json::from_slice(&whole).expect("the reply is one JSON object");
    assert_eq!(reply["id"], tail, "the reply answers `events.tail`");
    // Read to its last byte, the reply is freed (PORT-05).
    let freed = device.ask("transfer.get", Some(get_params(&transfer, 0, PART_BYTES)));
    assert_eq!(
        freed["error"],
        json!("unknown transfer"),
        "a held reply read to its end is freed"
    );

    assert_eq!(
        device.fragments.len(),
        1,
        "the stale part of the dropped reply ends as one line that is not JSON"
    );
    assert_eq!(
        device.link.counters().stalled_replies,
        1,
        "the stalled reply was dropped once"
    );
    assert_eq!(device.link.counters().sessions, 1, "one new host session");
    assert_eq!(
        device.link.counters().abandoned_replies,
        0,
        "the session dropped no reply, as the stall had dropped it"
    );

    assert_program_kept_running(&mut device, &reference);
}

#[test]
fn a_host_that_goes_away_during_an_upload_changes_nothing_for_the_program() {
    // Specification 34, ARCH-07 and TARGET-08 as in the test above; PORT-05: a new host session keeps an
    // open upload at its next offset, a `transfer.begin` while an upload is open is refused as busy, and
    // the upload completes when its last part arrives with the SHA-256 that `transfer.begin` gave.
    // Wrong behaviour caught: a session end that drops the open upload or restarts it at offset 0; a
    // returning host that is let begin a second upload over the open one; a loop that stops scanning
    // while the upload is open; and an upload whose offsets are off by one part, which the next offset
    // of each part and the complete state check. The SHA-256 refusal is not exercised here.
    let reference = reference_trace();
    let fixture = Fixture::board_probe();
    schedule_button(&fixture);
    let mut device = Device::new(&fixture);

    device.run_until(300);
    let data: Vec<u8> = (0..UPLOAD_BYTES)
        .map(|index| u8::try_from(index % 251).expect("a value below 251"))
        .collect();
    let digest = hex(&sha256(&data));
    let begin = device.ask(
        "transfer.begin",
        Some(json!({"bytes": data.len(), "sha256": digest})),
    );
    let transfer = begin["result"]["transfer"]
        .as_str()
        .expect("begin names the upload")
        .to_string();
    let first = device.ask(
        "transfer.part",
        Some(part_params(&transfer, 0, &data[..PART_BYTES])),
    );
    assert_eq!(
        first["result"],
        json!({"next_offset": PART_BYTES}),
        "the first part is stored"
    );

    // The host goes away between two parts. No reply is in flight, so the session drops nothing.
    let unplugged = device.now_ms();
    device.link.disconnect();
    device.run_until(unplugged + 500);
    device.link.connect();

    // The returning host finds the upload open at its next offset, and cannot begin another one.
    let status = device.ask("transfer.status", Some(json!({"transfer": transfer})));
    assert_eq!(
        status["result"],
        json!({"state": "receiving", "next_offset": PART_BYTES, "bytes": UPLOAD_BYTES}),
        "the upload stays open at its next offset"
    );
    let busy = device.ask(
        "transfer.begin",
        Some(json!({"bytes": data.len(), "sha256": digest})),
    );
    assert_eq!(
        busy["error"],
        json!(format!("transfer busy: {transfer}")),
        "one upload at a time"
    );

    // It resumes the upload at its next offset, and the last part completes it.
    for offset in (PART_BYTES..UPLOAD_BYTES).step_by(PART_BYTES) {
        let part = device.ask(
            "transfer.part",
            Some(part_params(
                &transfer,
                offset,
                &data[offset..offset + PART_BYTES],
            )),
        );
        assert_eq!(
            part["result"],
            json!({"next_offset": offset + PART_BYTES}),
            "the part at {offset} is stored"
        );
    }
    let complete = device.ask("transfer.status", Some(json!({"transfer": transfer})));
    assert_eq!(
        complete["result"],
        json!({"state": "complete", "next_offset": UPLOAD_BYTES, "bytes": UPLOAD_BYTES}),
        "the upload is complete with the SHA-256 that begin gave"
    );

    assert!(device.fragments.is_empty(), "no reply was dropped");
    assert_eq!(device.link.counters().sessions, 1, "one new host session");
    assert_program_kept_running(&mut device, &reference);
}
