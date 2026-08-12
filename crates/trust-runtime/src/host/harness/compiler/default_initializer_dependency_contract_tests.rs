use crate::harness::{CompileSession, SourceFile};
use crate::value::Value;
use crate::Runtime;

fn source_files(files: &[(&str, &str)]) -> Vec<SourceFile> {
    files
        .iter()
        .map(|(path, source)| SourceFile::with_path(*path, *source))
        .collect()
}

fn runtime(files: &[(&str, &str)]) -> Runtime {
    CompileSession::from_sources(source_files(files))
        .build_runtime()
        .unwrap_or_else(|error| panic!("default-initializer fixture must compile: {error}"))
}

fn compile_error(files: &[(&str, &str)]) -> String {
    match CompileSession::from_sources(source_files(files)).build_runtime() {
        Ok(_) => panic!("default-initializer fixture must fail"),
        Err(error) => error.to_string(),
    }
}

fn global<'a>(runtime: &'a Runtime, name: &str) -> &'a Value {
    runtime
        .storage()
        .get_global(name)
        .unwrap_or_else(|| panic!("missing global {name}"))
}

fn int_global(runtime: &Runtime, name: &str) -> i16 {
    match global(runtime, name) {
        Value::Int(value) => *value,
        other => panic!("expected INT global {name}, got {other:?}"),
    }
}

fn struct_field<'a>(runtime: &'a Runtime, global_name: &str, field: &str) -> &'a Value {
    let Value::Struct(value) = global(runtime, global_name) else {
        panic!("expected STRUCT global {global_name}");
    };
    value
        .field(field)
        .unwrap_or_else(|| panic!("missing field {global_name}.{field}"))
}

#[test]
fn default_initializer_dependency_type_default_uses_later_constant() {
    let runtime = runtime(&[(
        "main.st",
        r#"
TYPE Calibrated : INT := Limit + INT#1;
END_TYPE
VAR_GLOBAL
    Value : Calibrated;
END_VAR
VAR_GLOBAL CONSTANT
    Limit : INT := INT#6;
END_VAR
PROGRAM Main
END_PROGRAM
"#,
    )]);

    assert_eq!(int_global(&runtime, "Value"), 7);
}

#[test]
fn default_initializer_dependency_type_default_uses_later_source_constant() {
    let runtime = runtime(&[
        (
            "types.st",
            r#"
TYPE Calibrated : INT := Limit + INT#2;
END_TYPE
"#,
        ),
        (
            "values.st",
            r#"
VAR_GLOBAL
    Value : Calibrated;
END_VAR
"#,
        ),
        (
            "constants.st",
            r#"
VAR_GLOBAL CONSTANT
    Limit : INT := INT#8;
END_VAR
PROGRAM Main
END_PROGRAM
"#,
        ),
    ]);

    assert_eq!(int_global(&runtime, "Value"), 10);
}

#[test]
fn default_initializer_dependency_source_permutation_preserves_type_default() {
    const TYPES: &str = r#"
TYPE Calibrated : INT := Limit * INT#2;
END_TYPE
"#;
    const VALUES: &str = r#"
VAR_GLOBAL
    Value : Calibrated;
END_VAR
"#;
    const CONSTANTS: &str = r#"
VAR_GLOBAL CONSTANT
    Limit : INT := INT#5;
END_VAR
"#;
    const PROGRAM: &str = r#"
PROGRAM Main
END_PROGRAM
"#;

    let first = runtime(&[
        ("types.st", TYPES),
        ("values.st", VALUES),
        ("constants.st", CONSTANTS),
        ("main.st", PROGRAM),
    ]);
    let second = runtime(&[
        ("main.st", PROGRAM),
        ("constants.st", CONSTANTS),
        ("values.st", VALUES),
        ("types.st", TYPES),
    ]);

    assert_eq!(int_global(&first, "Value"), 10);
    assert_eq!(int_global(&second, "Value"), 10);
}

#[test]
fn default_initializer_dependency_struct_member_uses_later_constant() {
    let runtime = runtime(&[(
        "main.st",
        r#"
TYPE Packet : STRUCT
    Code : INT := DefaultCode;
END_STRUCT;
END_TYPE
VAR_GLOBAL
    Value : Packet;
END_VAR
VAR_GLOBAL CONSTANT
    DefaultCode : INT := INT#17;
END_VAR
PROGRAM Main
END_PROGRAM
"#,
    )]);

    assert_eq!(struct_field(&runtime, "Value", "Code"), &Value::Int(17));
}

#[test]
fn default_initializer_dependency_union_member_uses_later_constant() {
    let runtime = runtime(&[(
        "main.st",
        r#"
TYPE Payload : UNION
    Code : INT := DefaultCode;
    Flag : BOOL;
END_UNION;
END_TYPE
VAR_GLOBAL
    Value : Payload;
END_VAR
VAR_GLOBAL CONSTANT
    DefaultCode : INT := INT#19;
END_VAR
PROGRAM Main
END_PROGRAM
"#,
    )]);

    assert_eq!(struct_field(&runtime, "Value", "Code"), &Value::Int(19));
}

