use crate::common::*;

#[test]
fn assign_to_var_input_pointer_itself_is_rejected() {
    let mut db = Database::new();
    let file = FileId(0);
    db.set_source_text(
        file,
        r#"
FUNCTION_BLOCK FB_Test
    VAR_INPUT
        PT : POINTER TO INT;
    END_VAR
    VAR
        Other : INT;
    END_VAR
    PT := ADR(Other);
END_FUNCTION_BLOCK
"#
        .to_string(),
    );

    let diagnostics = db.diagnostics(file);
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.code == DiagnosticCode::InvalidAssignmentTarget
            && diagnostic
                .message
                .contains("cannot assign to input parameter 'PT'")
    }));
}

#[test]
fn assign_through_var_input_pointer_deref_is_accepted() {
    check_no_errors(
        r#"
FUNCTION_BLOCK FB_Test
    VAR_INPUT
        PT : POINTER TO INT;
    END_VAR
    PT^ := INT#42;
END_FUNCTION_BLOCK
"#,
    );
}

#[test]
fn assign_through_var_input_pointer_array_deref_is_accepted() {
    check_no_errors(
        r#"
FUNCTION_BLOCK FB_Test
    VAR_INPUT
        PT : POINTER TO ARRAY[0..9] OF BYTE;
    END_VAR
    PT^[2] := BYTE#7;
END_FUNCTION_BLOCK
"#,
    );
}

#[test]
fn assign_through_var_input_pointer_struct_deref_is_accepted() {
    check_no_errors(
        r#"
TYPE
    S : STRUCT
        F : INT;
    END_STRUCT
END_TYPE

FUNCTION_BLOCK FB_Test
    VAR_INPUT
        PT : POINTER TO S;
    END_VAR
    PT^.F := INT#3;
END_FUNCTION_BLOCK
"#,
    );
}

#[test]
fn assign_to_field_of_var_input_fb_instance_is_still_rejected() {
    let mut db = Database::new();
    let file = FileId(0);
    db.set_source_text(
        file,
        r#"
FUNCTION_BLOCK Inner
    VAR
        Field : INT;
    END_VAR
END_FUNCTION_BLOCK

FUNCTION_BLOCK Outer
    VAR_INPUT
        FbIn : Inner;
    END_VAR
    FbIn.Field := INT#1;
END_FUNCTION_BLOCK
"#
        .to_string(),
    );

    assert!(
        db.diagnostics(file)
            .iter()
            .any(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error),
        "expected assignment to VAR_INPUT FB field to stay rejected"
    );
}

#[test]
fn assign_through_var_input_pointer_to_nested_index_is_accepted() {
    check_no_errors(
        r#"
TYPE
    S : STRUCT
        F : INT;
    END_STRUCT
END_TYPE

FUNCTION_BLOCK FB_Test
    VAR_INPUT
        PT : POINTER TO ARRAY[0..9] OF S;
    END_VAR
    PT^[3].F := INT#5;
END_FUNCTION_BLOCK
"#,
    );
}
