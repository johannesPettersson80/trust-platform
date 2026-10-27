//! A later driver's failure restores flat Q but cannot revoke an earlier driver's accepted write.
//!
//! The tests need the toolchain of specification 44 §4.6 at TRUST_TOOLCHAIN.
use std::sync::{Arc, Mutex};

use trust_runtime::error::RuntimeError;
use trust_runtime::harness::TestHarness;
use trust_runtime::io::{IoAddress, IoDriver};
use trust_runtime::value::{Duration, Value};
use trust_runtime::CompiledRuntime;

/// A program that copies the button at `%IX0.0` to the lamp at `%QX0.0` every 10 ms.
const GPIO: &str = r#"
PROGRAM Gpio
VAR
    button AT %IX0.0 : BOOL;
    led AT %QX0.0 : BOOL;
END_VAR
led := button;
END_PROGRAM

CONFIGURATION BoardProbe
RESOURCE Controller ON PLC
TASK Scan (INTERVAL := T#10ms, PRIORITY := 0);
PROGRAM Plant WITH Scan : Gpio;
END_RESOURCE
END_CONFIGURATION
"#;

/// The program of [`GPIO`], loaded.
fn gpio() -> CompiledRuntime {
    TestHarness::from_source(GPIO)
        .expect("the GPIO program compiles")
        .into_runtime()
}

/// The output image's byte.
fn output_byte(runtime: &CompiledRuntime) -> Result<Value, RuntimeError> {
    runtime.read_io(&IoAddress::parse("%QB0").expect("output byte address"))
}

struct RecordingDriver {
    attempts: Arc<Mutex<Vec<Vec<u8>>>>,
    fail: bool,
}

impl IoDriver for RecordingDriver {
    fn read_inputs(&mut self, _: &mut [u8]) -> Result<(), RuntimeError> {
        Ok(())
    }

    fn write_outputs(&mut self, outputs: &[u8]) -> Result<(), RuntimeError> {
        self.attempts.lock().unwrap().push(outputs.to_vec());
        if self.fail {
            Err(RuntimeError::IoDriver("second driver rejected".into()))
        } else {
            Ok(())
        }
    }
}

/// Spec 34 HOST-09 (lines 514-517): a failed submission restores the touched flat Q
/// window, while the effect an earlier driver already accepted is not rolled back.
#[test]
fn hosted_observation_earlier_driver_effect_is_kept_when_a_later_driver_fails() {
    let mut runtime = gpio();
    let first = Arc::new(Mutex::new(Vec::new()));
    let second = Arc::new(Mutex::new(Vec::new()));
    runtime
        .write_io(&IoAddress::parse("%IX0.0").unwrap(), Value::Bool(true))
        .unwrap();
    runtime.add_io_driver(
        "first",
        Box::new(RecordingDriver {
            attempts: Arc::clone(&first),
            fail: false,
        }),
    );
    runtime.add_io_driver(
        "second",
        Box::new(RecordingDriver {
            attempts: Arc::clone(&second),
            fail: true,
        }),
    );
    runtime.set_current_time(Duration::from_millis(10));

    assert_eq!(
        runtime.execute_cycle(),
        Err(RuntimeError::IoDriver("second driver rejected".into()))
    );
    assert_eq!(
        first.lock().unwrap().as_slice(),
        &[vec![1]],
        "the earlier driver's accepted write is not revoked; it is never asked to write back"
    );
    assert_eq!(second.lock().unwrap().as_slice(), &[vec![1]]);
    assert_eq!(
        output_byte(&runtime),
        Ok(Value::Byte(0)),
        "the touched flat Q window returns to the last published image"
    );
    assert!(runtime.faulted());
}