#[test]
fn default_initializer_dependency_nested_members_use_complete_constant_graph() {
    let runtime = runtime(&[(
        "main.st",
        r#"
TYPE Inner : STRUCT
    Code : INT := Base + INT#1;
END_STRUCT;
Outer : STRUCT
    Item : Inner;
END_STRUCT;
END_TYPE
VAR_GLOBAL
    Value : Outer;
END_VAR
VAR_GLOBAL CONSTANT
    Base : INT := Seed * INT#2;
    Seed : INT := INT#4;
END_VAR
PROGRAM Main
END_PROGRAM
"#,
    )]);

    let Value::Struct(inner) = struct_field(&runtime, "Value", "Item") else {
        panic!("expected nested Inner value");
    };
    assert_eq!(inner.field("Code"), Some(&Value::Int(9)));
}

#[test]
fn default_initializer_dependency_preserves_elementary_value_tags() {
    let runtime = runtime(&[(
        "main.st",
        r#"
TYPE EnabledDefault : BOOL := Enabled;
RatioDefault : REAL := Ratio;
LabelDefault : STRING := Label;
END_TYPE
VAR_GLOBAL
    EnabledValue : EnabledDefault;
    RatioValue : RatioDefault;
    LabelValue : LabelDefault;
END_VAR
VAR_GLOBAL CONSTANT
    Enabled : BOOL := TRUE;
    Ratio : REAL := REAL#1.25;
    Label : STRING := 'ready';
END_VAR
PROGRAM Main
END_PROGRAM
"#,
    )]);

    assert_eq!(global(&runtime, "EnabledValue"), &Value::Bool(true));
    assert_eq!(global(&runtime, "RatioValue"), &Value::Real(1.25));
    assert_eq!(
        global(&runtime, "LabelValue"),
        &Value::String("ready".into())
    );
}

#[test]
fn default_initializer_dependency_qualified_namespace_constant_is_exact() {
    let runtime = runtime(&[(
        "main.st",
        r#"
NAMESPACE Left
VAR_GLOBAL CONSTANT
    Limit : INT := INT#3;
END_VAR
END_NAMESPACE
NAMESPACE Right
VAR_GLOBAL CONSTANT
    Limit : INT := INT#9;
END_VAR
END_NAMESPACE
TYPE Selected : INT := Right.Limit;
END_TYPE
VAR_GLOBAL
    Value : Selected;
END_VAR
PROGRAM Main
END_PROGRAM
"#,
    )]);

    assert_eq!(int_global(&runtime, "Value"), 9);
}

#[test]
fn default_initializer_dependency_same_namespace_constant_is_implicit() {
    let runtime = runtime(&[(
        "main.st",
        r#"
NAMESPACE Cell
TYPE Selected : INT := Limit + INT#1;
END_TYPE
VAR_GLOBAL CONSTANT
    Limit : INT := INT#12;
END_VAR
END_NAMESPACE
VAR_GLOBAL
    Value : Cell.Selected;
END_VAR
PROGRAM Main
END_PROGRAM
"#,
    )]);

    assert_eq!(int_global(&runtime, "Value"), 13);
}

#[test]
fn default_initializer_dependency_using_import_selects_namespace_constant() {
    let runtime = runtime(&[(
        "main.st",
        r#"
NAMESPACE Limits
VAR_GLOBAL CONSTANT
    DefaultValue : INT := INT#21;
END_VAR
END_NAMESPACE
USING Limits;
TYPE Selected : INT := DefaultValue;
END_TYPE
VAR_GLOBAL
    Value : Selected;
END_VAR
PROGRAM Main
END_PROGRAM
"#,
    )]);

    assert_eq!(int_global(&runtime, "Value"), 21);
}

#[test]
fn default_initializer_dependency_cross_source_using_preserves_provider_identity() {
    let runtime = runtime(&[
        (
            "consumer.st",
            r#"
USING Limits;
TYPE Selected : INT := DefaultValue + INT#2;
END_TYPE
VAR_GLOBAL
    Value : Selected;
END_VAR
"#,
        ),
        (
            "provider.st",
            r#"
NAMESPACE Limits
VAR_GLOBAL CONSTANT
    DefaultValue : INT := INT#23;
END_VAR
END_NAMESPACE
"#,
        ),
        (
            "main.st",
            r#"
PROGRAM Main
END_PROGRAM
"#,
        ),
    ]);

    assert_eq!(int_global(&runtime, "Value"), 25);
}

