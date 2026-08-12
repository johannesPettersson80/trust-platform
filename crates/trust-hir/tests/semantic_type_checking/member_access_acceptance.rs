use super::common::*;

// IEC 61131-3 Ed.3 Tables 48/53 and §§6.6.5.9-6.6.5.10,
// 6.6.7.6-6.6.7.7. PROPERTY cases exercise the documented truST extension.

fn assert_ok(source: &str) {
    check_no_errors(source);
}

fn symbol_visibility(source: &str, owner_name: &str, member_name: &str) -> Visibility {
    let mut db = Database::new();
    let file = FileId(0);
    db.set_source_text(file, source.to_string());
    let symbols = db.file_symbols(file);
    let owner = symbols
        .iter()
        .find(|symbol| symbol.name == owner_name)
        .unwrap_or_else(|| panic!("missing owner {owner_name}"));
    let visibility = symbols
        .iter()
        .find(|symbol| symbol.name == member_name && symbol.parent == Some(owner.id))
        .unwrap_or_else(|| panic!("missing member {owner_name}.{member_name}"))
        .visibility;
    visibility
}

#[test]
fn member_access_class_variable_defaults_to_protected() {
    assert_eq!(
        symbol_visibility(
            r#"
CLASS Device
VAR
    Value : INT;
END_VAR
END_CLASS
"#,
            "Device",
            "Value",
        ),
        Visibility::Protected
    );
}

#[test]
fn member_access_class_method_defaults_to_protected() {
    assert_eq!(
        symbol_visibility(
            r#"
CLASS Device
METHOD Read : INT
Read := INT#1;
END_METHOD
END_CLASS
"#,
            "Device",
            "Read",
        ),
        Visibility::Protected
    );
}

#[test]
fn member_access_class_property_defaults_to_protected() {
    assert_eq!(
        symbol_visibility(
            r#"
CLASS Device
PROPERTY Level : INT
GET
Level := INT#1;
END_GET
END_PROPERTY
END_CLASS
"#,
            "Device",
            "Level",
        ),
        Visibility::Protected
    );
}

#[test]
fn member_access_function_block_variable_defaults_to_protected() {
    assert_eq!(
        symbol_visibility(
            r#"
FUNCTION_BLOCK Device
VAR
    Value : INT;
END_VAR
END_FUNCTION_BLOCK
"#,
            "Device",
            "Value",
        ),
        Visibility::Protected
    );
}

#[test]
fn member_access_function_block_method_defaults_to_protected() {
    assert_eq!(
        symbol_visibility(
            r#"
FUNCTION_BLOCK Device
METHOD Read : INT
Read := INT#1;
END_METHOD
END_FUNCTION_BLOCK
"#,
            "Device",
            "Read",
        ),
        Visibility::Protected
    );
}

#[test]
fn member_access_function_block_property_defaults_to_protected() {
    assert_eq!(
        symbol_visibility(
            r#"
FUNCTION_BLOCK Device
PROPERTY Level : INT
GET
Level := INT#1;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK
"#,
            "Device",
            "Level",
        ),
        Visibility::Protected
    );
}

#[test]
fn member_access_explicit_class_variable_visibilities_are_collected() {
    let source = r#"
CLASS Device
VAR PUBLIC
    PublicValue : INT;
END_VAR
VAR PROTECTED
    ProtectedValue : INT;
END_VAR
VAR PRIVATE
    PrivateValue : INT;
END_VAR
VAR INTERNAL
    InternalValue : INT;
END_VAR
END_CLASS
"#;
    assert_eq!(
        symbol_visibility(source, "Device", "PublicValue"),
        Visibility::Public
    );
    assert_eq!(
        symbol_visibility(source, "Device", "ProtectedValue"),
        Visibility::Protected
    );
    assert_eq!(
        symbol_visibility(source, "Device", "PrivateValue"),
        Visibility::Private
    );
    assert_eq!(
        symbol_visibility(source, "Device", "InternalValue"),
        Visibility::Internal
    );
}

#[test]
fn member_access_explicit_method_visibilities_are_collected() {
    let source = r#"
CLASS Device
METHOD PUBLIC PublicCall END_METHOD
METHOD PROTECTED ProtectedCall END_METHOD
METHOD PRIVATE PrivateCall END_METHOD
METHOD INTERNAL InternalCall END_METHOD
END_CLASS
"#;
    assert_eq!(
        symbol_visibility(source, "Device", "PublicCall"),
        Visibility::Public
    );
    assert_eq!(
        symbol_visibility(source, "Device", "ProtectedCall"),
        Visibility::Protected
    );
    assert_eq!(
        symbol_visibility(source, "Device", "PrivateCall"),
        Visibility::Private
    );
    assert_eq!(
        symbol_visibility(source, "Device", "InternalCall"),
        Visibility::Internal
    );
}

