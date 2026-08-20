use super::common::*;

// Use-site rejection matrix for IEC 61131-3 Ed.3 Tables 48/53, Figure 29,
// and §§6.6.5.5, 6.6.5.9-6.6.5.10, 6.6.7.2.3, 6.6.7.6-6.6.7.7.

fn assert_invalid(source: &str) {
    check_has_error(source, DiagnosticCode::InvalidOperation);
}

#[test]
fn member_access_default_class_variable_rejects_external_access() {
    assert_invalid(
        r#"
CLASS Device
VAR
    Stored : INT;
END_VAR
END_CLASS
PROGRAM Main
VAR
    DeviceInstance : Device;
END_VAR
DeviceInstance.Stored := INT#1;
END_PROGRAM
"#,
    );
}

#[test]
fn member_access_default_class_method_rejects_external_call() {
    assert_invalid(
        r#"
CLASS Device
METHOD Read : INT
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
}

#[test]
fn member_access_default_class_property_rejects_external_read() {
    assert_invalid(
        r#"
CLASS Device
PROPERTY Value : INT
GET Value := INT#1; END_GET
END_PROPERTY
END_CLASS
PROGRAM Main
VAR
    DeviceInstance : Device;
    ResultValue : INT;
END_VAR
ResultValue := DeviceInstance.Value;
END_PROGRAM
"#,
    );
}

#[test]
fn member_access_default_fb_variable_rejects_external_access() {
    assert_invalid(
        r#"
FUNCTION_BLOCK Device
VAR
    Stored : INT;
END_VAR
END_FUNCTION_BLOCK
PROGRAM Main
VAR
    DeviceInstance : Device;
END_VAR
DeviceInstance.Stored := INT#1;
END_PROGRAM
"#,
    );
}

#[test]
fn member_access_default_fb_method_rejects_external_call() {
    assert_invalid(
        r#"
FUNCTION_BLOCK Device
METHOD Read : INT
Read := INT#1;
END_METHOD
END_FUNCTION_BLOCK
PROGRAM Main
VAR
    DeviceInstance : Device;
    ResultValue : INT;
END_VAR
ResultValue := DeviceInstance.Read();
END_PROGRAM
"#,
    );
}

#[test]
fn member_access_default_fb_property_rejects_external_write() {
    assert_invalid(
        r#"
FUNCTION_BLOCK Device
PROPERTY Value : INT
SET END_SET
END_PROPERTY
END_FUNCTION_BLOCK
PROGRAM Main
VAR
    DeviceInstance : Device;
END_VAR
DeviceInstance.Value := INT#1;
END_PROGRAM
"#,
    );
}

#[test]
fn member_access_private_variable_rejects_derived_access() {
    assert_invalid(
        r#"
CLASS Base
VAR PRIVATE
    Stored : INT;
END_VAR
END_CLASS
CLASS Derived EXTENDS Base
METHOD PUBLIC Exercise
Stored := INT#1;
END_METHOD
END_CLASS
"#,
    );
}

#[test]
fn member_access_private_variable_rejects_super_access() {
    assert_invalid(
        r#"
CLASS Base
VAR PRIVATE
    Stored : INT;
END_VAR
END_CLASS
CLASS Derived EXTENDS Base
METHOD PUBLIC Exercise
SUPER.Stored := INT#1;
END_METHOD
END_CLASS
"#,
    );
}

#[test]
fn member_access_private_method_rejects_derived_call() {
    assert_invalid(
        r#"
CLASS Base
METHOD PRIVATE Read : INT
Read := INT#1;
END_METHOD
END_CLASS
CLASS Derived EXTENDS Base
METHOD PUBLIC Exercise : INT
Exercise := THIS.Read();
END_METHOD
END_CLASS
"#,
    );
}

#[test]
fn member_access_private_property_rejects_derived_read() {
    assert_invalid(
        r#"
CLASS Base
PRIVATE PROPERTY Value : INT
GET Value := INT#1; END_GET
END_PROPERTY
END_CLASS
CLASS Derived EXTENDS Base
METHOD PUBLIC Exercise : INT
Exercise := THIS.Value;
END_METHOD
END_CLASS
"#,
    );
}

#[test]
fn member_access_private_property_rejects_derived_write() {
    assert_invalid(
        r#"
CLASS Base
PRIVATE PROPERTY Value : INT
SET END_SET
END_PROPERTY
END_CLASS
CLASS Derived EXTENDS Base
METHOD PUBLIC Exercise
THIS.Value := INT#1;
END_METHOD
END_CLASS
"#,
    );
}

#[test]
fn member_access_protected_variable_rejects_unrelated_same_namespace_access() {
    assert_invalid(
        r#"
NAMESPACE Cell
CLASS Device
VAR PROTECTED
    Stored : INT;
END_VAR
END_CLASS
PROGRAM Main
VAR
    DeviceInstance : Device;
END_VAR
DeviceInstance.Stored := INT#1;
END_PROGRAM
END_NAMESPACE
"#,
    );
}

#[test]
fn member_access_protected_method_rejects_unrelated_same_namespace_call() {
    assert_invalid(
        r#"
NAMESPACE Cell
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
END_NAMESPACE
"#,
    );
}

#[test]
fn member_access_protected_property_rejects_unrelated_same_namespace_read() {
    assert_invalid(
        r#"
NAMESPACE Cell
CLASS Device
PROTECTED PROPERTY Value : INT
GET Value := INT#1; END_GET
END_PROPERTY
END_CLASS
PROGRAM Main
VAR
    DeviceInstance : Device;
    ResultValue : INT;
END_VAR
ResultValue := DeviceInstance.Value;
END_PROGRAM
END_NAMESPACE
"#,
    );
}

#[test]
fn member_access_protected_member_rejects_external_access_through_derived_instance() {
    assert_invalid(
        r#"
CLASS Base
VAR PROTECTED
    Stored : INT;
END_VAR
END_CLASS
CLASS Derived EXTENDS Base
END_CLASS
PROGRAM Main
VAR
    DeviceInstance : Derived;
END_VAR
DeviceInstance.Stored := INT#1;
END_PROGRAM
"#,
    );
}

#[test]
fn member_access_internal_variable_rejects_global_access() {
    assert_invalid(
        r#"
NAMESPACE Cell
CLASS Device
VAR INTERNAL
    Stored : INT;
END_VAR
END_CLASS
END_NAMESPACE
PROGRAM Main
VAR
    DeviceInstance : Cell.Device;
END_VAR
DeviceInstance.Stored := INT#1;
END_PROGRAM
"#,
    );
}

#[test]
fn member_access_internal_method_rejects_global_call() {
    assert_invalid(
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
}

#[test]
fn member_access_internal_property_rejects_global_read() {
    assert_invalid(
        r#"
NAMESPACE Cell
CLASS Device
INTERNAL PROPERTY Value : INT
GET Value := INT#1; END_GET
END_PROPERTY
END_CLASS
END_NAMESPACE
PROGRAM Main
VAR
    DeviceInstance : Cell.Device;
    ResultValue : INT;
END_VAR
ResultValue := DeviceInstance.Value;
END_PROGRAM
"#,
    );
}

#[test]
fn member_access_internal_member_rejects_parent_namespace_access() {
    assert_invalid(
        r#"
NAMESPACE Outer.Inner
CLASS Device
METHOD INTERNAL Read : INT
Read := INT#1;
END_METHOD
END_CLASS
END_NAMESPACE
NAMESPACE Outer
PROGRAM Main
VAR
    DeviceInstance : Inner.Device;
    ResultValue : INT;
END_VAR
ResultValue := DeviceInstance.Read();
END_PROGRAM
END_NAMESPACE
"#,
    );
}

#[test]
fn member_access_internal_member_rejects_child_namespace_access() {
    assert_invalid(
        r#"
NAMESPACE Outer
CLASS Device
METHOD INTERNAL Read : INT
Read := INT#1;
END_METHOD
END_CLASS
END_NAMESPACE
NAMESPACE Outer.Inner
PROGRAM Main
VAR
    DeviceInstance : Outer.Device;
    ResultValue : INT;
END_VAR
ResultValue := DeviceInstance.Read();
END_PROGRAM
END_NAMESPACE
"#,
    );
}

#[test]
fn member_access_internal_member_rejects_sibling_namespace_access() {
    assert_invalid(
        r#"
NAMESPACE A
CLASS Device
METHOD INTERNAL Read : INT
Read := INT#1;
END_METHOD
END_CLASS
END_NAMESPACE
NAMESPACE B
PROGRAM Main
VAR
    DeviceInstance : A.Device;
    ResultValue : INT;
END_VAR
ResultValue := DeviceInstance.Read();
END_PROGRAM
END_NAMESPACE
"#,
    );
}

#[test]
fn member_access_internal_variable_is_not_inherited_across_namespace() {
    assert_invalid(
        r#"
NAMESPACE A
CLASS Base
VAR INTERNAL
    Stored : INT;
END_VAR
END_CLASS
END_NAMESPACE
NAMESPACE B
CLASS Derived EXTENDS A.Base
METHOD PUBLIC Exercise
Stored := INT#1;
END_METHOD
END_CLASS
END_NAMESPACE
"#,
    );
}

#[test]
fn member_access_internal_method_is_not_inherited_across_namespace() {
    assert_invalid(
        r#"
NAMESPACE A
CLASS Base
METHOD INTERNAL Read : INT
Read := INT#1;
END_METHOD
END_CLASS
END_NAMESPACE
NAMESPACE B
CLASS Derived EXTENDS A.Base
METHOD PUBLIC Exercise : INT
Exercise := THIS.Read();
END_METHOD
END_CLASS
END_NAMESPACE
"#,
    );
}

#[test]
fn member_access_fb_output_rejects_external_assignment() {
    assert_invalid(
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
}

#[test]
fn member_access_fb_in_out_rejects_external_member_read() {
    assert_invalid(
        r#"
FUNCTION_BLOCK Bump
VAR_IN_OUT
    Value : INT;
END_VAR
END_FUNCTION_BLOCK
PROGRAM Main
VAR
    BumpInstance : Bump;
    ResultValue : INT;
END_VAR
ResultValue := BumpInstance.Value;
END_PROGRAM
"#,
    );
}

#[test]
fn member_access_fb_in_out_rejects_external_member_write() {
    assert_invalid(
        r#"
FUNCTION_BLOCK Bump
VAR_IN_OUT
    Value : INT;
END_VAR
END_FUNCTION_BLOCK
PROGRAM Main
VAR
    BumpInstance : Bump;
END_VAR
BumpInstance.Value := INT#1;
END_PROGRAM
"#,
    );
}

#[test]
fn member_access_fb_temp_rejects_external_member_read() {
    assert_invalid(
        r#"
FUNCTION_BLOCK Device
VAR_TEMP
    Scratch : INT;
END_VAR
END_FUNCTION_BLOCK
PROGRAM Main
VAR
    DeviceInstance : Device;
    ResultValue : INT;
END_VAR
ResultValue := DeviceInstance.Scratch;
END_PROGRAM
"#,
    );
}

#[test]
fn member_access_fb_external_rejects_unrelated_instance_access() {
    assert_invalid(
        r#"
VAR_GLOBAL
    Shared : INT;
END_VAR
FUNCTION_BLOCK Device
VAR_EXTERNAL
    Shared : INT;
END_VAR
END_FUNCTION_BLOCK
PROGRAM Main
VAR
    DeviceInstance : Device;
    ResultValue : INT;
END_VAR
ResultValue := DeviceInstance.Shared;
END_PROGRAM
"#,
    );
}

#[test]
fn member_access_fb_method_rejects_in_out_access() {
    assert_invalid(
        r#"
FUNCTION_BLOCK Device
VAR_IN_OUT
    Shared : INT;
END_VAR
METHOD PUBLIC Read : INT
Read := Shared;
END_METHOD
END_FUNCTION_BLOCK
"#,
    );
}

#[test]
fn member_access_fb_method_rejects_temp_access() {
    assert_invalid(
        r#"
FUNCTION_BLOCK Device
VAR_TEMP
    Scratch : INT;
END_VAR
METHOD PUBLIC Read : INT
Read := Scratch;
END_METHOD
END_FUNCTION_BLOCK
"#,
    );
}

#[test]
fn member_access_private_getter_rejects_external_read_even_with_get() {
    assert_invalid(
        r#"
CLASS Device
PRIVATE PROPERTY Value : INT
GET Value := INT#1; END_GET
END_PROPERTY
END_CLASS
PROGRAM Main
VAR
    DeviceInstance : Device;
    ResultValue : INT;
END_VAR
ResultValue := DeviceInstance.Value;
END_PROGRAM
"#,
    );
}

#[test]
fn member_access_private_setter_rejects_external_write_even_with_set() {
    assert_invalid(
        r#"
CLASS Device
PRIVATE PROPERTY Value : INT
SET END_SET
END_PROPERTY
END_CLASS
PROGRAM Main
VAR
    DeviceInstance : Device;
END_VAR
DeviceInstance.Value := INT#1;
END_PROGRAM
"#,
    );
}

#[test]
fn member_access_internal_namespace_restricts_public_member_from_global_scope() {
    assert_invalid(
        r#"
NAMESPACE INTERNAL Hidden
CLASS Device
METHOD PUBLIC Read : INT
Read := INT#1;
END_METHOD
END_CLASS
END_NAMESPACE
PROGRAM Main
VAR
    DeviceInstance : Hidden.Device;
    ResultValue : INT;
END_VAR
ResultValue := DeviceInstance.Read();
END_PROGRAM
"#,
    );
}

#[test]
fn member_access_internal_outer_namespace_restricts_nested_public_member() {
    assert_invalid(
        r#"
NAMESPACE INTERNAL Outer
NAMESPACE Inner
CLASS Device
METHOD PUBLIC Read : INT
Read := INT#1;
END_METHOD
END_CLASS
END_NAMESPACE
END_NAMESPACE
PROGRAM Main
VAR
    DeviceInstance : Outer.Inner.Device;
    ResultValue : INT;
END_VAR
ResultValue := DeviceInstance.Read();
END_PROGRAM
"#,
    );
}
