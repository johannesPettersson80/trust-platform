#![cfg(unix)]
//! A late observation (telemetry) failure after driver submission keeps the committed outputs.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use trust_runtime::config::{
    OpenOtTelemetryConfig, OpenOtTelemetryFenceMode, OpenOtTelemetrySource,
};
use trust_runtime::error::RuntimeError;
use trust_runtime::io::{IoAddress, IoDriver};
use trust_runtime::value::{Duration, Value};
use trust_runtime::Runtime;

const GPIO: &[u8] = include_bytes!("fixtures/portability/f401/gpio.stbc");

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

fn temp_shm_path(name: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "trust-runtime-hosted-observation-{name}-{stamp}.shm"
    ))
}

fn one_record_telemetry(path: PathBuf) -> OpenOtTelemetryConfig {
    OpenOtTelemetryConfig {
        enabled: true,
        path,
        capacity: 1,
        fence_mode: OpenOtTelemetryFenceMode::Fenced,
        allow_unfenced_for_proof: false,
        source: OpenOtTelemetrySource::Heartbeat,
        producer_instance: None,
        producer_instances: Vec::new(),
        persistence: Default::default(),
    }
}

/// Spec 34 HOST-09 (lines 507-509 and 512-514): the driver accepted Q=1, then the
/// telemetry append failed. The fault is latched, but the accepted output is not undone.
#[test]
fn hosted_observation_late_telemetry_failure_keeps_the_committed_outputs() {
    let path = temp_shm_path("late-telemetry");
    let mut runtime = Runtime::from_bytes(GPIO, Default::default()).unwrap();
    let attempts = Arc::new(Mutex::new(Vec::new()));
    runtime
        .write_io(&IoAddress::parse("%IX0.0").unwrap(), Value::Bool(true))
        .unwrap();
    runtime.add_io_driver(
        "publication",
        Box::new(RecordingDriver {
            attempts: Arc::clone(&attempts),
        }),
    );
    runtime
        .configure_openot_telemetry(&one_record_telemetry(path.clone()), None)
        .expect("configure a one-record OpenOT ring");
    runtime.set_current_time(Duration::from_millis(10));

    let error = runtime
        .execute_cycle()
        .expect_err("a telemetry append failure must fail the cycle");
    assert!(error
        .to_string()
        .contains("OpenOT telemetry publish heartbeat"));
    assert_eq!(attempts.lock().unwrap().as_slice(), &[vec![1]]);
    assert_eq!(
        runtime.io().outputs(),
        &[1],
        "the driver-accepted image stays committed after the late failure"
    );
    assert!(runtime.faulted());

    drop(std::fs::remove_file(path));
}
