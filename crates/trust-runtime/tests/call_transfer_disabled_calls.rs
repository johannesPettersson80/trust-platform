//! Saved artifacts: a disabled call (EN := FALSE) skips its body, writable targets and instance
//! state. A disabled value-producing call returns the declared type default (spec 10 §7.5 step 3;
//! docs/IEC_DECISIONS.md, 2026-07-30 POU call evaluation).

use trust_runtime::harness::CompileSession;
use trust_runtime::value::Value;
use trust_runtime::Runtime;

fn saved_runtime(source: &str) -> Runtime {
    let bytes = CompileSession::from_source(source)
        .build_bytecode_module()
        .unwrap()
        .encode()
        .unwrap();
    Runtime::from_bytes(&bytes, Default::default()).expect("saved artifact constructs state")
}

fn program_var(runtime: &Runtime, name: &str) -> Value {
    let Some(Value::Instance(main)) = runtime.storage().get_global("Main") else {
        panic!("missing program root");
    };
    runtime
        .storage()
        .get_instance_var(*main, name)
        .cloned()
        .unwrap_or_else(|| panic!("missing Main.{name}"))
}

#[test]
fn saved_disabled_function_call_skips_body_and_targets_and_returns_declared_default() {
    let source = r#"
VAR_GLOBAL trace : INT; END_VAR
TYPE Setpoint : DINT := DINT#42; END_TYPE
FUNCTION Read : Setpoint
VAR_INPUT EN : BOOL := TRUE; END_VAR
VAR_IN_OUT counter : INT; END_VAR
VAR_EXTERNAL trace : INT; END_VAR
trace := trace + INT#1;
counter := counter + INT#1;
Read := DINT#5;
END_FUNCTION
PROGRAM Main
VAR calls : INT := INT#0; result : DINT := DINT#9; END_VAR
result := Read(EN := FALSE, counter := calls);
END_PROGRAM
"#;
    let mut runtime = saved_runtime(source);

    runtime
        .execute_cycle()
        .expect("a disabled call must not fault");

    assert_eq!(runtime.storage().get_global("trace"), Some(&Value::Int(0)));
    assert_eq!(program_var(&runtime, "calls"), Value::Int(0));
    assert_eq!(program_var(&runtime, "result"), Value::DInt(42));
}

#[test]
fn saved_disabled_function_block_call_keeps_instance_state_and_caller_targets() {
    let source = r#"
FUNCTION_BLOCK Accumulator
VAR_INPUT EN : BOOL := TRUE; amount : DINT := DINT#1; END_VAR
VAR_OUTPUT total : DINT; END_VAR
VAR state : DINT; END_VAR
state := state + amount;
total := state;
END_FUNCTION_BLOCK
PROGRAM Main
VAR fb : Accumulator; first : DINT; second : DINT; third : DINT; END_VAR
fb(amount := DINT#5, total => first);
fb(EN := FALSE, amount := DINT#100, total => second);
fb(amount := DINT#1, total => third);
END_PROGRAM
"#;
    let mut runtime = saved_runtime(source);

    runtime
        .execute_cycle()
        .expect("a disabled call must not fault");

    assert_eq!(program_var(&runtime, "first"), Value::DInt(5));
    assert_eq!(program_var(&runtime, "second"), Value::DInt(0));
    assert_eq!(program_var(&runtime, "third"), Value::DInt(6));
}
