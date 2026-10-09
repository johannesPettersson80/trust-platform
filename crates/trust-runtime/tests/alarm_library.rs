//! Specification 40: the shipped `Alarm` library, run on the VM: the ISA-18.2 lifecycle, delays,
//! latching, shelving, suppression, out of service, first out, the detection blocks, own
//! detection by extending the base, and operator actions through the instance's request.

use serde_json::json;
use trust_runtime::harness::TestHarness;
use trust_runtime::hmi::commands::write::{write_point, WriteAction};
use trust_runtime::hmi::member_paths::{resolve_member, MemberPath};
use trust_runtime::value::{Duration, Value};

const ALARM: &str = include_str!("../../../libraries/alarm/src/alarm.st");

const PROGRAM: &str = r#"
USING Alarm;

FUNCTION_BLOCK DryRunAlarm EXTENDS Alarm.Alarm
VAR_INPUT
    Running : BOOL;
    Flow : LREAL;
END_VAR
    Evaluate(Condition := Running AND Flow < 1.0);
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    Trip : BOOL;
    Delayed : BOOL;
    Level : LREAL := 50.0;
    Temperature : LREAL := 20.0;
    Setpoint : LREAL := 20.0;
    Start : BOOL;
    Running : BOOL;
    Flow : LREAL := 5.0;
    PumpRunning : BOOL;
    Stopped : BOOL;
    TripGroup : FirstOutGroup;
    Digital : DigitalAlarm;
    Slow : DigitalAlarm;
    Latch : DigitalAlarm;
    High : LimitAlarm;
    Low : LimitAlarm;
    Deviation : DeviationAlarm;
    FailedToStart : DiscrepancyAlarm;
    DryRun : DryRunAlarm;
    PressureHigh : DigitalAlarm;
    PressureLow : DigitalAlarm;
    GroupedA : BOOL;
    GroupedB : BOOL;
    ResetGroup : BOOL;
    AckButton : BOOL;
