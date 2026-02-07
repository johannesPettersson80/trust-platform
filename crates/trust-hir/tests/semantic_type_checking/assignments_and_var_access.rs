use crate::common::*;

#[test]
fn test_direct_address_usage() {
    check_no_errors(
        r#"
PROGRAM Test
    VAR x : BOOL; END_VAR
    x := %IX0.0;
END_PROGRAM
"#,
    );
}

#[test]
fn test_direct_address_type_mismatch() {
    check_has_error(
        r#"
PROGRAM Test
    VAR x : BOOL; END_VAR
    x := %IW0;
END_PROGRAM
"#,
        DiagnosticCode::IncompatibleAssignment,
    );
}

#[test]
fn test_direct_address_binding_recorded() {
    let mut db = Database::new();
    let file = FileId(0);
    db.set_source_text(
        file,
        r#"
PROGRAM Test
    VAR x AT %IX0.0 : BOOL; END_VAR
END_PROGRAM
"#
        .to_string(),
    );

    let symbols = db.file_symbols(file);
    let x = symbols.iter().find(|s| s.name == "x").unwrap();
    assert_eq!(x.direct_address.as_deref(), Some("%IX0.0"));
}

#[test]
fn test_invalid_assignment_target_field_of_call() {
    check_has_error(
        r#"
TYPE
    MyStruct : STRUCT
        field : INT;
    END_STRUCT;
END_TYPE

FUNCTION GetStruct : MyStruct
END_FUNCTION

PROGRAM Test
    GetStruct().field := 1;
END_PROGRAM
"#,
        DiagnosticCode::InvalidAssignmentTarget,
    );
}

#[test]
fn test_var_input_assignment_error() {
    check_has_error(
        r#"
FUNCTION FB_Test : INT
    VAR_INPUT
        InVal : INT;
    END_VAR
    InVal := 1;
    FB_Test := InVal;
END_FUNCTION
"#,
        DiagnosticCode::InvalidAssignmentTarget,
    );
}

#[test]
fn test_assignment_to_function_name_error() {
    check_has_error(
        r#"
FUNCTION Add : DINT
    Add := 1;
END_FUNCTION

PROGRAM Test
    Add := 2;
END_PROGRAM
"#,
        DiagnosticCode::InvalidAssignmentTarget,
    );
}

#[test]
fn test_assignment_to_this_error() {
    check_has_error(
        r#"
CLASS Example
    METHOD SetValue
        THIS := 1;
    END_METHOD
END_CLASS
"#,
        DiagnosticCode::InvalidAssignmentTarget,
    );
}

#[test]
fn test_property_without_setter_assignment_error() {
    check_has_error(
        r#"
FUNCTION_BLOCK FB_Test
    PROPERTY Value : INT
    GET
        RETURN 1;
    END_GET
    END_PROPERTY

    METHOD Update
        Value := 2;
    END_METHOD
END_FUNCTION_BLOCK
"#,
        DiagnosticCode::InvalidAssignmentTarget,
    );
}

#[test]
fn test_property_get_return_type_checked() {
    check_has_error(
        r#"
FUNCTION_BLOCK FB_Test
    PROPERTY Value : INT
    GET
        RETURN TRUE;
    END_GET
    END_PROPERTY
END_FUNCTION_BLOCK
"#,
        DiagnosticCode::InvalidReturnType,
    );
}

#[test]
fn test_property_set_rejects_return_value() {
    check_has_error(
        r#"
FUNCTION_BLOCK FB_Test
    PROPERTY Value : INT
    SET
        RETURN 1;
    END_SET
    END_PROPERTY
END_FUNCTION_BLOCK
"#,
        DiagnosticCode::InvalidReturnType,
    );
}

#[test]
fn test_function_missing_return_value() {
    check_has_error(
        r#"
FUNCTION Add : DINT
    VAR_INPUT
        a : DINT;
    END_VAR
END_FUNCTION
"#,
        DiagnosticCode::MissingReturn,
    );
}

