//! Plant library specification §4.1, §4.3, §4.6 and §14.1, §14.5: a device goes to its safe state
//! on an interlock, a latched fault and out of service, and a valve's safe state is its fail
//! position; protection trips latch until Reset after the trip is gone, and Reset never starts
//! anything; the permissive gates starts only; an interlock stops a running motor in every mode.

mod plant_support;

use plant_support::Rig;

const PROGRAM: &str = r#"
USING Plant;

PROGRAM Main
VAR
    OpenRequest : BOOL;
    RunRequest : BOOL;
    PositionRequest : LREAL := 40.0;
    Blocking : BOOL;
    Permitted : BOOL := TRUE;
    Overload : BOOL;
    ClosedSwitch : BOOL;
    CmdManual : BOOL;
    CmdOutOfService : BOOL;
    CmdStart : BOOL;
    CmdReset : BOOL;
    StartAccepted : BOOL;
    ResetAccepted : BOOL;
    (* Fail-open valves: one without feedback, one with a closed switch that is supervised. *)
    XV101 : Plant.FB_OnOffValve := (FailOpen := TRUE, HasOpenFeedback := FALSE,
        HasClosedFeedback := FALSE);
    XV102 : Plant.FB_OnOffValve := (FailOpen := TRUE, HasOpenFeedback := FALSE,
        CloseTimeout := T#2s);
    (* A fail-closed valve without feedback. *)
    XV103 : Plant.FB_OnOffValve := (HasOpenFeedback := FALSE, HasClosedFeedback := FALSE);
    FV201 : Plant.FB_ControlValve := (FailPosition := 100.0);
    P301 : Plant.FB_Pump := (HasFeedback := FALSE);
END_VAR
    IF CmdManual THEN
        P301.SetMode(Value := E_Mode#Manual);
        CmdManual := FALSE;
    END_IF;
    IF CmdOutOfService THEN
        XV101.SetMode(Value := E_Mode#OutOfService);
        XV103.SetMode(Value := E_Mode#OutOfService);
        CmdOutOfService := FALSE;
    END_IF;
    IF CmdStart THEN
        StartAccepted := P301.Start();
        CmdStart := FALSE;
    END_IF;
    IF CmdReset THEN
        ResetAccepted := P301.Reset();
        XV102.Reset();
        CmdReset := FALSE;
    END_IF;
    XV101(AutoOpen := OpenRequest, Interlock := Blocking);
    XV102(AutoOpen := OpenRequest, ClosedFeedback := ClosedSwitch);
    XV103(AutoOpen := OpenRequest, Interlock := Blocking);
    FV201(AutoPosition := PositionRequest, Interlock := Blocking);
    P301(AutoRun := RunRequest, Interlock := Blocking, InterlockText := 'Low suction pressure',
        Permissive := Permitted, PermissiveText := 'Suction valve closed', Trip := Overload);
END_PROGRAM
"#;

fn rig() -> Rig {
    let mut rig = Rig::new(PROGRAM);
    rig.scan();
    rig
}

#[test]
fn a_fail_open_valve_opens_on_an_interlock() {
    let mut rig = rig();
    assert!(
        !rig.flag("XV101.OpenCommand"),
        "the program keeps it closed"
    );
    assert!(
        rig.flag("XV101.Solenoid"),
        "a fail-open valve is energised to stay closed"
    );
    rig.set_bool("Blocking", true);
    rig.scan();
    assert!(rig.flag("XV101.Interlocked"));
    assert!(
        rig.flag("XV101.OpenCommand"),
        "the interlock sends it to its fail position: open"
    );
    assert!(!rig.flag("XV101.Solenoid"), "its solenoid is de-energised");
}

#[test]
fn a_fail_open_valve_opens_on_a_latched_fault_until_it_is_reset() {
    let mut rig = rig();
    rig.scans(14);
    assert!(
        !rig.flag("XV102.Fault"),
        "1.4 s is within the 2 s close timeout"
    );
    assert!(rig.flag("XV102.Solenoid"), "energised to close");
    rig.scans(10);
    assert!(
        rig.flag("XV102.FailedToClose") && rig.flag("XV102.Fault"),
        "not closed within the close timeout"
    );
    assert!(
        rig.flag("XV102.OpenCommand"),
        "the latched fault sends it to its fail position: open"
    );
    assert!(!rig.flag("XV102.Solenoid"));
    rig.set_bool("ClosedSwitch", true);
    rig.scans(5);
    assert!(
        rig.flag("XV102.Fault") && rig.flag("XV102.OpenCommand"),
        "the fault stays latched until Reset"
    );
    rig.press("CmdReset");
    rig.scans(2);
    assert!(
        !rig.flag("XV102.Fault"),
        "Reset clears a failure whose cause is gone"
    );
    assert!(
        !rig.flag("XV102.OpenCommand") && rig.flag("XV102.Solenoid"),
        "in Auto the program closes it again"
    );
}

#[test]
fn a_fail_open_valve_opens_out_of_service() {
    let mut rig = rig();
    assert!(!rig.flag("XV101.OpenCommand"));
    rig.press("CmdOutOfService");
    assert!(rig.flag("XV101.OutOfService"));
    assert!(
        rig.flag("XV101.OpenCommand"),
        "out of service a fail-open valve is open"
    );
    assert!(!rig.flag("XV101.Solenoid"));
}

#[test]
fn a_fail_closed_valve_closes_on_an_interlock_and_out_of_service() {
    let mut rig = rig();
    rig.set_bool("OpenRequest", true);
    rig.scan();
    assert!(
        rig.flag("XV103.OpenCommand") && rig.flag("XV103.Solenoid"),
        "energised to open"
    );
    rig.set_bool("Blocking", true);
    rig.scan();
    assert!(
        !rig.flag("XV103.OpenCommand") && !rig.flag("XV103.Solenoid"),
        "the interlock closes it"
    );
    rig.set_bool("Blocking", false);
    rig.scan();
    assert!(
        rig.flag("XV103.OpenCommand"),
        "in Auto the program opens it again once nothing blocks it"
    );
    rig.press("CmdOutOfService");
    assert!(
        !rig.flag("XV103.OpenCommand") && !rig.flag("XV103.Solenoid"),
        "out of service a fail-closed valve is closed"
    );
}

#[test]
fn a_control_valve_goes_to_its_fail_position_on_an_interlock() {
    let mut rig = rig();
    assert_eq!(
        rig.real("FV201.PositionSetpoint"),
        40.0,
        "the program positions it"
    );
    rig.set_bool("Blocking", true);
    rig.scan();
    assert_eq!(
        rig.real("FV201.PositionSetpoint"),
        100.0,
        "the interlock sends it to FailPosition"
    );
    rig.set_bool("Blocking", false);
    rig.scan();
    assert_eq!(
        rig.real("FV201.PositionSetpoint"),
        40.0,
        "in Auto it follows the program again"
    );
}

#[test]
fn a_protection_trip_latches_until_reset_after_the_trip_is_gone() {
    let mut rig = rig();
    rig.set_bool("RunRequest", true);
    rig.scan();
    assert!(rig.flag("P301.RunCommand"));
    rig.set_bool("Overload", true);
    rig.scan();
    assert!(
        rig.flag("P301.Tripped") && rig.flag("P301.Fault"),
        "the trip latches"
    );
    assert!(!rig.flag("P301.RunCommand"), "a trip stops the motor");
    assert_eq!(
        rig.text("P301.BlockReason"),
        "Protection trip: reset after the cause is cleared"
    );
    assert!(
        rig.flag("P301.TripAlarm.Active"),
        "the trip alarm is active while the trip is latched"
    );
    rig.press("CmdReset");
    assert!(rig.flag("ResetAccepted"), "Reset is accepted in every mode");
    assert!(
        rig.flag("P301.Tripped") && rig.flag("P301.Fault"),
        "a reset while the trip input is TRUE leaves the trip latched"
    );
    rig.set_bool("Overload", false);
    rig.scans(5);
    assert!(
        rig.flag("P301.Tripped") && !rig.flag("P301.RunCommand"),
        "the trip stays latched after its input is gone"
    );
    assert!(rig.flag("P301.TripAlarm.Active"));
    rig.press("CmdReset");
    assert!(
        !rig.flag("P301.Tripped") && !rig.flag("P301.Fault"),
        "Reset clears the trip once its input is gone"
    );
    assert!(
        rig.flag("P301.RunCommand"),
        "in Auto the program's AutoRun commands it again once nothing blocks it (§4.3.4)"
    );
}

#[test]
fn a_reset_never_starts_a_motor_the_operator_ran() {
    let mut rig = rig();
    rig.press("CmdManual");
    rig.press("CmdStart");
    assert!(rig.flag("P301.RunCommand"));
    rig.set_bool("Overload", true);
    rig.scan();
    assert!(!rig.flag("P301.RunCommand"), "the trip stops it");
    rig.set_bool("Overload", false);
    rig.scan();
    rig.press("CmdReset");
    assert!(!rig.flag("P301.Fault"), "the trip is reset");
    rig.scans(5);
    assert!(
        !rig.flag("P301.RunCommand"),
        "a reset never starts the motor: the fault stopped it for good"
    );
    rig.press("CmdStart");
    assert!(
        rig.flag("StartAccepted") && rig.flag("P301.RunCommand"),
        "the operator starts it again"
    );
}

#[test]
fn the_permissive_gates_an_operator_start_but_not_a_running_motor() {
    let mut rig = rig();
    rig.press("CmdManual");
    rig.set_bool("Permitted", false);
    rig.scan();
    assert!(!rig.flag("P301.StartPermit"));
    assert_eq!(
        rig.text("P301.BlockReason"),
        "Not permitted: Suction valve closed"
    );
    rig.press("CmdStart");
    assert!(
        !rig.flag("StartAccepted"),
        "a start without the permissive is refused"
    );
    assert!(!rig.flag("P301.RunCommand"));
    rig.set_bool("Permitted", true);
    rig.scan();
    rig.press("CmdStart");
    assert!(rig.flag("StartAccepted") && rig.flag("P301.RunCommand"));
    rig.set_bool("Permitted", false);
    rig.scans(5);
    assert!(
        rig.flag("P301.RunCommand"),
        "a running motor keeps running when the permissive drops"
    );
    assert!(rig.flag("P301.Blocked"), "a new start would be refused");
}

#[test]
fn in_auto_the_permissive_gates_the_programs_start_only() {
    let mut rig = rig();
    rig.set_bool("Permitted", false);
    rig.set_bool("RunRequest", true);
    rig.scans(3);
    assert!(
        !rig.flag("P301.RunCommand"),
        "the program cannot start it without the permissive"
    );
    rig.set_bool("Permitted", true);
    rig.scan();
    assert!(rig.flag("P301.RunCommand"));
    rig.set_bool("Permitted", false);
    rig.scans(3);
    assert!(
        rig.flag("P301.RunCommand"),
        "it keeps running when the permissive drops"
    );
}

#[test]
fn an_interlock_stops_a_running_motor_in_auto_and_in_manual() {
    let mut rig = rig();
    rig.set_bool("RunRequest", true);
    rig.scan();
    rig.set_bool("Blocking", true);
    rig.scan();
    assert!(
        !rig.flag("P301.RunCommand"),
        "the interlock stops it in Auto"
    );
    assert!(rig.flag("P301.Interlocked"));
    assert_eq!(
        rig.text("P301.BlockReason"),
        "Interlocked: Low suction pressure"
    );
    rig.set_bool("Blocking", false);
    rig.scan();
    assert!(
        rig.flag("P301.RunCommand"),
        "in Auto the program runs it again once the interlock clears"
    );
    rig.press("CmdManual");
    assert!(rig.flag("P301.RunCommand"), "bumpless into Manual");
    rig.set_bool("Blocking", true);
    rig.scan();
    assert!(
        !rig.flag("P301.RunCommand"),
        "the interlock stops it in Manual"
    );
    rig.set_bool("Blocking", false);
    rig.scans(3);
    assert!(
        !rig.flag("P301.RunCommand"),
        "in Manual it stays stopped until the operator starts it"
    );
}
