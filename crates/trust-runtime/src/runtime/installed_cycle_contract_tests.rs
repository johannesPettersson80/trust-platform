//! Hosted driver publication remains the old transaction's commit boundary.
use std::sync::{Arc, Mutex};
use std::time::{Duration as PhysicalDuration, Instant};

use super::io_subsystem::is_cycle_operational_error;
use super::Runtime;
use crate::{
    debug::RuntimeEvent,
    error::RuntimeError,
    io::{IoAddress, IoDriver, IoSafeState},
    value::{Duration, Value},
    watchdog::FaultPolicy,
};

const GPIO: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/portability/f401/gpio.stbc"
));

struct PublicationDriver {
    attempts: Arc<Mutex<Vec<Vec<u8>>>>,
    finish_at: Option<Instant>,
    fail: bool,
}

impl IoDriver for PublicationDriver {
    fn read_inputs(&mut self, _: &mut [u8]) -> Result<(), RuntimeError> {
        Ok(())
    }
    fn write_outputs(&mut self, outputs: &[u8]) -> Result<(), RuntimeError> {
        self.attempts.lock().unwrap().push(outputs.to_vec());
        if let Some(finish_at) = self.finish_at
            && let Some(remaining) = finish_at.checked_duration_since(Instant::now())
        {
            std::thread::sleep(remaining);
        }
        if self.fail {
            Err(RuntimeError::IoDriver("publication failed".into()))
        } else {
            Ok(())
        }
    }
}

#[test]
fn successful_driver_submission_is_not_rolled_back_by_the_final_host_deadline() {
    let mut runtime = Runtime::from_bytes(GPIO, Default::default()).unwrap();
    let attempts = Arc::new(Mutex::new(Vec::new()));
    runtime
        .write_io(&IoAddress::parse("%IX0.0").unwrap(), Value::Bool(true))
        .unwrap();
    runtime.set_current_time(Duration::from_millis(30));
    // Expire inside physical submission, after all precommit checks. Execution
    // has no deadline, so this pins the final hosted output-deadline owner.
    let deadline = Instant::now() + PhysicalDuration::from_secs(1);
    runtime.set_output_commit_deadline(Some(deadline));
    runtime.add_io_driver(
        "publication",
        Box::new(PublicationDriver {
            attempts: Arc::clone(&attempts),
            finish_at: Some(deadline),
            fail: false,
        }),
    );
    assert_eq!(runtime.execute_cycle(), Err(RuntimeError::WatchdogTimeout));
    assert_eq!(attempts.lock().unwrap().as_slice(), &[vec![1]]);
    assert_eq!(
        runtime.io().outputs(),
        &[1],
        "successful physical submission already committed flat Q"
    );
    assert_eq!(runtime.last_fault(), Some(&RuntimeError::WatchdogTimeout));
    assert_eq!(
        runtime.cycle_counter(),
        0,
        "failed cycle does not emit a successful completion"
    );
}

#[test]
fn failed_driver_submission_restores_flat_q_without_reverting_plc_task_effects() {
    let mut runtime = Runtime::from_bytes(GPIO, Default::default()).unwrap();
    let attempts = Arc::new(Mutex::new(Vec::new()));
    runtime
        .write_io(&IoAddress::parse("%IX0.0").unwrap(), Value::Bool(true))
        .unwrap();
    runtime.set_current_time(Duration::from_millis(30));
    runtime.add_io_driver(
        "publication",
        Box::new(PublicationDriver {
            attempts: Arc::clone(&attempts),
            finish_at: None,
            fail: true,
        }),
    );
    assert_eq!(
        runtime.execute_cycle(),
        Err(RuntimeError::IoDriver("publication failed".into()))
    );
    assert_eq!(
        attempts.lock().unwrap().as_slice(),
        &[vec![1]],
        "driver received the staged attempt"
    );
    assert_eq!(
        runtime.io().outputs(),
        &[0],
        "failed submission keeps the prior published Q image"
    );
    let Some(Value::Instance(plant)) = runtime.storage().get_global("Plant") else {
        panic!("admitted root")
    };
    assert_eq!(
        runtime.storage().get_instance_var(*plant, "led"),
        Some(&Value::Bool(true)),
        "the established rollback domain does not include already executed PLC values"
    );
    assert!(runtime.faulted());
}

