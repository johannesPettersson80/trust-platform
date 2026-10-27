//! Specification 39, §11.1: values as the runtime display receives them.

use serde_json::json;

use super::*;
use crate::harness::TestHarness;
use crate::hmi::member_paths::{locate_member, MemberPath};
use crate::value::{
    DateTimeValue, DateValue, Duration, LDateTimeValue, LDateValue, LTimeOfDayValue, TimeOfDayValue,
};

/// The library block and a program calling it, so the running resource serves the screens
/// (specification 39, §18 Phase 5).
const HMI: &str = include_str!("../../../../../libraries/hmi/src/hmi.st");
const SCREENS: &str = r#"
PROGRAM Screens
VAR
    Display : Hmi.Display;
END_VAR
    Display();
END_PROGRAM
"#;

const PROGRAM: &str = r#"
TYPE Mode : (Off, Manual, Auto); END_TYPE

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
    END_IF;
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    Op : Mode := Mode#Auto;
    Levels : ARRAY[1..3] OF INT := [10, 20, 30];
    Grid : ARRAY[1..2, 0..2] OF INT;
    Level : Reading := (Value := 62.5, Valid := TRUE);
    P101 : Pump;
    Huge : ARRAY[1..30000] OF BYTE;
END_VAR
    Grid[2, 0] := 7;
    P101(StartCmd := TRUE);
END_PROGRAM
"#;

/// The ticks of 2026-10-01 in the default profile (milliseconds since 1970-01-01).
const OCTOBER_FIRST_MS: i64 = 20_727 * 86_400_000;

fn json_of(wire: WireValue) -> Json {
    serde_json::to_value(wire).expect("the value serializes")
}

fn wire(value: Value) -> Option<Json> {
    wire_value(&value, DateTimeProfile::default()).map(json_of)
}

fn shown(value: Value) -> Json {
    wire(value.clone())
        .unwrap_or_else(|| panic!("{value:?} can be shown"))
        .get("value")
        .cloned()
        .unwrap_or_else(|| panic!("{value:?} has a value"))
}

fn scanned() -> TestHarness {
    let mut harness = TestHarness::from_source(PROGRAM).expect("the program compiles");
    harness.cycle();
    harness
}

/// The value at `path` in the running program, as the display receives it.
fn wire_of(harness: &TestHarness, path: &str) -> Option<WireValue> {
    let parsed = MemberPath::parse(path).unwrap_or_else(|error| panic!("{path}: {error}"));
    harness.runtime().with_program_view(|program| {
        let at = locate_member(&parsed, program).unwrap_or_else(|| panic!("{path} resolves"));
        located_wire_value(program, &at, DateTimeProfile::default())
    })
}

fn wire_at(harness: &TestHarness, path: &str) -> Option<Json> {
    wire_of(harness, path).map(json_of)
}

#[test]
fn numbers_booleans_and_text_are_sent_as_json() {
    assert_eq!(shown(Value::Bool(true)), json!(true));
    assert_eq!(shown(Value::Int(-12)), json!(-12));
    assert_eq!(shown(Value::UDInt(4_000_000_000)), json!(4_000_000_000_u64));
    assert_eq!(
        shown(Value::LInt(9_007_199_254_740_991)),
        json!(9_007_199_254_740_991_i64)
    );
    assert_eq!(shown(Value::Real(62.5)), json!(62.5));
    assert_eq!(
        shown(Value::Real(0.1)),
        json!(0.1),
        "a REAL is sent with its shortest digits"
    );
    assert_eq!(shown(Value::LReal(-3.25)), json!(-3.25));
    assert_eq!(shown(Value::String("Pump 1".into())), json!("Pump 1"));
    assert_eq!(shown(Value::Char(b'A')), json!("A"));
}

#[test]
fn integers_too_large_for_a_json_number_and_non_finite_reals_are_text() {
    assert_eq!(
        shown(Value::LInt(9_007_199_254_740_993)),
        json!("9007199254740993")
    );
    assert_eq!(
        shown(Value::LInt(-9_007_199_254_740_993)),
        json!("-9007199254740993")
    );
    assert_eq!(shown(Value::ULInt(u64::MAX)), json!("18446744073709551615"));
    assert_eq!(shown(Value::LWord(u64::MAX)), json!("18446744073709551615"));
    assert_eq!(shown(Value::Real(f32::NAN)), json!("NaN"));
    assert_eq!(shown(Value::LReal(f64::INFINITY)), json!("+Inf"));
    assert_eq!(shown(Value::LReal(f64::NEG_INFINITY)), json!("-Inf"));
}

