//! Host proof that the shared M2E runner reproduces the A4 oracle through the fixture
//! board interfaces, before any board composition is involved.
use std::cell::Cell;
use trust_board_fixture::{
    application::decode,
    board::{Board, Clock, DigitalIo, InputSnapshot, Measurement, Memory, Watchdog},
    runner::run,
};
use trust_runtime_core::{crc32::checksum, error::RuntimeError};

const MAIN: &[u8] =
    include_bytes!("../../trust-runtime/tests/fixtures/portability/stbc-2.0/program-v2.stbc");
const NUMERIC: &[u8] =
    include_bytes!("../../trust-runtime/tests/fixtures/portability/stbc-2.0/numeric-v2.stbc");
const GPIO: &[u8] = include_bytes!("../../trust-runtime/tests/fixtures/portability/f401/gpio.stbc");
const ORACLE: &str =
    include_str!("../../trust-runtime/tests/fixtures/portability/stbc-2.0/expected-a4-trace.csv");

/// Scans that end with a watchdog feed: 101 main, 101 numeric, and 4 + 4 + 2 gpio.
const FED_SCANS: u32 = 212;

/// TRSTB002 bundle: magic, three lengths, three CRC-32 values, then the payloads.
fn bundle() -> Vec<u8> {
    let parts = [MAIN, NUMERIC, GPIO];
    let mut bytes = b"TRSTB002".to_vec();
    for part in parts {
        bytes.extend_from_slice(&u32::try_from(part.len()).unwrap().to_le_bytes());
    }
    for part in parts {
        bytes.extend_from_slice(&checksum(part).to_le_bytes());
    }
    for part in parts {
        bytes.extend_from_slice(part);
    }
    bytes
}

/// Advances only when the runner waits, so every sample lands on its scheduled instant.
struct FakeClock {
    now: Cell<u64>,
}

impl Clock for FakeClock {
    fn micros(&self) -> u64 {
        self.now.get()
    }
    fn wait_until_us(&self, deadline_us: u64) {
        self.now.set(self.now.get().max(deadline_us));
    }
    fn cycle_count(&self) -> u32 {
        0
    }
    fn irq_count(&self) -> u32 {
        0
    }
}

/// Fixed physical level, and a readback of the last published output bit.
struct FakeIo {
    pin_high: bool,
    output: bool,
}

impl DigitalIo for FakeIo {
    fn sample(&self) -> InputSnapshot {
        InputSnapshot {
            pin_high: self.pin_high,
            pressed: !self.pin_high,
        }
    }
    fn set_output(&mut self, on: bool) {
        self.output = on;
    }
    fn safe_off(&mut self) {
        self.output = false;
    }
    fn output_readback(&self) -> bool {
        self.output
    }
}

#[derive(Default)]
struct CountingWatchdog {
    feeds: u32,
}

impl Watchdog for CountingWatchdog {
    fn feed(&mut self) {
        self.feeds += 1;
    }
}

/// Fixed in-budget measurements: the stack pair sums to the window and keeps the margin.
struct FixedMemory;

impl Memory for FixedMemory {
    fn begin_phase(&self) {}
    fn measure(&self) -> Measurement {
        Measurement {
            used: 1024,
            peak: 2048,
            allocations: 8,
            failures: 0,
            stack_used: 4096,
            stack_remaining: 12_288,
        }
    }
    fn repaint_inactive_stack(&self) {}
    fn boot_stack_peak(&self) -> usize {
        4096
    }
}

#[test]
fn shared_runner_reproduces_the_a4_oracle_on_the_fixture_board() {
    let bundle = bundle();
    let clock = FakeClock { now: Cell::new(0) };
    let mut io = FakeIo {
        pin_high: true,
        output: false,
    };
    let mut console = String::new();
    let mut watchdog = CountingWatchdog::default();
    let memory = FixedMemory;
    let mut board = Board {
        clock: &clock,
        io: &mut io,
        console: &mut console,
        watchdog: &mut watchdog,
        memory: &memory,
    };
    let result = run(&mut board, || decode(&bundle), |_| Ok(()));
    assert_eq!(result, Ok(()));

    let lines: Vec<&str> = console.lines().collect();
    assert!(!lines.iter().any(|line| line.starts_with("B1,FAIL")));
    assert!(lines.contains(&"B1,STACK_PEAK,4096"));

    // Oracle columns: time_ms, counter, result, activations, ton_q, ton_et_ms,
    // nominal_deadline_ms. They are the first seven TRACE fields, in the same order.
    let oracle: Vec<Vec<&str>> = ORACLE
        .lines()
        .filter(|line| !line.starts_with('#'))
        .skip(1)
        .map(|line| line.split(',').collect())
        .collect();
    assert_eq!(oracle.len(), 101);
    let traces: Vec<Vec<&str>> = lines
        .iter()
        .filter_map(|line| line.strip_prefix("B1,TRACE,"))
        .map(|fields| fields.split(',').take(7).collect())
        .collect();
    assert_eq!(traces, oracle);

    let stop = lines
        .iter()
        .position(|line| *line == "B1,STATE,STOP")
        .expect("STOP record");
    let fault = lines
        .iter()
        .position(|line| line.starts_with("B1,STATE,FAULT,"))
        .expect("FAULT record");
    let code = RuntimeError::ExecutionTimeout.stable_code().to_string();
    assert_eq!(lines[fault], format!("B1,STATE,FAULT,{code}"));
    assert!(stop < fault, "STOP must precede FAULT");

    let count = |prefix: &str| lines.iter().filter(|line| line.starts_with(prefix)).count();
    assert_eq!(count("B1,NUM,"), 13);
    assert_eq!(count("B1,GPIO,"), 4);
    assert_eq!(count("B1,STACK,"), 4);
    assert!(
        watchdog.feeds >= FED_SCANS,
        "watchdog fed {} times, expected at least {FED_SCANS}",
        watchdog.feeds
    );
}
