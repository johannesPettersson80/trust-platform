use crate::harness::{CompileSession, TestHarness};
use crate::value::Value;
use crate::Runtime;
use trust_hir::symbols::ParamDirection;

fn runtime(source: &str) -> Runtime {
    CompileSession::from_source(source)
        .build_runtime()
        .unwrap_or_else(|error| panic!("POU fixture must compile: {error}"))
}

fn run(source: &str) -> TestHarness {
    let mut harness = TestHarness::from_source(source)
        .unwrap_or_else(|error| panic!("POU fixture must compile: {error}"));
    let cycle = harness.cycle();
    assert!(cycle.errors.is_empty(), "{:?}", cycle.errors);
    harness
}

fn compile_error(source: &str) -> String {
    match CompileSession::from_source(source).build_runtime() {
        Ok(_) => panic!("POU fixture must fail"),
        Err(error) => error.to_string(),
    }
}

#[test]
fn pou_function_contract_parameter_order_direction_and_defaults_are_preserved() {
    let runtime = runtime(
        r#"
FUNCTION Transfer : INT
VAR_INPUT
    source : INT := INT#4;
END_VAR
VAR_OUTPUT
    copied : INT;
END_VAR
VAR_IN_OUT
    target : INT;
END_VAR
Transfer := source;
END_FUNCTION
PROGRAM Main
END_PROGRAM
"#,
    );
    let function = runtime
        .functions()
        .values()
        .find(|function| function.name == "Transfer")
        .expect("Transfer");
    assert_eq!(
        function
            .params
            .iter()
            .map(|param| (param.name.as_str(), param.direction))
            .collect::<Vec<_>>(),
        vec![
            ("source", ParamDirection::In),
            ("copied", ParamDirection::Out),
            ("target", ParamDirection::InOut)
        ]
    );
    assert!(function.params[0].default.is_some());
    assert!(function.params[1].default.is_none());
    assert!(function.params[2].default.is_none());
}