#[test]
fn test_function_assignment_sets_return_value() {
    check_no_errors(
        r#"
FUNCTION Add : DINT
    VAR_INPUT
        a : DINT;
    END_VAR
    Add := a;
END_FUNCTION
"#,
    );
}

#[test]
fn test_function_return_expr_sets_return_value() {
    check_no_errors(
        r#"
FUNCTION Add : DINT
    VAR_INPUT
        a : DINT;
    END_VAR
    RETURN a;
END_FUNCTION
"#,
    );
}

#[test]
fn test_array_bounds_constant_expression() {
    let mut db = Database::new();
    let file = FileId(0);
    db.set_source_text(
        file,
        r#"
FUNCTION_BLOCK FB_Test
    VAR CONSTANT
        Max : DINT := 5;
    END_VAR
    VAR
        arr : ARRAY[0..Max + 1] OF INT;
    END_VAR
END_FUNCTION_BLOCK
"#
        .to_string(),
    );

    let symbols = db.file_symbols(file);
    let arr = symbols.iter().find(|s| s.name == "arr").unwrap();
    let type_id = symbols.resolve_alias_type(arr.type_id);
    let Type::Array { dimensions, .. } = symbols.type_by_id(type_id).unwrap() else {
        panic!("expected array type");
    };
    assert_eq!(dimensions, &vec![(0, 6)]);
}

#[test]
fn test_array_bounds_enum_values() {
    let mut db = Database::new();
    let file = FileId(0);
    db.set_source_text(
        file,
        r#"
TYPE Level : (Low := 1, High := 3)
END_TYPE

PROGRAM Test
    VAR
        arr : ARRAY[Low..High] OF INT;
    END_VAR
END_PROGRAM
"#
        .to_string(),
    );

    let symbols = db.file_symbols(file);
    let arr = symbols.iter().find(|s| s.name == "arr").unwrap();
    let type_id = symbols.resolve_alias_type(arr.type_id);
    let Type::Array { dimensions, .. } = symbols.type_by_id(type_id).unwrap() else {
        panic!("expected array type");
    };
    assert_eq!(dimensions, &vec![(1, 3)]);
}

#[test]
fn test_array_index_literal_out_of_bounds() {
    check_has_error(
        r#"
PROGRAM Test
    VAR arr : ARRAY[0..3] OF DINT; END_VAR
    arr[4] := 1;
END_PROGRAM
"#,
        DiagnosticCode::OutOfRange,
    );
}

#[test]
// IEC 61131-3 Ed.3 Tables 11, 15-16 (array bounds and indexing)
fn test_array_index_subrange_out_of_bounds() {
    check_has_error(
        r#"
TYPE Idx : INT(0..5);
END_TYPE

PROGRAM Test
    VAR i : Idx; arr : ARRAY[0..3] OF DINT; END_VAR
    arr[i] := 1;
END_PROGRAM
"#,
        DiagnosticCode::OutOfRange,
    );
}

#[test]
fn test_array_index_subrange_within_bounds() {
    check_no_errors(
        r#"
TYPE Idx : INT(1..3);
END_TYPE

PROGRAM Test
    VAR i : Idx; arr : ARRAY[1..3] OF DINT; END_VAR
    arr[i] := 1;
END_PROGRAM
"#,
    );
}

#[test]
fn test_array_index_dimension_too_many() {
    check_has_error(
        r#"
PROGRAM Test
    VAR arr : ARRAY[0..3] OF DINT; END_VAR
    arr[1, 2] := 1;
END_PROGRAM
"#,
        DiagnosticCode::InvalidArrayIndex,
    );
}

#[test]
fn test_array_index_dimension_too_few() {
    check_has_error(
        r#"
PROGRAM Test
    VAR arr : ARRAY[0..3, 1..2] OF DINT; END_VAR
    arr[1] := 1;
END_PROGRAM
"#,
        DiagnosticCode::InvalidArrayIndex,
    );
}

