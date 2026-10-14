//! A successful schema reload retires queued mutations and the debugger's thread targets.
use trust_runtime::harness::CompileSession;
use trust_runtime::value::{Duration, Value};

/// Spec 34 HOST-09 (lines 488-489): queued writes and forces are invalidated at the
/// successful schema boundary, so the next cycle applies neither of them.
#[test]
fn hosted_observation_successful_reload_drops_queued_writes_and_forces() {
    let source = "VAR_GLOBAL RETAIN count : DINT := DINT#3; END_VAR PROGRAM Main count := count + DINT#1; END_PROGRAM";
    let mut installed = CompileSession::from_source(source).build_runtime().unwrap();
    installed.write_global("count", Value::DInt(12)).unwrap();
    let debug = installed.enable_debug();
    debug.enqueue_global_write("count", Value::DInt(77));
    debug.force_global("count", Value::DInt(44));

    let replacement =
        CompileSession::from_source(source.replace("count + DINT#1", "count + DINT#2"))
            .build_bytecode_module()
            .unwrap();
    installed.set_current_time(Duration::from_millis(35));
    installed.apply_bytecode_module(&replacement, None).unwrap();
    installed.execute_cycle().unwrap();

    // Retained 12 plus the replacement's increment; neither 77 nor 44 is applied.
    assert_eq!(
        installed.storage().get_global("count"),
        Some(&Value::DInt(14))
    );
}

/// Spec 34 HOST-09 (lines 488-489): step and stop targets are invalidated at the
/// successful schema boundary; the debugger keeps no thread target into the new schema.
#[test]
fn hosted_observation_successful_reload_clears_the_debugger_thread_targets() {
    let source =
        "PROGRAM Main VAR counter : DINT; END_VAR counter := counter + DINT#1; END_PROGRAM";
    let mut installed = CompileSession::from_source(source).build_runtime().unwrap();
    let debug = installed.enable_debug();
    debug.set_current_thread(Some(7));
    debug.step_thread(9);
    assert_eq!(debug.current_thread(), Some(7));
    assert_eq!(debug.target_thread(), Some(9));

    let replacement = CompileSession::from_source(source.replace("+ DINT#1", "+ DINT#2"))
        .build_bytecode_module()
        .unwrap();
    installed.apply_bytecode_module(&replacement, None).unwrap();

    assert_eq!(debug.current_thread(), None);
    assert_eq!(debug.target_thread(), None);
}
