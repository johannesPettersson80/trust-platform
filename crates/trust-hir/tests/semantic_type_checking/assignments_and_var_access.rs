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
fn test_numeric_widening_assignment_uses_compatibility_matrix() {
    check_no_errors(
        r#"
PROGRAM Test
VAR
    small : INT;
    wide : DINT;
END_VAR
wide := small;
END_PROGRAM
"#,
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
fn test_function_return_variable_can_be_read_inside_function() {
    check_no_errors(
        r#"
FUNCTION CeilLike : DINT
    VAR_INPUT
        x : DINT;
    END_VAR
    CeilLike := x;
    IF CeilLike < DINT#10 THEN
        CeilLike := CeilLike + 1;
    END_IF
END_FUNCTION
"#,
    );
}

#[test]
fn test_method_return_variable_can_be_read_inside_method() {
    check_no_errors(
        r#"
FUNCTION CheckLoaded : BOOL
    VAR_INPUT
        Loaded : BOOL;
    END_VAR
    CheckLoaded := Loaded;
END_FUNCTION

CLASS Context
    METHOD PUBLIC LoadConstants : BOOL
        LoadConstants := TRUE;
        LoadConstants := CheckLoaded(Loaded := LoadConstants);
    END_METHOD
END_CLASS
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
fn test_function_bare_return_allowed_after_assigning_return_target_on_same_path() {
    check_no_errors(
        r#"
FUNCTION Add : DINT
    VAR_INPUT
        a : DINT;
    END_VAR
    Add := a;
    RETURN;
END_FUNCTION
"#,
    );
}

#[test]
fn test_function_bare_return_rejected_when_return_target_not_definitely_assigned() {
    check_has_error(
        r#"
FUNCTION Add : DINT
    VAR_INPUT
        a : DINT;
        cond : BOOL;
    END_VAR
    IF cond THEN
        Add := a;
    END_IF;
    RETURN;
END_FUNCTION
"#,
        DiagnosticCode::MissingReturn,
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
fn test_array_index_const_eval_error_reports_primary_diagnostic() {
    check_has_error(
        r#"
PROGRAM Test
    VAR arr : ARRAY[0..3] OF DINT; x : DINT; END_VAR
    x := arr[1 / 0];
END_PROGRAM
"#,
        DiagnosticCode::InvalidOperation,
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
fn test_subrange_assignment_const_eval_error_reports_primary_diagnostic() {
    check_has_error(
        r#"
TYPE Small : INT (0..10);
END_TYPE

PROGRAM Test
VAR
    value : Small;
END_VAR
value := MissingConstant;
END_PROGRAM
"#,
        DiagnosticCode::UndefinedVariable,
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
fn test_string_indexing_is_allowed() {
    check_no_errors(
        r#"
PROGRAM Test
    VAR s : STRING[8] := 'ABCD'; ws : WSTRING[8] := "WXYZ"; c : CHAR; wc : WCHAR; END_VAR
    c := s[2];
    wc := ws[3];
END_PROGRAM
"#,
    );
}

#[test]
fn test_string_indexing_requires_single_index() {
    check_has_error(
        r#"
PROGRAM Test
    VAR s : STRING[8] := 'ABCD'; c : CHAR; END_VAR
    c := s[1, 2];
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
fn test_var_access_undefined_target_error() {
    check_has_error(
        r#"
CONFIGURATION Conf
VAR_ACCESS
    A : MissingGlobal : INT READ_WRITE;
END_VAR
END_CONFIGURATION
"#,
        DiagnosticCode::UndefinedVariable,
    );
}

#[test]
fn test_var_config_undefined_target_error() {
    check_has_error(
        r#"
CONFIGURATION Conf
VAR_CONFIG
    MissingGlobal AT %QW0 : INT;
END_VAR
END_CONFIGURATION
"#,
        DiagnosticCode::UndefinedVariable,
    );
}

#[test]
fn test_cross_file_global_import_collision_reports_duplicate() {
    let mut db = Database::new();
    db.set_source_text(
        FileId(0),
        r#"
VAR_GLOBAL
    Shared : INT;
END_VAR
"#
        .to_string(),
    );
    db.set_source_text(
        FileId(1),
        r#"
VAR_GLOBAL
    Shared : DINT;
END_VAR
"#
        .to_string(),
    );
    db.set_source_text(
        FileId(2),
        r#"
PROGRAM Main
VAR
    x : DINT;
END_VAR
x := Shared;
END_PROGRAM
"#
        .to_string(),
    );

    let errors = db
        .diagnostics(FileId(2))
        .iter()
        .filter(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
        .map(|diagnostic| diagnostic.code)
        .collect::<Vec<_>>();
    assert!(
        errors.contains(&DiagnosticCode::DuplicateDeclaration),
        "expected duplicate imported global diagnostic, got {errors:?}"
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
fn test_program_config_wrong_kind_type_reports_diagnostic() {
    let errors = check_errors(
        r#"
FUNCTION_BLOCK NotAProgram
END_FUNCTION_BLOCK

CONFIGURATION Conf
RESOURCE R ON CPU
    PROGRAM P1 : NotAProgram;
END_RESOURCE
END_CONFIGURATION
"#,
    );
    assert!(
        errors.contains(&DiagnosticCode::InvalidOperation),
        "expected wrong-kind InvalidOperation diagnostic, got {errors:?}"
    );
    assert!(
        !errors.contains(&DiagnosticCode::UndefinedType)
            && !errors.contains(&DiagnosticCode::CannotResolve),
        "program config wrong-kind must not degrade into wrong-reason unresolved diagnostics, got {errors:?}"
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
fn test_var_external_matches_program_scoped_global() {
    check_no_errors(
        r#"
PROGRAM Main
VAR_GLOBAL
    G : INT;
END_VAR
END_PROGRAM

FUNCTION_BLOCK UsesGlobal
VAR_EXTERNAL
    G : INT;
END_VAR
END_FUNCTION_BLOCK
"#,
    );
}

#[test]
fn test_bare_global_access_is_accepted_across_pou_kinds() {
    check_no_errors(
        r#"
CONFIGURATION Conf
VAR_GLOBAL
    G : INT := 1;
END_VAR
END_CONFIGURATION

PROGRAM Main
VAR
    P : INT;
END_VAR
P := G;
END_PROGRAM

FUNCTION FnProbe : INT
FnProbe := G;
END_FUNCTION

FUNCTION_BLOCK FbProbe
VAR
    LocalCopy : INT;
END_VAR
LocalCopy := G;
END_FUNCTION_BLOCK

CLASS CProbe
METHOD PUBLIC DoThing
G := G + 1;
END_METHOD
END_CLASS
"#,
    );
}

#[test]
fn test_bare_configuration_global_access_resolves_across_files() {
    check_no_errors_multi(&[
        r#"
PROGRAM Main
VAR
    observed : INT;
END_VAR
observed := gConfig;
gConfig := gConfig + 1;
END_PROGRAM
"#,
        r#"
CONFIGURATION Conf
VAR_GLOBAL
    gConfig : INT := 4;
END_VAR
PROGRAM P1 : Main;
END_CONFIGURATION
"#,
    ]);
}

#[test]
fn test_function_block_bare_missing_name_is_rejected() {
    check_has_error(
        r#"
FUNCTION_BLOCK FbProbe
VAR
    LocalCopy : INT;
END_VAR
LocalCopy := Missing;
END_FUNCTION_BLOCK
"#,
        DiagnosticCode::UndefinedVariable,
    );
}

#[test]
fn test_program_bare_missing_name_is_rejected() {
    check_has_error(
        r#"
PROGRAM Main
VAR
    LocalCopy : INT;
END_VAR
LocalCopy := Missing;
END_PROGRAM
"#,
        DiagnosticCode::UndefinedVariable,
    );
}

#[test]
fn test_function_bare_missing_name_is_rejected() {
    check_has_error(
        r#"
FUNCTION FnProbe : INT
FnProbe := Missing;
END_FUNCTION
"#,
        DiagnosticCode::UndefinedVariable,
    );
}

#[test]
fn test_class_method_bare_missing_name_is_rejected() {
    check_has_error(
        r#"
CLASS CProbe
METHOD PUBLIC DoThing
Missing := Missing + 1;
END_METHOD
END_CLASS
"#,
        DiagnosticCode::UndefinedVariable,
    );
}

#[test]
fn test_assignment_unknown_source_suppression_has_primary_diagnostic() {
    let errors = check_errors(
        r#"
PROGRAM Main
VAR
    LocalCopy : INT;
END_VAR
LocalCopy := Missing;
END_PROGRAM
"#,
    );

    assert!(
        errors.contains(&DiagnosticCode::UndefinedVariable),
        "expected primary UndefinedVariable diagnostic, got {errors:?}"
    );
    assert!(
        !errors.contains(&DiagnosticCode::IncompatibleAssignment)
            && !errors.contains(&DiagnosticCode::TypeMismatch),
        "unknown assignment source must not emit wrong-reason assignment/type mismatch cascades, got {errors:?}"
    );
}

#[test]
fn test_binary_unknown_operand_suppression_has_primary_diagnostic() {
    let errors = check_errors(
        r#"
PROGRAM Main
VAR
    LocalCopy : INT;
END_VAR
LocalCopy := Missing + TRUE;
END_PROGRAM
"#,
    );

    assert!(
        errors.contains(&DiagnosticCode::UndefinedVariable),
        "expected primary UndefinedVariable diagnostic, got {errors:?}"
    );
    assert!(
        !errors.contains(&DiagnosticCode::InvalidArgumentType)
            && !errors.contains(&DiagnosticCode::TypeMismatch)
            && !errors.contains(&DiagnosticCode::IncompatibleAssignment),
        "unknown binary operand must not emit wrong-reason operator or assignment cascades, got {errors:?}"
    );
}

#[test]
fn test_unary_unknown_operand_suppression_has_primary_diagnostic() {
    let errors = check_errors(
        r#"
PROGRAM Main
VAR
    LocalCopy : INT;
END_VAR
LocalCopy := -Missing;
END_PROGRAM
"#,
    );

    assert!(
        errors.contains(&DiagnosticCode::UndefinedVariable),
        "expected primary UndefinedVariable diagnostic, got {errors:?}"
    );
    assert!(
        !errors.contains(&DiagnosticCode::TypeMismatch)
            && !errors.contains(&DiagnosticCode::IncompatibleAssignment),
        "unknown unary operand must not emit wrong-reason type or assignment cascades, got {errors:?}"
    );
}

#[test]
fn test_index_unknown_base_suppression_has_primary_diagnostic() {
    let errors = check_errors(
        r#"
PROGRAM Main
VAR
    LocalCopy : INT;
END_VAR
LocalCopy := Missing[0];
END_PROGRAM
"#,
    );

    assert!(
        errors.contains(&DiagnosticCode::UndefinedVariable),
        "expected primary UndefinedVariable diagnostic, got {errors:?}"
    );
    assert!(
        !errors.contains(&DiagnosticCode::TypeMismatch)
            && !errors.contains(&DiagnosticCode::InvalidArrayIndex)
            && !errors.contains(&DiagnosticCode::IncompatibleAssignment),
        "unknown index base must not emit wrong-reason index or assignment cascades, got {errors:?}"
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

#[test]
fn test_var_config_cross_file_program_instance_target_resolves_after_project_merge() {
    check_no_errors_multi(&[
        r#"
CONFIGURATION Conf
RESOURCE R1 ON CPU
    TASK MainTask (INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.DI0 AT %IX0.0 : BOOL;
    P1.DO0 AT %QX0.0 : BOOL;
END_VAR
END_CONFIGURATION
"#,
        r#"
PROGRAM Main
VAR
    DI0 : BOOL;
    DO0 : BOOL;
END_VAR
DO0 := DI0;
END_PROGRAM
"#,
    ]);
}

#[test]
fn test_var_config_duplicate_program_instance_name_is_ambiguous() {
    let errors = check_errors(
        r#"
FUNCTION_BLOCK BoolFb
VAR_OUTPUT
    out AT %Q*: BOOL;
END_VAR
END_FUNCTION_BLOCK

FUNCTION_BLOCK IntFb
VAR_OUTPUT
    out AT %Q*: INT;
END_VAR
END_FUNCTION_BLOCK

PROGRAM MainA
VAR
    fb : BoolFb;
END_VAR
END_PROGRAM

PROGRAM MainB
VAR
    fb : IntFb;
END_VAR
END_PROGRAM

CONFIGURATION Conf
RESOURCE R1 ON CPU
    PROGRAM P1 : MainA;
END_RESOURCE
RESOURCE R2 ON CPU
    PROGRAM P1 : MainB;
END_RESOURCE
VAR_CONFIG
    P1.fb.out AT %QX0.1 : BOOL;
END_VAR
END_CONFIGURATION
"#,
    );

    assert!(
        errors.contains(&DiagnosticCode::CannotResolve),
        "duplicate PROGRAM instance names must be ambiguous, got {errors:?}"
    );
    assert!(
        !errors.contains(&DiagnosticCode::TypeMismatch)
            && !errors.contains(&DiagnosticCode::InvalidOperation),
        "duplicate PROGRAM instance names must not silently choose one instance and report a wrong-reason cascade, got {errors:?}"
    );
}