#[test]
fn test_array_index_requires_integer() {
    check_has_error(
        r#"
PROGRAM Test
    VAR arr : ARRAY[0..3] OF DINT; idx : REAL; END_VAR
    arr[idx] := 1;
END_PROGRAM
"#,
        DiagnosticCode::InvalidArrayIndex,
    );
}

#[test]
// IEC 61131-3 Ed.3 Tables 13-16 (VAR_ACCESS typing)
fn test_var_access_type_mismatch() {
    check_has_error(
        r#"
CONFIGURATION Conf
VAR_GLOBAL
    G : INT;
END_VAR
VAR_ACCESS
    A : G : DINT READ_WRITE;
END_VAR
END_CONFIGURATION
"#,
        DiagnosticCode::TypeMismatch,
    );
}

#[test]
fn test_var_access_read_only_rejects_assignment() {
    check_has_error(
        r#"
CONFIGURATION Conf
VAR_GLOBAL
    G : INT;
END_VAR
VAR_ACCESS
    A : G : INT READ_ONLY;
END_VAR
END_CONFIGURATION

PROGRAM Test
    A := 1;
END_PROGRAM
"#,
        DiagnosticCode::ConstantModification,
    );
}

#[test]
fn test_var_config_type_mismatch() {
    check_has_error(
        r#"
CONFIGURATION Conf
VAR_GLOBAL
    G : INT;
END_VAR
VAR_CONFIG
    G : DINT := 1;
END_VAR
END_CONFIGURATION
"#,
        DiagnosticCode::TypeMismatch,
    );
}

#[test]
fn test_var_config_rejects_constant_init() {
    check_has_error(
        r#"
CONFIGURATION Conf
VAR_GLOBAL CONSTANT
    G : INT := 1;
END_VAR
VAR_CONFIG
    G : INT := 2;
END_VAR
END_CONFIGURATION
"#,
        DiagnosticCode::InvalidOperation,
    );
}

