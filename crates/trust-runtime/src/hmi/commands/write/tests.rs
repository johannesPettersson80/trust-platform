//! Specification 39, §12.2.4 and §12.8: the runtime thread's part of a command.

use serde_json::json;

use super::{WriteAction, WriteRefused, Written};
use crate::harness::TestHarness;
use crate::hmi::member_paths::{member_value, MemberPath};
use crate::runtime::CompiledRuntime;
use crate::value::Value;

/// A command's write at a boundary of the runtime's program, between its scans.
fn write_point(
    runtime: &mut CompiledRuntime,
    path: &MemberPath,
    action: &WriteAction,
) -> Result<Written, WriteRefused> {
    runtime.with_program(|program| super::write_point(program, path, action))
}

/// Writes together at a boundary of the runtime's program, between its scans.
fn write_points(
    runtime: &mut CompiledRuntime,
    writes: &[(MemberPath, WriteAction)],
) -> Result<Vec<Written>, WriteRefused> {
    runtime.with_program(|program| super::write_points(program, writes))
}

/// The library block and a program calling it, so the running resource serves the screens
/// (specification 39, §18 Phase 5).
const HMI: &str = include_str!("../../../../../../libraries/hmi/src/hmi.st");
const SCREENS: &str = r#"
PROGRAM Screens
VAR
    Display : Hmi.Display;
END_VAR
    Display();
END_PROGRAM
"#;

const PROGRAM: &str = r#"
TYPE Modes :
(
    Auto := 0,
    Manual := 1,
    Track := 2
);
END_TYPE

TYPE Limits :
STRUCT
    High : REAL;
    Low : REAL;
END_STRUCT
END_TYPE

FUNCTION_BLOCK Pump
VAR_INPUT
    StartCmd : BOOL;
END_VAR
VAR_OUTPUT
    Running : BOOL;
END_VAR
    Running := StartCmd;
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    Setpoint : REAL := 50.0;
    Count : INT;
    P101 : Pump;
    Alarm : Limits;
    Pumps : ARRAY[1..2] OF Pump;
    Mode : Modes := Modes#Auto;
END_VAR
VAR CONSTANT
    MaxFlow : REAL := 40.0;
END_VAR
    P101();
END_PROGRAM
"#;

fn harness() -> TestHarness {
    let mut harness = TestHarness::from_source(PROGRAM).expect("the program compiles");
    harness.cycle();
    harness
}

fn path(text: &str) -> MemberPath {
    MemberPath::parse(text).expect("a path")
}

fn read(harness: &TestHarness, text: &str) -> Value {
    harness
        .runtime()
        .with_program_view(|program| member_value(&path(text), program))
        .expect("a variable")
}

#[test]
fn a_program_variable_is_written_and_read_back() {
    let mut harness = harness();
    let written = write_point(
        harness.runtime_mut(),
        &path("Main.Setpoint"),
        &WriteAction::Set(json!(61.5)),
    )
    .expect("written");
    assert_eq!(
        written,
        Written {
            old: Value::Real(50.0),
            readback: Value::Real(61.5)
        }
    );
    assert_eq!(read(&harness, "Main.Setpoint"), Value::Real(61.5));
}

#[test]
fn members_fields_and_elements_are_written() {
    let mut harness = harness();
    write_point(
        harness.runtime_mut(),
        &path("Main.P101.StartCmd"),
        &WriteAction::Set(json!(true)),
    )
    .expect("an input");
    write_point(
        harness.runtime_mut(),
        &path("Main.Alarm.High"),
        &WriteAction::Set(json!(90)),
    )
    .expect("a field");
    write_point(
        harness.runtime_mut(),
        &path("Main.Pumps[2].StartCmd"),
        &WriteAction::Set(json!(true)),
    )
    .expect("an element");
    assert_eq!(read(&harness, "Main.P101.StartCmd"), Value::Bool(true));
    assert_eq!(read(&harness, "Main.Alarm.High"), Value::Real(90.0));
    assert_eq!(read(&harness, "Main.Pumps[2].StartCmd"), Value::Bool(true));
    assert_eq!(read(&harness, "Main.Pumps[1].StartCmd"), Value::Bool(false));
}

