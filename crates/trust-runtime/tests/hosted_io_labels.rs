//! Hosted presentation labels: each process-image binding of a compiled program is named in the
//! I/O snapshot by its declared name, a global's or a `VAR_CONFIG` path's (specification 44 §13.3).
//!
//! The tests need the toolchain of specification 44 §4.6 at TRUST_TOOLCHAIN.
use std::sync::mpsc;

use trust_runtime::{
    debug::DebugControl,
    harness::TestHarness,
    io::{IoAddress, IoSnapshotValue},
    value::Value,
};

/// The I/O snapshot that a scan of `source` publishes with `%IX0.0` TRUE names its one input
/// `label` and shows it TRUE.
fn assert_input(source: &str, label: &str) {
    let mut harness = TestHarness::from_source(source).unwrap();
    let debug = DebugControl::new();
    harness.runtime_mut().set_debug_control(debug.clone());
    let (sender, snapshots) = mpsc::channel();
    debug.set_io_sender(sender);
    let address = IoAddress::parse("%IX0.0").unwrap();
    harness
        .runtime_mut()
        .write_io(&address, Value::Bool(true))
        .unwrap();
    harness
        .runtime_mut()
        .execute_cycle()
        .expect("a scan publishes the I/O snapshot");
    let snapshot = snapshots
        .try_recv()
        .expect("the scan published an I/O snapshot");
    assert_eq!(snapshot.inputs.len(), 1);
    assert_eq!(snapshot.inputs[0].name.as_deref(), Some(label));
    assert!(matches!(
        snapshot.inputs[0].value,
        IoSnapshotValue::Value(Value::Bool(true))
    ));
}

#[test]
fn global_binding_keeps_its_name_and_value() {
    assert_input(
        "VAR_GLOBAL IN0 AT %IX0.0 : BOOL; END_VAR PROGRAM Main END_PROGRAM",
        "IN0",
    );
}

#[test]
fn configured_symbolic_binding_retains_qualified_path() {
    let source = r#"
PROGRAM Main
VAR enabled : BOOL; END_VAR
END_PROGRAM
CONFIGURATION C
PROGRAM P : Main;
VAR_CONFIG P.enabled AT %IX0.0 : BOOL; END_VAR
END_CONFIGURATION
"#;
    assert_input(source, "P.enabled");
}
