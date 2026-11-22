//! Plant library specification §5, §6.2.6, §6.3.3 and §6.9.5: a control valve, a drive and a
//! heater are controlled devices. `FollowsAuto()` is TRUE while the device follows its `Auto…`
//! value now, and `GetActualValue()` is what it does, so that a controller tracks the device while
//! it does not follow (§8.1). Each expected value is the specification's rule worked by hand.

mod plant_support;

use plant_support::Rig;

/// The three devices, each read through the interface, as a controller reads it.
const PROGRAM: &str = r#"
USING Plant;

PROGRAM Main
VAR
    Request : LREAL;
    Run : BOOL;
    Blocking : BOOL;
    Allowed : BOOL := TRUE;
    CmdManual : BOOL;
    CmdAuto : BOOL;
    FV301 : Plant.FB_ControlValve;
    M201 : Plant.FB_VariableSpeedMotor := (HasFeedback := FALSE);
    EH101 : Plant.FB_Heater := (HasFeedback := FALSE);
    Valve : IControlledDevice;
    Drive : IControlledDevice;
    Heater : IControlledDevice;
    ValveFollows : BOOL;
    ValveActual : LREAL;
    DriveFollows : BOOL;
    DriveActual : LREAL;
    HeaterFollows : BOOL;
    HeaterActual : LREAL;
END_VAR
    IF CmdManual THEN
        FV301.SetMode(Value := E_Mode#Manual);
        M201.SetMode(Value := E_Mode#Manual);
        EH101.SetMode(Value := E_Mode#Manual);
        CmdManual := FALSE;
    END_IF;
    IF CmdAuto THEN
        FV301.SetMode(Value := E_Mode#Auto);
        M201.SetMode(Value := E_Mode#Auto);
        EH101.SetMode(Value := E_Mode#Auto);
        CmdAuto := FALSE;
    END_IF;
    FV301(AutoPosition := Request, Interlock := Blocking, Permissive := Allowed);
    M201(AutoRun := Run, AutoSpeed := Request, Interlock := Blocking);
    EH101(AutoRun := Run, AutoPower := Request, Interlock := Blocking);
    Valve := FV301;
    Drive := M201;
    Heater := EH101;
    ValveFollows := Valve.FollowsAuto();
    ValveActual := Valve.GetActualValue();
    DriveFollows := Drive.FollowsAuto();
    DriveActual := Drive.GetActualValue();
    HeaterFollows := Heater.FollowsAuto();
    HeaterActual := Heater.GetActualValue();
END_PROGRAM
"#;

fn rig() -> Rig {
    let mut rig = Rig::new(PROGRAM);
    rig.set_real("Request", 37.5);
    rig.scan();
    rig
}

/// §6.9.5 with §6.9.4: catches a valve reported as following while its interlock holds it at its
/// fail position or before its permissive releases it, and one reported as not following after a
/// permissive that dropped once it was released.
#[test]
fn a_control_valve_follows_its_auto_position_while_in_auto_and_released() {
    let mut rig = rig();
    assert!(rig.flag("ValveFollows"), "in Auto and released at once");
    assert_eq!(rig.real("ValveActual"), 37.5, "it is where Auto put it");

    rig.set_bool("Blocking", true);
    rig.scan();
    assert!(!rig.flag("ValveFollows"), "the interlock holds it");
    assert_eq!(rig.real("ValveActual"), 0.0, "at its fail position");

    rig.set_bool("Allowed", false);
    rig.set_bool("Blocking", false);
    rig.scan();
    assert!(
        !rig.flag("ValveFollows"),
        "at its fail position it waits for its permissive"
    );
    assert_eq!(rig.real("ValveActual"), 0.0);

    rig.set_bool("Allowed", true);
    rig.scan();
    assert!(rig.flag("ValveFollows"), "the permissive releases it");
    assert_eq!(rig.real("ValveActual"), 37.5);

    rig.set_bool("Allowed", false);
    rig.scan();
    assert!(
        rig.flag("ValveFollows"),
        "once released it keeps following when the permissive drops"
    );

    rig.press("CmdManual");
    assert!(!rig.flag("ValveFollows"), "in Manual the operator moves it");
    assert_eq!(
        rig.real("ValveActual"),
        rig.real("FV301.Position"),
        "what it does is its position"
    );
}

/// §6.2.6: catches a drive reported as following while stopped, held by its interlock or in
/// Manual, and an actual value that is not its speed.
#[test]
fn a_drive_follows_its_auto_speed_while_in_auto_and_running() {
    let mut rig = rig();
    assert!(!rig.flag("DriveFollows"), "a stopped drive follows nothing");
    assert_eq!(rig.real("DriveActual"), 0.0, "a stopped drive runs at 0 %");

    rig.set_bool("Run", true);
    rig.scan();
    assert!(rig.flag("DriveFollows"));
    assert_eq!(rig.real("DriveActual"), 37.5);

    rig.set_bool("Blocking", true);
    rig.scan();
    assert!(!rig.flag("DriveFollows"), "the interlock stops it");
    assert_eq!(rig.real("DriveActual"), 0.0);

    rig.set_bool("Blocking", false);
    rig.scan();
    assert!(rig.flag("DriveFollows"), "Auto starts it again");
    rig.press("CmdManual");
    assert!(
        !rig.flag("DriveFollows"),
        "in Manual the operator sets its speed"
    );
    assert_eq!(rig.real("DriveActual"), rig.real("M201.Speed"));
}

/// §6.3.3: catches a heater reported as following while stopped or in Manual, and an actual value
/// that is not its power.
#[test]
fn a_heater_follows_its_auto_power_while_in_auto_and_running() {
    let mut rig = rig();
    assert!(
        !rig.flag("HeaterFollows"),
        "a stopped heater follows nothing"
    );
    assert_eq!(rig.real("HeaterActual"), 0.0, "a stopped heater gives 0 %");

    rig.set_bool("Run", true);
    rig.scan();
    assert!(rig.flag("HeaterFollows"));
    assert_eq!(rig.real("HeaterActual"), 37.5);

    rig.press("CmdManual");
    assert!(
        !rig.flag("HeaterFollows"),
        "in Manual the operator sets its power"
    );
    assert_eq!(rig.real("HeaterActual"), rig.real("EH101.PowerOutput"));

    rig.press("CmdAuto");
    rig.set_bool("Run", false);
    rig.scan();
    assert!(!rig.flag("HeaterFollows"), "stopped again");
    assert_eq!(rig.real("HeaterActual"), 0.0);
}
