//! Spec 10 section 4.8: a failed warm restart is prepared against isolated storage, so the live
//! storage, cycle count, forces, queued writes and fault state stay unchanged, and the old runtime
//! keeps running.
//!
//! The tests need the toolchain of specification 44 §4.6 at TRUST_TOOLCHAIN.

use trust_runtime::debug::{DebugControl, ForcedVarTarget};
use trust_runtime::error::RuntimeError;
use trust_runtime::harness::TestHarness;
use trust_runtime::retain::RetainStore;
use trust_runtime::value::Value;
use trust_runtime::{CompiledRuntime, RestartMode, RetainSnapshot};

const STORE_SOURCE: &str = r#"
VAR_GLOBAL RETAIN level : INT(0..10) := INT#2; END_VAR
VAR_GLOBAL count : DINT := DINT#0; queued : DINT := DINT#0; END_VAR
PROGRAM Main
count := count + DINT#1;
END_PROGRAM
"#;

/// Refuses to load: the checkpoint cannot be read.
struct FailingLoadStore;

impl RetainStore for FailingLoadStore {
    fn load(&self) -> Result<RetainSnapshot, RuntimeError> {
        Err(RuntimeError::RetainStore("injected load failure".into()))
    }

    fn store(&self, _snapshot: &RetainSnapshot) -> Result<(), RuntimeError> {
        Ok(())
    }
}

/// Loads a checkpoint whose retained `level` lies outside its declared subrange 0..10.
struct IncompatibleLoadStore;

impl RetainStore for IncompatibleLoadStore {
    fn load(&self) -> Result<RetainSnapshot, RuntimeError> {
        let mut snapshot = RetainSnapshot::default();
        snapshot.insert("level", Value::Int(100));
        Ok(snapshot)
    }

    fn store(&self, _snapshot: &RetainSnapshot) -> Result<(), RuntimeError> {
        Ok(())
    }
}

/// The live state a failed restart must leave unchanged, read through public accessors.
#[derive(Debug, PartialEq)]
struct LiveState {
    cycle_counter: u64,
    faulted: bool,
    last_fault: Option<RuntimeError>,
    forced_value: Option<Value>,
    forced_storage: Option<Value>,
    queued_storage: Option<Value>,
}

fn observe(runtime: &CompiledRuntime, forced: &str, queued: &str) -> LiveState {
    let forced_value = runtime
        .debug_control()
        .and_then(|debug| debug.forced_var_value(&ForcedVarTarget::Global(forced.into())));
    LiveState {
        cycle_counter: runtime.cycle_counter(),
        faulted: runtime.faulted(),
        last_fault: runtime.last_fault(),
        forced_value,
        forced_storage: runtime.read_variable(forced).ok(),
        queued_storage: runtime.read_variable(queued).ok(),
    }
}

/// A force of `count` to 50 and a queued write of 7 to `queued`, through a new debug control.
fn force_and_queue(runtime: &mut CompiledRuntime) {
    let debug = DebugControl::new();
    runtime.set_debug_control(debug.clone());
    debug
        .try_force_global("count", Value::DInt(50))
        .expect("a new force is inside the bound");
    debug
        .try_enqueue_global_write("queued", Value::DInt(7))
        .expect("a new target is inside the bound");
}

/// Three live cycles, so the cycle counter is 3 before any restart is attempted.
fn store_fixture() -> CompiledRuntime {
    let mut runtime = TestHarness::from_source(STORE_SOURCE)
        .expect("compile warm restart fixture")
        .into_runtime();
    for _ in 0..3 {
        runtime.execute_cycle().expect("establish live cycles");
    }
    runtime
}

/// A force on `count` and a queued write to `queued`. Both are set after the cycles because the
/// next cycle would apply the queued write.
fn store_fixture_with_live_state() -> CompiledRuntime {
    let mut runtime = store_fixture();
    force_and_queue(&mut runtime);
    runtime
}

