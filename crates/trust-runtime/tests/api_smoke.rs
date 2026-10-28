//! The smallest uses of the runtime's API: a program compiled for this computer loads, runs its
//! scans and reports them to its metrics (specification 44 §8.8).
//!
//! The tests need the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`.

use std::sync::{Arc, Mutex};
use trust_runtime::harness::TestHarness;
use trust_runtime::metrics::RuntimeMetrics;
use trust_runtime::value::{Duration, Value};

const COUNTER: &str = r#"
    PROGRAM Main
    VAR
        count : DINT := DINT#0;
    END_VAR
    count := count + DINT#1;
    END_PROGRAM
"#;

#[test]
fn loads_runtime() {
    // A loaded program has run no scan: its logical time is zero and it counted no cycle. Catches a
    // load that starts the program's time or runs a first scan.
    let harness =
        TestHarness::from_source("PROGRAM Main\nEND_PROGRAM\n").expect("build the empty program");
    let runtime = harness.runtime();
    assert_eq!(runtime.current_time(), Duration::ZERO);
    assert_eq!(runtime.cycle_counter(), 0);
    assert_eq!(
        trust_runtime::execution_backend::EXECUTION_BACKEND,
        "native"
    );
}

#[test]
fn a_scan_runs_the_compiled_program() {
    // One scan adds 1 to `count`, which starts at 0. Catches a scan that does not run the program's
    // body, and a cycle that is not counted.
    let mut harness = TestHarness::from_source(COUNTER).expect("build the program");
    let result = harness.cycle();
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    assert_eq!(harness.get_output("Main.count"), Some(Value::DInt(1)));
    assert_eq!(harness.runtime().cycle_counter(), 1);
}

#[test]
fn runtime_metrics_record_the_scans_and_the_program_memory() {
    // Two scans are two recorded cycles, and the metrics hold the program's memory figure from the
    // moment the sink is set (specification 34 MEM-12). Catches a sink that records no scan, and a
    // figure that only a later status sees.
    let mut harness = TestHarness::from_source(COUNTER).expect("build the program");
    let metrics = Arc::new(Mutex::new(RuntimeMetrics::default()));
    harness.runtime_mut().set_metrics_sink(metrics.clone());
    assert!(metrics
        .lock()
        .expect("metrics lock")
        .snapshot()
        .program_memory
        .is_some());
    harness.runtime_mut().execute_cycle().expect("first scan");
    harness.runtime_mut().execute_cycle().expect("second scan");

    let snapshot = metrics.lock().expect("metrics lock").snapshot();
    assert_eq!(snapshot.cycle.samples(), 2);
    assert_eq!(harness.get_output("Main.count"), Some(Value::DInt(2)));
}
