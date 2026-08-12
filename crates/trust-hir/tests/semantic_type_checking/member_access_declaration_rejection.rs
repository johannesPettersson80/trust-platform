use super::common::*;

// Declaration-side rejection matrix for IEC 61131-3 Ed.3 Tables 48/53,
// §§6.6.5.9-6.6.5.10, 6.6.6.3, and 6.6.7.6-6.6.7.7.

fn assert_invalid(source: &str) {
    check_has_error(source, DiagnosticCode::InvalidOperation);
}

#[test]
fn member_access_interface_method_rejects_explicit_public() {
    assert_invalid(
        r#"
INTERFACE IReadable
METHOD PUBLIC Read : INT
END_METHOD
END_INTERFACE
"#,
    );
}

#[test]
fn member_access_interface_method_rejects_explicit_protected() {
    assert_invalid(
        r#"
INTERFACE IReadable
METHOD PROTECTED Read : INT
END_METHOD
END_INTERFACE
"#,
    );
}

#[test]
fn member_access_interface_method_rejects_explicit_private() {
    assert_invalid(
        r#"
INTERFACE IReadable
METHOD PRIVATE Read : INT
END_METHOD
END_INTERFACE
"#,
    );
}

#[test]
fn member_access_interface_method_rejects_explicit_internal() {
    assert_invalid(
        r#"
INTERFACE IReadable
METHOD INTERNAL Read : INT
END_METHOD
END_INTERFACE
"#,
    );
}

#[test]
fn member_access_interface_property_rejects_explicit_public() {
    assert_invalid(
        r#"
INTERFACE IReadable
PUBLIC PROPERTY Value : INT
GET END_GET
END_PROPERTY
END_INTERFACE
"#,
    );
}

#[test]
fn member_access_interface_property_rejects_explicit_protected() {
    assert_invalid(
        r#"
INTERFACE IReadable
PROTECTED PROPERTY Value : INT
GET END_GET
END_PROPERTY
END_INTERFACE
"#,
    );
}

#[test]
fn member_access_interface_property_rejects_explicit_private() {
    assert_invalid(
        r#"
INTERFACE IReadable
PRIVATE PROPERTY Value : INT
GET END_GET
END_PROPERTY
END_INTERFACE
"#,
    );
}

#[test]
fn member_access_interface_property_rejects_explicit_internal() {
    assert_invalid(
        r#"
INTERFACE IReadable
INTERNAL PROPERTY Value : INT
GET END_GET
END_PROPERTY
END_INTERFACE
"#,
    );
}

#[test]
fn member_access_fb_input_rejects_explicit_public() {
    assert_invalid(
        r#"
FUNCTION_BLOCK Device
VAR_INPUT PUBLIC
    Value : INT;
END_VAR
END_FUNCTION_BLOCK
"#,
    );
}

#[test]
fn member_access_fb_input_rejects_explicit_private() {
    assert_invalid(
        r#"
FUNCTION_BLOCK Device
VAR_INPUT PRIVATE
    Value : INT;
END_VAR
END_FUNCTION_BLOCK
"#,
    );
}

#[test]
fn member_access_fb_output_rejects_explicit_public() {
    assert_invalid(
        r#"
FUNCTION_BLOCK Device
VAR_OUTPUT PUBLIC
    Value : INT;
END_VAR
END_FUNCTION_BLOCK
"#,
    );
}

#[test]
fn member_access_fb_output_rejects_explicit_protected() {
    assert_invalid(
        r#"
FUNCTION_BLOCK Device
VAR_OUTPUT PROTECTED
    Value : INT;
END_VAR
END_FUNCTION_BLOCK
"#,
    );
}

#[test]
fn member_access_fb_in_out_rejects_explicit_public() {
    assert_invalid(
        r#"
FUNCTION_BLOCK Device
VAR_IN_OUT PUBLIC
    Value : INT;
END_VAR
END_FUNCTION_BLOCK
"#,
    );
}

#[test]
fn member_access_fb_in_out_rejects_explicit_internal() {
    assert_invalid(
        r#"
FUNCTION_BLOCK Device
VAR_IN_OUT INTERNAL
    Value : INT;
END_VAR
END_FUNCTION_BLOCK
"#,
    );
}

#[test]
fn member_access_fb_external_rejects_explicit_protected() {
    assert_invalid(
        r#"
VAR_GLOBAL
    Shared : INT;
END_VAR
FUNCTION_BLOCK Device
VAR_EXTERNAL PROTECTED
    Shared : INT;
END_VAR
END_FUNCTION_BLOCK
"#,
    );
}

#[test]
fn member_access_fb_external_rejects_explicit_public() {
    assert_invalid(
        r#"
VAR_GLOBAL
    Shared : INT;
END_VAR
FUNCTION_BLOCK Device
VAR_EXTERNAL PUBLIC
    Shared : INT;
END_VAR
END_FUNCTION_BLOCK
"#,
    );
}

#[test]
fn member_access_fb_temp_rejects_explicit_private() {
    assert_invalid(
        r#"
FUNCTION_BLOCK Device
VAR_TEMP PRIVATE
    Scratch : INT;
END_VAR
END_FUNCTION_BLOCK
"#,
    );
}

#[test]
fn member_access_fb_temp_rejects_explicit_internal() {
    assert_invalid(
        r#"
FUNCTION_BLOCK Device
VAR_TEMP INTERNAL
    Scratch : INT;
END_VAR
END_FUNCTION_BLOCK
"#,
    );
}