/// Spec 10 section 4.8: a store-load failure in a warm restart leaves the live storage, cycle
/// count, forces, queued writes and fault state unchanged.
#[test]
fn failed_warm_restart_after_store_load_failure_keeps_live_state() {
    let mut runtime = store_fixture_with_live_state();
    runtime
        .set_retain_store(Some(Box::new(FailingLoadStore)), None)
        .expect("the store is taken");
    let before = observe(&runtime, "count", "queued");
    assert_eq!(before.cycle_counter, 3);
    assert_eq!(before.forced_value, Some(Value::DInt(50)));
    assert_eq!(before.queued_storage, Some(Value::DInt(0)));

    let error = runtime
        .restart_with_retain_store(RestartMode::Warm)
        .expect_err("store load failure must refuse the warm restart");
    assert!(
        matches!(error, RuntimeError::RetainStore(_)),
        "expected RetainStore, got {error:?}"
    );
    assert_eq!(observe(&runtime, "count", "queued"), before);

    runtime
        .execute_cycle()
        .expect("the old runtime must remain executable");
    let after_cycle = observe(&runtime, "count", "queued");
    assert_eq!(
        after_cycle.cycle_counter, 4,
        "the failed restart kept the count"
    );
    assert_eq!(
        after_cycle.queued_storage,
        Some(Value::DInt(7)),
        "the queued write is still pending and applies at the next cycle"
    );
    assert_eq!(
        after_cycle.forced_storage,
        Some(Value::DInt(50)),
        "the force is still active and is re-applied after the program runs"
    );
    assert_eq!(after_cycle.forced_value, Some(Value::DInt(50)));
}

/// Spec 10 section 4.8 with the retained-value migration failure as the refusal: the
/// checkpoint value is outside the declared subrange, so no retained or live value changes.
#[test]
fn failed_warm_restart_after_incompatible_retained_value_keeps_live_state() {
    let mut runtime = store_fixture_with_live_state();
    runtime
        .set_retain_store(Some(Box::new(IncompatibleLoadStore)), None)
        .expect("the store is taken");
    let before = observe(&runtime, "count", "queued");
    assert_eq!(runtime.read_variable("level"), Ok(Value::Int(2)));

    let error = runtime
        .restart_with_retain_store(RestartMode::Warm)
        .expect_err("out-of-range retained value must refuse the warm restart");
    assert!(
        matches!(
            &error,
            RuntimeError::RetainSubrangeViolation {
                name,
                value: 100,
                lower: 0,
                upper: 10,
            } if name.as_str() == "level"
        ),
        "expected RetainSubrangeViolation for level, got {error:?}"
    );
    assert_eq!(observe(&runtime, "count", "queued"), before);
    assert_eq!(runtime.read_variable("level"), Ok(Value::Int(2)));

    runtime
        .execute_cycle()
        .expect("the old runtime must remain executable");
    let after_cycle = observe(&runtime, "count", "queued");
    assert_eq!(after_cycle.cycle_counter, 4);
    assert_eq!(after_cycle.queued_storage, Some(Value::DInt(7)));
    assert_eq!(after_cycle.forced_storage, Some(Value::DInt(50)));
}

/// Spec 11 online-change section: a refused preparation leaves the runtime fault and status
/// state unchanged, so a latched fault and its cause survive the refusal and still stop scans.
#[test]
fn failed_warm_restart_keeps_a_latched_fault_and_its_cause() {
    let mut runtime = store_fixture();
    // The fault latch clears runtime mutations, so forces and queued writes are set after it.
    let _ = runtime.watchdog_timeout();
    force_and_queue(&mut runtime);
    runtime
        .set_retain_store(Some(Box::new(FailingLoadStore)), None)
        .expect("the store is taken");
    let before = observe(&runtime, "count", "queued");
    assert!(before.faulted);
    assert_eq!(before.last_fault, Some(RuntimeError::WatchdogTimeout));
    assert_eq!(before.queued_storage, Some(Value::DInt(0)));

    let error = runtime
        .restart_with_retain_store(RestartMode::Warm)
        .expect_err("store load failure must refuse the warm restart");
    assert!(
        matches!(error, RuntimeError::RetainStore(_)),
        "expected RetainStore, got {error:?}"
    );
    assert_eq!(observe(&runtime, "count", "queued"), before);

    assert!(
        matches!(runtime.execute_cycle(), Err(RuntimeError::ResourceFaulted)),
        "the latched fault still stops scans after the refused restart"
    );
    assert_eq!(
        observe(&runtime, "count", "queued"),
        before,
        "a refused scan changes neither the fault nor the pending mutations"
    );
}
