use trust_runtime::harness::TestHarness;
use trust_runtime::harness::{CompileSession, SourceFile};

#[test]
fn interface_conformance() {
    let source = r#"
INTERFACE ICounter
METHOD Inc : INT
VAR_INPUT
    delta : INT;
END_VAR
END_METHOD
END_INTERFACE

CLASS Counter IMPLEMENTS ICounter
VAR PUBLIC
    value : INT := INT#0;
END_VAR
METHOD PUBLIC Inc : INT
VAR_INPUT
    delta : INT;
END_VAR
value := value + delta;
Inc := value;
END_METHOD
END_CLASS

PROGRAM Main
VAR
    c : Counter;
    i : ICounter;
    out1 : INT := INT#0;
    out2 : INT := INT#0;
END_VAR
i := c;
out1 := i.Inc(INT#1);
out2 := c.Inc(INT#2);
END_PROGRAM
"#;

    let mut harness = TestHarness::from_source(source).unwrap();
    let result = harness.cycle();
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    harness.assert_eq("out1", 1i16);
    harness.assert_eq("out2", 3i16);
}

#[test]
fn interface_assignment_works_across_files_with_properties() {
    let sources = vec![
        SourceFile::with_path(
            "interfaces.st",
            r#"
INTERFACE IValve
    PROPERTY IsOpen : BOOL
    GET
    END_GET
    END_PROPERTY
END_INTERFACE
"#,
        ),
        SourceFile::with_path(
            "impl.st",
            r#"
FUNCTION_BLOCK ValveFb IMPLEMENTS IValve
VAR
    open_state : BOOL;
END_VAR

PUBLIC PROPERTY IsOpen : BOOL
GET
    IsOpen := open_state;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK
"#,
        ),
        SourceFile::with_path(
            "main.st",
            r#"
PROGRAM Main
VAR
    valve : ValveFb;
    as_interface : IValve;
END_VAR
as_interface := valve;
END_PROGRAM
"#,
        ),
    ];

    let session = CompileSession::from_sources(sources).label_errors(true);
    if let Err(err) = session.build_runtime() {
        panic!("cross-file interface assignment compile failed:\n{err}");
    }
}

#[test]
fn method_can_return_owned_function_block_as_interface() {
    let source = r#"
INTERFACE ICommand
    PROPERTY Done : BOOL
    GET
    END_GET
    END_PROPERTY

    METHOD SetDone : BOOL
    VAR_INPUT
        Value : BOOL;
    END_VAR
    END_METHOD
END_INTERFACE

FUNCTION_BLOCK Command IMPLEMENTS ICommand
VAR
    done_state : BOOL;
END_VAR

PUBLIC PROPERTY Done : BOOL
GET
    Done := done_state;
END_GET
END_PROPERTY

METHOD PUBLIC SetDone : BOOL
VAR_INPUT
    Value : BOOL;
END_VAR
done_state := Value;
SetDone := done_state;
END_METHOD
END_FUNCTION_BLOCK

FUNCTION_BLOCK Axis
VAR
    command : Command;
END_VAR

METHOD PUBLIC Start : ICommand
VAR
    Ignored : BOOL;
END_VAR
Ignored := command.SetDone(TRUE);
Start := command;
END_METHOD
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    axis : Axis;
    command : ICommand;
    done : BOOL;
END_VAR
command := axis.Start();
done := command.Done;
END_PROGRAM
"#;

    let mut harness = TestHarness::from_source(source).unwrap();
    let result = harness.cycle();
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    harness.assert_eq("done", true);
}
