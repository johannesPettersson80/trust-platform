//! A later driver's failure restores flat Q but cannot revoke an earlier driver's accepted write.
use std::sync::{Arc, Mutex};

use trust_runtime::error::RuntimeError;
use trust_runtime::io::{IoAddress, IoDriver};
use trust_runtime::value::{Duration, Value};
use trust_runtime::Runtime;

const GPIO: &[u8] = include_bytes!("fixtures/portability/f401/gpio.stbc");

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
    let mut runtime = Runtime::from_bytes(GPIO, Default::default()).unwrap();
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
        runtime.io().outputs(),
        &[0],
        "the touched flat Q window returns to the last published image"
    );
    assert!(runtime.faulted());
}