#[test]
fn test_task_missing_priority_error() {
    check_has_error(
        r#"
CONFIGURATION Conf
RESOURCE R ON CPU
    TASK Fast (INTERVAL := T#10ms);
    PROGRAM P1 WITH Fast : Main;
END_RESOURCE
END_CONFIGURATION
"#,
        DiagnosticCode::InvalidTaskConfig,
    );
}

#[test]
fn test_task_single_requires_bool_literal() {
    check_has_error(
        r#"
CONFIGURATION Conf
RESOURCE R ON CPU
    TASK Event (SINGLE := 1, PRIORITY := 1);
    PROGRAM P1 WITH Event : Main;
END_RESOURCE
END_CONFIGURATION
"#,
        DiagnosticCode::InvalidTaskConfig,
    );
}

#[test]
fn test_task_interval_requires_time_literal() {
    check_has_error(
        r#"
CONFIGURATION Conf
RESOURCE R ON CPU
    TASK Fast (INTERVAL := 1, PRIORITY := 1);
    PROGRAM P1 WITH Fast : Main;
END_RESOURCE
END_CONFIGURATION
"#,
        DiagnosticCode::InvalidTaskConfig,
    );
}

#[test]
fn test_program_with_unknown_task_error() {
    check_has_error(
        r#"
CONFIGURATION Conf
RESOURCE R ON CPU
    TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM P1 WITH Missing : Main;
END_RESOURCE
END_CONFIGURATION
"#,
        DiagnosticCode::UnknownTask,
    );
}

#[test]
// IEC 61131-3 Ed.3 Table 13 (VAR_EXTERNAL linkage)
fn test_var_external_missing_global() {
    check_has_error(
        r#"
PROGRAM Test
VAR_EXTERNAL
    G : INT;
END_VAR
END_PROGRAM
"#,
        DiagnosticCode::UndefinedVariable,
    );
}

#[test]
fn test_var_external_type_mismatch() {
    check_has_error(
        r#"
CONFIGURATION Conf
VAR_GLOBAL
    G : INT;
END_VAR
END_CONFIGURATION

PROGRAM Test
VAR_EXTERNAL
    G : DINT;
END_VAR
END_PROGRAM
"#,
        DiagnosticCode::TypeMismatch,
    );
}

#[test]
fn test_var_external_requires_constant_for_global_constant() {
    check_has_error(
        r#"
CONFIGURATION Conf
VAR_GLOBAL CONSTANT
    G : INT := 1;
END_VAR
END_CONFIGURATION

PROGRAM Test
VAR_EXTERNAL
    G : INT;
END_VAR
END_PROGRAM
"#,
        DiagnosticCode::InvalidOperation,
    );
}

#[test]
fn test_var_external_rejects_initializer() {
    check_has_error(
        r#"
CONFIGURATION Conf
VAR_GLOBAL
    G : INT;
END_VAR
END_CONFIGURATION

PROGRAM Test
VAR_EXTERNAL
    G : INT := 1;
END_VAR
END_PROGRAM
"#,
        DiagnosticCode::InvalidOperation,
    );
}

#[test]
// IEC 61131-3 Ed.3 Section 6.5.6 (RETAIN/NON_RETAIN qualifiers)
fn test_var_retain_non_retain_conflict() {
    check_has_error(
        r#"
PROGRAM Test
VAR RETAIN NON_RETAIN
    X : INT;
END_VAR
END_PROGRAM
"#,
        DiagnosticCode::InvalidOperation,
    );
}

#[test]
fn test_var_retain_not_allowed_in_in_out() {
    check_has_error(
        r#"
FUNCTION_BLOCK FB
VAR_IN_OUT RETAIN
    X : INT;
END_VAR
END_FUNCTION_BLOCK
"#,
        DiagnosticCode::InvalidOperation,
    );
}

#[test]
fn test_var_constant_retain_conflict() {
    check_has_error(
        r#"
PROGRAM Test
VAR CONSTANT RETAIN
    X : INT := 1;
END_VAR
END_PROGRAM
"#,
        DiagnosticCode::InvalidOperation,
    );
}

#[test]
fn test_var_persistent_allowed() {
    check_no_errors(
        r#"
PROGRAM Test
VAR PERSISTENT
    X : INT := 1;
END_VAR
END_PROGRAM
"#,
    );
}

#[test]
// IEC 61131-3 Ed.3 Table 16 (AT binding restrictions)
fn test_at_wildcard_not_allowed_in_var_input() {
    check_has_error(
        r#"
PROGRAM Test
VAR_INPUT
    Inp AT %I*: BOOL;
END_VAR
END_PROGRAM
"#,
        DiagnosticCode::InvalidOperation,
    );
}

#[test]
fn test_at_wildcard_requires_var_config() {
    check_has_error(
        r#"
PROGRAM Test
VAR
    Out AT %Q*: BOOL;
END_VAR
END_PROGRAM
"#,
        DiagnosticCode::InvalidOperation,
    );
}

#[test]
fn test_at_wildcard_var_config_requires_full_address() {
    check_has_error(
        r#"
CONFIGURATION Conf
VAR_CONFIG
    Out AT %Q*: BOOL;
END_VAR
END_CONFIGURATION

PROGRAM Test
VAR
    Out AT %Q*: BOOL;
END_VAR
END_PROGRAM
"#,
        DiagnosticCode::InvalidOperation,
    );
}

#[test]
fn test_at_wildcard_var_config_mapping_ok() {
    check_no_errors(
        r#"
CONFIGURATION Conf
VAR_CONFIG
    Out AT %QW0: BOOL;
END_VAR
END_CONFIGURATION

PROGRAM Test
VAR
    Out AT %Q*: BOOL;
END_VAR
END_PROGRAM
"#,
    );
}

#[test]
fn test_var_config_nested_access() {
    check_no_errors(
        r#"
CONFIGURATION Conf
VAR_CONFIG
    P1.fb.out AT %QX0.1 : BOOL;
END_VAR
PROGRAM P1 : Main;
END_CONFIGURATION

FUNCTION_BLOCK FB
VAR_OUTPUT
    out AT %Q*: BOOL;
END_VAR
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    fb : FB;
END_VAR
END_PROGRAM
"#,
    );
}
