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

/// Spec 34 §6.3 MEM-07: a stale handle shall not silently resolve to state in a replacement generation. MEM-08 shares this check, because a dynamic receiver after replacement is validated through the same handle identity. Expected to fail on the current code: the replacement storage restarts instance identities at zero, so a handle captured before the restart can name a replacement instance. Not run; predicted from reading the restart and storage construction.
#[test]
#[ignore = "MEM-07/UPDATE-04: references carry no generation before or after H; generation tokens belong to scope C (spec 34 §4.3.1, UPDATE-03 to UPDATE-08)"]
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
