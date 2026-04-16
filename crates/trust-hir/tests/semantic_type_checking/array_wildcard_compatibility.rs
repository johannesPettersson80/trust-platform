use crate::common::*;

#[test]
fn parse_array_star_in_regular_var_is_rejected() {
    let mut db = Database::new();
    let file = FileId(0);
    db.set_source_text(
        file,
        r#"
PROGRAM Test
VAR
    x : ARRAY[*] OF BYTE;
END_VAR
END_PROGRAM
"#
        .to_string(),
    );

    assert!(db.diagnostics(file).iter().any(|diagnostic| {
        diagnostic.code == DiagnosticCode::InvalidOperation
            && diagnostic.message.contains("array wildcard")
    }));
}

#[test]
fn parse_array_star_requires_single_dimension() {
    for source in [
        r#"
FUNCTION_BLOCK FB
VAR_IN_OUT
    arr : ARRAY[*, *] OF BYTE;
END_VAR
END_FUNCTION_BLOCK
"#,
        r#"
FUNCTION_BLOCK FB
VAR_IN_OUT
    arr : ARRAY[*, 0..3] OF BYTE;
END_VAR
END_FUNCTION_BLOCK
"#,
    ] {
        let mut db = Database::new();
        let file = FileId(0);
        db.set_source_text(file, source.to_string());
        assert!(db.diagnostics(file).iter().any(|diagnostic| {
            diagnostic.code == DiagnosticCode::InvalidOperation
                && diagnostic.message.contains("ARRAY[*]")
        }));
    }
}

#[test]
fn wildcard_array_param_accepts_any_concrete_bounds() {
    check_no_errors(
        r#"
FUNCTION UseByteArray : BOOL
    VAR_IN_OUT
        arr : ARRAY[*] OF BYTE;
    END_VAR
    UseByteArray := TRUE;
END_FUNCTION

PROGRAM Test
    VAR
        a0 : ARRAY[0..3] OF BYTE;
        a1 : ARRAY[10..20] OF BYTE;
        a2 : ARRAY[0..32767] OF BYTE;
        ok : BOOL;
    END_VAR
    ok := UseByteArray(arr := a0);
    ok := UseByteArray(arr := a1);
    ok := UseByteArray(arr := a2);
END_PROGRAM
"#,
    );
}

#[test]
fn wildcard_array_param_rejects_element_mismatch() {
    check_has_error(
        r#"
FUNCTION UseByteArray : BOOL
    VAR_IN_OUT
        arr : ARRAY[*] OF BYTE;
    END_VAR
    UseByteArray := TRUE;
END_FUNCTION

PROGRAM Test
    VAR
        words : ARRAY[0..3] OF WORD;
        ok : BOOL;
    END_VAR
    ok := UseByteArray(arr := words);
END_PROGRAM
"#,
        DiagnosticCode::InvalidArgumentType,
    );
}

#[test]
fn concrete_array_param_still_rejects_mismatched_bounds() {
    check_has_error(
        r#"
FUNCTION UseFixedArray : BOOL
    VAR_IN_OUT
        arr : ARRAY[0..32000] OF BYTE;
    END_VAR
    UseFixedArray := TRUE;
END_FUNCTION

PROGRAM Test
    VAR
        bytes : ARRAY[0..3] OF BYTE;
        ok : BOOL;
    END_VAR
    ok := UseFixedArray(arr := bytes);
END_PROGRAM
"#,
        DiagnosticCode::InvalidArgumentType,
    );
}

#[test]
fn pointer_to_wildcard_array_accepts_adr_of_any_sized_array() {
    check_no_errors(
        r#"
FUNCTION AcceptPtr : BOOL
    VAR_INPUT
        PT : POINTER TO ARRAY[*] OF BYTE;
    END_VAR
    AcceptPtr := TRUE;
END_FUNCTION

PROGRAM Test
    VAR
        small : ARRAY[0..3] OF BYTE;
        large : ARRAY[0..32000] OF BYTE;
        ok : BOOL;
    END_VAR
    ok := AcceptPtr(PT := ADR(small));
    ok := AcceptPtr(PT := ADR(large));
END_PROGRAM
"#,
    );
}

#[test]
fn pointer_to_concrete_array_still_rejects_mismatched_bounds() {
    check_has_error(
        r#"
FUNCTION AcceptPtr : BOOL
    VAR_INPUT
        PT : POINTER TO ARRAY[0..32000] OF BYTE;
    END_VAR
    AcceptPtr := TRUE;
END_FUNCTION

PROGRAM Test
    VAR
        small : ARRAY[0..3] OF BYTE;
        ok : BOOL;
    END_VAR
    ok := AcceptPtr(PT := ADR(small));
END_PROGRAM
"#,
        DiagnosticCode::InvalidArgumentType,
    );
}

#[test]
fn pointer_to_pointer_explicit_mismatch_diagnostic() {
    let mut db = Database::new();
    let file = FileId(0);
    db.set_source_text(
        file,
        r#"
FUNCTION AcceptPtr : BOOL
    VAR_INPUT
        PT : POINTER TO INT;
    END_VAR
    AcceptPtr := TRUE;
END_FUNCTION

PROGRAM Test
    VAR
        value : REAL;
        ok : BOOL;
    END_VAR
    ok := AcceptPtr(PT := ADR(value));
END_PROGRAM
"#
        .to_string(),
    );

    assert!(db.diagnostics(file).iter().any(|diagnostic| {
        diagnostic.code == DiagnosticCode::InvalidArgumentType
            && diagnostic.message.contains("pointer target")
    }));
}

#[test]
fn wildcard_array_returned_is_rejected() {
    let mut db = Database::new();
    let file = FileId(0);
    db.set_source_text(
        file,
        r#"
FUNCTION F : ARRAY[*] OF BYTE
END_FUNCTION
"#
        .to_string(),
    );

    assert!(db.diagnostics(file).iter().any(|diagnostic| {
        diagnostic.code == DiagnosticCode::InvalidOperation
            && diagnostic.message.contains("ARRAY[*]")
    }));
}
