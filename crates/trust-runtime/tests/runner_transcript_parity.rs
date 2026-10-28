#![cfg(feature = "full")]
//! Spec 34 STATE-08 and ARCH-11: one request sequence sent through trust-runtime's threaded host
//! composition and through the single-thread resource loop of a device without an operating
//! system, both running the board probe compiled for this computer (specification 44). Every reply
//! up to and after `shutdown` must be the same JSON after normalising only the values in
//! [`NORMALISED`]; after the stop, pause, resume and each restart the two also report the same
//! `status`, `events.tail` and `tasks.stats`. The device then shows what STATE-08 adds: it stays reachable in Stopped, and
//! `restart cold` starts the program again as a new process, where trust-runtime's process ends.
//!
//! How the two stay comparable: the host scans in real time on its own thread. The test moves the
//! host's events into its event store and publishes its metrics only at scan boundaries, and runs
//! the device's loop on a clock that it sets to the host's scan times, so both have run the same
//! scans at the same logical times when they answer. A request that changes the resource state
//! reaches the device between two of its scans, after every scan the host ran before its scan
//! thread took the request, and the device takes it at its own next boundary; so a device that
//! took a request at the wrong boundary, numbered its cycles wrongly after a restart or timed its
//! task wrongly after a resume or restart would answer differently.
//!
//! What it cannot compare:
//! - The host's paused passes leave no trace, so the device's paused boundaries are run at their
//!   nominal times, and the resume reaches the device just before the boundary at which the host
//!   resumed.
//! - On a restart the host starts its task timing, and its cycle grid with it, at a clock reading
//!   taken inside the restart, a few microseconds before its scan; the device starts both at that
//!   scan's time. They report the same activations unless a later host scan comes within those
//!   microseconds of its boundary.
//! - Faults, the cycle watchdog, stop outputs and retain saves: the host composition has none.
//!   The runner's own contract tests cover them.
//!
//! The tests need the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`.

mod runner_transcript_support;

use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration as StdDuration;

use serde_json::{json, Value};
use trust_runtime::scheduler::ResourceState;
use trust_runtime_services::events::EVENT_STORE_CAPACITY;
use trust_runtime_services::runner::Runner;

use runner_transcript_support::{
    board_probe_image, start_device_cycle, BoardPlatform, DeviceRunner, HostComposition, HostScan,
    MemoryLink, ReplayClock, ReplayWait, CYCLE_INTERVAL,
};

/// The reply values that differ between the two compositions by construction, as JSON pointers
/// (`*` stands for every element of an array). Every other value of every reply must be equal.
const NORMALISED: &[(&str, &str)] = &[
    // The host's id is its wall clock, a process-wide sequence and a random nonce; the device's
    // comes from its platform's audit sink.
    ("/audit_id", "audit record ids"),
    // The platform each composition runs on: the test stands the device in for a chip, and the
    // host reports the computer it runs on. `version` is the same on both, so it stays compared.
    ("/result/platform", "platform"),
    // The host's stable runtime ID comes from its project; the device reports its platform's value
    // (specification 11, `status`).
    ("/result/runtime_id", "runtime ID"),
    // Process uptime on the host, the time since the program started on the device.
    ("/result/uptime_ms", "uptime"),
    // Scan durations: measured in real time on the host, on the test's clock on the device.
    ("/result/metrics/cycle_ms/min", "scan duration"),
    ("/result/metrics/cycle_ms/avg", "scan duration"),
    ("/result/metrics/cycle_ms/max", "scan duration"),
    ("/result/metrics/cycle_ms/last", "scan duration"),
    ("/result/metrics/cycle_ms/p50", "scan duration"),
    ("/result/metrics/cycle_ms/p95", "scan duration"),
    ("/result/metrics/cycle_ms/p99", "scan duration"),
    // Program call durations and their share of the scan duration, in `status`...
    (
        "/result/metrics/profiling/top/*/avg_cycle_ms",
        "call duration",
    ),
    (
        "/result/metrics/profiling/top/*/cycle_pct",
        "share of the scan duration",
    ),
    ("/result/metrics/profiling/top/*/last_ms", "call duration"),
    (
        "/result/metrics/profiling/top/*/last_cycle_pct",
        "share of the scan duration",
    ),
    // ...and in `tasks.stats`.
    ("/result/top_contributors/*/avg_cycle_ms", "call duration"),
    (
        "/result/top_contributors/*/cycle_pct",
        "share of the scan duration",
    ),
    ("/result/top_contributors/*/last_ms", "call duration"),
    (
        "/result/top_contributors/*/last_cycle_pct",
        "share of the scan duration",
    ),
    // Task durations, in `tasks.stats`.
    ("/result/tasks/*/min_ms", "task duration"),
    ("/result/tasks/*/avg_ms", "task duration"),
    ("/result/tasks/*/max_ms", "task duration"),
    ("/result/tasks/*/last_ms", "task duration"),
];

