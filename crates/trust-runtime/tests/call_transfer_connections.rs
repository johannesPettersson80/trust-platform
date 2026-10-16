//! Saved artifacts: writable connections that coincide only at run time commit in formal
//! declaration order, and an input may snapshot the single output or in-out target it shares
//! (spec 10 §7.5 steps 4-6 and the closing overlap paragraph).

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

fn global(runtime: &Runtime, name: &str) -> Value {
    runtime
        .storage()
        .get_global(name)
        .cloned()
        .unwrap_or_else(|| panic!("missing global {name}"))
}

fn array_elements(value: &Value) -> Vec<Value> {
    let Value::Array(array) = value else {
        panic!("expected an array, got {value:?}");
    };
    array.elements().to_vec()
}

#[test]
fn saved_function_outputs_on_one_array_element_commit_in_declaration_order() {
    let source = r#"
FUNCTION Pair : INT
VAR_OUTPUT first : INT; second : INT; END_VAR
first := INT#1;
second := INT#2;
Pair := INT#0;
END_FUNCTION
PROGRAM Main
VAR values : ARRAY[0..1] OF INT; i : INT; j : INT; r : INT; END_VAR
i := INT#1;
j := INT#1;
r := Pair(first => values[i], second => values[j]);
END_PROGRAM
"#;
    let mut runtime = saved_runtime(source);
    runtime.execute_cycle().unwrap();
    assert_eq!(
        array_elements(&program_var(&runtime, "values")),
        vec![Value::Int(0), Value::Int(2)]
    );
}

#[test]
fn saved_function_block_outputs_on_one_array_element_commit_in_declaration_order() {
    let source = r#"
FUNCTION_BLOCK Split
VAR_OUTPUT first : INT; second : INT; END_VAR
first := INT#1;
second := INT#2;
END_FUNCTION_BLOCK
PROGRAM Main
VAR values : ARRAY[0..1] OF INT; i : INT; j : INT; splitter : Split; END_VAR
i := INT#1;
j := INT#1;
splitter(first => values[i], second => values[j]);
END_PROGRAM
"#;
    let mut runtime = saved_runtime(source);
    runtime.execute_cycle().unwrap();
    assert_eq!(
        array_elements(&program_var(&runtime, "values")),
        vec![Value::Int(0), Value::Int(2)]
    );
}

#[test]
fn saved_output_through_a_reference_and_its_variable_commits_the_last_declared_formal() {
    let source = r#"
VAR_GLOBAL x : INT := INT#0; END_VAR
FUNCTION Pair : INT
VAR_OUTPUT first : INT; second : INT; END_VAR
first := INT#1;
second := INT#2;
Pair := INT#0;
END_FUNCTION
PROGRAM Main
VAR p : REF_TO INT; r : INT; END_VAR
p := REF(x);
r := Pair(first => x, second => p^);
END_PROGRAM
"#;
    let mut runtime = saved_runtime(source);
    runtime.execute_cycle().unwrap();
    assert_eq!(global(&runtime, "x"), Value::Int(2));
}

#[test]
fn saved_input_may_snapshot_the_single_output_target_it_shares() {
    let source = r#"
VAR_GLOBAL x : INT := INT#5; END_VAR
FUNCTION Next : INT
VAR_INPUT a : INT; END_VAR
VAR_OUTPUT o : INT; END_VAR
o := a + INT#1;
Next := a;
END_FUNCTION
PROGRAM Main
VAR result : INT; END_VAR
result := Next(a := x, o => x);
END_PROGRAM
"#;
    let mut runtime = saved_runtime(source);
    runtime.execute_cycle().unwrap();
    assert_eq!(program_var(&runtime, "result"), Value::Int(5));
    assert_eq!(global(&runtime, "x"), Value::Int(6));
}

#[test]
fn saved_input_may_snapshot_the_single_in_out_target_it_shares() {
    let source = r#"
VAR_GLOBAL x : INT := INT#5; END_VAR
FUNCTION Bump : INT
VAR_INPUT a : INT; END_VAR
VAR_IN_OUT target : INT; END_VAR
target := target + INT#1;
Bump := a;
END_FUNCTION
PROGRAM Main
VAR result : INT; END_VAR
result := Bump(a := x, target := x);
END_PROGRAM
"#;
    let mut runtime = saved_runtime(source);
    runtime.execute_cycle().unwrap();
    assert_eq!(program_var(&runtime, "result"), Value::Int(5));
    assert_eq!(global(&runtime, "x"), Value::Int(6));
}
