use crate::common::*;

#[test]
fn test_string_length_constant_expression() {
    let mut db = Database::new();
    let file = FileId(0);
    db.set_source_text(
        file,
        r#"
FUNCTION_BLOCK FB_Test
    VAR CONSTANT
        Len : DINT := 4;
    END_VAR
    VAR
        name : STRING[Len + 1];
    END_VAR
END_FUNCTION_BLOCK
"#
        .to_string(),
    );

    let symbols = db.file_symbols(file);
    let name = symbols.iter().find(|s| s.name == "name").unwrap();
    let type_id = symbols.resolve_alias_type(name.type_id);
    let Type::String { max_len } = symbols.type_by_id(type_id).unwrap() else {
        panic!("expected string type");
    };
    assert_eq!(*max_len, Some(5));
}

#[test]
fn test_string_literal_length_in_initializer() {
    check_has_error(
        r#"
PROGRAM Test
    VAR
        s : STRING[3] := 'ABCD';
    END_VAR
END_PROGRAM
"#,
        DiagnosticCode::OutOfRange,
    );
}

#[test]
fn test_string_literal_length_in_assignment() {
    check_has_error(
        r#"
PROGRAM Test
    VAR s : STRING[2]; END_VAR
    s := 'ABC';
END_PROGRAM
"#,
        DiagnosticCode::OutOfRange,
    );
}

#[test]
fn test_string_length_assignment_between_lengths() {
    check_no_errors(
        r#"
PROGRAM Test
    VAR
        short : STRING[5];
        long : STRING[20];
    END_VAR
    short := long;
END_PROGRAM
"#,
    );
}

#[test]
fn test_type_alias_numeric_ops() {
    check_no_errors(
        r#"
TYPE MyInt : DINT;
END_TYPE

PROGRAM Test
    VAR x : MyInt; END_VAR
    x := 1;
    x := x + 1;
END_PROGRAM
"#,
    );
}

#[test]
fn test_sizeof_expression() {
    check_no_errors(
        r#"
PROGRAM Test
    VAR size : DINT; x : DINT; END_VAR
    size := SIZEOF(x);
    size := SIZEOF(DINT);
END_PROGRAM
"#,
    );
}

#[test]
fn test_method_call_on_instance() {
    check_no_errors(
        r#"
FUNCTION_BLOCK Counter
    METHOD Get : DINT
        Get := 1;
    END_METHOD
END_FUNCTION_BLOCK

PROGRAM Test
    VAR fb : Counter; value : DINT; END_VAR
    value := fb.Get();
END_PROGRAM
"#,
    );
}

#[test]
fn test_adr_requires_lvalue() {
    check_has_error(
        r#"
PROGRAM Test
    VAR p : POINTER TO DINT; END_VAR
    p := ADR(1);
END_PROGRAM
"#,
        DiagnosticCode::InvalidOperation,
    );
}

#[test]
// IEC 61131-3 Ed.3 Table 12 (reference operators)
fn test_ref_returns_reference() {
    check_no_errors(
        r#"
PROGRAM Test
    VAR x : DINT; r : REF_TO DINT; END_VAR
    r := REF(x);
    r^ := 10;
END_PROGRAM
"#,
    );
}

#[test]
fn test_ref_requires_lvalue() {
    check_has_error(
        r#"
PROGRAM Test
    VAR r : REF_TO INT; END_VAR
    r := REF(1);
END_PROGRAM
"#,
        DiagnosticCode::InvalidOperation,
    );
}

#[test]
fn test_ref_rejects_constant() {
    check_has_error(
        r#"
PROGRAM Test
    VAR CONSTANT c : INT := 1; END_VAR
    VAR r : REF_TO INT; END_VAR
    r := REF(c);
END_PROGRAM
"#,
        DiagnosticCode::InvalidOperation,
    );
}

#[test]
fn test_ref_rejects_temp_variable() {
    check_has_error(
        r#"
PROGRAM Test
    VAR_TEMP
        t : INT;
    END_VAR
    VAR
        r : REF_TO INT;
    END_VAR
    r := REF(t);
END_PROGRAM
"#,
        DiagnosticCode::InvalidOperation,
    );
}

#[test]
fn test_ref_rejects_function_local_variable() {
    check_has_error(
        r#"
FUNCTION Foo : INT
    VAR
        x : INT;
        r : REF_TO INT;
    END_VAR
    r := REF(x);
    Foo := x;
END_FUNCTION
"#,
        DiagnosticCode::InvalidOperation,
    );
}

#[test]
fn test_null_assignment_to_reference() {
    check_no_errors(
        r#"
PROGRAM Test
    VAR r : REF_TO INT; END_VAR
    r := NULL;
END_PROGRAM
"#,
    );
}

#[test]
// IEC 61131-3 Ed.3 Table 12 (reference assignment)
fn test_ref_assignment_requires_reference_target() {
    check_has_error(
        r#"
PROGRAM Test
    VAR x : INT; y : INT; END_VAR
    x ?= y;
END_PROGRAM
"#,
        DiagnosticCode::TypeMismatch,
    );
}

#[test]
fn test_ref_assignment_requires_reference_source() {
    check_has_error(
        r#"
PROGRAM Test
    VAR r : REF_TO INT; x : INT; END_VAR
    r ?= x;
END_PROGRAM
"#,
        DiagnosticCode::TypeMismatch,
    );
}

#[test]
fn test_ref_assignment_allows_reference_source() {
    check_no_errors(
        r#"
PROGRAM Test
    VAR x : INT; r1 : REF_TO INT; r2 : REF_TO INT; END_VAR
    r1 := REF(x);
    r2 ?= r1;
END_PROGRAM
"#,
    );
}

#[test]
fn test_null_comparison_reference() {
    check_no_errors(
        r#"
PROGRAM Test
    VAR r : REF_TO INT; END_VAR
    IF r = NULL THEN
    END_IF;
END_PROGRAM
"#,
    );
}
