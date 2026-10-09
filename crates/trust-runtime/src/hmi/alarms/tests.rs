//! Specification 40, §5: the runtime's alarm service: every alarm instance at any depth, its
//! state, the transitions between reads and the alarm list's order.

use super::*;
use crate::harness::TestHarness;
use crate::hmi::commands::write::{write_point, WriteAction};
use crate::hmi::member_paths::MemberPath;
use crate::value::{Duration, Value};

const ALARM: &str = include_str!("../../../../../libraries/alarm/src/alarm.st");

const PROGRAM: &str = r#"
USING Alarm;

FUNCTION_BLOCK Pump
VAR_INPUT
    Start : BOOL;
    Running : BOOL;
END_VAR
VAR
    FailedToStart : DiscrepancyAlarm;
END_VAR
    FailedToStart(Command := Start, Feedback := Running, Message := 'Pump failed to start', Priority := AlarmPriority#High);
END_FUNCTION_BLOCK

PROGRAM Tank
VAR
    Level : LREAL := 50.0;
    Trip : BOOL;
    High : LimitAlarm;
    Overflow : DigitalAlarm;
    P1 : Pump;
    Pumps : ARRAY[1..2] OF Pump;
END_VAR
    High(Value := Level, Limit := 90.0, Message := 'Tank level high', Priority := AlarmPriority#Medium);
    Overflow(Condition := Trip, Message := 'Tank overflow', Priority := AlarmPriority#High);
    P1();
    Pumps[1]();
    Pumps[2]();
END_PROGRAM
"#;

fn plant() -> TestHarness {
    let mut harness = TestHarness::from_sources(&[ALARM, PROGRAM]).expect("compiles");
    harness.cycle();
    harness
}

fn scan(harness: &mut TestHarness) {
    harness.advance_time(Duration::from_nanos(100_000_000));
    harness.cycle();
}

fn request(harness: &mut TestHarness, path: &str, value: serde_json::Value) {
    let path = MemberPath::parse(path).expect("a path");
    write_point(harness.runtime_mut(), &path, &WriteAction::Set(value)).expect("written");
}

/// A transmitter with its own alarm, connected through an interface to a loop and in an array of
/// references to a group: the references share the transmitter, they do not hold it.
const CONNECTED: &str = r#"
USING Alarm;

INTERFACE IMeasurement
METHOD Read : LREAL
END_METHOD
END_INTERFACE

FUNCTION_BLOCK Transmitter IMPLEMENTS IMeasurement
VAR
    Value : LREAL := 95.0;
    High : LimitAlarm;
END_VAR
    High(Value := Value, Limit := 90.0, Message := 'Level high', Priority := AlarmPriority#Medium);
METHOD PUBLIC Read : LREAL
    Read := Value;
END_METHOD
END_FUNCTION_BLOCK

FUNCTION_BLOCK Loop
VAR_INPUT
    Measurement : IMeasurement;
END_VAR
END_FUNCTION_BLOCK

FUNCTION_BLOCK Group
VAR_INPUT
    Members : ARRAY[1..2] OF IMeasurement;
END_VAR
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    Controller : Loop;
    Instruments : Group;
    Connected : ARRAY[1..2] OF IMeasurement;
    LT101 : Transmitter;
END_VAR
    Controller(Measurement := LT101);
    Connected[1] := LT101;
    Instruments(Members := Connected);
    LT101();
END_PROGRAM
"#;

#[test]
fn an_alarm_is_found_once_where_it_is_declared_not_where_a_reference_shares_it() {
    let mut harness = TestHarness::from_sources(&[ALARM, CONNECTED]).expect("compiles");
    harness.cycle();
    harness.cycle();
    assert_eq!(
        alarm_paths(harness.runtime()),
        vec!["Main.LT101.High".to_string()],
        "the loop's interface and the arrays of interfaces refer to the transmitter"
    );
}

#[test]
fn every_alarm_instance_is_found_at_any_depth_by_its_path() {
    let harness = plant();
    let paths = alarm_paths(harness.runtime());
    assert_eq!(
        paths,
        vec![
            "Tank.High",
            "Tank.Overflow",
            "Tank.P1.FailedToStart",
            "Tank.Pumps[1].FailedToStart",
            "Tank.Pumps[2].FailedToStart"
        ]
    );
}

#[test]
fn an_alarm_names_the_function_block_instance_that_holds_it_as_its_equipment() {
    let harness = plant();
    let statuses = read_alarms(harness.runtime());
    let equipment: Vec<(&str, Option<&str>)> = statuses
        .iter()
        .map(|status| (status.path.as_str(), status.equipment.as_deref()))
        .collect();
    assert_eq!(
        equipment,
        vec![
            ("Tank.High", None),
            ("Tank.Overflow", None),
            ("Tank.P1.FailedToStart", Some("Tank.P1")),
            ("Tank.Pumps[1].FailedToStart", Some("Tank.Pumps[1]")),
            ("Tank.Pumps[2].FailedToStart", Some("Tank.Pumps[2]")),
        ],
        "a program holds Tank.High directly"
    );
}

#[test]
fn an_alarms_status_is_read_from_its_outputs() {
    let mut harness = plant();
    harness.set_input("Level", Value::LReal(95.0));
    scan(&mut harness);
    let statuses = read_alarms(harness.runtime());
    let high = statuses
        .iter()
        .find(|status| status.path == "Tank.High")
        .expect("Tank.High");
    assert_eq!(high.message, "Tank level high");
    assert_eq!(high.priority, Priority::Medium);
    assert_eq!(high.state, "ActiveUnacknowledged");
    assert!(high.active && high.unacknowledged && high.annunciating);
    assert_eq!(high.raise_count, 1);
}

#[test]
fn transitions_between_reads_become_events_with_the_time() {
    let mut harness = plant();
    let mut board = AlarmBoard::default();
    board.update(read_alarms(harness.runtime()), 1_000);
    assert!(board.events().is_empty(), "nothing has happened yet");
    harness.set_input("Trip", Value::Bool(true));
    scan(&mut harness);
    harness.set_input("Trip", Value::Bool(false));
    scan(&mut harness);
    board.update(read_alarms(harness.runtime()), 2_000);
    let kinds: Vec<(&str, EventKind)> = board
        .events()
        .iter()
        .map(|event| (event.path.as_str(), event.kind))
        .collect();
    assert_eq!(
        kinds,
        vec![
            ("Tank.Overflow", EventKind::Raised),
            ("Tank.Overflow", EventKind::Cleared)
        ],
        "both, though they came between two reads"
    );
    assert_eq!(board.events()[0].time_ms, 2_000);
    assert_eq!(board.events()[0].message, "Tank overflow");
    request(
        &mut harness,
        "Tank.Overflow.Request.Acknowledge",
        serde_json::json!(true),
    );
    scan(&mut harness);
    board.update(read_alarms(harness.runtime()), 3_000);
    let last = board.events().back().expect("an event");
    assert_eq!(
        (last.kind, last.user.as_deref()),
        (EventKind::Acknowledged, None),
        "acknowledged by the program, without a user"
    );
}

#[test]
fn an_operators_action_is_recorded_once_with_the_user() {
    let mut harness = plant();
    let mut board = AlarmBoard::default();
    harness.set_input("Trip", Value::Bool(true));
    scan(&mut harness);
    board.update(read_alarms(harness.runtime()), 1_000);
    board.operator_action(
        "Tank.Overflow",
        EventKind::Acknowledged,
        "anna",
        None,
        None,
        1_500,
    );
    request(
        &mut harness,
        "Tank.Overflow.Request.Acknowledge",
        serde_json::json!(true),
    );
    scan(&mut harness);
    board.update(read_alarms(harness.runtime()), 2_000);
    let acknowledged: Vec<&AlarmEvent> = board
        .events()
        .iter()
        .filter(|event| event.kind == EventKind::Acknowledged)
        .collect();
    assert_eq!(acknowledged.len(), 1, "{:?}", board.events());
    assert_eq!(acknowledged[0].user.as_deref(), Some("anna"));
}

/// Specification 39, §5.9 item 2: a limit alarm's row gives its value against its limit as they
/// were when it was raised, and they stay while the value moves on; an alarm without a limit has
/// none.
#[test]
fn a_limit_alarms_row_keeps_its_value_and_limit_from_when_it_was_raised() {
    let mut harness = plant();
    let mut board = AlarmBoard::default();
    board.update(read_alarms(harness.runtime()), 1_000);
    harness.set_input("Level", Value::LReal(91.5));
    harness.set_input("Trip", Value::Bool(true));
    scan(&mut harness);
    board.update(read_alarms(harness.runtime()), 2_000);
    harness.set_input("Level", Value::LReal(97.0));
    scan(&mut harness);
    board.update(read_alarms(harness.runtime()), 3_000);
    let summary = board.summary();
    let row = |path: &str| {
        summary
            .active
            .iter()
            .find(|row| row.status.path == path)
            .unwrap_or_else(|| panic!("{path} in {:?}", summary.active))
    };
    assert_eq!(
        row("Tank.High").raised,
        Some(Raised {
            value: 91.5,
            limit: 90.0,
            high: true,
            source: None,
            unit: None,
        })
    );
    assert_eq!(row("Tank.Overflow").raised, None);
    let json = serde_json::to_value(row("Tank.High")).expect("serialises");
    assert_eq!(
        json["raised"],
        serde_json::json!({ "value": 91.5, "limit": 90.0, "high": true })
    );
}

#[test]
fn the_alarm_list_puts_unacknowledged_first_then_priority_then_newest() {
    let mut harness = plant();
    let mut board = AlarmBoard::default();
    harness.set_input("Level", Value::LReal(95.0));
    scan(&mut harness);
    board.update(read_alarms(harness.runtime()), 1_000);
    harness.set_input("Trip", Value::Bool(true));
    scan(&mut harness);
    board.update(read_alarms(harness.runtime()), 2_000);
    request(
        &mut harness,
        "Tank.High.Request.Acknowledge",
        serde_json::json!(true),
    );
    scan(&mut harness);
    board.update(read_alarms(harness.runtime()), 3_000);
    let summary = board.summary();
    let order: Vec<&str> = summary
        .active
        .iter()
        .map(|row| row.status.path.as_str())
        .collect();
    assert_eq!(
        order,
        vec!["Tank.Overflow", "Tank.High"],
        "the unacknowledged High one first"
    );
    assert_eq!(summary.active[0].since_ms, 2_000);
    assert_eq!((summary.unacknowledged, summary.annunciating), (1, 2));
    assert_eq!(
        summary.newest.as_ref().map(|row| row.status.path.as_str()),
        Some("Tank.Overflow")
    );
}

#[test]
fn shelved_suppressed_and_out_of_service_alarms_are_listed_apart_with_their_counts() {
    let mut harness = plant();
    let mut board = AlarmBoard::default();
    // The first read is the baseline; later reads record what changed.
    board.update(read_alarms(harness.runtime()), 500);
    request(
        &mut harness,
        "Tank.High.Request.Shelve",
        serde_json::json!(60.0),
    );
    request(
        &mut harness,
        "Tank.Overflow.Request.RemoveFromService",
        serde_json::json!(true),
    );
    scan(&mut harness);
    board.update(read_alarms(harness.runtime()), 1_000);
    let summary = board.summary();
    assert_eq!(
        summary
            .shelved
            .iter()
            .map(|row| row.status.path.as_str())
            .collect::<Vec<_>>(),
        vec!["Tank.High"]
    );
    assert_eq!(
        summary
            .out_of_service
            .iter()
            .map(|row| row.status.path.as_str())
            .collect::<Vec<_>>(),
        vec!["Tank.Overflow"]
    );
    assert!(summary.active.is_empty());
    let kinds: Vec<EventKind> = board.events().iter().map(|event| event.kind).collect();
    assert!(
        kinds.contains(&EventKind::Shelved) && kinds.contains(&EventKind::OutOfService),
        "{kinds:?}"
    );
}
