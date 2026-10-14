//! Actual current STBC construction/execution parity between prepared register plans
//! and the shared stack path used while a debugger is active.
use trust_runtime::bytecode::BytecodeVersion;
use trust_runtime::harness::CompileSession;
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::value::{DateTimeValue, Duration, Value};
use trust_runtime_core::vm::{ExecutionServices, PreparationLimits, PreparedModule};

struct Services {
    stack: bool,
}
impl ExecutionServices for Services {
    fn debug_active(&self) -> bool {
        self.stack
    }
    fn deadline_exceeded(&self) -> bool {
        false
    }
    fn has_wall_clock(&self) -> bool {
        false
    }
    fn current_dt(&self) -> Result<DateTimeValue, RuntimeError> {
        Err(RuntimeError::UndefinedFunction("CURRENT_DT".into()))
    }
}

#[test]
fn source_free_string_element_reads_and_comparisons_match_shared_stack() {
    let source = r#"
PROGRAM Main
VAR
    narrow : STRING[8] := 'AB';
    wide : WSTRING[8] := "αβ";
    narrow_char : CHAR;
    wide_char : WCHAR;
    equal : BOOL;
    cycles : DINT;
END_VAR
narrow_char := narrow[2];
wide_char := wide[2];
equal := (narrow_char = narrow[2]) AND (wide_char = wide[2]);
cycles := cycles + 1;
END_PROGRAM
"#;
    let bytes = CompileSession::from_source(source)
        .build_bytecode_module_for_version(BytecodeVersion::SOURCE_FREE)
        .expect("accepted source")
        .encode()
        .expect("serialize supplied application");
    let prepared = PreparedModule::from_bytes(&bytes, PreparationLimits::default())
        .expect("admitted saved STBC");
    let register_services = Services { stack: false };
    let stack_services = Services { stack: true };
    let mut register = prepared
        .instantiate_with_services(0, &register_services)
        .unwrap();
    let mut stack = prepared
        .instantiate_with_services(0, &stack_services)
        .unwrap();
    register.set_register_profile_enabled(true);
    register.set_tier1_enabled(true);
    stack.set_register_profile_enabled(true);
    let preparation = register.tier1_snapshot();
    for cycle in 1..=80 {
        let time = Duration::from_millis(cycle * 10);
        register.execute_cycle(time).unwrap();
        stack.execute_cycle(time).unwrap();
        let Some(Value::Instance(register_root)) = register.storage().get_global("Main") else {
            panic!("program root")
        };
        let Some(Value::Instance(stack_root)) = stack.storage().get_global("Main") else {
            panic!("program root")
        };
        for (name, expected) in [
            ("narrow_char", Value::Char(b'B')),
            ("wide_char", Value::WChar(0x03b2)),
            ("equal", Value::Bool(true)),
            ("cycles", Value::DInt(cycle as i32)),
        ] {
            assert_eq!(
                register.storage().get_instance_var(*register_root, name),
                Some(&expected)
            );
            assert_eq!(
                stack.storage().get_instance_var(*stack_root, name),
                Some(&expected)
            );
        }
        assert_eq!(register.outputs(), stack.outputs());
    }
    let optimized = register.register_profile_snapshot();
    assert!(
        optimized.register_programs_executed > 0,
        "actual register body execution: {optimized:?}"
    );
    assert!(
        optimized.fallback_reasons.is_empty(),
        "tested program must be eligible: {optimized:?}"
    );
    let debug = stack.register_profile_snapshot();
    assert_eq!(debug.register_programs_executed, 0);
    assert!(debug
        .fallback_reasons
        .iter()
        .any(|reason| reason.reason == "debug_mode" && reason.count > 0));
    let running = register.tier1_snapshot();
    assert!(
        running.block_executions > 0,
        "precompiled tier selected after default warmup: {running:?}"
    );
    assert_eq!(
        running.compile_attempts, preparation.compile_attempts,
        "RUN never compiles"
    );
    assert_eq!(running.compile_successes, preparation.compile_successes);
}

#[test]
fn nested_source_free_functions_and_function_blocks_keep_prepared_register_execution() {
    let source = r#"
FUNCTION Increment : INT
VAR_INPUT input : INT; END_VAR
Increment := input + 1;
END_FUNCTION
FUNCTION_BLOCK Counter
VAR_INPUT delta : INT; END_VAR
VAR_OUTPUT total : INT; END_VAR
total := total + Increment(delta);
END_FUNCTION_BLOCK
PROGRAM Main
VAR counter : Counter; result : INT; cycles : INT; END_VAR
counter(delta := 2);
result := Increment(counter.total);
cycles := cycles + 1;
END_PROGRAM
"#;
    let bytes = CompileSession::from_source(source)
        .build_bytecode_module_for_version(BytecodeVersion::SOURCE_FREE)
        .unwrap()
        .encode()
        .unwrap();
    let prepared = PreparedModule::from_bytes(&bytes, PreparationLimits::default()).unwrap();
    let register_services = Services { stack: false };
    let stack_services = Services { stack: true };
    let mut register = prepared
        .instantiate_with_services(0, &register_services)
        .unwrap();
    let mut stack = prepared
        .instantiate_with_services(0, &stack_services)
        .unwrap();
    register.set_register_profile_enabled(true);
    register.set_tier1_enabled(true);
    let preparation = register.tier1_snapshot();
    for cycle in 1..=80 {
        let time = Duration::from_millis(cycle * 10);
        register.execute_cycle(time).unwrap();
        stack.execute_cycle(time).unwrap();
        let Some(Value::Instance(root)) = register.storage().get_global("Main") else {
            panic!("program root")
        };
        let Some(Value::Instance(stack_root)) = stack.storage().get_global("Main") else {
            panic!("stack program root")
        };
        for (name, expected) in [
            ("result", Value::Int((cycle * 3 + 1) as i16)),
            ("cycles", Value::Int(cycle as i16)),
        ] {
            assert_eq!(
                register.storage().get_instance_var(*root, name),
                Some(&expected)
            );
            assert_eq!(
                stack.storage().get_instance_var(*stack_root, name),
                Some(&expected)
            );
        }
        assert_eq!(register.outputs(), stack.outputs());
    }
    let profile = register.register_profile_snapshot();
    assert_eq!(
        profile.register_programs_executed,
        80 * 4,
        "program, FB, and both function calls: {profile:?}"
    );
    assert!(
        profile.fallback_reasons.is_empty(),
        "eligible nested bodies stay optimized: {profile:?}"
    );
    let running = register.tier1_snapshot();
    assert!(running.block_executions > 0);
    assert_eq!(
        running.compile_attempts, preparation.compile_attempts,
        "RUN never compiles"
    );
    assert_eq!(running.compile_successes, preparation.compile_successes);
}
