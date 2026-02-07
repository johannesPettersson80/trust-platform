use crate::common::*;

#[test]
fn test_constant_modification() {
    check_has_error(
        r#"
PROGRAM Test
    VAR CONSTANT
        PI : REAL := 3.14159;
    END_VAR
    PI := 3.0;
END_PROGRAM
"#,
        DiagnosticCode::ConstantModification,
    );
}

#[test]
fn test_constant_struct_field_modification() {
    check_has_error(
        r#"
TYPE
    MyStruct : STRUCT
        field : INT;
    END_STRUCT;
END_TYPE

PROGRAM Test
    VAR CONSTANT
        s : MyStruct;
    END_VAR
    s.field := 1;
END_PROGRAM
"#,
        DiagnosticCode::ConstantModification,
    );
}

#[test]
fn test_boolean_condition_required() {
    check_has_error(
        r#"
PROGRAM Test
    VAR x : DINT; END_VAR
    IF x THEN
        x := 1;
    END_IF;
END_PROGRAM
"#,
        DiagnosticCode::TypeMismatch,
    );
}

#[test]
fn test_boolean_condition_ok() {
    check_no_errors(
        r#"
PROGRAM Test
    VAR x : BOOL; END_VAR
    IF x THEN
        x := FALSE;
    END_IF;
END_PROGRAM
"#,
    );
}

#[test]
fn test_contextual_int_literal_assignment() {
    check_no_errors(
        r#"
PROGRAM Test
    VAR x : INT; END_VAR
    x := 1;
END_PROGRAM
"#,
    );
}

#[test]
fn test_contextual_int_literal_return() {
    check_no_errors(
        r#"
FUNCTION Test : INT
    RETURN 1;
END_FUNCTION
"#,
    );
}

#[test]
fn test_contextual_real_literal_assignment() {
    check_no_errors(
        r#"
PROGRAM Test
    VAR x : REAL; END_VAR
    x := 1.0;
END_PROGRAM
"#,
    );
}

#[test]
fn test_contextual_real_literal_return() {
    check_no_errors(
        r#"
FUNCTION Test : REAL
    RETURN 1.0;
END_FUNCTION
"#,
    );
}

#[test]
fn test_real_literal_in_real_arithmetic() {
    check_no_errors(
        r#"
PROGRAM Test
    VAR sum : REAL; avg : REAL; END_VAR
    avg := sum / 4.0;
END_PROGRAM
"#,
    );
}

#[test]
fn test_real_literal_in_standard_numeric_function() {
    check_no_errors(
        r#"
PROGRAM Test
    VAR x : REAL; y : REAL; END_VAR
    y := MIN(x, 4.0);
END_PROGRAM
"#,
    );
}

#[test]
// IEC 61131-3 Ed.3 Table 11 (subrange types)
fn test_subrange_assignment_out_of_range() {
    check_has_error(
        r#"
TYPE
    Percent : INT(0..100);
END_TYPE

PROGRAM Test
    VAR p : Percent; END_VAR
    p := INT#150;
END_PROGRAM
"#,
        DiagnosticCode::OutOfRange,
    );
}

#[test]
fn test_subrange_assignment_in_range() {
    check_no_errors(
        r#"
TYPE
    Percent : INT(0..100);
END_TYPE

PROGRAM Test
    VAR p : Percent; END_VAR
    p := INT#42;
END_PROGRAM
"#,
    );
}

#[test]
// IEC 61131-3 Ed.3 Table 11 (subrange bounds)
fn test_subrange_bounds_invalid_order() {
    check_has_error(
        r#"
TYPE
    BadRange : INT(10..5);
END_TYPE
"#,
        DiagnosticCode::OutOfRange,
    );
}

#[test]
fn test_subrange_bounds_non_constant() {
    check_has_error(
        r#"
TYPE
    BadRange : INT(A..B);
END_TYPE
"#,
        DiagnosticCode::TypeMismatch,
    );
}

#[test]
fn test_unused_variable_warning() {
    let warnings = check_warnings(
        r#"
PROGRAM Test
    VAR x : INT; END_VAR
END_PROGRAM
"#,
    );
    assert!(warnings.contains(&DiagnosticCode::UnusedVariable));
}

