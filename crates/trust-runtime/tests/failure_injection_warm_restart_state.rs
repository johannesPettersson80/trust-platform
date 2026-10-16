//! Spec 10 section 4.8: a failed warm restart is prepared against isolated storage, so the live
//! storage, cycle count, forces, queued writes, fault state and executable module stay
//! unchanged, and the old runtime keeps running.

use trust_runtime::bytecode::{
    BytecodeModule, BytecodeVersion, InitializationStage, SectionData, SectionId, StorageOwner,
};
use trust_runtime::debug::ForcedVarTarget;
use trust_runtime::error::RuntimeError;
use trust_runtime::harness::CompileSession;
use trust_runtime::retain::RetainStore;
use trust_runtime::value::{Duration, Value};
use trust_runtime::{RestartMode, RetainSnapshot, Runtime};
use trust_runtime_core::vm::{PreparationLimits, PreparedModule};

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
    module: *const PreparedModule,
    forced_value: Option<Value>,
    forced_storage: Option<Value>,
    queued_storage: Option<Value>,
}

fn observe(runtime: &Runtime, forced: &str, queued: &str) -> LiveState {
    let forced_value = runtime
        .debug_control()
        .and_then(|debug| debug.forced_var_value(&ForcedVarTarget::Global(forced.into())));
    LiveState {
        cycle_counter: runtime.cycle_counter(),
        faulted: runtime.faulted(),
        last_fault: runtime.last_fault().cloned(),
        module: runtime.prepared() as *const PreparedModule,
        forced_value,
        forced_storage: runtime.storage().get_global(forced).cloned(),
        queued_storage: runtime.storage().get_global(queued).cloned(),
    }
}

/// Three live cycles, so the cycle counter is 3 before any restart is attempted.
fn store_fixture() -> Runtime {
    let mut runtime = CompileSession::from_source(STORE_SOURCE)
        .build_runtime()
        .expect("compile warm restart fixture");
    for _ in 0..3 {
        runtime.execute_cycle().expect("establish live cycles");
    }
    runtime
}

/// A force on `count` and a queued write to `queued`. Both are set after the cycles because the
/// next cycle would apply the queued write.
fn store_fixture_with_live_state() -> Runtime {
    let mut runtime = store_fixture();
    let debug = runtime.enable_debug();
    debug.force_global("count", Value::DInt(50));
    debug.enqueue_global_write("queued", Value::DInt(7));
    runtime
}

/// Spec 10 section 4.8: a store-load failure in a warm restart leaves the live storage, cycle
/// count, forces, queued writes, fault state and executable module unchanged.
#[test]
fn failed_warm_restart_after_store_load_failure_keeps_live_state() {
    let mut runtime = store_fixture_with_live_state();
    runtime.set_retain_store(Some(Box::new(FailingLoadStore)), None);
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
    runtime.set_retain_store(Some(Box::new(IncompatibleLoadStore)), None);
    let before = observe(&runtime, "count", "queued");
    assert!(matches!(
        runtime.storage().get_global("level"),
        Some(Value::Int(2))
    ));

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
    assert!(matches!(
        runtime.storage().get_global("level"),
        Some(Value::Int(2))
    ));

    runtime
        .execute_cycle()
        .expect("the old runtime must remain executable");
    let after_cycle = observe(&runtime, "count", "queued");
    assert_eq!(after_cycle.cycle_counter, 4);
    assert_eq!(after_cycle.queued_storage, Some(Value::DInt(7)));
    assert_eq!(after_cycle.forced_storage, Some(Value::DInt(50)));
}

/// Spec 11 divisor fixture, copied from `restart_generation_restart_clock.rs`: the
/// replacement initializer of `quotient` divides by the retained global `divisor`.
fn artifact(body: &str) -> Vec<u8> {
    let source = format!("{body}\nCONFIGURATION Conf RESOURCE Controller ON PLC TASK Tick (INTERVAL := T#10ms, PRIORITY := 0); PROGRAM Plant WITH Tick : Main; END_RESOURCE END_CONFIGURATION");
    CompileSession::from_source(&source)
        .build_bytecode_module_for_version(BytecodeVersion::SOURCE_FREE)
        .expect("accepted ST source")
        .encode()
        .expect("encode STBC")
}

