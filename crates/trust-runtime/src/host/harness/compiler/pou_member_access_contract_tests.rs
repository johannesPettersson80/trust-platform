use crate::harness::{CompileSession, TestHarness};
use crate::value::Value;

fn run(source: &str) -> TestHarness {
    let mut harness = TestHarness::from_source(source)
        .unwrap_or_else(|error| panic!("member-access fixture must compile: {error}"));
    let cycle = harness.cycle();
    assert!(cycle.errors.is_empty(), "{:?}", cycle.errors);
    harness
}

fn compile_error(source: &str) -> String {
    match CompileSession::from_source(source).build_runtime() {
        Ok(_) => panic!("member-access fixture must fail"),
        Err(error) => error.to_string(),
    }
}

#[test]
fn pou_member_access_public_class_field_method_and_property_execute() {
    let harness = run(r#"
CLASS Device
VAR PUBLIC
    Stored : INT;
END_VAR
METHOD PUBLIC Read : INT
Read := Stored;
END_METHOD
PUBLIC PROPERTY Value : INT
GET Value := Stored; END_GET
SET Stored := Value; END_SET
END_PROPERTY
END_CLASS
PROGRAM Main
VAR
    DeviceInstance : Device;
    ResultValue : INT;
END_VAR
DeviceInstance.Stored := INT#2;
DeviceInstance.Value := INT#7;
ResultValue := DeviceInstance.Read() + DeviceInstance.Value;
END_PROGRAM
"#);
    assert_eq!(harness.get_output("ResultValue"), Some(Value::Int(14)));
}

#[test]
fn pou_member_access_protected_class_members_execute_through_derived_wrapper() {
    let harness = run(r#"
CLASS Base
VAR PROTECTED
    Stored : INT;
END_VAR
METHOD PROTECTED Add
Stored := Stored + INT#2;
END_METHOD
PROTECTED PROPERTY Value : INT
GET Value := Stored; END_GET
SET Stored := Value; END_SET
END_PROPERTY
END_CLASS
CLASS Derived EXTENDS Base
METHOD PUBLIC Exercise : INT
THIS.Value := INT#5;
SUPER.Add();
Exercise := Stored + THIS.Value;
END_METHOD
END_CLASS
PROGRAM Main
VAR
    DeviceInstance : Derived;
    ResultValue : INT;
END_VAR
ResultValue := DeviceInstance.Exercise();
END_PROGRAM
"#);
    assert_eq!(harness.get_output("ResultValue"), Some(Value::Int(14)));
}

#[test]
fn pou_member_access_private_class_members_execute_inside_owner() {
    let harness = run(r#"
CLASS Vault
VAR PRIVATE
    Stored : INT;
END_VAR
METHOD PRIVATE Add
Stored := Stored + INT#3;
END_METHOD
METHOD PUBLIC Exercise : INT
Stored := INT#4;
THIS.Add();
Exercise := Stored;
END_METHOD
END_CLASS
PROGRAM Main
VAR
    VaultInstance : Vault;
    ResultValue : INT;
END_VAR
ResultValue := VaultInstance.Exercise();
END_PROGRAM
"#);
    assert_eq!(harness.get_output("ResultValue"), Some(Value::Int(7)));
}

#[test]
fn pou_member_access_internal_class_members_execute_in_same_namespace() {
    let harness = run(r#"
NAMESPACE Cell
CLASS Device
VAR INTERNAL
    Stored : INT;
END_VAR
METHOD INTERNAL Read : INT
Read := Stored;
END_METHOD
END_CLASS
PROGRAM Worker
VAR
    DeviceInstance : Device;
    ResultValue : INT;
END_VAR
DeviceInstance.Stored := INT#8;
ResultValue := DeviceInstance.Read();
END_PROGRAM
END_NAMESPACE
PROGRAM Main
END_PROGRAM
"#);
    assert_eq!(
        harness.get_output("Cell.Worker.ResultValue"),
        Some(Value::Int(8))
    );
}

#[test]
fn pou_member_access_public_override_dispatches_derived_method() {
    let harness = run(r#"
CLASS Base
METHOD PUBLIC Read : INT
Read := INT#1;
END_METHOD
END_CLASS
CLASS Derived EXTENDS Base
METHOD PUBLIC OVERRIDE Read : INT
Read := INT#9;
END_METHOD
END_CLASS
PROGRAM Main
VAR
    DeviceInstance : Derived;
    ResultValue : INT;
END_VAR
ResultValue := DeviceInstance.Read();
END_PROGRAM
"#);
    assert_eq!(harness.get_output("ResultValue"), Some(Value::Int(9)));
}

#[test]
fn pou_member_access_protected_override_dispatches_through_public_wrapper() {
    let harness = run(r#"
CLASS Base
METHOD PROTECTED Read : INT
Read := INT#1;
END_METHOD
END_CLASS
CLASS Derived EXTENDS Base
METHOD PROTECTED OVERRIDE Read : INT
Read := INT#6;
END_METHOD
METHOD PUBLIC Exercise : INT
Exercise := THIS.Read();
END_METHOD
END_CLASS
PROGRAM Main
VAR
    DeviceInstance : Derived;
    ResultValue : INT;
END_VAR
ResultValue := DeviceInstance.Exercise();
END_PROGRAM
"#);
    assert_eq!(harness.get_output("ResultValue"), Some(Value::Int(6)));
}

#[test]
fn pou_member_access_internal_override_dispatches_in_declaring_namespace() {
    let harness = run(r#"
NAMESPACE Cell
CLASS Base
METHOD INTERNAL Read : INT
Read := INT#1;
END_METHOD
END_CLASS
CLASS Derived EXTENDS Base
METHOD INTERNAL OVERRIDE Read : INT
Read := INT#5;
END_METHOD
METHOD PUBLIC Exercise : INT
Exercise := THIS.Read();
END_METHOD
END_CLASS
END_NAMESPACE
PROGRAM Main
VAR
    DeviceInstance : Cell.Derived;
    ResultValue : INT;
END_VAR
ResultValue := DeviceInstance.Exercise();
END_PROGRAM
"#);
    assert_eq!(harness.get_output("ResultValue"), Some(Value::Int(5)));
}

#[test]
fn pou_member_access_default_fb_members_execute_through_derived_wrapper() {
    let harness = run(r#"
FUNCTION_BLOCK Base
VAR
    Stored : INT;
END_VAR
METHOD Read : INT
Read := Stored;
END_METHOD
END_FUNCTION_BLOCK
FUNCTION_BLOCK Derived EXTENDS Base
METHOD PUBLIC Exercise : INT
Stored := INT#4;
Exercise := THIS.Read();
END_METHOD
END_FUNCTION_BLOCK
PROGRAM Main
VAR
    DeviceInstance : Derived;
    ResultValue : INT;
END_VAR
ResultValue := DeviceInstance.Exercise();
END_PROGRAM
"#);
    assert_eq!(harness.get_output("ResultValue"), Some(Value::Int(4)));
}

#[test]
fn pou_member_access_public_fb_method_executes_from_program() {
    let harness = run(r#"
FUNCTION_BLOCK Device
VAR PUBLIC
    Stored : INT;
END_VAR
METHOD PUBLIC Add : INT
VAR_INPUT
    Delta : INT;
END_VAR
Stored := Stored + Delta;
Add := Stored;
END_METHOD
END_FUNCTION_BLOCK
PROGRAM Main
VAR
    DeviceInstance : Device;
    ResultValue : INT;
END_VAR
DeviceInstance.Stored := INT#2;
ResultValue := DeviceInstance.Add(INT#5);
END_PROGRAM
"#);
    assert_eq!(harness.get_output("ResultValue"), Some(Value::Int(7)));
}

#[test]
fn pou_member_access_fb_output_is_readable_after_execution() {
    let harness = run(r#"
FUNCTION_BLOCK Producer
VAR_OUTPUT
    Value : INT;
END_VAR
Value := INT#8;
END_FUNCTION_BLOCK
PROGRAM Main
VAR
    ProducerInstance : Producer;
    ResultValue : INT;
END_VAR
ProducerInstance();
ResultValue := ProducerInstance.Value;
END_PROGRAM
"#);
    assert_eq!(harness.get_output("ResultValue"), Some(Value::Int(8)));
}

#[test]
fn pou_member_access_fb_input_is_readable_after_call_binding() {
    let harness = run(r#"
FUNCTION_BLOCK Consumer
VAR_INPUT
    Value : INT;
END_VAR
END_FUNCTION_BLOCK
PROGRAM Main
VAR
    ConsumerInstance : Consumer;
    ResultValue : INT;
END_VAR
ConsumerInstance(Value := INT#11);
ResultValue := ConsumerInstance.Value;
END_PROGRAM
"#);
    assert_eq!(harness.get_output("ResultValue"), Some(Value::Int(11)));
}

#[test]
fn pou_member_access_fb_in_out_mutates_only_through_call_binding() {
    let harness = run(r#"
FUNCTION_BLOCK Bump
VAR_IN_OUT
    Value : INT;
END_VAR
Value := Value + INT#1;
END_FUNCTION_BLOCK
PROGRAM Main
VAR
    BumpInstance : Bump;
    Stored : INT := INT#4;
    ResultValue : INT;
END_VAR
BumpInstance(Value := Stored);
ResultValue := Stored;
END_PROGRAM
"#);
    assert_eq!(harness.get_output("ResultValue"), Some(Value::Int(5)));
}

#[test]
fn pou_member_access_public_interface_dispatch_reaches_concrete_method() {
    let harness = run(r#"
INTERFACE IReadable
METHOD Read : INT
END_METHOD
END_INTERFACE
CLASS Device IMPLEMENTS IReadable
METHOD PUBLIC Read : INT
Read := INT#12;
END_METHOD
END_CLASS
PROGRAM Main
VAR
    DeviceInstance : Device;
    Contract : IReadable;
    ResultValue : INT;
END_VAR
Contract := DeviceInstance;
ResultValue := Contract.Read();
END_PROGRAM
"#);
    assert_eq!(harness.get_output("ResultValue"), Some(Value::Int(12)));
}

#[test]
fn pou_member_access_internal_interface_implementation_dispatches_via_public_contract() {
    let harness = run(r#"
NAMESPACE Cell
INTERFACE IReadable
METHOD Read : INT
END_METHOD
END_INTERFACE
CLASS Device IMPLEMENTS IReadable
METHOD INTERNAL Read : INT
Read := INT#13;
END_METHOD
END_CLASS
PROGRAM Worker
VAR
    DeviceInstance : Device;
    Contract : IReadable;
    ResultValue : INT;
END_VAR
Contract := DeviceInstance;
ResultValue := Contract.Read();
END_PROGRAM
END_NAMESPACE
PROGRAM Main
END_PROGRAM
"#);
    assert_eq!(
        harness.get_output("Cell.Worker.ResultValue"),
        Some(Value::Int(13))
    );
}

#[test]
fn pou_member_access_storage_then_visibility_order_lowers_public_state() {
    let harness = run(r#"
CLASS Device
VAR RETAIN PUBLIC
    Stored : INT := INT#3;
END_VAR
END_CLASS
PROGRAM Main
VAR
    DeviceInstance : Device;
    ResultValue : INT;
END_VAR
ResultValue := DeviceInstance.Stored;
END_PROGRAM
"#);
    assert_eq!(harness.get_output("ResultValue"), Some(Value::Int(3)));
}

#[test]
fn pou_member_access_visibility_then_storage_order_lowers_public_state() {
    let harness = run(r#"
CLASS Device
VAR PUBLIC RETAIN
    Stored : INT := INT#4;
END_VAR
END_CLASS
PROGRAM Main
VAR
    DeviceInstance : Device;
    ResultValue : INT;
END_VAR
ResultValue := DeviceInstance.Stored;
END_PROGRAM
"#);
    assert_eq!(harness.get_output("ResultValue"), Some(Value::Int(4)));
}

#[test]
fn pou_member_access_private_external_call_produces_no_runtime_model() {
    let error = compile_error(
        r#"
CLASS Vault
METHOD PRIVATE Read : INT
Read := INT#1;
END_METHOD
END_CLASS
PROGRAM Main
VAR
    VaultInstance : Vault;
    ResultValue : INT;
END_VAR
ResultValue := VaultInstance.Read();
END_PROGRAM
"#,
    );
    assert!(
        error.contains("PRIVATE") || error.contains("access"),
        "{error}"
    );
}

#[test]
fn pou_member_access_protected_external_call_produces_no_runtime_model() {
    let error = compile_error(
        r#"
CLASS Device
METHOD PROTECTED Read : INT
Read := INT#1;
END_METHOD
END_CLASS
PROGRAM Main
VAR
    DeviceInstance : Device;
    ResultValue : INT;
END_VAR
ResultValue := DeviceInstance.Read();
END_PROGRAM
"#,
    );
    assert!(
        error.contains("PROTECTED") || error.contains("access"),
        "{error}"
    );
}

#[test]
fn pou_member_access_internal_cross_namespace_call_produces_no_runtime_model() {
    let error = compile_error(
        r#"
NAMESPACE Cell
CLASS Device
METHOD INTERNAL Read : INT
Read := INT#1;
END_METHOD
END_CLASS
END_NAMESPACE
PROGRAM Main
VAR
    DeviceInstance : Cell.Device;
    ResultValue : INT;
END_VAR
ResultValue := DeviceInstance.Read();
END_PROGRAM
"#,
    );
    assert!(
        error.contains("INTERNAL") || error.contains("access"),
        "{error}"
    );
}

#[test]
fn pou_member_access_external_fb_output_write_produces_no_runtime_model() {
    let error = compile_error(
        r#"
FUNCTION_BLOCK Producer
VAR_OUTPUT
    Value : INT;
END_VAR
END_FUNCTION_BLOCK
PROGRAM Main
VAR
    ProducerInstance : Producer;
END_VAR
ProducerInstance.Value := INT#9;
END_PROGRAM
"#,
    );
    assert!(
        error.contains("output") || error.contains("assign"),
        "{error}"
    );
}
