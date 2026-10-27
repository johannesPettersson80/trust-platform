//! Specification 39, §7.2–7.3: member paths into function block instances, structures and
//! arrays, resolved against a compiled program after one scan.

use super::*;
use crate::harness::{CompileSession, TestHarness};

const PROGRAM: &str = r#"
TYPE Reading :
STRUCT
    Value : REAL;
    Valid : BOOL;
END_STRUCT
END_TYPE

FUNCTION_BLOCK Pump
VAR_INPUT
    StartCmd : BOOL;
END_VAR
VAR_OUTPUT
    Running : BOOL;
    Speed : REAL;
END_VAR
    Running := StartCmd;
    IF Running THEN
        Speed := 1460.0;
    ELSE
        Speed := 0.0;
    END_IF;
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    P101 : Pump;
    Pumps : ARRAY[1..3] OF Pump;
    Level : Reading := (Value := 62.5, Valid := TRUE);
    Levels : ARRAY[0..1] OF Reading;
    Grid : ARRAY[1..2, 1..3] OF INT;
END_VAR
    P101(StartCmd := TRUE);
    Pumps[2](StartCmd := TRUE);
    Levels[1].Value := 12.0;
    Grid[2, 3] := 7;
    Grid[1, 2] := 4;
END_PROGRAM
"#;

fn scanned() -> TestHarness {
    let mut harness = TestHarness::from_source(PROGRAM).expect("the program compiles");
    harness.cycle();
    harness
}

fn value(harness: &TestHarness, path: &str) -> Option<Value> {
    let parsed = MemberPath::parse(path).unwrap_or_else(|error| panic!("{path}: {error}"));
    harness
        .runtime()
        .with_program_view(|program| member_value(&parsed, program))
}

/// The declarations of `source`, which the editor resolves paths in.
fn declarations(source: &str) -> Declarations {
    CompileSession::from_source(source)
        .declarations()
        .expect("the program compiles")
}

#[test]
fn function_block_members_resolve() {
    let harness = scanned();
    assert_eq!(
        value(&harness, "Main.P101.Running"),
        Some(Value::Bool(true))
    );
    assert_eq!(
        value(&harness, "Main.P101.Speed"),
        Some(Value::Real(1460.0))
    );
}

#[test]
fn members_of_function_blocks_in_an_array_resolve_with_the_declared_bounds() {
    let harness = scanned();
    assert_eq!(
        value(&harness, "Main.Pumps[2].Running"),
        Some(Value::Bool(true))
    );
    assert_eq!(
        value(&harness, "Main.Pumps[1].Running"),
        Some(Value::Bool(false))
    );
    assert_eq!(
        value(&harness, "Main.Pumps[3].Speed"),
        Some(Value::Real(0.0))
    );
}

#[test]
fn structure_fields_resolve() {
    let harness = scanned();
    assert_eq!(value(&harness, "Main.Level.Value"), Some(Value::Real(62.5)));
    assert_eq!(value(&harness, "Main.Level.Valid"), Some(Value::Bool(true)));
    assert_eq!(
        value(&harness, "Main.Levels[1].Value"),
        Some(Value::Real(12.0))
    );
}

#[test]
fn multi_dimensional_indices_resolve() {
    let harness = scanned();
    assert_eq!(value(&harness, "Main.Grid[2, 3]"), Some(Value::Int(7)));
    assert_eq!(value(&harness, "Main.Grid[2,3]"), Some(Value::Int(7)));
    assert_eq!(value(&harness, "Main.Grid[1, 1]"), Some(Value::Int(0)));
    assert_eq!(value(&harness, "Main.Grid[1, 2]"), Some(Value::Int(4)));
    assert_eq!(value(&harness, "Main.Grid[2, 1]"), Some(Value::Int(0)));
}

#[test]
fn identifiers_match_without_regard_to_case() {
    let harness = scanned();
    assert_eq!(
        value(&harness, "main.p101.RUNNING"),
        Some(Value::Bool(true))
    );
}

#[test]
fn unknown_and_out_of_range_paths_resolve_to_nothing() {
    let harness = scanned();
    for path in [
        "Main.P101.Missing",
        "Main.Pumps[4].Running",
        "Main.Pumps[0].Running",
        "Main.Level[1]",
        "Main.Grid[2]",
        "Main.P101.Running.Extra",
        "Nobody.P101",
    ] {
        assert_eq!(value(&harness, path), None, "{path}");
    }
}

#[test]
fn malformed_paths_are_rejected_with_a_message() {
    for path in [
        "",
        "Main.",
        ".Main",
        "Main..P101",
        "Main.Pumps[",
        "Main.Pumps[a]",
        "Main.Pumps[]",
        "Main.Pumps[1",
    ] {
        let error = MemberPath::parse(path).expect_err(path);
        assert!(!error.is_empty(), "{path}");
    }
}

#[test]
fn parsed_path_has_its_root_and_steps() {
    assert_eq!(
        MemberPath::parse("FeedArea.Pumps[2].Running"),
        Ok(MemberPath {
            root: SmolStr::new("FeedArea"),
            segments: vec![
                PathSegment::Field(SmolStr::new("Pumps")),
                PathSegment::Index(vec![2]),
                PathSegment::Field(SmolStr::new("Running")),
            ],
        })
    );
}

/// The real control library, so the acceptance target `Control.PID` is what resolves.
const CONTROL_LIBRARY: &str = include_str!("../../../../../libraries/control/src/control.st");