fn normalise(mut reply: Value) -> Value {
    for (pointer, why) in NORMALISED {
        let segments: Vec<&str> = pointer.split('/').skip(1).collect();
        replace(&mut reply, &segments, why);
    }
    reply
}

fn replace(value: &mut Value, segments: &[&str], why: &str) {
    let Some((first, rest)) = segments.split_first() else {
        *value = json!(format!("<normalised: {why}>"));
        return;
    };
    if *first == "*" {
        if let Some(items) = value.as_array_mut() {
            for item in items {
                replace(item, rest, why);
            }
        }
        return;
    }
    if let Some(next) = value.get_mut(*first) {
        replace(next, rest, why);
    }
}

fn request(id: u64, kind: &str) -> String {
    json!({"id": id, "type": kind}).to_string()
}

fn restart(id: u64, mode: &str) -> String {
    json!({"id": id, "type": "restart", "params": {"mode": mode}}).to_string()
}

fn state_of(reply: &Value) -> &str {
    reply["result"]["state"]
        .as_str()
        .expect("status reports a state")
}

fn assert_same_reply(request: &str, host: &Value, device: &Value) {
    assert_eq!(
        host["ok"],
        json!(true),
        "the host refused {request}: {host}"
    );
    assert_eq!(
        normalise(host.clone()),
        normalise(device.clone()),
        "the two compositions answer {request} differently"
    );
}

/// The device composition: its loop, and the link and clock the test drives.
struct Device {
    runner: DeviceRunner,
    link: MemoryLink,
    clock: ReplayClock,
    scans: Rc<Cell<usize>>,
    /// The clock's reading at the device's last pass.
    now: StdDuration,
    /// When the device's last replayed scan started.
    last_scan: StdDuration,
    /// Where the device's cycle grid begins: its start, and again each restart (specification 10
    /// §4.3).
    grid_origin: StdDuration,
}

impl Device {
    /// Runs one pass at `at` and checks that it ran `scans` scans.
    fn pass_at(&mut self, at: StdDuration, scans: usize) {
        let before = self.scans.get();
        self.clock.set(at);
        self.now = at;
        self.runner
            .run_pass()
            .expect("the replay wait does not fail");
        assert_eq!(
            self.scans.get(),
            before + scans,
            "scans of the device's pass at {at:?}"
        );
    }

    /// Runs each scan the host ran, at the host's logical time; the loop must find each due.
    fn replay(&mut self, scans: &[HostScan]) {
        for scan in scans {
            self.pass_at(scan.at, 1);
            self.last_scan = scan.at;
        }
    }

    /// The device's next boundary after its last replayed scan: one cycle interval after the last
    /// boundary of its grid at or before that scan, where a scan that started late still has its
    /// nominal start (specification 10 §4.3).
    fn next_boundary(&self) -> StdDuration {
        let since = self.last_scan.saturating_sub(self.grid_origin);
        let passed = since.as_nanos() / CYCLE_INTERVAL.as_nanos();
        let passed = u32::try_from(passed).expect("a transcript of a few seconds");
        self.grid_origin + CYCLE_INTERVAL * (passed + 1)
    }

    /// Sends `line` a microsecond after the last pass, between two scans, and returns the reply;
    /// the pass that serves it runs no scan.
    fn request(&mut self, line: &str) -> Value {
        self.exchange(line, 0)
    }

