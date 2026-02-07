mod common;
use common::*;

// Scope Tests
#[test]
fn test_scope_resolution() {
    let mut db = Database::new();
    let file = FileId(0);
    db.set_source_text(
        file,
        r#"
PROGRAM Test
    VAR
        x : INT;
        y : BOOL;
    END_VAR
    x := 10;
    y := TRUE;
END_PROGRAM
"#
        .to_string(),
    );

    let symbols = db.file_symbols(file);

    // Find the program
    let prog = symbols.iter().find(|s| s.name == "Test").unwrap();
    assert!(matches!(prog.kind, SymbolKind::Program));

    // Find variables
    let x = symbols.iter().find(|s| s.name == "x").unwrap();
    let y = symbols.iter().find(|s| s.name == "y").unwrap();

    // Variables should have the program as parent
    assert_eq!(x.parent, Some(prog.id));
    assert_eq!(y.parent, Some(prog.id));
}

#[test]
fn test_test_program_scope_resolution() {
    let mut db = Database::new();
    let file = FileId(0);
    db.set_source_text(
        file,
        r#"
TEST_PROGRAM TestSuite
    VAR
        x : INT;
    END_VAR
    x := 10;
END_TEST_PROGRAM
"#
        .to_string(),
    );

    let symbols = db.file_symbols(file);
    let test_program = symbols.iter().find(|s| s.name == "TestSuite").unwrap();
    assert!(matches!(test_program.kind, SymbolKind::Program));

    let x = symbols.iter().find(|s| s.name == "x").unwrap();
    assert_eq!(x.parent, Some(test_program.id));
}

#[test]
fn test_test_function_block_scope_resolution() {
    let mut db = Database::new();
    let file = FileId(0);
    db.set_source_text(
        file,
        r#"
TEST_FUNCTION_BLOCK FB_TestCase
    VAR
        x : INT;
    END_VAR
    x := 10;
END_TEST_FUNCTION_BLOCK
"#
        .to_string(),
    );

    let symbols = db.file_symbols(file);
    let test_fb = symbols.iter().find(|s| s.name == "FB_TestCase").unwrap();
    assert!(matches!(test_fb.kind, SymbolKind::FunctionBlock));

    let x = symbols.iter().find(|s| s.name == "x").unwrap();
    assert_eq!(x.parent, Some(test_fb.id));
}

#[test]
fn test_method_scope_resolution() {
    check_no_errors(
        r#"
FUNCTION_BLOCK Counter
    VAR
        x : DINT;
    END_VAR

    METHOD DoIt
        VAR
            y : DINT;
        END_VAR
        y := x;
    END_METHOD
END_FUNCTION_BLOCK
"#,
    );
}

#[test]
fn test_super_field_access() {
    check_no_errors(
        r#"
FUNCTION_BLOCK Base
    VAR
        x : DINT;
    END_VAR
END_FUNCTION_BLOCK

FUNCTION_BLOCK Derived EXTENDS Base
    METHOD Update
        SUPER.x := 1;
    END_METHOD
END_FUNCTION_BLOCK
"#,
    );
}

#[test]
fn test_inherited_member_resolution() {
    check_no_errors(
        r#"
CLASS Base
    VAR
        Value : INT;
    END_VAR
END_CLASS

CLASS Derived EXTENDS Base
    METHOD Use
        Value := INT#1;
    END_METHOD
END_CLASS
"#,
    );
}

#[test]
fn test_private_member_access_error() {
    check_has_error(
        r#"
CLASS Base
    VAR PRIVATE
        Secret : INT;
    END_VAR
END_CLASS

CLASS Derived EXTENDS Base
    METHOD Use
        Secret := 1;
    END_METHOD
END_CLASS
"#,
        DiagnosticCode::InvalidOperation,
    );
}

#[test]
fn test_internal_member_access_outside_namespace_error() {
    check_has_error(
        r#"
NAMESPACE A
CLASS C
    VAR INTERNAL
        Hidden : INT;
    END_VAR
END_CLASS
END_NAMESPACE

PROGRAM Main
    VAR
        Obj : A.C;
    END_VAR
    Obj.Hidden := 1;
END_PROGRAM
"#,
        DiagnosticCode::InvalidOperation,
    );
}

#[test]
fn test_internal_member_access_inside_namespace_ok() {
    check_no_errors(
        r#"
NAMESPACE A
CLASS C
    VAR INTERNAL
        Hidden : INT;
    END_VAR
END_CLASS

PROGRAM Main
    VAR
        Obj : C;
    END_VAR
    Obj.Hidden := INT#1;
END_PROGRAM
END_NAMESPACE
"#,
    );
}

#[test]
fn test_property_missing_getter_error() {
    check_has_error(
        r#"
FUNCTION_BLOCK FB_Test
    PROPERTY WriteOnly : INT
    SET
    END_SET
    END_PROPERTY

    METHOD Use
        VAR
            x : INT;
        END_VAR
        x := WriteOnly;
    END_METHOD
END_FUNCTION_BLOCK
"#,
        DiagnosticCode::InvalidOperation,
    );
}

#[test]
fn test_property_setter_only_assignment_ok() {
    check_no_errors(
        r#"
FUNCTION_BLOCK FB_Test
    PROPERTY WriteOnly : INT
    SET
    END_SET
    END_PROPERTY

    METHOD Use
        WriteOnly := INT#1;
    END_METHOD
END_FUNCTION_BLOCK
"#,
    );
}

#[test]
fn test_action_body_type_checked() {
    check_has_error(
        r#"
FUNCTION_BLOCK FB_Test
    VAR
        x : INT;
    END_VAR

    ACTION DoIt
        x := 'oops';
    END_ACTION
END_FUNCTION_BLOCK
"#,
        DiagnosticCode::IncompatibleAssignment,
    );
}