#[test]
fn a_toggle_writes_the_opposite() {
    let mut harness = harness();
    let first = write_point(
        harness.runtime_mut(),
        &path("Main.P101.StartCmd"),
        &WriteAction::Toggle,
    )
    .expect("toggled");
    assert_eq!(
        (first.old, first.readback),
        (Value::Bool(false), Value::Bool(true))
    );
    let refused = write_point(
        harness.runtime_mut(),
        &path("Main.Setpoint"),
        &WriteAction::Toggle,
    )
    .expect_err("not a BOOL");
    assert!(refused.contains("BOOL"), "{refused}");
}

#[test]
fn what_cannot_be_written_is_refused_with_the_reason() {
    let mut harness = harness();
    let refuse = |harness: &mut TestHarness, text: &str, value: serde_json::Value, words: &str| {
        let error = write_point(harness.runtime_mut(), &path(text), &WriteAction::Set(value))
            .expect_err(text);
        assert!(error.contains(words), "{text}: {error}");
    };
    refuse(&mut harness, "Main.MaxFlow", json!(1.0), "is a constant");
    refuse(
        &mut harness,
        "Main.P101.Running",
        json!(true),
        "is an output of Pump",
    );
    refuse(&mut harness, "Main.Gone", json!(1.0), "not a variable");
    refuse(&mut harness, "Main.Count", json!(1.5), "whole number");
    refuse(&mut harness, "Main.Count", json!(40000), "INT");
    assert_eq!(
        read(&harness, "Main.Count"),
        Value::Int(0),
        "nothing was written"
    );
}

/// Specification 39, §12.5 and §18 Phase 5: the running resource writes a command at the
/// program's `Hmi.Display` call and reads it back.
#[test]
fn the_running_resource_writes_a_command_and_reads_it_back() {
    use crate::scheduler::{ResourceRunner, StdClock};

    let harness =
        TestHarness::from_sources(&[HMI, PROGRAM, SCREENS]).expect("the program compiles");
    let mut handle = ResourceRunner::new(
        harness.into_runtime(),
        StdClock::new(),
        crate::value::Duration::from_millis(5),
    )
    .spawn("hmi-write")
    .expect("the resource starts");
    let exchange = handle.control().hmi_exchange();
    let send = |text: &str, action: WriteAction| {
        exchange
            .write(path(text), action, std::time::Duration::from_secs(2))
            .expect("the program's call applies it")
    };
    let written = send("Main.Setpoint", WriteAction::Set(json!(72.0)));
    let refused = send("Main.MaxFlow", WriteAction::Set(json!(1.0)));
    handle.stop();
    let _ = handle.join();
    assert_eq!(
        written.map(|written| written.readback),
        Ok(Value::Real(72.0))
    );
    assert!(refused.is_err_and(|reason| reason.contains("constant")));
}

#[test]
fn an_enumeration_is_written_by_value_name_or_number() {
    let mut harness = harness();
    let mode = path("Main.Mode");
    let named = write_point(
        harness.runtime_mut(),
        &mode,
        &WriteAction::Set(json!("manual")),
    )
    .expect("written by name, ignoring case");
    let Value::Enum(manual) = &named.readback else {
        panic!("an enumeration: {:?}", named.readback);
    };
    assert_eq!(
        (manual.variant_name().as_str(), manual.numeric_value()),
        ("Manual", 1)
    );
    let numbered = write_point(harness.runtime_mut(), &mode, &WriteAction::Set(json!(2)))
        .expect("written by number");
    let Value::Enum(track) = &numbered.readback else {
        panic!("an enumeration: {:?}", numbered.readback);
    };
    assert_eq!(track.variant_name().as_str(), "Track");
    for (requested, words) in [
        (json!("Fast"), "not a value of Modes"),
        (json!(7), "not a value of Modes"),
        (json!(true), "value name"),
    ] {
        let error = write_point(
            harness.runtime_mut(),
            &mode,
            &WriteAction::Set(requested.clone()),
        )
        .expect_err("refused");
        assert!(error.contains(words), "{requested}: {error}");
    }
}