    /// Sends `line` a microsecond after the last pass and returns the reply; the pass that serves
    /// it runs `scans` scans.
    fn exchange(&mut self, line: &str, scans: usize) -> Value {
        self.link.send(line);
        self.pass_at(self.now + StdDuration::from_micros(1), scans);
        let mut replies = self.link.take_replies();
        assert_eq!(replies.len(), 1, "one reply to {line}: {replies:?}");
        replies.remove(0)
    }

    /// The resource state the device reports now.
    fn state(&mut self, id: u64) -> String {
        state_of(&self.request(&request(id, "status"))).to_owned()
    }
}

/// Sends each read to both compositions and requires the same reply.
fn compare_reads(host: &HostComposition, device: &mut Device, reads: &[(u64, &str)]) {
    for (id, kind) in reads {
        let line = request(*id, kind);
        assert_same_reply(&line, &host.request(&line), &device.request(&line));
    }
}

/// A Live Values request line with its parameters.
fn live(id: u64, kind: &str, params: Value) -> String {
    json!({"id": id, "type": kind, "params": params}).to_string()
}

/// Sends `line` to both compositions, replays the host's scans on the device and requires the same
/// reply; then requires the same `reads`. A one-shot write is overwritten by the program, a force
/// holds across scans, so a few scans make the effect settle in both before the reads.
fn live_request(
    host: &mut HostComposition,
    device: &mut Device,
    line: &str,
    reads: &[(u64, &str)],
) {
    let host_reply = host.request(line);
    let device_reply = device.request(line);
    assert_same_reply(line, &host_reply, &device_reply);
    device.replay(&host.settle_until(|scans| scans.len() >= 3));
    compare_reads(host, device, reads);
}

/// Spec 11, "Live Values": both compositions queue, apply and report `io.write`, `io.force`,
/// `io.unforce`, `set`, `var.force`, `var.unforce` and `var.forced` the same, and answer `io.list`
/// and `io.read` from their own published image. The device would answer differently if it applied
/// a request at the wrong phase, dropped a force, kept a released one or published the wrong
/// image.
fn live_values(host: &mut HostComposition, device: &mut Device) {
    compare_reads(host, device, &[(40, "io.list"), (41, "io.read")]);

    live_request(
        host,
        device,
        &live(
            42,
            "io.force",
            json!({"address": "%QX0.0", "value": "TRUE"}),
        ),
        &[(43, "io.list"), (44, "io.read")],
    );
    live_request(
        host,
        device,
        &live(45, "io.unforce", json!({"address": "%QX0.0"})),
        &[(46, "io.list"), (47, "io.read")],
    );
    live_request(
        host,
        device,
        &live(
            48,
            "io.write",
            json!({"address": "%QX0.0", "value": "TRUE"}),
        ),
        &[(49, "io.list")],
    );
    live_request(
        host,
        device,
        &live(
            50,
            "set",
            json!({"target": "global:probe_depth", "value": "7"}),
        ),
        &[(51, "status")],
    );
    live_request(
        host,
        device,
        &live(
            52,
            "var.force",
            json!({"target": "global:probe_depth", "value": "9"}),
        ),
        &[(53, "var.forced")],
    );
    live_request(
        host,
        device,
        &live(54, "var.unforce", json!({"target": "global:probe_depth"})),
        &[(55, "var.forced")],
    );
}

/// `pause`: the host's scan thread takes it at its next pass, which runs no scan; the device takes
/// it at its next boundary on its grid. Returns that boundary.
fn pause(host: &mut HostComposition, device: &mut Device) -> StdDuration {
    let line = request(6, "pause");
    let host_reply = host.request(&line);
    host.wait_for_state(ResourceState::Paused);
    device.replay(&host.settle());
    assert_same_reply(&line, &host_reply, &device.request(&line));
    assert_eq!(
        device.state(100),
        "running",
        "the device takes the pause at its next boundary, not when it answers"
    );
    let boundary = device.next_boundary();
    device.pass_at(boundary, 0);
    compare_reads(
        host,
        device,
        &[(7, "status"), (8, "events.tail"), (9, "tasks.stats")],
    );
    boundary
}

