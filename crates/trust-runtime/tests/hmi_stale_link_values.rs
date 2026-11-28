//! Specification 42, §5 item 4: a page value located at an input a driver fills turns stale while
//! that driver's link exchanges no data, with the link's time since its last data as its age; a
//! value outside the driver's bytes, or computed by the program, is not marked.

use std::ops::Range;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use trust_runtime::error::RuntimeError;
use trust_runtime::harness::TestHarness;
use trust_runtime::hmi::exchange::HmiExchange;
use trust_runtime::hmi::member_paths::MemberPath;
use trust_runtime::io::{IoDriver, IoDriverHealth};
use trust_runtime::value::Duration;

const HMI: &str = include_str!("../../../libraries/hmi/src/hmi.st");

const PLANT: &str = r#"
PROGRAM Plant
VAR
    Level : BYTE;
    Other : BYTE;
    Computed : INT;
    Screens : Hmi.Display;
END_VAR
    Computed := BYTE_TO_INT(Level) * 2;
    Screens();
END_PROGRAM

CONFIGURATION Cell
RESOURCE Cpu ON PLC
    TASK Fast (INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM Main WITH Fast : Plant;
END_RESOURCE
VAR_CONFIG
    Main.Level AT %IB1 : BYTE;
    Main.Other AT %IB9 : BYTE;
END_VAR
END_CONFIGURATION
"#;

/// A driver filling input bytes 0 to 3, whose link works or fails as the test says.
struct Field {
    working: Arc<AtomicBool>,
}

impl IoDriver for Field {
    fn read_inputs(&mut self, inputs: &mut [u8]) -> Result<(), RuntimeError> {
        if let Some(byte) = inputs.get_mut(1) {
            *byte = 7;
        }
        Ok(())
    }

    fn write_outputs(&mut self, _outputs: &[u8]) -> Result<(), RuntimeError> {
        Ok(())
    }

    fn health(&self) -> IoDriverHealth {
        if self.working.load(Ordering::SeqCst) {
            IoDriverHealth::Ok
        } else {
            IoDriverHealth::Faulted {
                error: "no answer".into(),
            }
        }
    }

    fn input_bytes(&self) -> Vec<Range<usize>> {
        std::iter::once(0..4).collect()
    }
}

fn run(harness: &mut TestHarness, ms: i64) {
    for _ in 0..ms / 100 {
        harness.advance_time(Duration::from_millis(100));
        let result = harness.cycle();
        assert!(
            result.errors.is_empty(),
            "the cycle runs: {:?}",
            result.errors
        );
    }
}

fn ages(exchange: &HmiExchange, paths: &[MemberPath]) -> Vec<Option<u64>> {
    exchange
        .read(paths)
        .expect("the program serves the screens")
        .values
        .into_iter()
        .map(|value| value.and_then(|wire| wire.stale_ms))
        .collect()
}

#[test]
fn a_value_at_the_inputs_of_a_link_without_data_turns_stale_with_its_age() {
    let working = Arc::new(AtomicBool::new(true));
    let mut harness = TestHarness::from_sources(&[HMI, PLANT]).expect("the plant compiles");
    harness.runtime_mut().add_io_driver_with_link(
        "field",
        "modbus_tcp:0",
        Box::new(Field {
            working: Arc::clone(&working),
        }),
    );
    let exchange = Arc::new(HmiExchange::new());
    harness
        .runtime_mut()
        .set_hmi_exchange(Arc::clone(&exchange));
    let paths: Vec<MemberPath> = ["Main.Level", "Main.Other", "Main.Computed"]
        .iter()
        .map(|path| MemberPath::parse(path).expect("a path"))
        .collect();
    let _screen = exchange.open_screen(paths.clone());
    run(&mut harness, 500);
    assert_eq!(
        ages(&exchange, &paths),
        vec![None, None, None],
        "the link works"
    );

    working.store(false, Ordering::SeqCst);
    run(&mut harness, 2000);
    let stale = ages(&exchange, &paths);
    assert!(
        stale[0].is_some_and(|age| (1900..=2100).contains(&age)),
        "the value at its inputs is stale with the time since the link's last data: {stale:?}"
    );
    assert_eq!(stale[1], None, "outside the bytes the driver fills");
    assert_eq!(stale[2], None, "computed by the program");

    working.store(true, Ordering::SeqCst);
    run(&mut harness, 200);
    assert_eq!(
        ages(&exchange, &paths),
        vec![None, None, None],
        "data again"
    );
}

const MIRRORED: &str = r#"
PROGRAM Mirrored
VAR
    Copied : BYTE;
    Beyond : BYTE;
    Command : BYTE;
    Screens : Hmi.Display;
END_VAR
    Command := Copied;
    Screens();
END_PROGRAM

CONFIGURATION Cell
RESOURCE Cpu ON PLC
    TASK Fast (INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM Main WITH Fast : Mirrored;
END_RESOURCE
VAR_CONFIG
    Main.Copied AT %IB0 : BYTE;
    Main.Beyond AT %IB1 : BYTE;
    Main.Command AT %QB0 : BYTE;
END_VAR
END_CONFIGURATION
"#;

/// A driver without data that fills the inputs from the outputs, from byte 0 as far as the output
/// image the drivers get reaches, as the loopback driver does.
struct Mirror;

impl IoDriver for Mirror {
    fn read_inputs(&mut self, _inputs: &mut [u8]) -> Result<(), RuntimeError> {
        Ok(())
    }

    fn write_outputs(&mut self, _outputs: &[u8]) -> Result<(), RuntimeError> {
        Ok(())
    }

    fn health(&self) -> IoDriverHealth {
        IoDriverHealth::Faulted {
            error: "no answer".into(),
        }
    }

    fn input_bytes_with_outputs(&self, outputs: usize) -> Vec<Range<usize>> {
        std::iter::once(0..outputs).collect()
    }
}

#[test]
fn a_driver_that_mirrors_the_outputs_marks_only_the_inputs_its_copy_reaches() {
    // Specification 42, §5 item 4: the runtime asks a driver for the input bytes it fills with
    // the length of the output image the drivers get, which a driver that copies the outputs over
    // the inputs, as the loopback driver does, fills from byte 0. The output image here is 1 byte
    // (%QB0) and the input image 2 (%IB0 and %IB1), so while the driver's link has no data, the
    // value at %IB0 turns stale and the value at %IB1, beyond the copy, does not. Catches the
    // runtime asking with no output image, which marks nothing, and with the input image or
    // more, which marks a value the driver never fills.
    let mut harness = TestHarness::from_sources(&[HMI, MIRRORED]).expect("the program compiles");
    harness
        .runtime_mut()
        .add_io_driver_with_link("mirror", "loopback:0", Box::new(Mirror));
    let exchange = Arc::new(HmiExchange::new());
    harness
        .runtime_mut()
        .set_hmi_exchange(Arc::clone(&exchange));
    let paths: Vec<MemberPath> = ["Main.Copied", "Main.Beyond"]
        .iter()
        .map(|path| MemberPath::parse(path).expect("a path"))
        .collect();
    let _screen = exchange.open_screen(paths.clone());
    run(&mut harness, 500);
    let stale = ages(&exchange, &paths);
    assert!(stale[0].is_some(), "the value the copy reaches: {stale:?}");
    assert_eq!(stale[1], None, "the value beyond the 1-byte copy");
}