#[test]
fn member_access_function_local_rejects_access_specifier() {
    assert_invalid(
        r#"
FUNCTION Compute : INT
VAR PUBLIC
    Scratch : INT;
END_VAR
Compute := Scratch;
END_FUNCTION
"#,
    );
}

#[test]
fn member_access_method_local_rejects_access_specifier() {
    assert_invalid(
        r#"
CLASS Device
METHOD PUBLIC Compute : INT
VAR PRIVATE
    Scratch : INT;
END_VAR
Compute := Scratch;
END_METHOD
END_CLASS
"#,
    );
}

#[test]
fn member_access_program_local_rejects_access_specifier() {
    assert_invalid(
        r#"
PROGRAM Main
VAR PROTECTED
    Scratch : INT;
END_VAR
END_PROGRAM
"#,
    );
}

#[test]
fn member_access_global_section_rejects_access_specifier() {
    assert_invalid(
        r#"
VAR_GLOBAL INTERNAL
    Shared : INT;
END_VAR
PROGRAM Main
END_PROGRAM
"#,
    );
}

#[test]
fn member_access_variable_section_rejects_duplicate_same_specifier() {
    assert_invalid(
        r#"
CLASS Device
VAR PUBLIC PUBLIC
    Value : INT;
END_VAR
END_CLASS
"#,
    );
}

#[test]
fn member_access_variable_section_rejects_conflicting_specifiers() {
    assert_invalid(
        r#"
CLASS Device
VAR PUBLIC PRIVATE
    Value : INT;
END_VAR
END_CLASS
"#,
    );
}

#[test]
fn member_access_method_rejects_duplicate_same_specifier() {
    assert_invalid(
        r#"
CLASS Device
METHOD PUBLIC PUBLIC Read
END_METHOD
END_CLASS
"#,
    );
}

#[test]
fn member_access_method_rejects_conflicting_specifiers() {
    assert_invalid(
        r#"
CLASS Device
METHOD PUBLIC PRIVATE Read
END_METHOD
END_CLASS
"#,
    );
}

#[test]
fn member_access_property_rejects_duplicate_same_specifier() {
    assert_invalid(
        r#"
CLASS Device
PUBLIC PUBLIC PROPERTY Value : INT
GET Value := INT#1; END_GET
END_PROPERTY
END_CLASS
"#,
    );
}

#[test]
fn member_access_property_rejects_conflicting_specifiers() {
    assert_invalid(
        r#"
CLASS Device
PUBLIC PRIVATE PROPERTY Value : INT
GET Value := INT#1; END_GET
END_PROPERTY
END_CLASS
"#,
    );
}

#[test]
fn member_access_public_override_rejects_protected_base_visibility() {
    assert_invalid(
        r#"
CLASS Base
METHOD PROTECTED Read : INT
Read := INT#1;
END_METHOD
END_CLASS
CLASS Derived EXTENDS Base
METHOD PUBLIC OVERRIDE Read : INT
Read := INT#2;
END_METHOD
END_CLASS
"#,
    );
}

#[test]
fn member_access_protected_override_rejects_public_base_visibility() {
    assert_invalid(
        r#"
CLASS Base
METHOD PUBLIC Read : INT
Read := INT#1;
END_METHOD
END_CLASS
CLASS Derived EXTENDS Base
METHOD PROTECTED OVERRIDE Read : INT
Read := INT#2;
END_METHOD
END_CLASS
"#,
    );
}

#[test]
fn member_access_private_method_cannot_be_overridden() {
    assert_invalid(
        r#"
CLASS Base
METHOD PRIVATE Read : INT
Read := INT#1;
END_METHOD
END_CLASS
CLASS Derived EXTENDS Base
METHOD PRIVATE OVERRIDE Read : INT
Read := INT#2;
END_METHOD
END_CLASS
"#,
    );
}

#[test]
fn member_access_internal_method_cannot_be_overridden_from_other_namespace() {
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
METHOD INTERNAL OVERRIDE Read : INT
Read := INT#2;
END_METHOD
END_CLASS
END_NAMESPACE
"#,
    );
}

#[test]
fn member_access_interface_method_rejects_protected_implementation() {
    assert_invalid(
        r#"
INTERFACE IReadable
METHOD Read : INT
END_METHOD
END_INTERFACE
CLASS Device IMPLEMENTS IReadable
METHOD PROTECTED Read : INT
Read := INT#1;
END_METHOD
END_CLASS
"#,
    );
}

#[test]
fn member_access_interface_method_rejects_private_implementation() {
    assert_invalid(
        r#"
INTERFACE IReadable
METHOD Read : INT
END_METHOD
END_INTERFACE
CLASS Device IMPLEMENTS IReadable
METHOD PRIVATE Read : INT
Read := INT#1;
END_METHOD
END_CLASS
"#,
    );
}

#[test]
fn member_access_interface_property_rejects_protected_implementation() {
    assert_invalid(
        r#"
INTERFACE IReadable
PROPERTY Value : INT
GET END_GET
END_PROPERTY
END_INTERFACE
CLASS Device IMPLEMENTS IReadable
PROTECTED PROPERTY Value : INT
GET Value := INT#1; END_GET
END_PROPERTY
END_CLASS
"#,
    );
}

#[test]
fn member_access_interface_property_rejects_private_implementation() {
    assert_invalid(
        r#"
INTERFACE IReadable
PROPERTY Value : INT
GET END_GET
END_PROPERTY
END_INTERFACE
CLASS Device IMPLEMENTS IReadable
PRIVATE PROPERTY Value : INT
GET Value := INT#1; END_GET
END_PROPERTY
END_CLASS
"#,
    );
}
