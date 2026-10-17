use trust_runtime::bytecode::BytecodeVersion;
use trust_runtime::harness::CompileSession;
use trust_runtime_core::retain::RestartMode;
use trust_runtime_core::value::{Duration, Value};
use trust_runtime_core::vm::{PreparationLimits, PreparedModule};

fn artifact(body: &str) -> Vec<u8> {
    let source = format!("{body}\nCONFIGURATION Conf RESOURCE Controller ON PLC TASK Tick (INTERVAL := T#10ms, PRIORITY := 0); PROGRAM Plant WITH Tick : Main; END_RESOURCE END_CONFIGURATION");
    CompileSession::from_source(&source)
        .build_bytecode_module_for_version(BytecodeVersion::SOURCE_FREE)
        .expect("accepted ST source")
        .encode()
        .expect("encode STBC")
}

/// Spec 34 §6.3 MEM-07: a stale handle shall not silently resolve to state in a replacement generation. MEM-08 shares this check, because a dynamic receiver after replacement is validated through the same handle identity. The warm restart constructs the next installation generation (spec 34 §11.3), so the handle captured before it names no state.
#[test]
fn handle_taken_before_warm_restart_does_not_resolve_into_the_replacement_generation() {
    let bytes = artifact(
        r#"
FUNCTION_BLOCK Leaf
VAR_OUTPUT value : INT := INT#4; END_VAR
value := value + INT#1;
END_FUNCTION_BLOCK
VAR_GLOBAL RETAIN fresh : Leaf; END_VAR
VAR_GLOBAL view : REF_TO INT; END_VAR
PROGRAM Main
fresh();
view := REF(fresh.value);
END_PROGRAM
"#,
    );
    let prepared = PreparedModule::from_bytes(&bytes, PreparationLimits::default()).unwrap();
    let mut state = prepared.instantiate(0).unwrap();
    state.execute_cycle(Duration::from_millis(10)).unwrap();
    let Some(Value::Reference(Some(stale))) = state.storage().get_global("view").cloned() else {
        panic!("reference taken before the restart")
    };
    assert_eq!(
        state.storage().read_by_ref_ref(&stale),
        Some(&Value::Int(5))
    );

    state.restart(RestartMode::Warm).unwrap();

    assert_eq!(state.storage().read_by_ref_ref(&stale), None);
}

/// Spec 34 §11.3: the warm-restart transfer binds a retained reference to the new generation, so
/// the PLC's own copy keeps naming the retained target, while the same reference copied out
/// before the restart names no state.
#[test]
fn a_retained_reference_is_rebound_inside_the_plc_while_an_outside_copy_goes_stale() {
    let bytes = artifact(
        r#"
VAR_GLOBAL RETAIN level : INT := INT#4; held : REF_TO INT; END_VAR
PROGRAM Main
level := level + INT#1;
held := REF(level);
END_PROGRAM
"#,
    );
    let prepared = PreparedModule::from_bytes(&bytes, PreparationLimits::default()).unwrap();
    let mut state = prepared.instantiate(0).unwrap();
    state.execute_cycle(Duration::from_millis(10)).unwrap();
    let Some(Value::Reference(Some(outside))) = state.storage().get_global("held").cloned() else {
        panic!("reference taken before the restart")
    };
    assert_eq!(
        state.storage().read_by_ref_ref(&outside),
        Some(&Value::Int(5))
    );

    state.restart(RestartMode::Warm).unwrap();

    let Some(Value::Reference(Some(inside))) = state.storage().get_global("held").cloned() else {
        panic!("retained reference after the restart")
    };
    assert_eq!(inside.generation, state.storage().generation());
    assert_eq!(
        state.storage().read_by_ref_ref(&inside),
        Some(&Value::Int(5))
    );
    assert_eq!(state.storage().read_by_ref_ref(&outside), None);
    state.execute_cycle(Duration::from_millis(20)).unwrap();
    // A reference of the installed generation keeps working across scans.
    assert_eq!(
        state.storage().read_by_ref_ref(&inside),
        Some(&Value::Int(6))
    );
    assert_eq!(state.storage().read_by_ref_ref(&outside), None);
}