/// `resume`: the host takes it at one of its paused passes and scans at once there; the device
/// gets it just before the paused boundary at or before that scan, and scans at its time.
fn resume(host: &mut HostComposition, device: &mut Device, paused_at: StdDuration) {
    let line = request(10, "resume");
    let host_reply = host.request(&line);
    host.wait_for_state(ResourceState::Running);
    let after = host.settle_until(|scans| !scans.is_empty());
    let resumed_at = after[0].at;
    let intervals = (resumed_at - paused_at).as_nanos() / CYCLE_INTERVAL.as_nanos();
    let intervals = u32::try_from(intervals).expect("a short pause");
    assert!(intervals >= 1, "the host resumed at a later paused pass");
    for interval in 1..intervals {
        device.pass_at(paused_at + CYCLE_INTERVAL * interval, 0);
    }
    assert_same_reply(&line, &host_reply, &device.request(&line));
    assert_eq!(
        device.state(101),
        "paused",
        "the resume waits for the boundary"
    );
    device.replay(&after);
    compare_reads(
        host,
        device,
        &[(11, "status"), (12, "events.tail"), (13, "tasks.stats")],
    );
}

/// `restart`: the host's scan thread takes it at its next pass, whose scan is the first without
/// a due task, since the restart starts the task timing again; the device gets it after the last
/// scan before that pass and takes it at its next boundary, at that scan.
fn restart_in_place(host: &mut HostComposition, device: &mut Device, id: u64, mode: &str) {
    let line = restart(id, mode);
    let host_reply = host.request(&line);
    host.wait_for_restart_taken();
    let scans = host.settle_until(|scans| scans.iter().any(|scan| !scan.task));
    let restarted = scans
        .iter()
        .position(|scan| !scan.task)
        .expect("the restart pass scanned");
    device.replay(&scans[..restarted]);
    assert_same_reply(&line, &host_reply, &device.request(&line));
    // The restart begins the device's grid again at the scan that takes it.
    device.grid_origin = scans[restarted].at;
    device.replay(&scans[restarted..]);
    compare_reads(
        host,
        device,
        &[
            (id + 1, "status"),
            (id + 2, "events.tail"),
            (id + 3, "tasks.stats"),
        ],
    );
}

/// `shutdown`: the host's scan thread takes the stop at its next pass, and the device at its next
/// boundary on its grid; both then report Stopped and keep serving (spec 34 STATE-08).
fn shutdown(host: &mut HostComposition, device: &mut Device) {
    let line = request(22, "shutdown");
    let host_reply = host.request(&line);
    host.wait_for_state(ResourceState::Stopped);
    device.replay(&host.settle());
    assert_same_reply(&line, &host_reply, &device.request(&line));
    assert_eq!(
        device.state(104),
        "running",
        "the stop waits for the boundary"
    );
    let boundary = device.next_boundary();
    device.pass_at(boundary, 0);
    compare_reads(
        host,
        device,
        &[
            (23, "status"),
            (24, "health"),
            (25, "events.tail"),
            (26, "tasks.stats"),
            (27, "faults"),
        ],
    );
}

/// STATE-08: the stopped device stays reachable, and `restart cold` starts the program as a new
/// trust-runtime process would, with empty records. The host's restart from Stopped is proved
/// against the real process in `computer_stopped_keeps_runtime_up.rs`.
fn start_again(device: &mut Device) {
    assert_eq!(device.state(28), "stopped");
    let restarted = device.exchange(&restart(29, "cold"), 1);
    assert_eq!(
        restarted["result"],
        json!({"status": "restart queued"}),
        "the stopped device starts the program at once, and the pass scans it"
    );
    let status = device.request(&request(30, "status"));
    assert_eq!(state_of(&status), "running");
    assert_eq!(status["result"]["fault"], Value::Null);
    assert_eq!(
        status["result"]["metrics"]["cycle_ms"]["window_samples"],
        json!(1)
    );
    assert_eq!(status["result"]["metrics"]["overruns"], json!(0));
    assert_eq!(status["result"]["uptime_ms"], json!(0));
    let events = device.request(&request(31, "events.tail"));
    assert_eq!(
        events["result"]["events"],
        json!([]),
        "the new program's records start empty: the overruns of the program before are gone, and \
         its first scan, at the start of its logical time, records no event"
    );
    let tasks = device.request(&request(32, "tasks.stats"));
    assert_eq!(tasks["result"]["tasks"], json!([]));
}