#[test]
fn pou_function_contract_local_and_temp_slots_are_per_call() {
    let harness = run(r#"
FUNCTION Once : INT
VAR
    localValue : INT;
END_VAR
VAR_TEMP
    tempValue : INT;
END_VAR
localValue := localValue + INT#1;
tempValue := tempValue + INT#1;
Once := localValue * INT#10 + tempValue;
END_FUNCTION
PROGRAM Main
VAR
    first : INT;
    second : INT;
END_VAR
first := Once();
second := Once();
END_PROGRAM
"#);
    assert_eq!(harness.get_output("first"), Some(Value::Int(11)));
    assert_eq!(harness.get_output("second"), Some(Value::Int(11)));
}

#[test]
fn pou_function_contract_var_stat_persists_across_calls_and_cycles() {
    let mut harness = run(r#"
FUNCTION Next : INT
VAR_STAT
    count : INT;
END_VAR
count := count + INT#1;
Next := count;
END_FUNCTION
PROGRAM Main
VAR
    first : INT;
    second : INT;
END_VAR
first := Next();
second := Next();
END_PROGRAM
"#);
    assert_eq!(harness.get_output("first"), Some(Value::Int(1)));
    assert_eq!(harness.get_output("second"), Some(Value::Int(2)));
    let cycle = harness.cycle();
    assert!(cycle.errors.is_empty());
    assert_eq!(harness.get_output("first"), Some(Value::Int(3)));
    assert_eq!(harness.get_output("second"), Some(Value::Int(4)));
}

#[test]
fn pou_function_contract_var_stat_storage_is_function_qualified() {
    let harness = run(r#"
FUNCTION First : INT
VAR_STAT
    count : INT;
END_VAR
count := count + INT#1;
First := count;
END_FUNCTION
FUNCTION Second : INT
VAR_STAT
    count : INT := INT#10;
END_VAR
count := count + INT#1;
Second := count;
END_FUNCTION
PROGRAM Main
VAR
    a : INT;
    b : INT;
END_VAR
a := First();
b := Second();
END_PROGRAM
"#);
    assert_eq!(harness.get_output("a"), Some(Value::Int(1)));
    assert_eq!(harness.get_output("b"), Some(Value::Int(11)));
    assert_eq!(
        harness
            .runtime()
            .storage()
            .get_global("__STAT::First::count"),
        Some(&Value::Int(1))
    );
    assert_eq!(
        harness
            .runtime()
            .storage()
            .get_global("__STAT::Second::count"),
        Some(&Value::Int(11))
    );
}

#[test]
fn pou_function_contract_var_external_writes_existing_global() {
    let harness = run(r#"
VAR_GLOBAL
    shared : INT;
END_VAR
FUNCTION Bump : INT
VAR_EXTERNAL
    shared : INT;
END_VAR
shared := shared + INT#1;
Bump := shared;
END_FUNCTION
PROGRAM Main
VAR
    result : INT;
END_VAR
result := Bump();
END_PROGRAM
"#);
    assert_eq!(harness.get_output("result"), Some(Value::Int(1)));
    assert_eq!(
        harness.runtime().storage().get_global("shared"),
        Some(&Value::Int(1))
    );
}

#[test]
fn pou_function_contract_var_in_out_writes_caller_target() {
    let harness = run(r#"
FUNCTION Bump : INT
VAR_IN_OUT
    value : INT;
END_VAR
value := value + INT#1;
Bump := value;
END_FUNCTION
PROGRAM Main
VAR
    target : INT := INT#5;
    result : INT;
END_VAR
result := Bump(target);
END_PROGRAM
"#);
    assert_eq!(harness.get_output("target"), Some(Value::Int(6)));
    assert_eq!(harness.get_output("result"), Some(Value::Int(6)));
}

#[test]
fn pou_function_contract_var_output_copies_back_to_target() {
    let harness = run(r#"
FUNCTION Produce : INT
VAR_OUTPUT
    outputValue : INT;
END_VAR
outputValue := INT#9;
Produce := INT#1;
END_FUNCTION
PROGRAM Main
VAR
    target : INT;
    result : INT;
END_VAR
result := Produce(outputValue => target);
END_PROGRAM
"#);
    assert_eq!(harness.get_output("target"), Some(Value::Int(9)));
    assert_eq!(harness.get_output("result"), Some(Value::Int(1)));
}

#[test]
fn pou_function_contract_omitted_defaulted_input_uses_initializer() {
    let harness = run(r#"
FUNCTION Defaulted : INT
VAR_INPUT
    value : INT := INT#7;
END_VAR
Defaulted := value;
END_FUNCTION
PROGRAM Main
VAR
    result : INT;
END_VAR
result := Defaulted();
END_PROGRAM
"#);
    assert_eq!(harness.get_output("result"), Some(Value::Int(7)));
}

#[test]
fn pou_function_contract_missing_result_assignment_is_rejected() {
    let error = compile_error(
        r#"
FUNCTION Empty : DINT
END_FUNCTION
PROGRAM Main
VAR
    result : DINT := DINT#9;
END_VAR
result := Empty();
END_PROGRAM
"#,
    );
    assert!(error.contains("missing return value"), "{error}");
}

#[test]
fn pou_function_contract_constant_local_is_available_to_body() {
    let harness = run(r#"
FUNCTION Scale : INT
VAR CONSTANT
    factor : INT := INT#4;
END_VAR
Scale := factor * INT#3;
END_FUNCTION
PROGRAM Main
VAR
    result : INT;
END_VAR
result := Scale();
END_PROGRAM
"#);
    assert_eq!(harness.get_output("result"), Some(Value::Int(12)));
}

#[test]
fn pou_function_contract_direct_input_is_retained_in_metadata() {
    let runtime = runtime(
        r#"
FUNCTION ReadPort : BOOL
VAR_INPUT
    port AT %IX1.2 : BOOL;
END_VAR
ReadPort := port;
END_FUNCTION
PROGRAM Main
END_PROGRAM
"#,
    );
    let function = runtime
        .functions()
        .values()
        .find(|function| function.name == "ReadPort")
        .expect("ReadPort");
    let address = function.params[0].address.as_ref().expect("direct address");
    assert_eq!((address.byte, address.bit), (1, 2));
}

#[test]
fn pou_function_contract_wildcard_input_address_is_rejected() {
    let error = compile_error(
        r#"
FUNCTION ReadPort : BOOL
VAR_INPUT
    port AT %I* : BOOL;
END_VAR
ReadPort := port;
END_FUNCTION
PROGRAM Main
END_PROGRAM
"#,
    );
    assert!(
        error.contains("wildcard") || error.contains("VAR_CONFIG"),
        "{error}"
    );
}

#[test]
fn pou_function_contract_unknown_local_type_is_rejected() {
    let error = compile_error(
        r#"
FUNCTION Broken : INT
VAR
    value : MissingType;
END_VAR
Broken := INT#0;
END_FUNCTION
PROGRAM Main
END_PROGRAM
"#,
    );
    assert!(
        error.contains("MissingType") || error.contains("unknown type"),
        "{error}"
    );
}

#[test]
fn pou_function_contract_case_insensitive_duplicate_name_is_rejected() {
    let error = compile_error(
        r#"
FUNCTION Calculate : INT
Calculate := INT#1;
END_FUNCTION
FUNCTION calculate : INT
calculate := INT#2;
END_FUNCTION
PROGRAM Main
END_PROGRAM
"#,
    );
    assert!(error.contains("duplicate"), "{error}");
}

#[test]
fn pou_function_contract_forward_call_resolves_predeclared_function() {
    let harness = run(r#"
FUNCTION First : INT
First := Second();
END_FUNCTION
FUNCTION Second : INT
Second := INT#12;
END_FUNCTION
PROGRAM Main
VAR
    result : INT;
END_VAR
result := First();
END_PROGRAM
"#);
    assert_eq!(harness.get_output("result"), Some(Value::Int(12)));
}

#[test]
fn pou_function_contract_namespace_and_using_are_preserved() {
    let runtime = runtime(
        r#"
NAMESPACE Support
FUNCTION Identity : INT
VAR_INPUT
    value : INT;
END_VAR
Identity := value;
END_FUNCTION
END_NAMESPACE
FUNCTION Caller : INT
USING Support;
Caller := Identity(INT#3);
END_FUNCTION
PROGRAM Main
END_PROGRAM
"#,
    );
    assert!(runtime
        .functions()
        .values()
        .any(|function| function.name == "Support.Identity"));
    let caller = runtime
        .functions()
        .values()
        .find(|function| function.name == "Caller")
        .expect("Caller");
    assert_eq!(caller.using.as_slice(), ["Support"]);
}

#[test]
fn pou_function_contract_multiple_declared_names_keep_source_order() {
    let runtime = runtime(
        r#"
FUNCTION Sum : INT
VAR_INPUT
    first, second, third : INT;
END_VAR
Sum := first + second + third;
END_FUNCTION
PROGRAM Main
END_PROGRAM
"#,
    );
    let function = runtime
        .functions()
        .values()
        .find(|function| function.name == "Sum")
        .expect("Sum");
    assert_eq!(
        function
            .params
            .iter()
            .map(|param| param.name.as_str())
            .collect::<Vec<_>>(),
        ["first", "second", "third"]
    );
}