END_VAR
    Digital(Condition := Trip, Message := 'Trip', Priority := AlarmPriority#High, Suppressed := Stopped);
    Slow(Condition := Delayed, OnDelay := T#2s, OffDelay := T#1s);
    Latch(Condition := Trip, Latching := TRUE);
    High(Value := Level, Limit := 90.0, Deadband := 5.0);
    Low(Value := Level, Limit := 10.0, Deadband := 5.0, Direction := LimitDirection#Low);
    Deviation(Value := Temperature, Reference := Setpoint, Limit := 3.0, Deadband := 1.0);
    FailedToStart(Command := Start, Feedback := Running, OnDelay := T#5s);
    DryRun(Running := PumpRunning, Flow := Flow);
    PressureHigh(Condition := GroupedA, Group := REF(TripGroup));
    PressureLow(Condition := GroupedB, Group := REF(TripGroup));
    IF AckButton THEN
        Digital.Acknowledge();
        AckButton := FALSE;
    END_IF;
    IF ResetGroup THEN
        TripGroup.Reset();
        ResetGroup := FALSE;
    END_IF;
END_PROGRAM
"#;

struct Plant {
    harness: TestHarness,
}

impl Plant {
    fn new() -> Self {
        let harness = TestHarness::from_sources(&[ALARM, PROGRAM]);
        assert!(
            harness.is_ok(),
            "the library and the program compile: {:?}",
            harness.as_ref().err()
        );
        let mut plant = Self {
            harness: harness.expect("compiled"),
        };
        plant.scan();
        plant
    }

    /// Advances the clock by 100 ms and runs one scan.
    fn scan(&mut self) {
        self.harness.advance_time(Duration::from_nanos(100_000_000));
        let result = self.harness.cycle();
        assert!(
            result.errors.is_empty(),
            "the scan runs: {:?}",
            result.errors
        );
    }

    fn scans(&mut self, count: usize) {
        for _ in 0..count {
            self.scan();
        }
    }

    fn set(&mut self, name: &str, value: Value) {
        self.harness.set_input(name, value);
    }

    fn read(&self, path: &str) -> Value {
        let path = MemberPath::parse(&format!("Main.{path}")).expect("a path");
        resolve_member(&path, self.harness.runtime().storage())
            .cloned()
            .unwrap_or_else(|| panic!("Main.{path:?} exists"))
    }

    fn flag(&self, path: &str) -> bool {
        self.read(path) == Value::Bool(true)
    }

    fn state(&self, alarm: &str) -> String {
        match self.read(&format!("{alarm}.State")) {
            Value::Enum(value) => value.variant_name().to_string(),
            other => panic!("{alarm}.State is an enumeration: {other:?}"),
        }
    }

    fn count(&self, alarm: &str) -> u32 {
        match self.read(&format!("{alarm}.RaiseCount")) {
            Value::UDInt(count) => count,
            other => panic!("{alarm}.RaiseCount is a UDINT: {other:?}"),
        }
    }

    /// An operator action as the runtime applies it: the request field is written.
    fn request(&mut self, alarm: &str, field: &str, value: serde_json::Value) {
        let path = MemberPath::parse(&format!("Main.{alarm}.Request.{field}")).expect("a path");
        write_point(self.harness.runtime_mut(), &path, &WriteAction::Set(value))
            .expect("the request is written");
    }
}

#[test]
fn an_alarm_goes_through_the_isa_18_2_states() {
    let mut plant = Plant::new();
    assert_eq!(plant.state("Digital"), "Normal");
    plant.set("Trip", Value::Bool(true));
    plant.scan();
    assert_eq!(plant.state("Digital"), "ActiveUnacknowledged");
    assert!(
        plant.flag("Digital.Active")
            && plant.flag("Digital.Unacknowledged")
            && plant.flag("Digital.Annunciating")
    );
    assert_eq!(plant.count("Digital"), 1);
    plant.request("Digital", "Acknowledge", json!(true));
    plant.scan();
    assert_eq!(plant.state("Digital"), "ActiveAcknowledged");
    assert!(!plant.flag("Digital.Unacknowledged"));
    assert!(
        !plant.flag("Digital.Request.Acknowledge"),
        "a request is applied once and cleared"
    );
    plant.set("Trip", Value::Bool(false));
    plant.scan();
    assert_eq!(plant.state("Digital"), "Normal");
    assert!(!plant.flag("Digital.Annunciating"));
    plant.set("Trip", Value::Bool(true));
    plant.scan();
    plant.set("Trip", Value::Bool(false));
    plant.scan();
    assert_eq!(
        plant.state("Digital"),
        "ClearedUnacknowledged",
        "cleared before it was acknowledged"
    );
    assert!(!plant.flag("Digital.Active") && plant.flag("Digital.Annunciating"));
    assert_eq!(plant.count("Digital"), 2);
    plant.request("Digital", "Acknowledge", json!(true));
    plant.scan();
    assert_eq!(plant.state("Digital"), "Normal");
}

#[test]
fn on_and_off_delays_hold_back_raising_and_clearing() {
    let mut plant = Plant::new();
    plant.set("Delayed", Value::Bool(true));
    plant.scans(15);
    assert!(
        !plant.flag("Slow.Active"),
        "1.5 s is shorter than the on-delay"
    );
    plant.scans(6);
    assert!(plant.flag("Slow.Active"), "after 2 s");
    plant.set("Delayed", Value::Bool(false));
    plant.scans(5);
    assert!(
        plant.flag("Slow.Active"),
        "0.5 s is shorter than the off-delay"
    );
    plant.scans(6);
    assert!(!plant.flag("Slow.Active"), "after 1 s");
}

#[test]
fn a_latching_alarm_stays_active_until_it_is_reset_after_its_condition_is_gone() {
    let mut plant = Plant::new();
    plant.set("Trip", Value::Bool(true));
    plant.scan();
    plant.set("Trip", Value::Bool(false));
    plant.scan();
    assert!(plant.flag("Latch.Active"), "latched");
    plant.set("Trip", Value::Bool(true));
    plant.scan();
    plant.request("Latch", "Reset", json!(true));
    plant.scan();
    assert!(
        plant.flag("Latch.Active"),
        "a reset while the condition holds changes nothing"
    );
    plant.set("Trip", Value::Bool(false));
    plant.scan();
    plant.request("Latch", "Acknowledge", json!(true));
    plant.scan();
    plant.request("Latch", "Reset", json!(true));
    plant.scan();
    assert!(!plant.flag("Latch.Active"));
    assert_eq!(
        plant.state("Latch"),
        "Normal",
        "acknowledged before the reset"
    );
}

#[test]
fn a_shelved_alarm_does_not_annunciate_and_returns_by_itself() {
    let mut plant = Plant::new();
    plant.request("Digital", "Shelve", json!(3.0));
    plant.scan();
    assert!(plant.flag("Digital.Shelved"));
    plant.set("Trip", Value::Bool(true));
    plant.scan();
    assert!(
        !plant.flag("Digital.Annunciating") && !plant.flag("Digital.Unacknowledged"),
        "shelved"
    );
    assert_eq!(plant.count("Digital"), 1, "its condition is still followed");
    plant.scans(30);
    assert!(!plant.flag("Digital.Shelved"), "returned after 3 s");
    assert_eq!(
        plant.state("Digital"),
        "ActiveUnacknowledged",
        "its condition holds on return"
    );
    plant.request("Digital", "Shelve", json!(3600.0));
    plant.scan();
    plant.request("Digital", "Unshelve", json!(true));
    plant.scan();
    assert!(!plant.flag("Digital.Shelved"), "unshelved by the operator");
}

#[test]
fn suppressed_by_design_and_out_of_service_alarms_do_not_annunciate() {
    let mut plant = Plant::new();
    plant.set("Stopped", Value::Bool(true));
    plant.set("Trip", Value::Bool(true));
    plant.scan();
    assert!(plant.flag("Digital.SuppressedByDesign") && !plant.flag("Digital.Annunciating"));
    plant.set("Stopped", Value::Bool(false));
    plant.scan();
    assert_eq!(
        plant.state("Digital"),
        "ActiveUnacknowledged",
        "returns as from shelving"
    );
    plant.request("Digital", "RemoveFromService", json!(true));
    plant.scan();
    assert!(plant.flag("Digital.OutOfService") && !plant.flag("Digital.Annunciating"));
    plant.request("Digital", "ReturnToService", json!(true));
    plant.scan();
    assert!(!plant.flag("Digital.OutOfService") && plant.flag("Digital.Annunciating"));
}

#[test]
fn limit_alarms_raise_beyond_their_limit_and_clear_inside_the_deadband() {
    let mut plant = Plant::new();
    plant.set("Level", Value::LReal(91.0));
    plant.scan();
    assert!(plant.flag("High.Active"));
    plant.set("Level", Value::LReal(87.0));
    plant.scan();
    assert!(plant.flag("High.Active"), "inside the deadband");
    plant.set("Level", Value::LReal(84.0));
    plant.scan();
    assert!(!plant.flag("High.Active"));
    plant.set("Level", Value::LReal(9.0));
    plant.scan();
    assert!(plant.flag("Low.Active"));
    plant.set("Level", Value::LReal(14.0));
    plant.scan();
    assert!(plant.flag("Low.Active"), "inside the deadband");
    plant.set("Level", Value::LReal(f64::NAN));
    plant.scan();
    assert!(
        plant.flag("High.Active") && plant.flag("Low.Active"),
        "a broken measurement is beyond both limits"
    );
}

#[test]
fn deviation_and_discrepancy_alarms() {
    let mut plant = Plant::new();
    plant.set("Temperature", Value::LReal(24.0));
    plant.scan();
    assert!(plant.flag("Deviation.Active"));
    plant.set("Temperature", Value::LReal(22.5));
    plant.scan();
    assert!(plant.flag("Deviation.Active"), "inside the deadband");
    plant.set("Temperature", Value::LReal(21.5));
    plant.scan();
    assert!(!plant.flag("Deviation.Active"));
    plant.set("Start", Value::Bool(true));
    plant.scans(40);
    assert!(
        !plant.flag("FailedToStart.Active"),
        "4 s is within the time to follow"
    );
    plant.scans(12);
    assert!(
        plant.flag("FailedToStart.Active"),
        "failed to start after 5 s"
    );
}

#[test]
fn own_detection_extends_the_base_and_gets_the_whole_lifecycle() {
    let mut plant = Plant::new();
    plant.set("PumpRunning", Value::Bool(true));
    plant.set("Flow", Value::LReal(0.2));
    plant.scan();
    assert_eq!(plant.state("DryRun"), "ActiveUnacknowledged");
    plant.request("DryRun", "Acknowledge", json!(true));
    plant.scan();
    assert_eq!(plant.state("DryRun"), "ActiveAcknowledged");
}

#[test]
fn the_first_alarm_of_a_group_is_marked_first_out_until_the_group_is_reset() {
    let mut plant = Plant::new();
    plant.set("GroupedB", Value::Bool(true));
    plant.scan();
    plant.set("GroupedA", Value::Bool(true));
    plant.scan();
    assert!(plant.flag("PressureLow.FirstOut"), "became active first");
    assert!(!plant.flag("PressureHigh.FirstOut"));
    plant.set("GroupedB", Value::Bool(false));
    plant.scan();
    assert!(plant.flag("PressureLow.FirstOut"), "kept after it clears");
    plant.set("ResetGroup", Value::Bool(true));
    // The program resets the group after calling its alarms, so they see it in the next scan.
    plant.scans(2);
    assert!(
        !plant.flag("PressureLow.FirstOut") && !plant.flag("PressureHigh.FirstOut"),
        "the group is reset"
    );
    plant.set("GroupedA", Value::Bool(false));
    plant.scan();
    plant.set("GroupedA", Value::Bool(true));
    plant.scan();
    assert!(
        plant.flag("PressureHigh.FirstOut"),
        "the first after the reset"
    );
}

#[test]
fn a_comment_request_is_cleared_after_the_scan() {
    let mut plant = Plant::new();
    plant.request("Digital", "Comment", json!("Checked on site"));
    plant.scan();
    assert_eq!(
        plant.read("Digital.Request.Comment"),
        Value::String("".into())
    );
}

#[test]
fn a_program_acknowledges_through_the_method_as_a_local_push_button_does() {
    let mut plant = Plant::new();
    plant.set("Trip", Value::Bool(true));
    plant.scan();
    plant.set("AckButton", Value::Bool(true));
    plant.scan();
    assert_eq!(plant.state("Digital"), "ActiveAcknowledged");
}

/// Specification 40, §3.8: an alarm declared in a RETAIN section keeps its state across a warm
/// restart; its timers start over.
#[test]
fn a_retained_alarm_keeps_its_state_across_a_warm_restart() {
    let program = r#"
USING Alarm;
PROGRAM Main
VAR
    Trip : BOOL;
END_VAR
VAR RETAIN
    Kept : DigitalAlarm;
END_VAR
VAR
    Lost : DigitalAlarm;
END_VAR
    Kept(Condition := Trip);
    Lost(Condition := Trip);
END_PROGRAM
"#;
    let harness = TestHarness::from_sources(&[ALARM, program]);
    let mut plant = Plant {
        harness: harness.expect("compiled"),
    };
    plant.set("Trip", Value::Bool(true));
    plant.scan();
    plant.request("Kept", "Acknowledge", json!(true));
    plant.request("Kept", "Shelve", json!(60.0));
    plant.scan();
    assert!(plant.flag("Kept.Shelved"));
    plant
        .harness
        .restart(trust_runtime::RestartMode::Warm)
        .expect("a warm restart");
    plant.set("Trip", Value::Bool(true));
    plant.scan();
    assert!(plant.flag("Kept.Shelved"), "the shelving survived");
    assert_eq!(plant.count("Kept"), 1, "no new raise: it stayed active");
    assert_eq!(
        plant.count("Lost"),
        1,
        "the alarm without RETAIN started over and was raised again"
    );
}
