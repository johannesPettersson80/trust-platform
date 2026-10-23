//! Checked writes validate the current binding of a reference before committing.
#![cfg(feature = "full")]
use trust_runtime::{error::RuntimeError, harness::CompileSession, value::Value};

const BODY_V1: &str = "FUNCTION_BLOCK CounterFb VAR count : DINT := DINT#3; END_VAR END_FUNCTION_BLOCK \
    PROGRAM Main VAR q AT %QX0.0 : BOOL := FALSE; block : CounterFb; marker : DINT := DINT#0; END_VAR \
    marker := marker + DINT#1; END_PROGRAM";

/// Spec 34 UPDATE-04 and §4.3.1: a checked write through an instance reference resolved before a supported code replacement is rejected with ReferenceLifetime, leaving the new generation unchanged.
// EXPECTED-FAIL (spec 34 UPDATE-04): instance ids restart at 0 in every candidate storage
// (crates/trust-runtime-core/src/memory/instances.rs:11-14; crates/trust-runtime-core/src/vm/engine/mod.rs:224
// `VariableStorage::new()`; initialization/construction.rs:31-35 reserves roots first), and `ValueRef` carries no
// generation (crates/trust-runtime-core/src/value/reference.rs:23-30). The old `block` id therefore names the
// new generation's block, so the write is accepted. crates/trust-runtime-core/src/vm/engine/owned.rs:437-438
// documents that callers must re-resolve bindings instead.
#[test]
#[ignore = "MEM-07/UPDATE-04: references carry no generation before or after H; generation tokens belong to scope C (spec 34 §4.3.1, UPDATE-03 to UPDATE-08)"]
fn stale_instance_reference_across_a_body_replacement_is_rejected_without_side_effects() {
    let mut installed = CompileSession::from_source(BODY_V1)
        .build_runtime()
        .unwrap();
    let Some(Value::Instance(main)) = installed.storage().get_global("Main") else {
        panic!("Main")
    };
    let main = *main;
    let Some(Value::Instance(block)) = installed.storage().get_instance_var(main, "block") else {
        panic!("block")
    };
    let block = *block;
    let stale = installed
        .storage()
        .ref_for_instance_recursive(block, "count")
        .unwrap();
    let changed =
        CompileSession::from_source(BODY_V1.replace("marker + DINT#1", "marker + DINT#2"))
            .build_bytecode_module()
            .unwrap();
    installed.apply_bytecode_module(&changed, None).unwrap();

    let Some(Value::Instance(main)) = installed.storage().get_global("Main") else {
        panic!("Main after replacement")
    };
    let main = *main;
    let Some(Value::Instance(block)) = installed.storage().get_instance_var(main, "block") else {
        panic!("block after replacement")
    };
    let block = *block;
    assert_eq!(
        installed.storage().get_instance_var(block, "count"),
        Some(&Value::DInt(3)),
        "the replacement restarts the non-retained block"
    );
    let outputs = installed.io().outputs().to_vec();
    let cycles = installed.cycle_counter();
    let identity = std::ptr::from_ref(installed.prepared());

    assert_eq!(
        installed.write_reference(&stale, Value::DInt(99)),
        Err(RuntimeError::ReferenceLifetime)
    );
    assert_eq!(
        installed.storage().get_instance_var(block, "count"),
        Some(&Value::DInt(3))
    );
    assert_eq!(installed.io().outputs(), outputs.as_slice());
    assert!(std::ptr::eq(installed.prepared(), identity));
    assert_eq!(installed.cycle_counter(), cycles);
    assert!(!installed.faulted());
    installed.execute_cycle().unwrap();
    assert_eq!(installed.cycle_counter(), cycles + 1);
    assert_eq!(
        installed.storage().get_instance_var(main, "marker"),
        Some(&Value::DInt(2))
    );
}

const REMOVAL_V1: &str =
    "FUNCTION_BLOCK CounterFb VAR count : DINT := DINT#3; END_VAR END_FUNCTION_BLOCK \
    PROGRAM Main VAR q AT %QX0.0 : BOOL := FALSE; g_a : CounterFb; g_b : CounterFb; \
    marker : DINT := DINT#0; END_VAR marker := marker + DINT#1; END_PROGRAM";
const REMOVAL_V2: &str =
    "FUNCTION_BLOCK CounterFb VAR count : DINT := DINT#3; END_VAR END_FUNCTION_BLOCK \
    PROGRAM Main VAR q AT %QX0.0 : BOOL := FALSE; marker : DINT := DINT#0; END_VAR \
    marker := marker + DINT#1; END_PROGRAM";

/// Spec 34 UPDATE-04 and §4.3.1: a checked write through a reference to an instance that a supported replacement removed is rejected with ReferenceLifetime, leaving the destination, process image and scan counter unchanged.
#[test]
fn stale_instance_reference_to_an_instance_removed_by_replacement_is_rejected_without_side_effects()
{
    let mut installed = CompileSession::from_source(REMOVAL_V1)
        .build_runtime()
        .unwrap();
    let Some(Value::Instance(main)) = installed.storage().get_global("Main") else {
        panic!("Main")
    };
    let main = *main;
    let Some(Value::Instance(removed)) = installed.storage().get_instance_var(main, "g_b") else {
        panic!("g_b")
    };
    let removed = *removed;
    let stale = installed
        .storage()
        .ref_for_instance_recursive(removed, "count")
        .unwrap();
    let changed = CompileSession::from_source(REMOVAL_V2)
        .build_bytecode_module()
        .unwrap();
    installed.apply_bytecode_module(&changed, None).unwrap();

    let Some(Value::Instance(main)) = installed.storage().get_global("Main") else {
        panic!("Main after replacement")
    };
    let main = *main;
    assert_eq!(
        installed.storage().get_instance_var(main, "marker"),
        Some(&Value::DInt(0))
    );
    let outputs = installed.io().outputs().to_vec();
    let cycles = installed.cycle_counter();
    let identity = std::ptr::from_ref(installed.prepared());

    assert_eq!(
        installed.write_reference(&stale, Value::DInt(99)),
        Err(RuntimeError::ReferenceLifetime)
    );
    assert_eq!(
        installed.storage().get_instance_var(main, "marker"),
        Some(&Value::DInt(0))
    );
    assert_eq!(installed.io().outputs(), outputs.as_slice());
    assert!(std::ptr::eq(installed.prepared(), identity));
    assert_eq!(installed.cycle_counter(), cycles);
    assert!(!installed.faulted());
    installed.execute_cycle().unwrap();
    assert_eq!(installed.cycle_counter(), cycles + 1);
    assert_eq!(
        installed.storage().get_instance_var(main, "marker"),
        Some(&Value::DInt(1))
    );
}