#[test]
fn default_initializer_dependency_namespaces_keep_same_leaf_constants_distinct() {
    let runtime = runtime(&[(
        "main.st",
        r#"
NAMESPACE Left
VAR_GLOBAL CONSTANT
    Limit : INT := INT#4;
END_VAR
TYPE Selected : INT := Limit;
END_TYPE
END_NAMESPACE
NAMESPACE Right
VAR_GLOBAL CONSTANT
    Limit : INT := INT#10;
END_VAR
TYPE Selected : INT := Limit;
END_TYPE
END_NAMESPACE
VAR_GLOBAL
    LeftValue : Left.Selected;
    RightValue : Right.Selected;
END_VAR
PROGRAM Main
END_PROGRAM
"#,
    )]);

    assert_eq!(int_global(&runtime, "LeftValue"), 4);
    assert_eq!(int_global(&runtime, "RightValue"), 10);
    assert_eq!(int_global(&runtime, "Left.Limit"), 4);
    assert_eq!(int_global(&runtime, "Right.Limit"), 10);
}

#[test]
fn default_initializer_dependency_rejects_ambiguous_using_constants() {
    let error = compile_error(&[(
        "ambiguous.st",
        r#"
NAMESPACE Left
VAR_GLOBAL CONSTANT
    Limit : INT := INT#4;
END_VAR
END_NAMESPACE
NAMESPACE Right
VAR_GLOBAL CONSTANT
    Limit : INT := INT#10;
END_VAR
END_NAMESPACE
USING Left, Right;
TYPE Selected : INT := Limit;
END_TYPE
PROGRAM Main
END_PROGRAM
"#,
    )]);

    assert!(
        error.to_ascii_lowercase().contains("ambiguous")
            && error.to_ascii_lowercase().contains("limit"),
        "{error}"
    );
}

#[test]
fn default_initializer_dependency_rejects_unimported_namespace_constant() {
    let error = compile_error(&[(
        "missing_import.st",
        r#"
NAMESPACE Limits
VAR_GLOBAL CONSTANT
    DefaultValue : INT := INT#7;
END_VAR
END_NAMESPACE
TYPE Selected : INT := DefaultValue;
END_TYPE
PROGRAM Main
END_PROGRAM
"#,
    )]);

    assert!(
        error.to_ascii_lowercase().contains("defaultvalue")
            && (error.to_ascii_lowercase().contains("undefined")
                || error.to_ascii_lowercase().contains("cannot resolve")
                || error.to_ascii_lowercase().contains("unknown")),
        "{error}"
    );
}

#[test]
fn default_initializer_dependency_using_parent_does_not_import_nested_namespace() {
    let error = compile_error(&[(
        "nested_import.st",
        r#"
NAMESPACE Limits.Nested
VAR_GLOBAL CONSTANT
    DefaultValue : INT := INT#7;
END_VAR
END_NAMESPACE
USING Limits;
TYPE Selected : INT := DefaultValue;
END_TYPE
PROGRAM Main
END_PROGRAM
"#,
    )]);

    assert!(
        error.to_ascii_lowercase().contains("defaultvalue")
            && (error.to_ascii_lowercase().contains("undefined")
                || error.to_ascii_lowercase().contains("cannot resolve")
                || error.to_ascii_lowercase().contains("unknown")),
        "{error}"
    );
}

#[test]
fn default_initializer_dependency_rejects_mutable_namespaced_value() {
    let error = compile_error(&[(
        "mutable.st",
        r#"
NAMESPACE Limits
VAR_GLOBAL
    DefaultValue : INT := INT#7;
END_VAR
END_NAMESPACE
TYPE Selected : INT := Limits.DefaultValue;
END_TYPE
PROGRAM Main
END_PROGRAM
"#,
    )]);

    assert!(
        error.contains("type/member default initializer must be a constant expression")
            || error.contains("variable initializer must be a literal or constant expression"),
        "{error}"
    );
}

#[test]
fn default_initializer_dependency_precedence_is_recursive_and_non_destructive() {
    let runtime = runtime(&[(
        "main.st",
        r#"
TYPE BaseValue : INT := INT#2;
Packet : STRUCT
    FromType : BaseValue;
    FromMember : BaseValue := INT#3;
END_STRUCT;
PacketDefault : Packet := (FromType := INT#4);
END_TYPE
VAR_GLOBAL
    FromTypeDefault : PacketDefault;
    FromVariableDefault : PacketDefault := (FromMember := INT#5);
END_VAR
PROGRAM Main
END_PROGRAM
"#,
    )]);

    assert!(
        runtime
            .initializer_catalog()
            .type_default(runtime.registry().lookup("PacketDefault").expect("type"))
            .is_some(),
        "PacketDefault must retain its TYPE-level aggregate initializer"
    );

    assert_eq!(
        struct_field(&runtime, "FromTypeDefault", "FromType"),
        &Value::Int(4)
    );
    assert_eq!(
        struct_field(&runtime, "FromTypeDefault", "FromMember"),
        &Value::Int(3)
    );
    assert_eq!(
        struct_field(&runtime, "FromVariableDefault", "FromType"),
        &Value::Int(4)
    );
    assert_eq!(
        struct_field(&runtime, "FromVariableDefault", "FromMember"),
        &Value::Int(5)
    );
}
