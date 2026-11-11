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
        DiagnosticCode::InterfaceConformance,
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
        DiagnosticCode::InterfaceConformance,
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
        DiagnosticCode::InterfaceConformance,
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
        DiagnosticCode::InterfaceConformance,
    );
}

/// Specification 04: `__SYSTEM.IQueryInterface` after an interface's EXTENDS and in an IMPLEMENTS
/// list adds nothing, so a block that implements the interface conforms with the interface's own
/// method, and an interface variable takes the block. Catches the system name taken for a base the
/// analysis cannot find (E102), and the block's own interface lost beside it.
#[test]
fn test_system_query_interface_extends_and_implements_add_nothing() {
    check_no_errors(
        r#"
INTERFACE ILamp EXTENDS __SYSTEM.IQueryInterface
    METHOD Toggle : BOOL
    END_METHOD
END_INTERFACE

FUNCTION_BLOCK FB_Lamp IMPLEMENTS __SYSTEM.IQueryInterface, ILamp
VAR
    isOn : BOOL;
END_VAR
    METHOD PUBLIC Toggle : BOOL
        isOn := NOT isOn;
        Toggle := isOn;
    END_METHOD
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    lamp : FB_Lamp;
    itf : ILamp;
    lit : BOOL;
END_VAR
    itf := lamp;
    lit := itf.Toggle();
END_PROGRAM
"#,
    );
}

/// Specification 04: a block still implements every method of its ordinary interfaces beside the
/// system interface. Catches the system name hiding the conformance check of the names after it.
#[test]
fn test_system_query_interface_keeps_the_other_interfaces_checked() {
    check_has_error(
        r#"
INTERFACE ILamp
    METHOD Toggle : BOOL
    END_METHOD
END_INTERFACE

FUNCTION_BLOCK FB_Lamp IMPLEMENTS __SYSTEM.IQueryInterface, ILamp
END_FUNCTION_BLOCK
"#,
        DiagnosticCode::InterfaceConformance,
    );
}

/// Specification 04: truST knows only `__SYSTEM.IQueryInterface`, and only an interface extends
/// it. Catches any `__SYSTEM` name accepted, and a function block's EXTENDS of it accepted.
#[test]
fn test_unknown_system_names_are_refused() {
    check_has_error(
        r#"
INTERFACE ILamp EXTENDS __SYSTEM.IUnknown
END_INTERFACE
"#,
        DiagnosticCode::UndefinedType,
    );
    check_has_error(
        r#"
FUNCTION_BLOCK FB_Lamp EXTENDS __SYSTEM.IQueryInterface
END_FUNCTION_BLOCK
"#,
        DiagnosticCode::InvalidOperation,
    );
}

