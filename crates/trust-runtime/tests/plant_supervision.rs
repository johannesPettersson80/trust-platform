//! Plant library specification §4.3, §4.4, §4.5, §6.1.3, §6.6.2 and §14.2, §14.5: feedback
//! supervision runs in Auto and in Manual whoever commanded the device, and only out of service is
//! it switched off, with the device's alarms suppressed by design; a supervision failure latches
//! and, after a Reset with its cause still present, returns after a full supervision time.

mod plant_support;

use plant_support::Rig;

const PROGRAM: &str = r#"
USING Plant;

PROGRAM Main
VAR
    RunRequest : BOOL;
    Feedback : BOOL;
    OpenRequest : BOOL;
    OpenSwitch : BOOL;
    ClosedSwitch : BOOL := TRUE;
    CmdManual : BOOL;
    CmdOutOfService : BOOL;
    CmdStart : BOOL;
    CmdReset : BOOL;
    M101 : Plant.FB_Motor := (StartTimeout := T#2s, StopTimeout := T#2s);
    XV201 : Plant.FB_OnOffValve := (OpenTimeout := T#2s, CloseTimeout := T#2s);
END_VAR
    IF CmdManual THEN
        M101.SetMode(Value := E_Mode#Manual);
        CmdManual := FALSE;
    END_IF;
    IF CmdOutOfService THEN
        M101.SetMode(Value := E_Mode#OutOfService);
        XV201.SetMode(Value := E_Mode#OutOfService);
        CmdOutOfService := FALSE;
    END_IF;
    IF CmdStart THEN
        M101.Start();
        CmdStart := FALSE;
    END_IF;
    IF CmdReset THEN
        M101.Reset();
        XV201.Reset();
        CmdReset := FALSE;
    END_IF;
    M101(AutoRun := RunRequest, RunningFeedback := Feedback);
    XV201(AutoOpen := OpenRequest, OpenFeedback := OpenSwitch, ClosedFeedback := ClosedSwitch);
END_PROGRAM
"#;

const MOTOR_ALARMS: [&str; 3] = ["TripAlarm", "FailedToStartAlarm", "FailedToStopAlarm"];
const VALVE_ALARMS: [&str; 3] = ["FailedToOpenAlarm", "FailedToCloseAlarm", "ConflictAlarm"];

fn rig() -> Rig {
    let mut rig = Rig::new(PROGRAM);
    rig.scan();
    rig
}

#[test]
fn a_motor_without_feedback_fails_to_start_after_its_start_timeout_in_auto() {
    let mut rig = rig();
    rig.set_bool("RunRequest", true);
    rig.scans(15);
    assert!(
        rig.flag("M101.RunCommand") && !rig.flag("M101.FailedToStart"),
        "1.5 s is within the 2 s start timeout"
    );
    rig.scans(10);
    assert!(
        rig.flag("M101.FailedToStart") && rig.flag("M101.Fault"),
        "no running feedback within the start timeout"
    );
    assert!(
        !rig.flag("M101.RunCommand"),
        "the failure puts the motor in its safe state"
    );
    assert_eq!(
        rig.text("M101.BlockReason"),
        "Failed to start: reset to try again"
    );
    assert!(rig.flag("M101.FailedToStartAlarm.Active"));
}

#[test]
fn a_motor_without_feedback_fails_to_start_after_its_start_timeout_in_manual() {
    let mut rig = rig();
    rig.press("CmdManual");
    rig.press("CmdStart");
    rig.scans(14);
    assert!(
        rig.flag("M101.RunCommand") && !rig.flag("M101.FailedToStart"),
        "1.5 s is within the 2 s start timeout"
    );
    rig.scans(10);
    assert!(
        rig.flag("M101.FailedToStart") && rig.flag("M101.Fault"),
        "supervision does not depend on who commanded the motor"
    );
    assert!(!rig.flag("M101.RunCommand"));
}

#[test]
fn a_motor_that_keeps_running_after_a_stop_fails_to_stop() {
    let mut rig = rig();
    rig.set_bool("RunRequest", true);
    rig.set_bool("Feedback", true);
    rig.scans(3);
    assert!(rig.flag("M101.Running") && !rig.flag("M101.Fault"));
    rig.set_bool("RunRequest", false);
    rig.scans(15);
    assert!(
        !rig.flag("M101.FailedToStop"),
        "1.5 s is within the 2 s stop timeout"
    );
    rig.scans(10);
    assert!(
        rig.flag("M101.FailedToStop") && rig.flag("M101.Fault"),
        "still running a stop timeout after the stop"
    );
    assert_eq!(
        rig.text("M101.BlockReason"),
        "Failed to stop: check the contactor, then reset"
    );
}

#[test]
fn out_of_service_supervision_stops_and_the_alarms_are_suppressed_by_design() {
    let mut rig = rig();
    for alarm in MOTOR_ALARMS {
        assert!(
            !rig.flag(&format!("M101.{alarm}.SuppressedByDesign")),
            "in service the motor's {alarm} is not suppressed"
        );
    }
    rig.press("CmdOutOfService");
    // The motor runs by hand and both end switches of the valve report: neither is supervised.
    rig.set_bool("Feedback", true);
    rig.set_bool("OpenSwitch", true);
    rig.scans(50);
    assert!(
        !rig.flag("M101.FailedToStop") && !rig.flag("M101.Fault"),
        "a motor out of service is not supervised"
    );
    assert_eq!(
        rig.count("M101.Starts"),
        0,
        "counters stand still out of service"
    );
    assert_eq!(rig.real("M101.RunHours"), 0.0);
    assert!(
        !rig.flag("XV201.FeedbackConflict") && !rig.flag("XV201.Fault"),
        "a valve out of service is not supervised"
    );
    for alarm in MOTOR_ALARMS {
        assert!(
            rig.flag(&format!("M101.{alarm}.SuppressedByDesign")),
            "out of service the motor's {alarm} is suppressed by design"
        );
    }
    for alarm in VALVE_ALARMS {
        assert!(
            rig.flag(&format!("XV201.{alarm}.SuppressedByDesign")),
            "out of service the valve's {alarm} is suppressed by design"
        );
    }
}

#[test]
fn after_a_reset_with_the_cause_still_present_the_failure_returns_after_a_full_timeout() {
    let mut rig = rig();
    rig.set_bool("RunRequest", true);
    rig.scans(25);
    assert!(rig.flag("M101.FailedToStart"));
    rig.press("CmdReset");
    assert!(
        !rig.flag("M101.FailedToStart") && !rig.flag("M101.Fault"),
        "Reset clears the supervision failure"
    );
    assert!(
        rig.flag("M101.RunCommand"),
        "in Auto the program commands the motor again"
    );
    rig.scans(14);
    assert!(
        !rig.flag("M101.FailedToStart"),
        "1.4 s after the reset the start timeout has not passed again"
    );
    rig.scans(10);
    assert!(
        rig.flag("M101.FailedToStart"),
        "the failure returns after a full start timeout"
    );
}

#[test]
fn a_valve_that_does_not_reach_open_in_time_fails_to_open() {
    let mut rig = rig();
    assert!(rig.flag("XV201.Closed") && !rig.flag("XV201.Fault"));
    rig.set_bool("OpenRequest", true);
    rig.scans(15);
    assert!(
        rig.flag("XV201.Moving") && !rig.flag("XV201.FailedToOpen"),
        "1.5 s is within the 2 s open timeout"
    );
    rig.scans(10);
    assert!(
        rig.flag("XV201.FailedToOpen") && rig.flag("XV201.Fault"),
        "not open within the open timeout"
    );
    assert!(
        !rig.flag("XV201.OpenCommand"),
        "the failure sends it to its fail position: closed"
    );
    assert_eq!(
        rig.text("XV201.BlockReason"),
        "Failed to open: reset to try again"
    );
}

#[test]
fn a_valve_that_does_not_reach_closed_in_time_fails_to_close() {
    let mut rig = Rig::new(PROGRAM);
    rig.set_bool("OpenRequest", true);
    rig.set_bool("OpenSwitch", true);
    rig.set_bool("ClosedSwitch", false);
    rig.scans(3);
    assert!(rig.flag("XV201.Opened") && !rig.flag("XV201.Fault"));
    rig.set_bool("OpenRequest", false);
    rig.scans(15);
    assert!(
        !rig.flag("XV201.FailedToClose"),
        "1.5 s is within the 2 s close timeout"
    );
    rig.scans(10);
    assert!(
        rig.flag("XV201.FailedToClose") && rig.flag("XV201.Fault"),
        "not closed within the close timeout"
    );
}

#[test]
fn a_valve_that_leaves_its_closed_position_fails_to_close() {
    let mut rig = rig();
    rig.set_bool("ClosedSwitch", false);
    rig.scans(15);
    assert!(
        !rig.flag("XV201.Fault"),
        "1.5 s is within the close timeout"
    );
    rig.scans(10);
    assert!(
        rig.flag("XV201.FailedToClose"),
        "leaving the commanded end position is a failure to hold it"
    );
}

#[test]
fn both_end_switches_raise_a_feedback_conflict() {
    let mut rig = rig();
    rig.set_bool("OpenSwitch", true);
    rig.scans(5);
    assert!(!rig.flag("XV201.FeedbackConflict"), "not within 0.5 s");
    rig.scans(10);
    assert!(
        rig.flag("XV201.FeedbackConflict") && rig.flag("XV201.Fault"),
        "both end switches for longer than a second"
    );
    assert_eq!(
        rig.text("XV201.BlockReason"),
        "Both end switches active: check them, then reset"
    );
}