#[test]
fn shared_host_cycle_records_each_scheduled_and_background_program_call() {
    let mut harness = crate::harness::TestHarness::from_source(
        r#"
        PROGRAM First VAR count : INT; END_VAR count := count + 1; END_PROGRAM
        PROGRAM Second VAR count : INT; END_VAR count := count + 1; END_PROGRAM
        PROGRAM Background VAR count : INT; END_VAR count := count + 1; END_PROGRAM
        CONFIGURATION Plant
            RESOURCE CPU ON PLC
                TASK Fast(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM A WITH Fast : First;
                PROGRAM B WITH Fast : Second;
                PROGRAM Idle : Background;
            END_RESOURCE
        END_CONFIGURATION
    "#,
    )
    .unwrap();
    let metrics = Arc::new(Mutex::new(crate::metrics::RuntimeMetrics::new()));
    let runtime = harness.runtime_mut();
    runtime.set_metrics_sink(Arc::clone(&metrics));
    for ms in [10, 20] {
        runtime.set_current_time(Duration::from_millis(ms));
        runtime.execute_cycle().unwrap();
    }
    let snapshot = metrics.lock().unwrap().snapshot();
    let mut calls = snapshot
        .profiling
        .calls
        .iter()
        .map(|row| (row.kind.as_str(), row.name.as_str(), row.calls))
        .collect::<Vec<_>>();
    calls.sort_unstable();
    assert_eq!(
        calls,
        [
            ("program", "A", 2),
            ("program", "B", 2),
            ("program", "Idle", 2)
        ]
    );
    assert_eq!(snapshot.faults, 0);
    assert_eq!(runtime.cycle_counter(), 2);
}

#[test]
fn shared_host_cycle_records_the_faulting_program_without_counting_unreached_programs() {
    let mut harness = crate::harness::TestHarness::from_source(
        r#"
        PROGRAM First VAR count : INT; END_VAR count := count + 1; END_PROGRAM
        PROGRAM Fails VAR denominator : INT; result : INT; END_VAR
            result := 10 / denominator;
        END_PROGRAM
        PROGRAM Unreached VAR count : INT; END_VAR count := count + 1; END_PROGRAM
        PROGRAM Background VAR count : INT; END_VAR count := count + 1; END_PROGRAM
        CONFIGURATION Plant
            RESOURCE CPU ON PLC
                TASK Fast(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM A WITH Fast : First;
                PROGRAM B WITH Fast : Fails;
                PROGRAM C WITH Fast : Unreached;
                PROGRAM Idle : Background;
            END_RESOURCE
        END_CONFIGURATION
    "#,
    )
    .unwrap();
    let metrics = Arc::new(Mutex::new(crate::metrics::RuntimeMetrics::new()));
    let runtime = harness.runtime_mut();
    runtime.set_metrics_sink(Arc::clone(&metrics));
    runtime.set_current_time(Duration::from_millis(10));
    assert_eq!(runtime.execute_cycle(), Err(RuntimeError::DivisionByZero));
    assert_eq!(runtime.last_fault(), Some(&RuntimeError::DivisionByZero));
    assert_eq!(runtime.cycle_counter(), 0);
    let snapshot = metrics.lock().unwrap().snapshot();
    let mut calls = snapshot
        .profiling
        .calls
        .iter()
        .map(|row| (row.kind.as_str(), row.name.as_str(), row.calls))
        .collect::<Vec<_>>();
    calls.sort_unstable();
    assert_eq!(calls, [("program", "A", 1), ("program", "B", 1)]);
    assert_eq!(snapshot.faults, 1);
}

#[test]
fn debugger_cycle_error_classification_preserves_enclosing_exhaustion_only() {
    assert!(is_cycle_operational_error(&RuntimeError::WatchdogTimeout));
    assert!(is_cycle_operational_error(&RuntimeError::ExecutionTimeout));
    assert!(is_cycle_operational_error(&RuntimeError::ExecutionLimit {
        kind: trust_runtime_core::error::ExecutionLimitKind::AllocationBytes,
        used: 4,
        requested: 8,
        limit: 10,
    }));
    assert!(!is_cycle_operational_error(&RuntimeError::TypeMismatch));
    assert!(!is_cycle_operational_error(
        &RuntimeError::InvalidIoAddress("destination".into())
    ));
}

fn large_payload() -> Value {
    // Empty STRING defaults are cheap to construct. Assignment normalization
    // scans and charges this text, so it crosses cycle Work without exhausting
    // startup.
    Value::String("A".repeat(1024).into())
}

fn runtime_with_cycle_work_limit(source: &str, limit: usize) -> Runtime {
    let module = crate::harness::CompileSession::from_source(source)
        .build_bytecode_module()
        .unwrap();
    let bytes = module.encode().unwrap();
    let limits = trust_runtime_core::vm::PreparationLimits {
        max_work: limit,
        ..Default::default()
    };
    Runtime::from_bytes(&bytes, limits).unwrap()
}

fn assert_cycle_allowance_fault_applies_safe_output(
    runtime: &mut Runtime,
    attempts: &Arc<Mutex<Vec<Vec<u8>>>>,
) {
    let address = IoAddress::parse("%QX0.0").unwrap();
    runtime.set_io_safe_state(IoSafeState {
        outputs: vec![(address, Value::Bool(false))],
    });
    runtime.set_fault_policy(FaultPolicy::SafeHalt);
    runtime.add_io_driver(
        "cycle-limit",
        Box::new(PublicationDriver {
            attempts: Arc::clone(attempts),
            finish_at: None,
            fail: false,
        }),
    );
    let error = runtime
        .execute_cycle()
        .expect_err("cycle allowance must fault");
    assert!(matches!(
        &error,
        RuntimeError::ExecutionLimit {
            kind: trust_runtime_core::error::ExecutionLimitKind::Work,
            ..
        }
    ));
    assert_eq!(runtime.last_fault(), Some(&error));
    assert!(runtime.faulted());
    assert_eq!(runtime.cycle_counter(), 0);
    assert_eq!(*attempts.lock().unwrap(), [vec![0]]);
    assert_eq!(runtime.io().outputs(), &[0]);
}

const LARGE_DEBUG_VALUE_SOURCE: &str = r#"
VAR_GLOBAL
    payload : STRING[1024];
    lamp AT %QX0.0 : BOOL;
END_VAR
PROGRAM Main
    lamp := TRUE;
END_PROGRAM
CONFIGURATION Plant
    RESOURCE CPU ON PLC
        TASK Fast(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM A WITH Fast : Main;
    END_RESOURCE
END_CONFIGURATION
"#;

#[test]
fn queued_debugger_write_exhaustion_faults_the_cycle_and_applies_safe_output() {
    let mut runtime = runtime_with_cycle_work_limit(LARGE_DEBUG_VALUE_SOURCE, 512);
    runtime.set_current_time(Duration::from_millis(10));
    let attempts = Arc::new(Mutex::new(Vec::new()));
    let debug = runtime.enable_debug();
    debug.enqueue_global_write("payload", large_payload());
    assert_cycle_allowance_fault_applies_safe_output(&mut runtime, &attempts);
    assert_eq!(
        runtime.storage().get_global("payload"),
        Some(&Value::String(String::new().into())),
        "the refused queued write must leave its destination unchanged"
    );
}

#[test]
fn debugger_force_exhaustion_faults_the_cycle_and_applies_safe_output() {
    let mut runtime = runtime_with_cycle_work_limit(LARGE_DEBUG_VALUE_SOURCE, 512);
    runtime.set_current_time(Duration::from_millis(10));
    let attempts = Arc::new(Mutex::new(Vec::new()));
    let debug = runtime.enable_debug();
    debug.force_global("payload", large_payload());
    assert_cycle_allowance_fault_applies_safe_output(&mut runtime, &attempts);
    assert_eq!(
        runtime.storage().get_global("payload"),
        Some(&Value::String(String::new().into())),
        "the refused force must leave its destination unchanged"
    );
}

#[cfg(feature = "debug")]
const DEBUG_OBSERVED_SOURCE: &str = r#"
VAR_GLOBAL
    payload AT %IB0 : STRING[300];
    lamp AT %QX0.0 : BOOL;
END_VAR
PROGRAM Main
    lamp := TRUE;
END_PROGRAM
CONFIGURATION Plant
    RESOURCE CPU ON PLC
        TASK Fast(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM A WITH Fast : Main;
    END_RESOURCE
END_CONFIGURATION
"#;

/// Run one scan under `limit`; with `debug`, a debugger observes the scan.
#[cfg(feature = "debug")]
fn observed_scan(limit: usize, debug: bool) -> (Result<(), RuntimeError>, Vec<RuntimeEvent>) {
    let mut runtime = runtime_with_cycle_work_limit(DEBUG_OBSERVED_SOURCE, limit);
    runtime.set_current_time(Duration::from_millis(10));
    let (events_tx, events_rx) = std::sync::mpsc::channel();
    if debug {
        runtime.enable_debug().set_runtime_sender(events_tx);
    }
    let result = runtime.execute_cycle();
    (result, events_rx.try_iter().collect())
}

#[cfg(feature = "debug")]
#[test]
fn debug_snapshot_adds_no_cycle_work() {
    // TIME-08: observing a scan is not PLC work. Find the exact allowance the
    // unobserved scan needs; the observed scan must complete under the same one.
    let (mut low, mut high) = (1usize, 500usize);
    assert!(
        observed_scan(high, false).0.is_ok(),
        "the upper probe admits the scan"
    );
    while low < high {
        let middle = low + (high - low) / 2;
        if observed_scan(middle, false).0.is_ok() {
            high = middle;
        } else {
            low = middle + 1;
        }
    }
    let (result, events) = observed_scan(low, true);
    result.expect("debugger observation must not consume cycle work");
    assert!(
        events
            .iter()
            .any(|event| matches!(event, RuntimeEvent::TaskEnd { .. })),
        "the debugger observed the completed program body"
    );
    assert!(
        matches!(
            observed_scan(low - 1, true).0,
            Err(RuntimeError::ExecutionLimit {
                kind: trust_runtime_core::error::ExecutionLimitKind::Work,
                ..
            })
        ),
        "one unit less still refuses the observed scan exactly like the unobserved one"
    );
}