/// Source-free artifact whose `quotient` replacement initializer divides by the retained global `divisor`. The source initializer `INT#8 / INT#1` is rewritten so its denominator is loaded from `divisor` instead of a constant.
fn divisor_fixture() -> Vec<u8> {
    let bytes = artifact(
        r#"
VAR_GLOBAL RETAIN divisor : INT := INT#1; END_VAR
VAR_GLOBAL stamp : TIME := T#0ms; quotient : INT := INT#8 / INT#1; END_VAR
VAR_GLOBAL incoming AT %IB0 : BYTE; outgoing AT %QB0 : BYTE; END_VAR
PROGRAM Main outgoing := BYTE#42; END_PROGRAM
"#,
    );
    let mut module = BytecodeModule::decode(&bytes).unwrap();
    let Some(SectionData::StorageLayout(layout)) = module.section(SectionId::StorageLayout) else {
        panic!("layout")
    };
    let Some(SectionData::StringTable(strings)) = module.section(SectionId::StringTable) else {
        panic!("strings")
    };
    let quotient = layout
        .entries
        .iter()
        .position(|entry| {
            entry.owner == StorageOwner::Global
                && strings.entries[entry.name_idx as usize] == "quotient"
        })
        .unwrap();
    let divisor = layout
        .entries
        .iter()
        .find(|entry| {
            entry.owner == StorageOwner::Global
                && strings.entries[entry.name_idx as usize] == "divisor"
        })
        .unwrap()
        .ref_idx
        .unwrap();
    let Some(SectionData::Initializers(initializers)) = module.section(SectionId::Initializers)
    else {
        panic!("initializers")
    };
    let quotient_action = initializers
        .entries
        .iter()
        .find(|entry| {
            entry.declaration_idx == Some(quotient as u32)
                && entry.stage == InitializationStage::Explicit
        })
        .unwrap()
        .clone();
    let Some(SectionData::PouBodies(code)) = module.section_mut(SectionId::PouBodies) else {
        panic!("code")
    };
    let quotient_body = &mut code[quotient_action.code_offset as usize
        ..(quotient_action.code_offset + quotient_action.code_length) as usize];
    let denominator = quotient_body
        .windows(6)
        .enumerate()
        .filter(|(_, bytes)| bytes[0] == 0x10 && bytes[5] == 0x43)
        .map(|(offset, _)| offset)
        .collect::<Vec<_>>();
    assert_eq!(
        denominator.len(),
        1,
        "one denominator LOAD_CONST directly precedes DIV"
    );
    quotient_body[denominator[0]] = 0x20;
    quotient_body[denominator[0] + 1..denominator[0] + 5].copy_from_slice(&divisor.to_le_bytes());
    module.encode().unwrap()
}

/// Spec 10 section 4.8: an initializer failure in a warm restart leaves the live storage, cycle
/// count, forces, queued writes, fault state and executable module unchanged.
#[test]
fn failed_warm_restart_after_initializer_failure_keeps_live_state() {
    let bytes = divisor_fixture();
    let mut runtime =
        Runtime::from_bytes(&bytes, PreparationLimits::default()).expect("admit divisor fixture");
    for _ in 0..2 {
        runtime.advance_time(Duration::from_millis(10));
        runtime.execute_cycle().expect("establish live cycles");
    }
    let debug = runtime.enable_debug();
    debug.force_global("quotient", Value::Int(50));
    debug.enqueue_global_write("stamp", Value::Time(Duration::from_millis(7)));
    runtime
        .write_global("divisor", Value::Int(0))
        .expect("set the retained divisor to zero");
    let before = observe(&runtime, "quotient", "stamp");
    assert_eq!(before.cycle_counter, 2);
    assert_eq!(before.forced_value, Some(Value::Int(50)));
    assert_eq!(
        before.queued_storage,
        Some(Value::Time(Duration::from_millis(0)))
    );

    let error = runtime
        .restart(RestartMode::Warm)
        .expect_err("a replacement initializer that divides by zero must refuse the restart");
    assert!(
        matches!(error, RuntimeError::DivisionByZero),
        "expected DivisionByZero, got {error:?}"
    );
    assert_eq!(observe(&runtime, "quotient", "stamp"), before);

    runtime.advance_time(Duration::from_millis(10));
    runtime
        .execute_cycle()
        .expect("the old runtime must remain executable");
    let after_cycle = observe(&runtime, "quotient", "stamp");
    assert_eq!(after_cycle.cycle_counter, 3);
    assert_eq!(
        after_cycle.queued_storage,
        Some(Value::Time(Duration::from_millis(7)))
    );
    assert_eq!(after_cycle.forced_storage, Some(Value::Int(50)));
}

/// Spec 11 online-change section: a refused preparation leaves the runtime fault and status
/// state unchanged, so a latched fault and its cause survive the refusal and still stop scans
/// until the owner clears the fault.
#[test]
fn failed_warm_restart_keeps_a_latched_fault_and_its_cause() {
    let mut runtime = store_fixture();
    // The fault latch clears runtime mutations, so forces and queued writes are set after it.
    let _ = runtime.watchdog_timeout();
    let debug = runtime.enable_debug();
    debug.force_global("count", Value::DInt(50));
    debug.enqueue_global_write("queued", Value::DInt(7));
    runtime.set_retain_store(Some(Box::new(FailingLoadStore)), None);
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

    runtime.clear_fault();
    runtime
        .execute_cycle()
        .expect("the cleared runtime runs the old program");
    let after_cycle = observe(&runtime, "count", "queued");
    assert_eq!(after_cycle.cycle_counter, 4);
    assert_eq!(after_cycle.queued_storage, Some(Value::DInt(7)));
    assert_eq!(after_cycle.forced_storage, Some(Value::DInt(50)));
}