/// Specification 39, §21.4 item 4: settings are written together or not at all; one value that
/// does not fit leaves every point as it was.
#[test]
fn points_written_together_are_written_all_or_none() {
    let mut harness = harness();
    let refused = write_points(
        harness.runtime_mut(),
        &[
            (path("Main.Setpoint"), WriteAction::Set(json!(61.5))),
            (path("Main.Count"), WriteAction::Set(json!("much"))),
        ],
    )
    .expect_err("a value that does not fit");
    assert!(refused.contains("INT"), "{refused}");
    assert_eq!(
        read(&harness, "Main.Setpoint"),
        Value::Real(50.0),
        "nothing was written"
    );
    let written = write_points(
        harness.runtime_mut(),
        &[
            (path("Main.Setpoint"), WriteAction::Set(json!(61.5))),
            (path("Main.Count"), WriteAction::Set(json!(3))),
        ],
    )
    .expect("both fit");
    assert_eq!(
        written
            .iter()
            .map(|written| written.readback.clone())
            .collect::<Vec<_>>(),
        [Value::Real(61.5), Value::Int(3)]
    );
    assert_eq!(read(&harness, "Main.Count"), Value::Int(3));
}

/// Specification 39, §12.8 item 5: a `STRING[n]` point takes at most `n` bytes of UTF-8. "ÄB" is
/// three bytes and fits a `STRING[3]`; "ÄÖ" is two characters in four bytes and is refused with
/// the stable code `runtime_string_capacity_exceeded`, and the point keeps its value. Catches a
/// capacity counted in characters, and a text cut to fit instead of refused.
#[test]
fn a_text_is_counted_in_bytes_of_utf8_against_its_points_capacity() {
    let source = "PROGRAM Main\nVAR\n    label : STRING[3] := 'OK';\nEND_VAR\nEND_PROGRAM\n";
    let mut harness = TestHarness::from_source(source).expect("the program compiles");
    harness.cycle();
    let fits = write_point(
        harness.runtime_mut(),
        &path("Main.label"),
        &WriteAction::Set(json!("ÄB")),
    )
    .expect("three bytes fit STRING[3]");
    assert_eq!(fits.readback, Value::String("ÄB".into()));
    let refused = write_point(
        harness.runtime_mut(),
        &path("Main.label"),
        &WriteAction::Set(json!("ÄÖ")),
    )
    .expect_err("four bytes do not fit STRING[3]");
    assert_eq!(
        refused.error_code,
        Some("runtime_string_capacity_exceeded"),
        "{refused}"
    );
    assert_eq!(read(&harness, "Main.label"), Value::String("ÄB".into()));
}

/// Specification 39, §12.8 item 5: `STRING` and `STRING[80]` are one type, so both points take 80
/// bytes and refuse 81 with `runtime_string_capacity_exceeded`, keeping their value. Catches a
/// bare `STRING` given another capacity than 80.
#[test]
fn a_string_without_a_length_takes_80_bytes() {
    let source = "PROGRAM Main\nVAR\n    plain : STRING := 'OK';\n    sized : STRING[80] := 'OK';\nEND_VAR\nEND_PROGRAM\n";
    let mut harness = TestHarness::from_source(source).expect("the program compiles");
    harness.cycle();
    let longest = "x".repeat(80);
    for point in ["Main.plain", "Main.sized"] {
        let fits = write_point(
            harness.runtime_mut(),
            &path(point),
            &WriteAction::Set(json!(longest)),
        )
        .unwrap_or_else(|refused| panic!("80 bytes fit {point}: {refused}"));
        assert_eq!(fits.readback, Value::String(longest.as_str().into()));
        let refused = write_point(
            harness.runtime_mut(),
            &path(point),
            &WriteAction::Set(json!("x".repeat(81))),
        )
        .expect_err("81 bytes do not fit");
        assert_eq!(
            refused.error_code,
            Some("runtime_string_capacity_exceeded"),
            "{point}: {refused}"
        );
        assert_eq!(
            read(&harness, point),
            Value::String(longest.as_str().into()),
            "{point} keeps its value"
        );
    }
}