#[test]
fn members_of_a_control_pid_instance_resolve() {
    let program = r#"
PROGRAM Main
VAR
    Controller : Control.PID;
    Settings : Control.PIDParameters := (ProportionalGain := 2.5, IntegralGain := 0.1, OutputMaximum := 100.0);
END_VAR
    Controller.Configure(Parameters := Settings);
END_PROGRAM
"#;
    let mut harness = TestHarness::from_sources(&[CONTROL_LIBRARY, program])
        .expect("the program compiles with the control library");
    harness.cycle();
    assert_eq!(
        value(&harness, "Main.Controller.ParametersValue.ProportionalGain"),
        Some(Value::LReal(2.5))
    );
    assert_eq!(
        value(&harness, "Main.Controller.ParametersValue.OutputMaximum"),
        Some(Value::LReal(100.0))
    );
    assert_eq!(
        value(&harness, "Main.Settings.IntegralGain"),
        Some(Value::LReal(0.1))
    );
}

/// Members a function block inherits (IEC 61131-3 Edition 3, 6.6.5.3) are reached through the
/// derived instance's path, with the declaring type reported for the write checks.
#[test]
fn inherited_members_resolve_through_the_derived_instance() {
    let source = r#"
FUNCTION_BLOCK Base
VAR_INPUT
    Gain : INT := 4;
END_VAR
VAR_OUTPUT
    Result : INT := 9;
END_VAR
END_FUNCTION_BLOCK

FUNCTION_BLOCK Derived EXTENDS Base
VAR_INPUT
    Value : INT := 2;
END_VAR
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    Block : Derived;
END_VAR
    Block();
END_PROGRAM
"#;
    let declarations = declarations(source);
    let storage = declarations.storage();
    let path = |text: &str| MemberPath::parse(text).expect("a path");
    assert_eq!(
        resolve_member(&path("Main.Block.Value"), storage),
        Some(&Value::Int(2))
    );
    assert_eq!(
        resolve_member(&path("Main.Block.Gain"), storage),
        Some(&Value::Int(4)),
        "an inherited input"
    );
    assert_eq!(
        resolve_member(&path("Main.Block.Result"), storage),
        Some(&Value::Int(9)),
        "an inherited output"
    );
    let mut owners = Vec::new();
    resolve_member_with(&path("Main.Block.Result"), storage, |type_name, member| {
        owners.push(format!("{type_name}.{member}"))
    });
    assert!(
        owners
            .last()
            .is_some_and(|owner| owner.eq_ignore_ascii_case("Base.Result")),
        "the declaring type: {owners:?}"
    );
}

/// Specification 39, §7.8: what each named part of a path names, so a rename can follow it.
#[test]
fn each_named_part_of_a_path_names_its_declaring_type_or_structure() {
    let declarations = declarations(PROGRAM);
    let steps = |path: &str| path_steps(&MemberPath::parse(path).expect("a path"), &declarations);
    let root = PathStep::Root {
        instance_type: Some("Main".into()),
    };
    let member = |declaring: &str| PathStep::Member {
        declaring_type: declaring.into(),
    };
    assert_eq!(
        steps("Main.P101.Running"),
        Some(vec![root.clone(), member("Main"), member("Pump")])
    );
    assert_eq!(
        steps("Main.Pumps[2].Running"),
        Some(vec![root.clone(), member("Main"), member("Pump")]),
        "indices name no part"
    );
    assert_eq!(
        steps("Main.Levels[1].Value"),
        Some(vec![
            root.clone(),
            member("Main"),
            PathStep::Field {
                structure_type: "Reading".into()
            }
        ])
    );
    assert_eq!(steps("Main.Gone"), None, "a path that does not resolve");
}

#[test]
fn an_inherited_member_names_the_base_that_declares_it() {
    let source = "FUNCTION_BLOCK Base\nVAR_INPUT\n    Gain : INT := 1;\nEND_VAR\nEND_FUNCTION_BLOCK\n\nFUNCTION_BLOCK Scaled EXTENDS Base\nVAR_INPUT\n    Value : INT;\nEND_VAR\nEND_FUNCTION_BLOCK\n\nPROGRAM Main\nVAR\n    Block : Scaled;\nEND_VAR\n    Block(Value := 1);\nEND_PROGRAM\n";
    let declarations = declarations(source);
    let steps = path_steps(
        &MemberPath::parse("Main.Block.Gain").expect("a path"),
        &declarations,
    );
    assert_eq!(
        steps.and_then(|steps| steps.last().cloned()),
        Some(PathStep::Member {
            declaring_type: "Base".into()
        })
    );
}

/// Specification 39, §7.8: a program instance named in a configuration (`PROGRAM Tank WITH Fast
/// : TankLevelControl`) is of its program's type, so a rename of the program's variable follows
/// the paths that start with the instance's name.
#[test]
fn a_configured_program_instance_names_its_program_type() {
    let source = "PROGRAM TankLevelControl\nVAR\n    Level : REAL;\nEND_VAR\nEND_PROGRAM\n\nCONFIGURATION Plant\nRESOURCE Cpu ON PLC\n    TASK Fast (INTERVAL := T#100ms, PRIORITY := 1);\n    PROGRAM Tank WITH Fast : TankLevelControl;\nEND_RESOURCE\nEND_CONFIGURATION\n";
    let declarations = declarations(source);
    let steps = path_steps(
        &MemberPath::parse("Tank.Level").expect("a path"),
        &declarations,
    );
    assert_eq!(
        steps,
        Some(vec![
            PathStep::Root {
                instance_type: Some("TankLevelControl".into())
            },
            PathStep::Member {
                declaring_type: "TankLevelControl".into()
            },
        ])
    );
}