#[test]
fn test_var_config_marks_symbol_used_across_files() {
    let mut db = Database::new();
    db.set_source_text(
        FileId(0),
        r#"
CONFIGURATION Conf
RESOURCE R ON CPU
    TASK Fast (INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH Fast : Main;
END_RESOURCE
VAR_CONFIG
    P1.InSignal : BOOL;
END_VAR
END_CONFIGURATION
"#
        .to_string(),
    );
    db.set_source_text(
        FileId(1),
        r#"
PROGRAM Main
VAR
    InSignal : BOOL;
END_VAR
END_PROGRAM
"#
        .to_string(),
    );

    let warnings: Vec<DiagnosticCode> = db
        .diagnostics(FileId(1))
        .iter()
        .filter(|d| d.severity == DiagnosticSeverity::Warning)
        .map(|d| d.code)
        .collect();

    assert!(
        !warnings.contains(&DiagnosticCode::UnusedVariable),
        "Unexpected unused variable warning: {warnings:?}"
    );
}

#[test]
fn test_unused_parameter_warning() {
    let warnings = check_warnings(
        r#"
FUNCTION Add : INT
    VAR_INPUT
        a : INT;
    END_VAR
    Add := 1;
END_FUNCTION
"#,
    );
    assert!(warnings.contains(&DiagnosticCode::UnusedParameter));
}

#[test]
fn test_implicit_conversion_warning() {
    let warnings = check_warnings(
        r#"
PROGRAM Test
    VAR x : REAL; END_VAR
    x := 1;
END_PROGRAM
"#,
    );
    assert!(warnings.contains(&DiagnosticCode::ImplicitConversion));
}

#[test]
fn test_cyclomatic_complexity_warning() {
    let mut body = String::new();
    for _ in 0..15 {
        body.push_str("    IF TRUE THEN\n        x := x + 1;\n    END_IF;\n");
    }
    let source = format!(
        r#"
PROGRAM Test
    VAR
        x : INT;
    END_VAR
{body}
END_PROGRAM
"#
    );
    let warnings = check_warnings(&source);
    assert!(warnings.contains(&DiagnosticCode::HighComplexity));
}

#[test]
fn test_unused_pou_warning() {
    let warnings = check_warnings(
        r#"
FUNCTION Foo : INT
    Foo := 1;
END_FUNCTION
"#,
    );
    assert!(warnings.contains(&DiagnosticCode::UnusedPou));
}

#[test]
fn test_unreachable_code_warning() {
    let warnings = check_warnings(
        r#"
FUNCTION Foo : INT
VAR
    x : INT;
END_VAR
    Foo := 0;
    RETURN;
    x := 1;
END_FUNCTION
"#,
    );
    assert!(warnings.contains(&DiagnosticCode::UnreachableCode));
}

#[test]
fn test_unreachable_if_false_branch_warning() {
    let warnings = check_warnings(
        r#"
PROGRAM Test
VAR
    x : INT;
END_VAR
    IF FALSE THEN
        x := 1;
    ELSE
        x := 2;
    END_IF;
END_PROGRAM
"#,
    );
    assert!(warnings.contains(&DiagnosticCode::UnreachableCode));
}

#[test]
fn test_unreachable_elsif_false_branch_warning() {
    let warnings = check_warnings(
        r#"
PROGRAM Test
VAR
    x : INT;
END_VAR
    IF FALSE THEN
        x := 1;
    ELSIF FALSE THEN
        x := 2;
    ELSE
        x := 3;
    END_IF;
END_PROGRAM
"#,
    );
    assert!(warnings.contains(&DiagnosticCode::UnreachableCode));
}

#[test]
fn test_nondeterministic_time_date_warning() {
    let warnings = check_warnings(
        r#"
PROGRAM Test
    VAR
        t : TIME;
        d : DATE;
    END_VAR
END_PROGRAM
"#,
    );
    assert!(warnings.contains(&DiagnosticCode::NondeterministicTimeDate));
}

#[test]
fn test_nondeterministic_io_warning() {
    let warnings = check_warnings(
        r#"
PROGRAM Test
    VAR
        input AT %IX0.0 : BOOL;
        output AT %QW1 : INT;
    END_VAR
END_PROGRAM
"#,
    );
    assert!(warnings.contains(&DiagnosticCode::NondeterministicIo));
}

#[test]
fn test_shared_global_task_hazard_warning() {
    let warnings = check_warnings(
        r#"
CONFIGURATION Conf
VAR_GLOBAL
    Shared : INT;
END_VAR
RESOURCE R ON CPU
    TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);
    TASK Slow (INTERVAL := T#20ms, PRIORITY := 2);
    PROGRAM P1 WITH Fast : Writer;
    PROGRAM P2 WITH Slow : Reader;
END_RESOURCE
END_CONFIGURATION

PROGRAM Writer
    Shared := Shared + 1;
END_PROGRAM

PROGRAM Reader
    VAR x : INT; END_VAR
    x := Shared;
END_PROGRAM
"#,
    );
    assert!(warnings.contains(&DiagnosticCode::SharedGlobalTaskHazard));
}

#[test]
fn test_shared_global_task_hazard_single_task_no_warning() {
    let warnings = check_warnings(
        r#"
CONFIGURATION Conf
VAR_GLOBAL
    Shared : INT;
END_VAR
RESOURCE R ON CPU
    TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM P1 WITH Fast : Writer;
    PROGRAM P2 WITH Fast : Reader;
END_RESOURCE
END_CONFIGURATION

PROGRAM Writer
    Shared := Shared + 1;
END_PROGRAM

PROGRAM Reader
    VAR x : INT; END_VAR
    x := Shared;
END_PROGRAM
"#,
    );
    assert!(!warnings.contains(&DiagnosticCode::SharedGlobalTaskHazard));
}

#[test]
fn test_used_function_no_unused_pou_warning() {
    let warnings = check_warnings(
        r#"
CONFIGURATION Conf
RESOURCE R ON CPU
    TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM P1 WITH Fast : Main;
END_RESOURCE
END_CONFIGURATION

FUNCTION Foo : INT
    Foo := 1;
END_FUNCTION

PROGRAM Main
VAR
    x : INT;
END_VAR
    x := Foo();
END_PROGRAM
"#,
    );
    assert!(
        !warnings.contains(&DiagnosticCode::UnusedPou),
        "Unexpected unused POU warning: {warnings:?}"
    );
}

#[test]
fn test_function_block_used_as_type_no_unused_pou_warning() {
    let warnings = check_warnings(
        r#"
CONFIGURATION Conf
RESOURCE R ON CPU
    TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM P1 WITH Fast : Main;
END_RESOURCE
END_CONFIGURATION

FUNCTION_BLOCK FB
VAR
    x : INT;
END_VAR
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    inst : FB;
END_VAR
END_PROGRAM
"#,
    );
    assert!(
        !warnings.contains(&DiagnosticCode::UnusedPou),
        "Unexpected unused POU warning: {warnings:?}"
    );
}
