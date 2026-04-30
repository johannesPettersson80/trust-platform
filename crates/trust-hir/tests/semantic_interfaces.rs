mod common;
use common::*;

// Interface Implementation Tests
#[test]
fn test_interface_missing_method_error() {
    check_has_error(
        r#"
INTERFACE IDevice
    METHOD Start
    END_METHOD
    METHOD Stop
    END_METHOD
END_INTERFACE

CLASS Motor IMPLEMENTS IDevice
    METHOD PUBLIC Start
    END_METHOD
END_CLASS
"#,
        DiagnosticCode::InvalidOperation,
    );
}

#[test]
fn test_interface_missing_method_allowed_on_abstract_class() {
    check_no_errors(
        r#"
INTERFACE IDevice
    METHOD Start
    END_METHOD
    METHOD Stop
    END_METHOD
END_INTERFACE

CLASS ABSTRACT Motor IMPLEMENTS IDevice
    METHOD PUBLIC Start
    END_METHOD
    METHOD PUBLIC ABSTRACT Stop
    END_METHOD
END_CLASS
"#,
    );
}

#[test]
fn namespaced_function_block_implements_sibling_interface_not_global_bare_name() {
    check_no_errors(
        r#"
INTERFACE IProbe
    METHOD Wrong : INT
    END_METHOD
END_INTERFACE

NAMESPACE CellA
INTERFACE IProbe
    METHOD Read : INT
    END_METHOD
END_INTERFACE

FUNCTION_BLOCK Probe IMPLEMENTS IProbe
    METHOD PUBLIC Read : INT
        Read := INT#7;
    END_METHOD
END_FUNCTION_BLOCK
END_NAMESPACE
"#,
    );
}

#[test]
fn namespaced_interface_extends_sibling_interface_not_global_bare_name() {
    check_no_errors(
        r#"
INTERFACE IBase
    METHOD Wrong : INT
    END_METHOD
END_INTERFACE

NAMESPACE CellA
INTERFACE IBase
    METHOD Read : INT
    END_METHOD
END_INTERFACE

INTERFACE IChild EXTENDS IBase
END_INTERFACE

FUNCTION_BLOCK Probe IMPLEMENTS IChild
    METHOD PUBLIC Read : INT
        Read := INT#7;
    END_METHOD
END_FUNCTION_BLOCK
END_NAMESPACE
"#,
    );
}

#[test]
fn test_interface_conformance_cross_file() {
    let mut db = Database::new();
    db.set_source_text(
        FileId(0),
        r#"
CLASS Motor IMPLEMENTS IDevice
    METHOD PUBLIC Start
    END_METHOD
END_CLASS
"#
        .to_string(),
    );
    db.set_source_text(
        FileId(1),
        r#"
INTERFACE IDevice
    METHOD Start
    END_METHOD
END_INTERFACE
"#
        .to_string(),
    );

    let errors: Vec<_> = db
        .diagnostics(FileId(0))
        .iter()
        .filter(|d| d.severity == DiagnosticSeverity::Error)
        .map(|d| d.code)
        .collect();
    assert!(errors.is_empty(), "Expected no errors, got: {:?}", errors);
}

#[test]
fn test_cross_file_concrete_implements_interface_assignment() {
    check_no_errors_multi(&[
        r#"
TEST_PROGRAM Probe
VAR
    concrete : Motor;
    contract : IDevice;
END_VAR
contract := concrete;
contract.Start();
END_TEST_PROGRAM
"#,
        r#"
INTERFACE IDevice
    METHOD Start
    END_METHOD
END_INTERFACE

FUNCTION_BLOCK Motor IMPLEMENTS IDevice
    METHOD PUBLIC Start
    END_METHOD
END_FUNCTION_BLOCK
"#,
    ]);
}

#[test]
fn test_interface_extends_non_interface_error() {
    check_has_error(
        r#"
CLASS Base
END_CLASS

INTERFACE IChild EXTENDS Base
END_INTERFACE
"#,
        DiagnosticCode::InvalidOperation,
    );
}

#[test]
fn test_interface_extends_cycle_error() {
    check_has_error(
        r#"
INTERFACE IA EXTENDS IB
END_INTERFACE

INTERFACE IB EXTENDS IA
END_INTERFACE
"#,
        DiagnosticCode::CyclicDependency,
    );
}

#[test]
fn test_function_block_extends_invalid_type_error() {
    check_has_error(
        r#"
INTERFACE IDevice
    METHOD Start
    END_METHOD
END_INTERFACE

FUNCTION_BLOCK FB EXTENDS IDevice
END_FUNCTION_BLOCK
"#,
        DiagnosticCode::InvalidOperation,
    );
}

#[test]
fn test_function_block_extends_cycle_error() {
    check_has_error(
        r#"
FUNCTION_BLOCK A EXTENDS B
END_FUNCTION_BLOCK

FUNCTION_BLOCK B EXTENDS A
END_FUNCTION_BLOCK
"#,
        DiagnosticCode::CyclicDependency,
    );
}

#[test]
fn test_function_block_extends_final_class_error() {
    check_has_error(
        r#"
CLASS FINAL Base
END_CLASS

FUNCTION_BLOCK FB EXTENDS Base
END_FUNCTION_BLOCK
"#,
        DiagnosticCode::InvalidOperation,
    );
}

#[test]
fn test_interface_signature_mismatch_error() {
    check_has_error(
        r#"
INTERFACE ICalc
    METHOD Compute : INT
        VAR_INPUT
            Value : INT;
        END_VAR
    END_METHOD
END_INTERFACE

CLASS Calc IMPLEMENTS ICalc
    METHOD PUBLIC Compute : INT
        VAR_INPUT
            Value : REAL;
        END_VAR
    END_METHOD
END_CLASS
"#,
        DiagnosticCode::InvalidOperation,
    );
}

#[test]
fn test_interface_visibility_error() {
    check_has_error(
        r#"
INTERFACE IDevice
    METHOD Start
    END_METHOD
END_INTERFACE

CLASS Motor IMPLEMENTS IDevice
    METHOD PRIVATE Start
    END_METHOD
END_CLASS
"#,
        DiagnosticCode::InvalidOperation,
    );
}

#[test]
fn test_interface_property_accessor_error() {
    check_has_error(
        r#"
INTERFACE IProp
    PROPERTY Value : INT
    GET END_GET
    SET END_SET
    END_PROPERTY
END_INTERFACE

CLASS Impl IMPLEMENTS IProp
    PROPERTY Value : INT
    GET
        RETURN 1;
    END_GET
    END_PROPERTY
END_CLASS
"#,
        DiagnosticCode::InvalidOperation,
    );
}