#[test]
fn times_and_dates_are_iec_literals() {
    assert_eq!(
        shown(Value::Time(Duration::from_millis(3_723_500))),
        json!("T#1h2m3s500ms")
    );
    assert_eq!(shown(Value::Time(Duration::ZERO)), json!("T#0s"));
    assert_eq!(
        shown(Value::Time(Duration::from_millis(-1_500))),
        json!("T#-1s500ms")
    );
    assert_eq!(
        shown(Value::Time(Duration::from_millis(90_061_000))),
        json!("T#1d1h1m1s")
    );
    assert_eq!(
        shown(Value::LTime(Duration::from_nanos(1_500))),
        json!("LT#1us500ns")
    );
    assert_eq!(
        shown(Value::Date(DateValue::new(OCTOBER_FIRST_MS))),
        json!("D#2026-10-01")
    );
    assert_eq!(
        shown(Value::Tod(TimeOfDayValue::new(45_015_500))),
        json!("TOD#12:30:15.500")
    );
    assert_eq!(
        shown(Value::Tod(TimeOfDayValue::new(28_800_000))),
        json!("TOD#08:00:00")
    );
    assert_eq!(
        shown(Value::Dt(DateTimeValue::new(OCTOBER_FIRST_MS + 45_015_000))),
        json!("DT#2026-10-01-12:30:15")
    );
    let midnight_ns = OCTOBER_FIRST_MS * 1_000_000;
    assert_eq!(
        shown(Value::LDate(LDateValue::new(midnight_ns))),
        json!("LD#2026-10-01")
    );
    assert_eq!(
        shown(Value::LTod(LTimeOfDayValue::new(28_800_250_000_000))),
        json!("LTOD#08:00:00.250")
    );
    assert_eq!(
        shown(Value::Ldt(LDateTimeValue::new(
            midnight_ns + 28_800_250_000_000
        ))),
        json!("LDT#2026-10-01-08:00:00.250")
    );
}

#[test]
fn an_enumerated_value_is_its_name() {
    let harness = scanned();
    assert_eq!(
        wire_at(&harness, "Main.Op"),
        Some(json!({ "value": "Auto" }))
    );
}

#[test]
fn arrays_carry_the_declared_lower_bound_of_each_dimension() {
    let harness = scanned();
    assert_eq!(
        wire_at(&harness, "Main.Levels"),
        Some(json!({ "value": [10, 20, 30], "lowerBounds": [1] }))
    );
    assert_eq!(
        wire_at(&harness, "Main.Grid"),
        Some(json!({ "value": [[0, 0, 0], [7, 0, 0]], "lowerBounds": [1, 0] }))
    );
}

#[test]
fn structures_and_function_block_instances_carry_their_members_by_name() {
    let harness = scanned();
    assert_eq!(
        wire_at(&harness, "Main.Level"),
        Some(json!({ "value": { "Value": 62.5, "Valid": true } }))
    );
    let pump = wire_at(&harness, "Main.P101").expect("an instance can be shown");
    let members = &pump["value"];
    assert_eq!(members["StartCmd"], json!(true), "{pump}");
    assert_eq!(members["Running"], json!(true), "{pump}");
    assert_eq!(members["Speed"], json!(1460.0), "{pump}");
}

/// The members as the display receives them, in the order they are sent.
fn sent_text(harness: &TestHarness, path: &str) -> String {
    let wire = wire_of(harness, path).unwrap_or_else(|| panic!("{path} is shown"));
    serde_json::to_string(&wire).expect("the value serializes")
}

#[test]
fn members_keep_their_declared_order() {
    let harness = scanned();
    let level = sent_text(&harness, "Main.Level");
    assert!(level.contains(r#"{"Value":62.5,"Valid":true}"#), "{level}");
    let pump = sent_text(&harness, "Main.P101");
    let position = |name: &str| {
        pump.find(&format!("\"{name}\""))
            .unwrap_or_else(|| panic!("{name} in {pump}"))
    };
    assert!(
        position("StartCmd") < position("Running") && position("Running") < position("Speed"),
        "{pump}"
    );
}

#[test]
fn a_value_with_more_elements_than_the_limit_a_reference_or_null_is_unknown() {
    let harness = scanned();
    assert!(
        wire_at(&harness, "Main.Levels").is_some(),
        "a small array is shown"
    );
    assert!(wire(Value::Bool(true)).is_some(), "a single value is shown");
    assert_eq!(wire_at(&harness, "Main.Huge"), None);
    assert_eq!(wire(Value::Reference(None)), None);
    assert_eq!(wire(Value::Null), None);
}

/// Specification 39, §11.1 and §18 Phase 5: the running resource copies a display's paths, with
/// its cycle count, at the program's `Hmi.Display` call.
#[test]
fn the_running_resource_reads_subscribed_paths_with_its_cycle_count() {
    use crate::scheduler::{ResourceRunner, StdClock};

    let harness =
        TestHarness::from_sources(&[HMI, PROGRAM, SCREENS]).expect("the program compiles");
    let mut handle = ResourceRunner::new(
        harness.into_runtime(),
        StdClock::new(),
        Duration::from_millis(5),
    )
    .spawn("hmi-read")
    .expect("the resource starts");
    let exchange = handle.control().hmi_exchange();
    let paths: Vec<MemberPath> = ["Main.Levels", "Main.Gone", "Main.P101.Running"]
        .iter()
        .map(|path| MemberPath::parse(path).expect("a valid path"))
        .collect();
    let read = || {
        exchange
            .read_next(&paths, std::time::Duration::from_secs(2))
            .expect("the program's call answers")
    };
    let first = read();
    std::thread::sleep(std::time::Duration::from_millis(60));
    let second = read();
    handle.stop();
    let _ = handle.join();

    // Each read is answered by a call; the second comes several scans after the first.
    assert_eq!(
        second
            .values
            .into_iter()
            .map(|value| value.map(json_of))
            .collect::<Vec<_>>(),
        vec![
            Some(json!({ "value": [10, 20, 30], "lowerBounds": [1] })),
            None,
            Some(json!({ "value": true })),
        ]
    );
    assert!(
        second.cycle > first.cycle,
        "the cycle count advances: {} then {}",
        first.cycle,
        second.cycle
    );
}
