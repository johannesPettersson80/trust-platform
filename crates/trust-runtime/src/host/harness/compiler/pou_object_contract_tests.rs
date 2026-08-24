use crate::harness::{CompileSession, TestHarness};
use crate::program_model::{property_setter_method_name, FunctionBlockBase};
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
fn pou_object_contract_fb_parameter_directions_keep_declaration_order() {
    let runtime = runtime(
        r#"
FUNCTION_BLOCK Transfer
VAR_INPUT
    source : INT := INT#2;
END_VAR
VAR_OUTPUT
    copied : INT;
END_VAR
VAR_IN_OUT
    target : INT;
END_VAR
END_FUNCTION_BLOCK
PROGRAM Main
VAR
    transfer : Transfer;
END_VAR
END_PROGRAM
"#,
    );
    let fb = runtime
        .function_blocks()
        .values()
        .find(|fb| fb.name == "Transfer")
        .expect("Transfer");
    assert_eq!(
        fb.params
            .iter()
            .map(|param| (param.name.as_str(), param.direction))
            .collect::<Vec<_>>(),
        vec![
            ("source", ParamDirection::In),
            ("copied", ParamDirection::Out),
            ("target", ParamDirection::InOut)
        ]
    );
    assert!(fb.params[0].default.is_some());
}