#[test]
fn member_access_explicit_property_visibilities_are_collected() {
    let source = r#"
CLASS Device
PUBLIC PROPERTY PublicValue : INT GET PublicValue := INT#1; END_GET END_PROPERTY
PROTECTED PROPERTY ProtectedValue : INT GET ProtectedValue := INT#2; END_GET END_PROPERTY
PRIVATE PROPERTY PrivateValue : INT GET PrivateValue := INT#3; END_GET END_PROPERTY
INTERNAL PROPERTY InternalValue : INT GET InternalValue := INT#4; END_GET END_PROPERTY
END_CLASS
"#;
    assert_eq!(
        symbol_visibility(source, "Device", "PublicValue"),
        Visibility::Public
    );
    assert_eq!(
        symbol_visibility(source, "Device", "ProtectedValue"),
        Visibility::Protected
    );
    assert_eq!(
        symbol_visibility(source, "Device", "PrivateValue"),
        Visibility::Private
    );
    assert_eq!(
        symbol_visibility(source, "Device", "InternalValue"),
        Visibility::Internal
    );
}

#[test]
fn member_access_fb_inputs_and_outputs_are_implicitly_public() {
    let source = r#"
FUNCTION_BLOCK Device
VAR_INPUT
    Requested : INT;
END_VAR
VAR_OUTPUT
    Actual : INT;
END_VAR
END_FUNCTION_BLOCK
"#;
    assert_eq!(
        symbol_visibility(source, "Device", "Requested"),
        Visibility::Public
    );
    assert_eq!(
        symbol_visibility(source, "Device", "Actual"),
        Visibility::Public
    );
}

#[test]
fn member_access_fb_external_is_implicitly_protected() {
    assert_eq!(
        symbol_visibility(
            r#"
VAR_GLOBAL
    Shared : INT;
END_VAR
FUNCTION_BLOCK Device
VAR_EXTERNAL
    Shared : INT;
END_VAR
END_FUNCTION_BLOCK
"#,
            "Device",
            "Shared",
        ),
        Visibility::Protected
    );
}

#[test]
fn member_access_private_members_are_usable_in_defining_class() {
    assert_ok(
        r#"
CLASS Vault
VAR PRIVATE
    Secret : INT;
END_VAR
METHOD PRIVATE Bump
Secret := Secret + INT#1;
END_METHOD
PRIVATE PROPERTY Code : INT
GET Code := Secret; END_GET
SET Secret := Code; END_SET
END_PROPERTY
METHOD PUBLIC Exercise : INT
THIS.Bump();
THIS.Code := INT#7;
Exercise := THIS.Code;
END_METHOD
END_CLASS
"#,
    );
}

#[test]
fn member_access_protected_members_are_usable_in_derived_class() {
    assert_ok(
        r#"
CLASS Base
VAR PROTECTED
    Stored : INT;
END_VAR
METHOD PROTECTED Add
Stored := Stored + INT#1;
END_METHOD
PROTECTED PROPERTY Value : INT
GET Value := Stored; END_GET
SET Stored := Value; END_SET
END_PROPERTY
END_CLASS
CLASS Derived EXTENDS Base
METHOD PUBLIC Exercise : INT
SUPER.Add();
THIS.Value := INT#8;
Exercise := Stored + THIS.Value;
END_METHOD
END_CLASS
"#,
    );
}

#[test]
fn member_access_protected_members_cross_multiple_derivation_levels() {
    assert_ok(
        r#"
FUNCTION_BLOCK Root
VAR PROTECTED
    Stored : INT;
END_VAR
METHOD PROTECTED Read : INT
Read := Stored;
END_METHOD
END_FUNCTION_BLOCK
FUNCTION_BLOCK Middle EXTENDS Root
END_FUNCTION_BLOCK
FUNCTION_BLOCK Leaf EXTENDS Middle
METHOD PUBLIC Exercise : INT
Stored := INT#9;
Exercise := THIS.Read();
END_METHOD
END_FUNCTION_BLOCK
"#,
    );
}

#[test]
fn member_access_protected_inheritance_crosses_namespace_boundaries() {
    assert_ok(
        r#"
NAMESPACE A
CLASS Base
VAR PROTECTED
    Stored : INT;
END_VAR
METHOD PROTECTED Read : INT
Read := Stored;
END_METHOD
END_CLASS
END_NAMESPACE
NAMESPACE B
CLASS Derived EXTENDS A.Base
METHOD PUBLIC Exercise : INT
Stored := INT#3;
Exercise := SUPER.Read();
END_METHOD
END_CLASS
END_NAMESPACE
"#,
    );
}

#[test]
fn member_access_public_members_are_usable_from_external_program() {
    assert_ok(
        r#"
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
DeviceInstance.Value := INT#5;
ResultValue := DeviceInstance.Read() + DeviceInstance.Value;
END_PROGRAM
"#,
    );
}

#[test]
fn member_access_internal_members_are_usable_by_sibling_pou_in_namespace() {
    assert_ok(
        r#"
NAMESPACE Cell
CLASS Device
VAR INTERNAL
    Stored : INT;
END_VAR
METHOD INTERNAL Read : INT
Read := Stored;
END_METHOD
INTERNAL PROPERTY Value : INT
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
DeviceInstance.Value := INT#6;
ResultValue := DeviceInstance.Read() + DeviceInstance.Value;
END_PROGRAM
END_NAMESPACE
"#,
    );
}

