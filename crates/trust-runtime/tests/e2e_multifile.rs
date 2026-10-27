//! Multi-file and namespaced programs: names resolve across files and namespaces, namespaced POUs
//! are registered under their qualified names, and a duplicate program name is refused.
//!
//! The tests need the toolchain of specification 44 §4.6 at TRUST_TOOLCHAIN.

use trust_runtime::harness::{CompileSession, TestHarness};

#[test]
fn namespace_resolution() {
    let library = r#"
        NAMESPACE Utilities
        FUNCTION Helper : DINT
        VAR_INPUT
            x: DINT;
        END_VAR
        Helper := x + 1;
        END_FUNCTION
        END_NAMESPACE
    "#;

    let program = r#"
        USING Utilities;
        PROGRAM Multi
        VAR
            count: DINT := 0;
        END_VAR
        count := Helper(count);
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

    let declarations = CompileSession::from_source(source).declarations().unwrap();
    assert!(
        declarations
            .metadata()
            .programs()
            .values()
            .any(|program| program.name == "CellA.Main"),
        "expected namespaced PROGRAM to be registered"
    );
    let mut harness = TestHarness::from_source(source).unwrap();
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

    let declarations = CompileSession::from_source(source).declarations().unwrap();
    let runtime = declarations.metadata();
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
    let mut harness = TestHarness::from_source(source).unwrap();
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

// Project-wide analysis: global constants of another file give array bounds and string
// lengths, a configuration's VAR_CONFIG entries initialize program instance variables, and
// globals are imported from other files. The runtime compiler gets the same values and
// diagnostics from it as before the project analysis cached these project-wide results.
const PROJECT_CONSTANTS: &str = r#"
VAR_GLOBAL CONSTANT
    Size : INT := 3;
    Extra : INT := Size + 1;
END_VAR

VAR_GLOBAL
    Shared : DINT := 5;
END_VAR
"#;

const PROJECT_PROGRAM: &str = r#"
PROGRAM Main
VAR
    buffer : ARRAY[1..Size] OF INT;
    name : STRING(Extra);
    i : INT;
    total : INT;
    level : INT;
    copied : DINT;
END_VAR
FOR i := 1 TO Size DO
    buffer[i] := i * 10;
END_FOR;
total := buffer[1] + buffer[Size];
name := 'abcd';
copied := Shared;
END_PROGRAM
"#;

const PROJECT_CONFIGURATION: &str = r#"
CONFIGURATION Plant
RESOURCE Cpu ON PLC
    TASK Cyclic (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM P1 WITH Cyclic : Main;
END_RESOURCE
VAR_CONFIG
    P1.level : INT := 42;
END_VAR
END_CONFIGURATION
"#;

#[test]
fn cross_file_constants_and_var_config_entries_compile_and_run() {
    let mut harness =
        TestHarness::from_sources(&[PROJECT_CONSTANTS, PROJECT_PROGRAM, PROJECT_CONFIGURATION])
            .expect("compile the project");
    harness.advance_time(trust_runtime::value::Duration::from_millis(10));
    let cycle = harness.cycle();
    assert!(cycle.errors.is_empty(), "cycle errors: {:?}", cycle.errors);
    harness.assert_eq("total", 40i16);
    harness.assert_eq("level", 42i16);
    harness.assert_eq("copied", 5i32);
    harness.assert_eq("name", trust_runtime::value::Value::String("abcd".into()));
}

#[test]
fn cross_file_project_errors_are_listed_per_file_in_a_stable_order() {
    // The string capacity in the third error comes from the other file's `Extra`, the
    // narrowing in the fifth from the imported DINT `Shared`.
    let program = PROJECT_PROGRAM
        .replace(
            "    copied : DINT;\n",
            "    copied : DINT;\n    pin AT %I* : BOOL;\n    spare AT %Q* : BOOL;\n",
        )
        .replace("name := 'abcd';", "name := 'abcdefgh';")
        .replace("copied := Shared;", "copied := Missing;\ni := Shared;");
    let configuration =
        PROJECT_CONFIGURATION.replace("P1.level : INT := 42;", "P1.level : DINT := 42;");
    let error = TestHarness::from_sources(&[PROJECT_CONSTANTS, &program, &configuration])
        .err()
        .expect("the project has errors");
    let expected = [
        "file 1: error[E202]: direct address '%I*' requires VAR_CONFIG mapping (at 150..153)",
        "file 1: error[E202]: direct address '%Q*' requires VAR_CONFIG mapping (at 173..178)",
        "file 1: error[E304]: STRING literal length 8 exceeds STRING[4] capacity (at 300..311)",
        "file 1: error[E101]: undefined identifier 'Missing' (at 322..330)",
        "file 1: error[E203]: cannot assign 'DINT' to 'INT' (at 332..345)",
        "file 2: error[E201]: VAR_CONFIG type 'DINT' does not match target type 'INT' (at 152..166)",
    ];
    let message = error.to_string();
    let mut reported = message.lines().collect::<Vec<_>>();
    reported.sort_unstable();
    let mut sorted_expected = expected.to_vec();
    sorted_expected.sort_unstable();
    assert_eq!(reported, sorted_expected, "the same errors");
    // The two missing VAR_CONFIG mappings follow their declarations; they were listed in
    // hash order before the project analysis walked symbol tables in declaration order.
    assert_eq!(message, expected.join("\n"), "in declaration order");
}
