//! Specification 42, §4 items 6–8: a link's counters and response time, switching an `io.toml`
//! driver's link off and on, and testing it.

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
    FieldOn : BOOL := TRUE;
    FieldTest : BOOL;
    FieldReset : BOOL;
END_VAR
    Field(Name := 'modbus_tcp:0', AlarmDelay := T#1s, Enable := FieldOn, Test := FieldTest, Reset := FieldReset);
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
TASK Fast(INTERVAL := T#100ms, PRIORITY := 1);
PROGRAM Main WITH Fast : Main;
END_RESOURCE
END_CONFIGURATION
"#;

/// What the test driver's device does and how often it was asked.
#[derive(Default)]
struct Device {
    reads: usize,
    writes: usize,
    delay_ms: u64,
    refuse: Option<String>,
    faulted: bool,
    detail: Option<String>,
}

struct TestDriver(Arc<Mutex<Device>>);

impl IoDriver for TestDriver {
    fn read_inputs(&mut self, _inputs: &mut [u8]) -> Result<(), RuntimeError> {
        let (delay, refuse) = {
            let mut device = self.0.lock().expect("device");
            device.reads += 1;
            (device.delay_ms, device.refuse.clone())
        };
        std::thread::sleep(std::time::Duration::from_millis(delay));
        match refuse {
            Some(error) => Err(RuntimeError::IoDriver(error.into())),
            None => Ok(()),
        }
    }
    fn write_outputs(&mut self, _outputs: &[u8]) -> Result<(), RuntimeError> {
        self.0.lock().expect("device").writes += 1;
        Ok(())
    }
    fn detail(&self) -> Option<smol_str::SmolStr> {
        self.0
            .lock()
            .expect("device")
            .detail
            .as_deref()
            .map(smol_str::SmolStr::new)
    }
    fn health(&self) -> IoDriverHealth {
        if self.0.lock().expect("device").faulted {
            IoDriverHealth::Faulted {
                error: "timeout".into(),
            }
        } else {
            IoDriverHealth::Ok
        }
    }
}

struct Plant {
    harness: TestHarness,
    device: Arc<Mutex<Device>>,
}

impl Plant {
    fn new() -> Self {
        let mut harness = TestHarness::from_sources(&[ALARM, COMM, PROGRAM]).expect("compiles");
        let device = Arc::new(Mutex::new(Device::default()));
        harness.runtime_mut().add_io_driver_with_link(
            "modbus-tcp",
            "modbus_tcp:0",
            Box::new(TestDriver(device.clone())),
        );
        Self { harness, device }
    }

    fn scans(&mut self, count: usize) {
        for _ in 0..count {
            self.harness.advance_time(Duration::from_millis(100));
            let result = self.harness.cycle();
            assert!(result.errors.is_empty(), "{:?}", result.errors);
        }
    }

    fn device(&self) -> std::sync::MutexGuard<'_, Device> {
        self.device.lock().expect("device")
    }

    /// The reads and writes so far, under one lock.
    fn exchanges(&self) -> (usize, usize) {
        let device = self.device();
        (device.reads, device.writes)
    }

    fn set(&mut self, name: &str, value: bool) {
        self.harness.set_input(name, Value::Bool(value));
    }

    /// The value at `Main.<path>`, as the last scan left it.
    fn read(&self, path: &str) -> Value {
        let path = MemberPath::parse(&format!("Main.{path}")).expect("a path");
        self.harness
            .runtime()
            .with_program_view(|program| member_value(&path, program))
            .unwrap_or_else(|| panic!("{path:?} exists"))
    }

    fn count(&self, path: &str) -> u64 {
        match self.read(path) {
            Value::UDInt(count) => u64::from(count),
            other => panic!("{path} is a count: {other:?}"),
        }
    }

    fn flag(&self, path: &str) -> bool {
        match self.read(path) {
            Value::Bool(flag) => flag,
            other => panic!("{path} is a flag: {other:?}"),
        }
    }

    fn text(&self, path: &str) -> String {
        match self.read(path) {
            Value::Enum(value) => value.variant_name().to_string(),
            Value::String(text) => text.to_string(),
            other => format!("{other:?}"),
        }
    }
}

#[test]
fn a_link_counts_its_exchanges_and_failures_and_starts_again_on_reset() {
    let mut plant = Plant::new();
    plant.scans(3);
    assert_eq!(
        (
            plant.count("Field.Exchanges"),
            plant.count("Field.Failures")
        ),
        (3, 0)
    );
    plant.device().faulted = true;
    plant.scans(2);
    assert_eq!(
        (
            plant.count("Field.Exchanges"),
            plant.count("Field.Failures")
        ),
        (3, 2)
    );
    plant.device().faulted = false;
    plant.set("Main.FieldReset", true);
    // The program passes Reset in one scan; the runtime takes its rising edge at the next.
    plant.scans(2);
    assert_eq!(
        (
            plant.count("Field.Exchanges"),
            plant.count("Field.Failures")
        ),
        (0, 0),
        "counting starts again"
    );
    plant.scans(1);
    assert_eq!(plant.count("Field.Exchanges"), 1);
}

#[test]
fn a_driver_links_response_time_is_its_last_read_and_write() {
    let mut plant = Plant::new();
    plant.device().delay_ms = 20;
    plant.scans(2);
    match plant.read("Field.ResponseTime") {
        Value::Time(time) => assert!(time.as_millis() >= 20, "{time:?}"),
        other => panic!("a time: {other:?}"),
    }
}

#[test]
fn a_link_switched_off_is_neither_read_nor_written_and_raises_no_alarm() {
    let mut plant = Plant::new();
    plant.scans(2);
    plant.set("Main.FieldOn", false);
    plant.scans(2);
    assert_eq!(plant.text("Field.State"), "Disabled");
    assert!(plant.flag("Field.Disabled"), "switched off, for the symbol");
    let (reads, writes) = plant.exchanges();
    plant.scans(12);
    assert_eq!(plant.exchanges(), (reads, writes), "the driver rests");
    assert_eq!(plant.text("Field.State"), "Disabled");
    assert!(
        matches!(plant.read("Field.LinkFailed.Active"), Value::Bool(false)),
        "no alarm for a link switched off"
    );
    plant.set("Main.FieldOn", true);
    // Passed in one scan, taken at the next scan's start, after its reads: read from the third.
    plant.scans(3);
    assert_eq!(plant.text("Field.State"), "Connected");
    assert!(!plant.flag("Field.Disabled"), "on again");
    assert!(plant.device().reads > reads, "read again");
}

#[test]
fn a_test_reads_the_device_once_and_reports_what_happened() {
    let mut plant = Plant::new();
    plant.set("Main.FieldOn", false);
    plant.scans(3);
    plant.device().refuse = Some("no answer from 10.0.0.5".to_string());
    let reads = plant.device().reads;
    plant.set("Main.FieldTest", true);
    plant.scans(2);
    assert_eq!(plant.device().reads, reads + 1, "one read for the test");
    assert!(
        plant
            .text("Field.TestResult")
            .contains("no answer from 10.0.0.5"),
        "{}",
        plant.text("Field.TestResult")
    );
    plant.device().refuse = None;
    plant.set("Main.FieldTest", false);
    plant.scans(1);
    plant.set("Main.FieldTest", true);
    plant.scans(2);
    assert!(
        plant.text("Field.TestResult").starts_with("answered in"),
        "{}",
        plant.text("Field.TestResult")
    );
}

/// Specification 42, §4 item 9: the block gives the driver's own words for its protocol's state,
/// and nothing once the driver reports none.
#[test]
fn a_link_gives_its_protocols_own_state() {
    let mut plant = Plant::new();
    let words = "10.0.0.5:502 unit 1, exception 02 illegal data address";
    plant.device().detail = Some(words.to_string());
    plant.scans(2);
    assert_eq!(plant.text("Field.Detail"), words);
    plant.device().detail = None;
    plant.scans(2);
    assert_eq!(plant.text("Field.Detail"), "");
}