#[test]
fn pou_object_contract_fb_var_persists_across_calls_and_cycles() {
    let mut harness = run(r#"
FUNCTION_BLOCK Counter
VAR_OUTPUT
    count : INT;
END_VAR
count := count + INT#1;
END_FUNCTION_BLOCK
PROGRAM Main
VAR
    counter : Counter;
    first : INT;
    second : INT;
END_VAR
counter();
first := counter.count;
counter();
second := counter.count;
END_PROGRAM
"#);
    assert_eq!(harness.get_output("first"), Some(Value::Int(1)));
    assert_eq!(harness.get_output("second"), Some(Value::Int(2)));
    harness.cycle();
    assert_eq!(harness.get_output("first"), Some(Value::Int(3)));
    assert_eq!(harness.get_output("second"), Some(Value::Int(4)));
}

#[test]
fn pou_object_contract_separate_fb_instances_do_not_share_state() {
    let harness = run(r#"
FUNCTION_BLOCK Counter
VAR_OUTPUT
    count : INT;
END_VAR
count := count + INT#1;
END_FUNCTION_BLOCK
PROGRAM Main
VAR
    firstCounter : Counter;
    secondCounter : Counter;
    first : INT;
    second : INT;
END_VAR
firstCounter();
firstCounter();
secondCounter();
first := firstCounter.count;
second := secondCounter.count;
END_PROGRAM
"#);
    assert_eq!(harness.get_output("first"), Some(Value::Int(2)));
    assert_eq!(harness.get_output("second"), Some(Value::Int(1)));
}

#[test]
fn pou_object_contract_fb_temp_resets_for_each_call() {
    let harness = run(r#"
FUNCTION_BLOCK Counter
VAR_OUTPUT
    outputValue : INT;
END_VAR
VAR_TEMP
    delta : INT;
END_VAR
delta := delta + INT#1;
outputValue := outputValue + delta;
END_FUNCTION_BLOCK
PROGRAM Main
VAR
    counter : Counter;
    result : INT;
END_VAR
counter();
counter();
result := counter.outputValue;
END_PROGRAM
"#);
    assert_eq!(harness.get_output("result"), Some(Value::Int(2)));
}

#[test]
fn pou_object_contract_omitted_fb_input_reuses_last_stored_value() {
    let harness = run(r#"
FUNCTION_BLOCK Echo
VAR_INPUT
    value : INT := INT#2;
END_VAR
VAR_OUTPUT
    outputValue : INT;
END_VAR
outputValue := value;
END_FUNCTION_BLOCK
PROGRAM Main
VAR
    echo : Echo;
    first : INT;
    second : INT;
END_VAR
echo(value := INT#7);
first := echo.outputValue;
echo();
second := echo.outputValue;
END_PROGRAM
"#);
    assert_eq!(harness.get_output("first"), Some(Value::Int(7)));
    assert_eq!(harness.get_output("second"), Some(Value::Int(7)));
}

#[test]
fn pou_object_contract_first_omitted_fb_input_uses_declared_default() {
    let harness = run(r#"
FUNCTION_BLOCK Echo
VAR_INPUT
    value : INT := INT#6;
END_VAR
VAR_OUTPUT
    outputValue : INT;
END_VAR
outputValue := value;
END_FUNCTION_BLOCK
PROGRAM Main
VAR
    echo : Echo;
    result : INT;
END_VAR
echo();
result := echo.outputValue;
END_PROGRAM
"#);
    assert_eq!(harness.get_output("result"), Some(Value::Int(6)));
}

#[test]
fn pou_object_contract_fb_var_external_mutates_existing_global() {
    let harness = run(r#"
VAR_GLOBAL
    shared : INT;
END_VAR
FUNCTION_BLOCK Bump
VAR_EXTERNAL
    shared : INT;
END_VAR
shared := shared + INT#1;
END_FUNCTION_BLOCK
PROGRAM Main
VAR
    bump : Bump;
END_VAR
bump();
END_PROGRAM
"#);
    assert_eq!(
        harness.runtime().storage().get_global("shared"),
        Some(&Value::Int(1))
    );
}

#[test]
fn pou_object_contract_fb_extends_function_block_and_super_dispatches() {
    let harness = run(r#"
FUNCTION_BLOCK Base
VAR PUBLIC
    count : INT := INT#10;
END_VAR
METHOD PUBLIC Read : INT
Read := count;
END_METHOD
END_FUNCTION_BLOCK
FUNCTION_BLOCK Derived EXTENDS Base
METHOD PUBLIC ReadBase : INT
ReadBase := SUPER.Read();
END_METHOD
END_FUNCTION_BLOCK
PROGRAM Main
VAR
    derived : Derived;
    result : INT;
END_VAR
result := derived.ReadBase();
END_PROGRAM
"#);
    assert_eq!(harness.get_output("result"), Some(Value::Int(10)));
}

#[test]
fn pou_object_contract_fb_extends_class_and_inherits_field() {
    let runtime = runtime(
        r#"
CLASS Base
VAR PUBLIC
    baseValue : INT := INT#5;
END_VAR
END_CLASS
FUNCTION_BLOCK Derived EXTENDS Base
END_FUNCTION_BLOCK
PROGRAM Main
VAR
    derived : Derived;
END_VAR
END_PROGRAM
"#,
    );
    let derived = runtime
        .function_blocks()
        .values()
        .find(|fb| fb.name == "Derived")
        .expect("Derived");
    assert!(matches!(
        &derived.base,
        Some(FunctionBlockBase::Class(name)) if name == "Base"
    ));
}

#[test]
fn pou_object_contract_fb_rejects_non_object_base_type() {
    let error = compile_error(
        r#"
TYPE Plain :
STRUCT
    value : INT;
END_STRUCT
END_TYPE
FUNCTION_BLOCK Broken EXTENDS Plain
END_FUNCTION_BLOCK
PROGRAM Main
END_PROGRAM
"#,
    );
    assert!(
        error.contains("EXTENDS") || error.contains("base"),
        "{error}"
    );
}

#[test]
fn pou_object_contract_class_extends_base_fields() {
    let harness = run(r#"
CLASS Base
VAR PUBLIC
    baseValue : INT := INT#1;
END_VAR
END_CLASS
CLASS Derived EXTENDS Base
VAR PUBLIC
    childValue : INT := INT#2;
END_VAR
END_CLASS
PROGRAM Main
VAR
    object : Derived;
    result : INT;
END_VAR
result := object.baseValue + object.childValue;
END_PROGRAM
"#);
    assert_eq!(harness.get_output("result"), Some(Value::Int(3)));
}

#[test]
fn pou_object_contract_class_method_mutates_owning_instance() {
    let harness = run(r#"
CLASS Counter
VAR PUBLIC
    value : INT;
END_VAR
METHOD PUBLIC Add : INT
VAR_INPUT
    delta : INT;
END_VAR
value := value + delta;
Add := value;
END_METHOD
END_CLASS
PROGRAM Main
VAR
    counter : Counter;
    result : INT;
END_VAR
counter.Add(INT#2);
result := counter.Add(INT#3);
END_PROGRAM
"#);
    assert_eq!(harness.get_output("result"), Some(Value::Int(5)));
}

#[test]
fn pou_object_contract_method_static_is_isolated_per_instance() {
    let harness = run(r#"
CLASS Counter
METHOD PUBLIC Next : INT
VAR_STAT
    count : INT;
END_VAR
count := count + INT#1;
Next := count;
END_METHOD
END_CLASS
PROGRAM Main
VAR
    firstCounter : Counter;
    secondCounter : Counter;
    first : INT;
    second : INT;
END_VAR
firstCounter.Next();
first := firstCounter.Next();
second := secondCounter.Next();
END_PROGRAM
"#);
    assert_eq!(harness.get_output("first"), Some(Value::Int(2)));
    assert_eq!(harness.get_output("second"), Some(Value::Int(1)));
}

#[test]
fn pou_object_contract_property_lowering_synthesizes_getter_and_setter() {
    let runtime = runtime(
        r#"
CLASS Device
VAR PRIVATE
    stored : INT;
END_VAR
PUBLIC PROPERTY Level : INT
GET
    Level := stored;
END_GET
SET
    stored := Level;
END_SET
END_PROPERTY
END_CLASS
PROGRAM Main
VAR
    device : Device;
END_VAR
END_PROGRAM
"#,
    );
    let class = runtime
        .classes()
        .values()
        .find(|class| class.name == "Device")
        .expect("Device");
    assert!(class.methods.iter().any(|method| method.name == "Level"));
    let setter_name = property_setter_method_name(&"Level".into());
    let setter = class
        .methods
        .iter()
        .find(|method| method.name == setter_name)
        .expect("property setter");
    assert_eq!(setter.params.len(), 1);
    assert_eq!(setter.params[0].name, "Level");
}

#[test]
fn pou_object_contract_property_assignment_and_read_call_accessors() {
    let harness = run(r#"
CLASS Device
VAR PRIVATE
    stored : INT;
END_VAR
PUBLIC PROPERTY Level : INT
GET
    Level := stored;
END_GET
SET
    stored := Level;
END_SET
END_PROPERTY
END_CLASS
PROGRAM Main
VAR
    device : Device;
    result : INT;
END_VAR
device.Level := INT#9;
result := device.Level;
END_PROGRAM
"#);
    assert_eq!(harness.get_output("result"), Some(Value::Int(9)));
}

#[test]
fn pou_object_contract_interface_dispatch_reaches_concrete_method() {
    let harness = run(r#"
INTERFACE ICounter
METHOD Next : INT
END_METHOD
END_INTERFACE
CLASS Counter IMPLEMENTS ICounter
VAR PUBLIC
    value : INT;
END_VAR
METHOD PUBLIC Next : INT
value := value + INT#1;
Next := value;
END_METHOD
END_CLASS
PROGRAM Main
VAR
    Impl : Counter;
    P : ICounter;
    ResultValue : INT;
END_VAR
P := Impl;
ResultValue := P.Next();
END_PROGRAM
"#);
    assert_eq!(harness.get_output("ResultValue"), Some(Value::Int(1)));
}

#[test]
fn pou_object_contract_interface_base_identity_is_preserved() {
    let runtime = runtime(
        r#"
INTERFACE IBase
METHOD Read : INT
END_METHOD
END_INTERFACE
INTERFACE IDerived EXTENDS IBase
METHOD Write
END_METHOD
END_INTERFACE
PROGRAM Main
END_PROGRAM
"#,
    );
    let derived = runtime
        .interfaces()
        .values()
        .find(|interface| interface.name == "IDerived")
        .expect("IDerived");
    assert_eq!(derived.base.as_deref(), Some("IBase"));
    assert!(derived.methods.iter().any(|method| method.name == "Write"));
}

#[test]
fn pou_object_contract_program_var_persists_and_temp_resets_each_cycle() {
    let mut harness = run(r#"
PROGRAM Main
VAR
    StoredValue : INT;
END_VAR
VAR_TEMP
    TempValue : INT;
END_VAR
StoredValue := StoredValue + INT#1;
TempValue := TempValue + INT#1;
StoredValue := StoredValue + TempValue;
END_PROGRAM
"#);
    assert_eq!(harness.get_output("StoredValue"), Some(Value::Int(2)));
    harness.cycle();
    assert_eq!(harness.get_output("StoredValue"), Some(Value::Int(4)));
}

#[test]
fn pou_object_contract_program_global_is_lifted_and_external_is_not_duplicated() {
    let harness = run(r#"
VAR_GLOBAL
    shared : INT := INT#3;
END_VAR
PROGRAM Main
VAR_GLOBAL
    programGlobal : INT := INT#4;
END_VAR
VAR_EXTERNAL
    shared : INT;
END_VAR
shared := shared + programGlobal;
END_PROGRAM
"#);
    assert_eq!(
        harness.runtime().storage().get_global("shared"),
        Some(&Value::Int(7))
    );
    assert_eq!(
        harness.runtime().storage().get_global("programGlobal"),
        Some(&Value::Int(4))
    );
}

#[test]
fn pou_object_contract_program_wildcard_input_is_rejected() {
    let error = compile_error(
        r#"
PROGRAM Main
VAR_INPUT
    port AT %I* : BOOL;
END_VAR
END_PROGRAM
"#,
    );
    assert!(
        error.contains("wildcard") || error.contains("VAR_CONFIG"),
        "{error}"
    );
}

#[test]
fn pou_object_contract_namespace_qualifies_object_pous() {
    let runtime = runtime(
        r#"
NAMESPACE Devices
INTERFACE IDevice
METHOD Read : INT
END_METHOD
END_INTERFACE
CLASS Device
END_CLASS
FUNCTION_BLOCK Controller
END_FUNCTION_BLOCK
END_NAMESPACE
PROGRAM Main
END_PROGRAM
"#,
    );
    assert!(runtime
        .interfaces()
        .values()
        .any(|item| item.name == "Devices.IDevice"));
    assert!(runtime
        .classes()
        .values()
        .any(|item| item.name == "Devices.Device"));
    assert!(runtime
        .function_blocks()
        .values()
        .any(|item| item.name == "Devices.Controller"));
}
