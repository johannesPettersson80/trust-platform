//! Specification 42: the runtime fills every `Comm.Link` of the program at the start of each scan
//! from the link its `Name` names.

use std::sync::{Arc, Mutex};

use trust_runtime::error::RuntimeError;
use trust_runtime::harness::TestHarness;
use trust_runtime::hmi::member_paths::{member_value, MemberPath};
use trust_runtime::io::{IoDriver, IoDriverHealth};
use trust_runtime::value::{Duration, Value};

const ALARM: &str = include_str!("../../../libraries/alarm/src/alarm.st");
const COMM: &str = include_str!("../../../libraries/comm/src/comm.st");
const PROGRAM: &str = r#"
USING Comm;

PROGRAM Main
VAR
    Field : Link;
    Missing : Link;
    Spare : Link;
END_VAR
    Field(Name := 'modbus_tcp:0', AlarmDelay := T#1s);
    Missing(Name := 'mqtt:7', AlarmDelay := T#1s);
    Spare(Name := 'mqtt:1', AlarmDelay := T#1s);
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
TASK Fast(INTERVAL := T#100ms, PRIORITY := 1);
PROGRAM Main WITH Fast : Main;
END_RESOURCE
END_CONFIGURATION
"#;

/// A driver whose health the test sets.
struct TestDriver(Arc<Mutex<IoDriverHealth>>);

impl IoDriver for TestDriver {
    fn read_inputs(&mut self, _inputs: &mut [u8]) -> Result<(), RuntimeError> {
        Ok(())
    }
    fn write_outputs(&mut self, _outputs: &[u8]) -> Result<(), RuntimeError> {
        Ok(())
    }
    fn health(&self) -> IoDriverHealth {
        self.0.lock().expect("health").clone()
    }
}

struct Plant {
    harness: TestHarness,
    health: Arc<Mutex<IoDriverHealth>>,
}

impl Plant {
    fn new() -> Self {
        let mut harness = TestHarness::from_sources(&[ALARM, COMM, PROGRAM]).expect("compiles");
        let health = Arc::new(Mutex::new(IoDriverHealth::Ok));
        harness.runtime_mut().add_io_driver_with_link(
            "modbus-tcp",
            "modbus_tcp:0",
            Box::new(TestDriver(health.clone())),
        );
        harness.runtime_mut().add_disabled_link("mqtt:1");
        Self { harness, health }
    }

    fn scans(&mut self, count: usize) {
        for _ in 0..count {
            self.harness.advance_time(Duration::from_millis(100));
            let result = self.harness.cycle();
            assert!(result.errors.is_empty(), "{:?}", result.errors);
        }
    }

    fn set(&self, health: IoDriverHealth) {
        *self.health.lock().expect("health") = health;
    }

    /// The value at `Main.<path>`, as the last scan left it.
    fn read(&self, path: &str) -> Value {
        let path = MemberPath::parse(&format!("Main.{path}")).expect("a path");
        self.harness
            .runtime()
            .with_program_view(|program| member_value(&path, program))
            .unwrap_or_else(|| panic!("{path:?} exists"))
    }

    /// An enumerated value's name, a text's content or a value's words.
    fn word(&self, path: &str) -> String {
        match self.read(path) {
            Value::Enum(value) => value.variant_name().to_string(),
            Value::String(text) => text.to_string(),
            other => format!("{other:?}"),
        }
    }

    fn flag(&self, path: &str) -> bool {
        matches!(self.read(path), Value::Bool(true))
    }

    fn time_ms(&self, path: &str) -> i64 {
        match self.read(path) {
            Value::Time(duration) => duration.as_millis(),
            other => panic!("{path} is a time: {other:?}"),
        }
    }
}

#[test]
fn a_link_follows_its_driver_through_its_states() {
    let mut plant = Plant::new();
    plant.scans(2);
    assert_eq!(plant.word("Field.State"), "Connected");
    assert!(plant.flag("Field.Connected") && plant.flag("Field.Known"));
    assert_eq!(plant.time_ms("Field.LastGoodAge"), 0, "data is exchanged");

    plant.set(IoDriverHealth::Faulted {
        error: "timeout after 500 ms".into(),
    });
    plant.scans(1);
    assert_eq!(plant.word("Field.State"), "Failed");
    assert!(!plant.flag("Field.Connected"));
    assert_eq!(plant.word("Field.LastError"), "timeout after 500 ms");
    assert_eq!(
        plant.time_ms("Field.StateTime"),
        0,
        "the state just changed"
    );
    plant.scans(3);
    assert_eq!(
        plant.time_ms("Field.StateTime"),
        300,
        "three scans in the state"
    );
    assert_eq!(
        plant.time_ms("Field.LastGoodAge"),
        400,
        "data was last exchanged four scans ago"
    );

    plant.set(IoDriverHealth::Degraded {
        error: "input not fresh".into(),
    });
    plant.scans(1);
    assert_eq!(plant.word("Field.State"), "Degraded");
    assert!(
        plant.flag("Field.Connected"),
        "a degraded link exchanges data"
    );

    plant.set(IoDriverHealth::Ok);
    plant.scans(1);
    assert_eq!(plant.word("Field.State"), "Connected");
    assert_eq!(
        plant.word("Field.LastError"),
        "input not fresh",
        "the latest error is kept"
    );
}

#[test]
fn an_unknown_link_and_a_disabled_one_read_as_such() {
    let mut plant = Plant::new();
    plant.scans(2);
    assert_eq!(plant.word("Missing.State"), "Unknown");
    assert!(!plant.flag("Missing.Known"));
    assert_eq!(
        plant.word("Missing.LastError"),
        "no link mqtt:7 in this runtime"
    );
    plant.scans(3);
    assert_eq!(
        plant.time_ms("Missing.StateTime"),
        300,
        "unknown since the runtime first looked"
    );
    assert_eq!(
        plant.time_ms("Missing.LastGoodAge"),
        300,
        "no data since the runtime first looked"
    );
    assert_eq!(plant.word("Spare.State"), "Disabled");
    assert!(plant.flag("Spare.Known") && !plant.flag("Spare.Connected"));
    assert!(
        plant.flag("Spare.Disabled"),
        "switched off in its configuration"
    );
    assert!(
        !plant.flag("Missing.Disabled"),
        "an unknown link is not switched off"
    );
}

#[test]
fn a_failed_link_raises_its_alarm_after_its_delay() {
    let mut plant = Plant::new();
    plant.scans(2);
    plant.set(IoDriverHealth::Faulted {
        error: "connection refused".into(),
    });
    plant.scans(5);
    assert!(
        !plant.flag("Field.LinkFailed.Active"),
        "not before the delay"
    );
    plant.scans(7);
    assert!(
        plant.flag("Field.LinkFailed.Active"),
        "after one second failed"
    );
    assert_eq!(
        plant.word("Field.LinkFailed.Message"),
        "modbus_tcp:0 failed"
    );
}

#[test]
fn a_link_without_data_raises_its_alarm_unless_it_is_switched_off() {
    // A misnamed link delivers no data, as a failed one does (specification 42, §4); a disabled
    // link is switched off on purpose.
    let mut plant = Plant::new();
    plant.scans(15);
    assert!(
        plant.flag("Missing.LinkFailed.Active"),
        "an unknown link raises its alarm"
    );
    assert_eq!(plant.word("Missing.LinkFailed.Message"), "mqtt:7 failed");
    assert!(
        !plant.flag("Spare.LinkFailed.Active"),
        "a disabled link raises none"
    );
    assert!(
        !plant.flag("Field.LinkFailed.Active"),
        "a connected link raises none"
    );
}
