use super::common::*;

fn error_messages(source: &str) -> Vec<String> {
    let mut db = Database::new();
    let file = FileId(0);
    db.set_source_text(file, source.to_string());
    db.diagnostics(file)
        .iter()
        .filter(|d| d.severity == DiagnosticSeverity::Error)
        .map(ToString::to_string)
        .collect()
}

#[test]
fn test_sizeof_accepts_variable_operand() {
    check_no_errors(
        r#"
PROGRAM Test
    VAR
        size : DINT;
        x : DINT;
    END_VAR
    size := SIZEOF(x);
END_PROGRAM
"#,
    );
}

#[test]
fn test_sizeof_accepts_explicit_type_operand() {
    check_no_errors(
        r#"
PROGRAM Test
    VAR size : DINT; END_VAR
    size := SIZEOF(DINT);
    size := SIZEOF(ARRAY[0..3] OF BYTE);
END_PROGRAM
"#,
    );
}

#[test]
fn test_sizeof_accepts_this_field_operand_inside_method() {
    check_no_errors(
        r#"
CLASS Counter
VAR
    value : DINT;
END_VAR

METHOD PUBLIC Measure : DINT
Measure := SIZEOF(THIS.value);
END_METHOD
END_CLASS
"#,
    );
}

#[test]
fn test_sizeof_accepts_variable_operand_in_array_bounds() {
    check_no_errors(
        r#"
TYPE Packet :
STRUCT
    a : DINT;
    b : BOOL;
END_STRUCT
END_TYPE

PROGRAM Test
VAR
    packet : Packet;
    bytes : ARRAY[0..SIZEOF(packet)-1] OF BYTE;
END_VAR
END_PROGRAM
"#,
    );
}

#[test]
fn test_sizeof_rejects_call_operand() {
    check_has_error(
        r#"
FUNCTION Value : DINT
Value := DINT#1;
END_FUNCTION

PROGRAM Test
    VAR size : DINT; END_VAR
    size := SIZEOF(Value());
END_PROGRAM
"#,
        DiagnosticCode::InvalidOperation,
    );
}

#[test]
fn test_sizeof_rejects_non_lvalue_expression_operand() {
    check_has_error(
        r#"
PROGRAM Test
    VAR
        size : DINT;
        x : DINT;
    END_VAR
    size := SIZEOF(x + DINT#1);
END_PROGRAM
"#,
        DiagnosticCode::InvalidOperation,
    );
}

#[test]
fn test_sizeof_rejects_unknown_identifier_cleanly() {
    let messages = error_messages(
        r#"
PROGRAM Test
    VAR size : DINT; END_VAR
    size := SIZEOF(DoesNotExist);
END_PROGRAM
"#,
    );
    assert!(
        messages
            .iter()
            .any(|message| message.contains("SIZEOF") && message.contains("DoesNotExist")),
        "expected SIZEOF-specific unknown-identifier diagnostic, got {messages:?}"
    );
}

#[test]
fn test_sizeof_rejects_open_array_operand() {
    check_has_error(
        r#"
FUNCTION_BLOCK BufferUser
VAR_IN_OUT
    arr : ARRAY[*] OF BYTE;
END_VAR
VAR
    size : DINT;
END_VAR
size := SIZEOF(arr);
END_FUNCTION_BLOCK
"#,
        DiagnosticCode::InvalidOperation,
    );
}

#[test]
fn test_sizeof_rejects_function_block_instance_operand() {
    check_has_error(
        r#"
FUNCTION_BLOCK Counter
VAR
    value : DINT;
END_VAR
END_FUNCTION_BLOCK

PROGRAM Test
    VAR
        size : DINT;
        fb : Counter;
    END_VAR
    size := SIZEOF(fb);
END_PROGRAM
"#,
        DiagnosticCode::InvalidOperation,
    );
}

#[test]
fn test_sizeof_rejects_this_operand_for_unsupported_receiver_storage_size() {
    check_has_error(
        r#"
CLASS Counter
VAR
    value : DINT;
END_VAR

METHOD PUBLIC Measure : DINT
Measure := SIZEOF(THIS);
END_METHOD
END_CLASS
"#,
        DiagnosticCode::InvalidOperation,
    );
}
