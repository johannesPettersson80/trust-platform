use trust_runtime::harness::TestHarness;

#[test]
fn namespace_resolution() {
    let library = r#"
        NAMESPACE Utilities
        FUNCTION Helper : INT
        VAR_INPUT
            x: INT;
        END_VAR
        Helper := x;
        END_FUNCTION
        END_NAMESPACE
    "#;

    let program = r#"
        USING Utilities;
        PROGRAM Multi
        VAR
            count: DINT := 0;
        END_VAR
        count := count + 1;
        END_PROGRAM
    "#;

    let mut harness = TestHarness::from_sources(&[library, program]).unwrap();
    harness.cycle();
    harness.assert_eq("count", 1i32);
}

#[test]
fn namespaced_programs_are_runtime_entry_points() {
    let source = r#"
        NAMESPACE CellA
        PROGRAM Main
        VAR
            count: DINT := 0;
        END_VAR
        count := count + 1;
        END_PROGRAM
        END_NAMESPACE
    "#;

    let mut harness = TestHarness::from_source(source).unwrap();
    assert!(
        harness
            .runtime()
            .programs()
            .values()
            .any(|program| program.name == "CellA.Main"),
        "expected namespaced PROGRAM to be registered"
    );
    harness.cycle();
    harness.assert_eq("count", 1i32);
}

#[test]
fn namespaced_pous_resolve_sibling_interfaces() {
    let source = r#"
        NAMESPACE CellA
        INTERFACE IProbe
            METHOD Read : INT
            END_METHOD
        END_INTERFACE

        FUNCTION AddOne : INT
        VAR_INPUT
            x : INT;
        END_VAR
        AddOne := x + INT#1;
        END_FUNCTION

        FUNCTION_BLOCK Probe IMPLEMENTS IProbe
        METHOD PUBLIC Read : INT
        Read := INT#7;
        END_METHOD
        END_FUNCTION_BLOCK

        CLASS Helper
        METHOD PUBLIC Twice : INT
        VAR_INPUT
            x : INT;
        END_VAR
        Twice := x * INT#2;
        END_METHOD
        END_CLASS

        PROGRAM Main
        VAR
            P : IProbe;
            Impl : Probe;
            H : Helper;
            Value : INT;
        END_VAR
        P := Impl;
        Value := AddOne(Impl.Read());
        END_PROGRAM
        END_NAMESPACE
    "#;

    let mut harness = TestHarness::from_source(source).unwrap();
    let runtime = harness.runtime();
    assert!(
        runtime
            .functions()
            .values()
            .any(|function| function.name == "CellA.AddOne"),
        "expected namespaced FUNCTION to be registered"
    );
    assert!(
        runtime
            .function_blocks()
            .values()
            .any(|function_block| function_block.name == "CellA.Probe"),
        "expected namespaced FUNCTION_BLOCK to be registered"
    );
    assert!(
        runtime
            .classes()
            .values()
            .any(|class| class.name == "CellA.Helper"),
        "expected namespaced CLASS to be registered"
    );
    assert!(
        runtime
            .interfaces()
            .values()
            .any(|interface| interface.name == "CellA.IProbe"),
        "expected namespaced INTERFACE to be registered"
    );
    let cycle = harness.cycle();
    assert!(
        cycle.errors.is_empty(),
        "namespaced POU cycle failed: {:?}",
        cycle.errors
    );
    harness.assert_eq("Value", 8i16);
}

#[test]
fn duplicate_program_name_errors() {
    let first = r#"
        PROGRAM Demo
        VAR
            count: DINT := 0;
        END_VAR
        END_PROGRAM
    "#;
    let second = r#"
        PROGRAM demo
        VAR
            count: DINT := 1;
        END_VAR
        END_PROGRAM
    "#;

    let err = TestHarness::from_sources(&[first, second])
        .err()
        .expect("expected duplicate program error");
    let message = err.to_string();
    assert!(
        message.to_ascii_lowercase().contains("duplicate") && message.contains("Demo"),
        "expected duplicate diagnostic for Demo, got: {message}"
    );
}