#[test]
fn member_access_internal_members_are_usable_in_separate_same_namespace_declarations() {
    assert_ok(
        r#"
NAMESPACE Cell
CLASS Device
METHOD INTERNAL Read : INT
Read := INT#4;
END_METHOD
END_CLASS
END_NAMESPACE
NAMESPACE Cell
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
fn member_access_internal_method_can_be_overridden_in_same_namespace() {
    assert_ok(
        r#"
NAMESPACE Cell
CLASS Base
METHOD INTERNAL Read : INT
Read := INT#1;
END_METHOD
END_CLASS
CLASS Derived EXTENDS Base
METHOD INTERNAL OVERRIDE Read : INT
Read := INT#2;
END_METHOD
END_CLASS
END_NAMESPACE
"#,
    );
}

#[test]
fn member_access_public_method_can_be_overridden_with_public() {
    assert_ok(
        r#"
CLASS Base
METHOD PUBLIC Read : INT
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
fn member_access_protected_method_can_be_overridden_with_protected() {
    assert_ok(
        r#"
CLASS Base
METHOD PROTECTED Read : INT
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
fn member_access_private_same_name_in_derived_is_new_method() {
    assert_ok(
        r#"
CLASS Base
METHOD PRIVATE Read : INT
Read := INT#1;
END_METHOD
END_CLASS
CLASS Derived EXTENDS Base
METHOD PUBLIC Read : INT
Read := INT#2;
END_METHOD
END_CLASS
"#,
    );
}

#[test]
fn member_access_private_same_name_in_derived_is_new_variable() {
    assert_ok(
        r#"
CLASS Base
VAR PRIVATE
    Stored : INT;
END_VAR
END_CLASS
CLASS Derived EXTENDS Base
VAR PUBLIC
    Stored : INT;
END_VAR
END_CLASS
"#,
    );
}

#[test]
fn member_access_interface_method_may_be_implemented_publicly() {
    assert_ok(
        r#"
INTERFACE IReadable
METHOD Read : INT
END_METHOD
END_INTERFACE
CLASS Device IMPLEMENTS IReadable
METHOD PUBLIC Read : INT
Read := INT#1;
END_METHOD
END_CLASS
"#,
    );
}

#[test]
fn member_access_interface_method_may_be_implemented_internal_in_same_namespace() {
    assert_ok(
        r#"
NAMESPACE Cell
INTERFACE IReadable
METHOD Read : INT
END_METHOD
END_INTERFACE
CLASS Device IMPLEMENTS IReadable
METHOD INTERNAL Read : INT
Read := INT#1;
END_METHOD
END_CLASS
END_NAMESPACE
"#,
    );
}

#[test]
fn member_access_interface_property_may_be_implemented_publicly() {
    assert_ok(
        r#"
INTERFACE IReadable
PROPERTY Value : INT
GET END_GET
END_PROPERTY
END_INTERFACE
CLASS Device IMPLEMENTS IReadable
PUBLIC PROPERTY Value : INT
GET Value := INT#1; END_GET
END_PROPERTY
END_CLASS
"#,
    );
}

#[test]
fn member_access_interface_property_may_be_implemented_internal_in_same_namespace() {
    assert_ok(
        r#"
NAMESPACE Cell
INTERFACE IReadable
PROPERTY Value : INT
GET END_GET
END_PROPERTY
END_INTERFACE
CLASS Device IMPLEMENTS IReadable
INTERNAL PROPERTY Value : INT
GET Value := INT#1; END_GET
END_PROPERTY
END_CLASS
END_NAMESPACE
"#,
    );
}

#[test]
fn member_access_storage_qualifier_before_access_specifier_is_valid() {
    assert_ok(
        r#"
CLASS Device
VAR RETAIN PUBLIC
    Stored : INT;
END_VAR
END_CLASS
"#,
    );
}

#[test]
fn member_access_access_specifier_before_storage_qualifier_is_valid() {
    assert_ok(
        r#"
CLASS Device
VAR PUBLIC RETAIN
    Stored : INT;
END_VAR
END_CLASS
"#,
    );
}

#[test]
fn member_access_fb_output_is_externally_readable() {
    assert_ok(
        r#"
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
"#,
    );
}

#[test]
fn member_access_fb_input_is_externally_readable_after_call_binding() {
    assert_ok(
        r#"
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
ConsumerInstance(Value := INT#7);
ResultValue := ConsumerInstance.Value;
END_PROGRAM
"#,
    );
}

#[test]
fn member_access_fb_in_out_is_valid_in_body_and_call_statement() {
    assert_ok(
        r#"
FUNCTION_BLOCK Bump
VAR_IN_OUT
    Value : INT;
END_VAR
Value := Value + INT#1;
END_FUNCTION_BLOCK
PROGRAM Main
VAR
    BumpInstance : Bump;
    Stored : INT;
END_VAR
BumpInstance(Value := Stored);
END_PROGRAM
"#,
    );
}