#[test]
fn the_single_thread_loop_answers_the_transcript_exactly_as_the_threaded_host() {
    let mut host = HostComposition::start(board_probe_image());
    let warmup = host.settle_until(|scans| scans.len() >= 20);

    let clock = ReplayClock::default();
    let scans = Rc::new(Cell::new(0));
    let (cycle, live) = start_device_cycle(
        board_probe_image(),
        clock.clone(),
        Rc::clone(&scans),
        host.watchdog,
        host.fault_policy,
    );
    let link = MemoryLink::default();
    let runner = Runner::new(
        link.clone(),
        ReplayWait::default(),
        cycle,
        BoardPlatform::configured_as(&host, live),
        clock.clone(),
        EVENT_STORE_CAPACITY,
    );
    let mut device = Device {
        runner,
        link,
        clock: clock.clone(),
        scans,
        now: StdDuration::ZERO,
        last_scan: StdDuration::ZERO,
        grid_origin: StdDuration::ZERO,
    };
    device.replay(&warmup);

    // Reads while the program runs, all at the same scan boundary.
    device.replay(&host.settle());
    compare_reads(
        &host,
        &mut device,
        &[
            (1, "status"),
            (2, "health"),
            (3, "tasks.stats"),
            (4, "events.tail"),
            (5, "faults"),
        ],
    );

    live_values(&mut host, &mut device);

    let paused_at = pause(&mut host, &mut device);
    resume(&mut host, &mut device, paused_at);
    restart_in_place(&mut host, &mut device, 14, "warm");
    restart_in_place(&mut host, &mut device, 18, "cold");
    shutdown(&mut host, &mut device);
    host.join();
    start_again(&mut device);
}

/// Specification 11 `status` and O4 revised: the host and the device differ in `platform` by
/// design, and report the same `program_control`: since decision Q11 on computers (18 October
/// 2026) the host's `shutdown` stops the program and keeps trust-runtime running, as the device's
/// does (spec 34 STATE-08). Both name the platform they run on.
#[test]
fn the_runner_and_the_host_report_their_own_platform_and_the_same_program_control() {
    let mut host = HostComposition::start(board_probe_image());
    let warmup = host.settle_until(|scans| !scans.is_empty());

    let clock = ReplayClock::default();
    let scans = Rc::new(Cell::new(0));
    let (cycle, live) = start_device_cycle(
        board_probe_image(),
        clock.clone(),
        Rc::clone(&scans),
        host.watchdog,
        host.fault_policy,
    );
    let link = MemoryLink::default();
    let runner = Runner::new(
        link.clone(),
        ReplayWait::default(),
        cycle,
        BoardPlatform::configured_as(&host, live),
        clock.clone(),
        EVENT_STORE_CAPACITY,
    );
    let mut device = Device {
        runner,
        link,
        clock: clock.clone(),
        scans,
        now: StdDuration::ZERO,
        last_scan: StdDuration::ZERO,
        grid_origin: StdDuration::ZERO,
    };
    device.replay(&warmup);

    let host_status = host.request(&request(1, "status"));
    let device_status = device.request(&request(2, "status"));
    for (composition, reply) in [("host", &host_status), ("runner", &device_status)] {
        assert_eq!(
            reply["result"]["program_control"],
            json!("keeps_control"),
            "the {composition} keeps control after `shutdown`"
        );
    }
    for (composition, reply) in [("host", &host_status), ("runner", &device_status)] {
        assert!(
            reply["result"]["platform"]
                .as_str()
                .is_some_and(|platform| !platform.is_empty()),
            "the {composition} names the platform it runs on: {reply}"
        );
    }
    host.join();
}