/// The errors of one source file, with their messages and their places.
fn errors_with_places(source: &str) -> Vec<(DiagnosticCode, String, usize, usize)> {
    let mut db = Database::new();
    db.set_source_text(FileId(0), source.to_string());
    let mut errors: Vec<_> = db
        .diagnostics(FileId(0))
        .iter()
        .filter(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
        .map(|diagnostic| {
            (
                diagnostic.code,
                diagnostic.message.clone(),
                usize::from(diagnostic.range.start()),
                usize::from(diagnostic.range.end()),
            )
        })
        .collect();
    errors.sort_by(|left, right| left.1.cmp(&right.1));
    errors
}

const START_AND_STOP: &str = r#"
INTERFACE IStart
    METHOD Start : BOOL
    END_METHOD
END_INTERFACE

INTERFACE IStop
    METHOD Stop : BOOL
    END_METHOD
END_INTERFACE
"#;

/// Specification 04, "Interface Inheritance": an interface extends several interfaces and inherits
/// every prototype of each, so a block implementing it implements them all, and a value of it, or a
/// block implementing it, converts to each base and calls each base's method (IEC 61131-3 Ed.3
/// §6.6.6.6, §6.6.6.5.1). Catches the second base lost: its method missing from the interface, a
/// value refused for a variable of the second base, and the call through the value unresolved.
#[test]
fn an_interface_that_extends_several_interfaces_inherits_each_base() {
    check_no_errors(&format!(
        r#"{START_AND_STOP}
INTERFACE IMotor EXTENDS IStart, IStop
    METHOD Speed : INT
    END_METHOD
END_INTERFACE

FUNCTION_BLOCK FB_Motor IMPLEMENTS IMotor
    METHOD PUBLIC Start : BOOL
        Start := TRUE;
    END_METHOD
    METHOD PUBLIC Stop : BOOL
        Stop := TRUE;
    END_METHOD
    METHOD PUBLIC Speed : INT
        Speed := 3;
    END_METHOD
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    motor : FB_Motor;
    itf : IMotor;
    starter : IStart;
    stopper : IStop;
    done : BOOL;
    speed : INT;
END_VAR
    itf := motor;
    starter := itf;
    stopper := itf;
    stopper := motor;
    done := itf.Start() AND itf.Stop();
    speed := itf.Speed();
END_PROGRAM
"#
    ));
}

/// Specification 04: a block that implements an interface owes the prototypes of every base.
/// Catches the conformance check reading only the first base.
#[test]
fn a_block_owes_the_methods_of_every_base_of_its_interface() {
    let errors = errors_with_places(&format!(
        r#"{START_AND_STOP}
INTERFACE IMotor EXTENDS IStart, IStop
END_INTERFACE

FUNCTION_BLOCK FB_Motor IMPLEMENTS IMotor
    METHOD PUBLIC Start : BOOL
        Start := TRUE;
    END_METHOD
END_FUNCTION_BLOCK
"#
    ));
    let messages: Vec<&str> = errors.iter().map(|error| error.1.as_str()).collect();
    assert_eq!(
        messages,
        vec!["type 'FB_Motor' must implement method 'Stop' from interface 'IMotor'"]
    );
}

/// Specification 04 rule 8: each name after an interface's EXTENDS is an interface. Catches only
/// the first base checked.
#[test]
fn each_base_of_an_interface_must_be_an_interface() {
    check_has_error(
        r#"
INTERFACE IStart
END_INTERFACE

CLASS Motor
END_CLASS

INTERFACE IMotor EXTENDS IStart, Motor
END_INTERFACE
"#,
        DiagnosticCode::InvalidOperation,
    );
}

/// Specification 04 rule 8 (IEC 61131-3 Ed.3 §6.6.6.6.2): an interface is not its own base through
/// any of its bases. Catches the cycle check following only the first base.
#[test]
fn an_interface_is_not_its_own_base_through_a_second_base() {
    check_has_error(
        r#"
INTERFACE IStart
END_INTERFACE

INTERFACE IMotor EXTENDS IStart, IDrive
END_INTERFACE

INTERFACE IDrive EXTENDS IMotor
END_INTERFACE
"#,
        DiagnosticCode::CyclicDependency,
    );
}

/// Specification 04 and `docs/IEC_DECISIONS.md`: two bases that give a method and a property of one
/// name with different signatures make the interface an error (E208), at the later base. Worked by
/// hand: `Run` returns BOOL in IStart and INT in ICount, `Level` is an INT in IStart and a REAL in
/// ICount, and ICount is named second. Catches the second prototype dropped silently, which a call
/// through a value of ICount would then reach with a result the method does not give.
#[test]
fn two_bases_with_different_prototypes_of_one_name_are_an_error() {
    let source = r#"
INTERFACE IStart
    METHOD Run : BOOL
    END_METHOD
    PROPERTY Level : INT
    GET END_GET
    END_PROPERTY
END_INTERFACE

INTERFACE ICount
    METHOD Run : INT
    END_METHOD
    PROPERTY Level : REAL
    GET END_GET
    END_PROPERTY
END_INTERFACE

INTERFACE IMotor EXTENDS IStart, ICount
END_INTERFACE
"#;
    let second_base = source.rfind("ICount").expect("the second base");
    let at = (second_base, second_base + "ICount".len());
    assert_eq!(
        errors_with_places(source),
        vec![
            (
                DiagnosticCode::InterfaceConformance,
                "interface 'IMotor' inherits method 'Run' from 'IStart' and from 'ICount' with different signatures".to_string(),
                at.0,
                at.1,
            ),
            (
                DiagnosticCode::InterfaceConformance,
                "interface 'IMotor' inherits property 'Level' from 'IStart' and from 'ICount' with different signatures".to_string(),
                at.0,
                at.1,
            ),
        ]
    );
}

/// Specification 04 and `docs/IEC_DECISIONS.md`: prototypes of one signature are one, from a common
/// base (IEC 61131-3 Ed.3 Figure 19) or from two bases, and one method implements them. Catches the
/// common base's method, or two identical prototypes, reported as a conflict.
#[test]
fn bases_with_prototypes_of_the_same_signature_give_one_prototype() {
    check_no_errors(
        r#"
INTERFACE IBase
    METHOD Reset : BOOL
    END_METHOD
END_INTERFACE

INTERFACE IStart EXTENDS IBase
    METHOD Ready : BOOL
    END_METHOD
END_INTERFACE

INTERFACE IStop EXTENDS IBase
    METHOD Ready : BOOL
    END_METHOD
END_INTERFACE

INTERFACE IMotor EXTENDS IStart, IStop
END_INTERFACE

FUNCTION_BLOCK FB_Motor IMPLEMENTS IMotor
    METHOD PUBLIC Reset : BOOL
        Reset := TRUE;
    END_METHOD
    METHOD PUBLIC Ready : BOOL
        Ready := TRUE;
    END_METHOD
END_FUNCTION_BLOCK
"#,
    );
}
