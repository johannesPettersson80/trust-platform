//! Saved artifacts: an omitted call input takes its declared default on the first call (spec 10
//! §7.6-7.7, spec 12 §7.7).

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
fn saved_omitted_function_block_input_uses_its_declared_initializer() {
    let source = r#"
FUNCTION_BLOCK Probe
VAR_INPUT x : DINT := DINT#3; END_VAR
VAR_OUTPUT seen : DINT; END_VAR
seen := x;
END_FUNCTION_BLOCK
PROGRAM Main
VAR sampler : Probe; outX : DINT; END_VAR
sampler();
outX := sampler.seen;
END_PROGRAM
"#;
    let mut runtime = saved_runtime(source);
    runtime.execute_cycle().unwrap();
    assert_eq!(program_var(&runtime, "outX"), Value::DInt(3));
}

#[test]
fn saved_omitted_function_block_input_without_initializer_uses_the_type_default() {
    let source = r#"
FUNCTION_BLOCK Probe
VAR_INPUT y : DINT; END_VAR
VAR_OUTPUT seen : DINT; END_VAR
seen := y;
END_FUNCTION_BLOCK
PROGRAM Main
VAR sampler : Probe; outY : DINT := DINT#7; END_VAR
sampler();
outY := sampler.seen;
END_PROGRAM
"#;
    let mut runtime = saved_runtime(source);
    runtime.execute_cycle().unwrap();
    assert_eq!(program_var(&runtime, "outY"), Value::DInt(0));
}

#[test]
fn saved_omitted_function_input_without_initializer_takes_the_type_default() {
    let source = r#"
FUNCTION Echo : DINT
VAR_INPUT x : DINT; END_VAR
Echo := x;
END_FUNCTION
PROGRAM Main
VAR result : DINT := DINT#7; END_VAR
result := Echo();
END_PROGRAM
"#;
    let mut runtime = saved_runtime(source);
    runtime.execute_cycle().unwrap();
    assert_eq!(program_var(&runtime, "result"), Value::DInt(0));
}
