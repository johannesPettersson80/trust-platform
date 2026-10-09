//! Specification 39, §18 Phase 5: the HMI runs where the program calls `Hmi.Display`. After the
//! task in which the program calls it, the runtime copies the values the screens show, applies
//! their queued commands and reads the alarms; nothing of the HMI happens between scans or in
//! another task.

use std::sync::Arc;

use serde_json::json;
use trust_runtime::harness::TestHarness;
use trust_runtime::hmi::alarms::EventKind;
use trust_runtime::hmi::commands::write::WriteAction;
use trust_runtime::hmi::exchange::HmiExchange;
use trust_runtime::hmi::member_paths::{resolve_member, MemberPath};
use trust_runtime::value::{Duration, Value};

const ALARM: &str = include_str!("../../../libraries/alarm/src/alarm.st");
const HMI: &str = include_str!("../../../libraries/hmi/src/hmi.st");

/// A fast control task counting, and a slow task whose program calls `Hmi.Display`.
const PLANT: &str = r#"
USING Alarm;

PROGRAM Control
VAR
    Counter : DINT;
    Setpoint : DINT;
    High : DigitalAlarm;
END_VAR
    Counter := Counter + 1;
    High(Condition := Counter >= 12, Message := 'Counter high', Priority := AlarmPriority#High);
END_PROGRAM

PROGRAM Screens
VAR
    Hmi : Hmi.Display;
END_VAR
    Hmi();
END_PROGRAM

CONFIGURATION Plant
RESOURCE Cpu ON PLC
    TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);
    TASK Slow (INTERVAL := T#100ms, PRIORITY := 20);
    PROGRAM Main WITH Fast : Control;
    PROGRAM ScreensRun WITH Slow : Screens;
END_RESOURCE
END_CONFIGURATION
"#;

/// The same plant without any `Hmi.Display` call.
const UNSERVED: &str = r#"
PROGRAM Control
VAR
    Counter : DINT;
    Setpoint : DINT;
END_VAR
    Counter := Counter + 1;
END_PROGRAM

CONFIGURATION Plant
RESOURCE Cpu ON PLC
    TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM Main WITH Fast : Control;
END_RESOURCE
END_CONFIGURATION
"#;

struct Plant {
    harness: TestHarness,
    exchange: Arc<HmiExchange>,
}

impl Plant {
    fn new(program: &str) -> Self {
        let mut harness =
            TestHarness::from_sources(&[ALARM, HMI, program]).expect("the plant compiles");
        let exchange = Arc::new(HmiExchange::new());
        harness
            .runtime_mut()
            .set_hmi_exchange(Arc::clone(&exchange));
        Self { harness, exchange }
    }

    /// One cycle at the present time.
    fn cycle(&mut self) {
        let result = self.harness.cycle();
        assert!(
            result.errors.is_empty(),
            "the cycle runs: {:?}",
            result.errors
        );
    }

    /// Ten milliseconds later, one cycle.
    fn step(&mut self) {
        self.harness.advance_time(Duration::from_millis(10));
        self.cycle();
    }

    fn variable(&self, path: &str) -> Value {
        let path = MemberPath::parse(path).expect("a path");
        resolve_member(&path, self.harness.runtime().storage())
            .cloned()
            .unwrap_or_else(|| panic!("{path:?} exists"))
    }

    /// The value the screens last got for `path`, as JSON.
    fn shown(&self, path: &str) -> Option<serde_json::Value> {
        let path = MemberPath::parse(path).expect("a path");
        let read = self.exchange.read(&[path])?;
        let value = read.values.into_iter().next().flatten()?;
        Some(serde_json::to_value(value).expect("a wire value")["value"].clone())
    }
}

fn path(text: &str) -> MemberPath {
    MemberPath::parse(text).expect("a path")
}

#[test]
fn the_screens_get_their_values_only_after_the_task_that_calls_hmi_display() {
    let mut plant = Plant::new(PLANT);
    let _screen = plant.exchange.open_screen(vec![path("Main.Counter")]);
    // A periodic task first runs one interval after the start: the fast one at 10 ms, the slow
    // one, calling Hmi.Display, at 100 ms.
    for _ in 0..9 {
        plant.step();
    }
    assert_eq!(plant.variable("Main.Counter"), Value::DInt(9));
    assert_eq!(
        plant.shown("Main.Counter"),
        None,
        "the fast task alone copies nothing"
    );
    plant.step();
    assert_eq!(
        plant.shown("Main.Counter"),
        Some(json!(10)),
        "copied after the slow task"
    );
    for _ in 0..9 {
        plant.step();
    }
    assert_eq!(
        plant.shown("Main.Counter"),
        Some(json!(10)),
        "unchanged until the slow task runs again"
    );
    plant.step();
    assert_eq!(
        plant.shown("Main.Counter"),
        Some(json!(20)),
        "copied after its next run"
    );
}

#[test]
fn a_command_is_applied_only_when_the_program_calls_hmi_display() {
    let mut plant = Plant::new(PLANT);
    let outcome = plant
        .exchange
        .queue_write(path("Main.Setpoint"), WriteAction::Set(json!(42)));
    for _ in 0..9 {
        plant.step();
        assert_eq!(
            plant.variable("Main.Setpoint"),
            Value::DInt(0),
            "not applied between the calls"
        );
    }
    assert!(outcome.try_recv().is_err(), "no outcome before the call");
    plant.step();
    assert_eq!(
        plant.variable("Main.Setpoint"),
        Value::DInt(42),
        "applied at the call"
    );
    let written = outcome.try_recv().expect("the outcome").expect("applied");
    assert_eq!(written.readback, Value::DInt(42));
}

#[test]
fn the_alarms_are_read_only_after_the_task_that_calls_hmi_display() {
    let mut plant = Plant::new(PLANT);
    let raised = |plant: &Plant| {
        plant
            .exchange
            .alarm_snapshot(None, 0)
            .events
            .iter()
            .any(|event| event.kind == EventKind::Raised && event.path.ends_with("High"))
    };
    // The first read, at 100 ms, finds the alarm clear; its condition holds from 120 ms.
    for _ in 0..12 {
        plant.step();
    }
    assert_eq!(
        plant.variable("Main.Counter"),
        Value::DInt(12),
        "the condition holds from here"
    );
    for _ in 0..7 {
        assert!(!raised(&plant), "not read before the slow task runs again");
        plant.step();
    }
    assert!(!raised(&plant), "not read before the slow task runs again");
    plant.step();
    assert!(raised(&plant), "read after the slow task");
}

#[test]
fn without_a_call_the_screens_get_nothing_and_commands_wait() {
    let mut plant = Plant::new(UNSERVED);
    let _screen = plant.exchange.open_screen(vec![path("Main.Counter")]);
    let outcome = plant
        .exchange
        .queue_write(path("Main.Setpoint"), WriteAction::Set(json!(7)));
    plant.cycle();
    for _ in 0..20 {
        plant.step();
    }
    assert_eq!(plant.shown("Main.Counter"), None, "nothing copied");
    assert_eq!(
        plant.variable("Main.Setpoint"),
        Value::DInt(0),
        "nothing applied"
    );
    assert!(outcome.try_recv().is_err(), "the command still waits");
    assert_eq!(plant.exchange.calls(), 0);
}

/// A one-off read (a faceplate's instance type, a command's readback, Live mode) waits for the
/// next call instead of asking the PLC thread between scans.
#[test]
fn a_one_off_read_waits_for_the_next_call() {
    let mut plant = Plant::new(PLANT);
    let exchange = Arc::clone(&plant.exchange);
    let reader = std::thread::spawn(move || {
        exchange.read_next(&[path("Main.Counter")], std::time::Duration::from_secs(5))
    });
    for _ in 0..10 {
        std::thread::sleep(std::time::Duration::from_millis(5));
        plant.step();
    }
    let read = reader
        .join()
        .expect("the reader")
        .expect("answered by the call");
    let value = read.values.into_iter().next().flatten().expect("a value");
    assert_eq!(
        serde_json::to_value(value).expect("wire")["value"],
        json!(10)
    );
}

/// A command the program has not taken within the timeout is withdrawn and never applied later,
/// so a "timed out" is never followed by a surprise write.
#[test]
fn a_command_not_taken_in_time_is_withdrawn_and_never_applied() {
    let mut plant = Plant::new(PLANT);
    let exchange = Arc::clone(&plant.exchange);
    let writer = std::thread::spawn(move || {
        exchange.write(
            path("Main.Setpoint"),
            WriteAction::Set(json!(5)),
            std::time::Duration::from_millis(50),
        )
    });
    assert!(writer.join().expect("the writer").is_none(), "timed out");
    for _ in 0..10 {
        plant.step();
    }
    assert_eq!(
        plant.variable("Main.Setpoint"),
        Value::DInt(0),
        "withdrawn, not applied later"
    );
}

/// The time between the program's calls is measured, so the display knows how often to expect
/// values.
#[test]
fn the_time_between_calls_is_measured() {
    let mut plant = Plant::new(PLANT);
    for _ in 0..10 {
        plant.step();
    }
    assert_eq!(plant.exchange.calls(), 1);
    assert_eq!(plant.exchange.period(), None, "one call: no period yet");
    for _ in 0..10 {
        plant.step();
    }
    assert_eq!(plant.exchange.calls(), 2);
    assert!(plant.exchange.period().is_some(), "two calls: a period");
}

/// A screen that has just opened gets no values until a call has copied its paths, so it never
/// shows empty values for a period (specification 39, §11.1).
#[test]
fn a_screen_just_opened_gets_values_only_once_a_call_copied_its_paths() {
    let mut plant = Plant::new(PLANT);
    let _first = plant.exchange.open_screen(vec![path("Main.Counter")]);
    for _ in 0..10 {
        plant.step();
    }
    assert_eq!(
        plant.shown("Main.Counter"),
        Some(json!(10)),
        "the first screen is served"
    );
    let _second = plant.exchange.open_screen(vec![path("Main.Setpoint")]);
    assert!(
        plant.exchange.read(&[path("Main.Setpoint")]).is_none(),
        "nothing before a call copied its path"
    );
    for _ in 0..10 {
        plant.step();
    }
    assert_eq!(
        plant.shown("Main.Setpoint"),
        Some(json!(0)),
        "copied at the next call"
    );
}

/// A screen's stream waits for the program's next call, so it sends the values as soon as they are
/// copied rather than up to a period later; without a call it gives up after the timeout.
#[test]
fn a_stream_wakes_at_the_next_call() {
    let mut plant = Plant::new(PLANT);
    let exchange = Arc::clone(&plant.exchange);
    let waiter = std::thread::spawn(move || {
        let started = std::time::Instant::now();
        let calls = exchange.wait_for_call(0, std::time::Duration::from_secs(5));
        (calls, started.elapsed())
    });
    std::thread::sleep(std::time::Duration::from_millis(20));
    for _ in 0..10 {
        plant.step();
    }
    let (calls, waited) = waiter.join().expect("the waiter");
    assert_eq!(calls, 1, "woken by the first call");
    assert!(
        waited < std::time::Duration::from_secs(4),
        "not by the timeout: {waited:?}"
    );
    let started = std::time::Instant::now();
    assert_eq!(
        plant
            .exchange
            .wait_for_call(1, std::time::Duration::from_millis(30)),
        1
    );
    assert!(
        started.elapsed() >= std::time::Duration::from_millis(30),
        "no call: the timeout"
    );
}
