//! Hosted pre-submission failures restore the touched flat Q window and reach no driver.
use std::sync::{Arc, Mutex};
use std::time::Instant;

use super::Runtime;
use crate::{
    error::RuntimeError,
    io::{IoAddress, IoDriver},
    value::{Duration, Value},
};

const GPIO: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/portability/f401/gpio.stbc"
));

struct RecordingDriver {
    attempts: Arc<Mutex<Vec<Vec<u8>>>>,
}

impl IoDriver for RecordingDriver {
    fn read_inputs(&mut self, _: &mut [u8]) -> Result<(), RuntimeError> {
        Ok(())
    }

    fn write_outputs(&mut self, outputs: &[u8]) -> Result<(), RuntimeError> {
        self.attempts.lock().unwrap().push(outputs.to_vec());
        Ok(())
    }
}

/// Spec 34 HOST-09 (lines 510-512): a hosted deadline before submission restores the
/// touched flat-Q window, sends no driver write, and keeps the executed PLC value.
#[test]
fn hosted_observation_pre_submission_deadline_restores_flat_q_without_a_driver_write() {
    let mut runtime = Runtime::from_bytes(GPIO, Default::default()).unwrap();
    let attempts = Arc::new(Mutex::new(Vec::new()));
    runtime
        .write_io(&IoAddress::parse("%IX0.0").unwrap(), Value::Bool(true))
        .unwrap();
    runtime.set_current_time(Duration::from_millis(30));
    // Already expired when the staged image reaches the pre-submission output check.
    runtime.set_output_commit_deadline(Some(Instant::now()));
    runtime.add_io_driver(
        "publication",
        Box::new(RecordingDriver {
            attempts: Arc::clone(&attempts),
        }),
    );

    assert_eq!(runtime.execute_cycle(), Err(RuntimeError::WatchdogTimeout));
    assert!(
        attempts.lock().unwrap().is_empty(),
        "no driver receives the expired staged image"
    );
    assert_eq!(
        runtime.io().outputs(),
        &[0],
        "a pre-submission failure restores the prior flat Q image"
    );
    let Some(Value::Instance(plant)) = runtime.storage().get_global("Plant") else {
        panic!("admitted root")
    };
    assert_eq!(
        runtime.storage().get_instance_var(*plant, "led"),
        Some(&Value::Bool(true)),
        "the already executed PLC value is outside the output rollback domain"
    );
    assert_eq!(runtime.last_fault(), Some(&RuntimeError::WatchdogTimeout));
}
