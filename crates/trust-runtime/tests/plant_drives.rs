//! Plant library specification §6.2, §6.3, §4.8.2 and §14.6: a drive and a heater start at 0 %:
//! the operator's speed or power is 0 % after a restart and after every stop, may be chosen while
//! stopped, and is clamped to the block's range; a heater gives no power while stopped and
//! time-proportions its pulse output over the pulse period.

mod plant_support;

use plant_support::Rig;

const PROGRAM: &str = r#"
USING Plant;

PROGRAM Main
VAR
    RunRequest : BOOL;
    SpeedRequest : LREAL;
    PowerRequest : LREAL;
    SpeedValue : LREAL;
    PowerValue : LREAL;
    CmdManual : BOOL;
    CmdStart : BOOL;
    CmdStop : BOOL;
    CmdSetSpeed : BOOL;
    CmdSetPower : BOOL;
    SpeedAccepted : BOOL;
    PowerAccepted : BOOL;
    M101 : Plant.FB_VariableSpeedMotor := (HasFeedback := FALSE);
    M102 : Plant.FB_VariableSpeedPump := (HasFeedback := FALSE, MinSpeed := 20.0,
        MaxSpeed := 80.0);
    H201 : Plant.FB_Heater := (HasFeedback := FALSE, PulsePeriod := T#10s);
END_VAR
    IF CmdManual THEN
        M101.SetMode(Value := E_Mode#Manual);
        M102.SetMode(Value := E_Mode#Manual);
        H201.SetMode(Value := E_Mode#Manual);
        CmdManual := FALSE;
    END_IF;
    IF CmdStart THEN
        M101.Start();
        M102.Start();
        H201.Start();
        CmdStart := FALSE;
    END_IF;
    IF CmdStop THEN
        M101.Stop();
        M102.Stop();
        H201.Stop();
        CmdStop := FALSE;
    END_IF;
    IF CmdSetSpeed THEN
        SpeedAccepted := M101.SetSpeed(Value := SpeedValue);
        M102.SetSpeed(Value := SpeedValue);
        CmdSetSpeed := FALSE;
    END_IF;
    IF CmdSetPower THEN
        PowerAccepted := H201.SetPower(Value := PowerValue);
        CmdSetPower := FALSE;
    END_IF;
    M101(AutoRun := RunRequest, AutoSpeed := SpeedRequest);
    M102(AutoRun := RunRequest, AutoSpeed := SpeedRequest);
    H201(AutoRun := RunRequest, AutoPower := PowerRequest);
END_PROGRAM
"#;

fn rig() -> Rig {
    let mut rig = Rig::new(PROGRAM);
    rig.scan();
    rig
}

#[test]
fn a_drive_and_a_heater_start_at_zero_after_a_warm_restart() {
    let mut rig = rig();
    rig.press("CmdManual");
    rig.set_real("SpeedValue", 60.0);
    rig.press("CmdSetSpeed");
    rig.set_real("PowerValue", 40.0);
    rig.press("CmdSetPower");
    rig.press("CmdStart");
    assert_eq!(rig.real("M101.SpeedSetpoint"), 60.0);
    assert_eq!(rig.real("H201.PowerOutput"), 40.0);
    rig.warm_restart();
    rig.scan();
    assert_eq!(rig.state("M101.Mode"), "Manual", "the mode is retained");
    assert!(
        !rig.flag("M101.RunCommand") && !rig.flag("H201.RunCommand"),
        "manual run commands are not retained"
    );
    assert_eq!(
        rig.real("M101.ManualSpeed"),
        0.0,
        "the drive's manual speed is 0 % after the restart"
    );
    assert_eq!(
        rig.real("H201.ManualPower"),
        0.0,
        "the heater's manual power is 0 % after the restart"
    );
    rig.press("CmdStart");
    assert!(rig.flag("M101.RunCommand") && rig.flag("H201.RunCommand"));
    assert_eq!(
        rig.real("M101.SpeedSetpoint"),
        0.0,
        "started again, the drive runs at 0 %"
    );
    assert_eq!(
        rig.real("H201.PowerOutput"),
        0.0,
        "started again, the heater gives 0 %"
    );
}

#[test]
fn a_speed_chosen_while_stopped_is_used_at_the_start_and_returns_to_zero_after_a_stop() {
    let mut rig = rig();
    rig.press("CmdManual");
    rig.set_real("SpeedValue", 45.0);
    rig.press("CmdSetSpeed");
    assert!(
        rig.flag("SpeedAccepted"),
        "SetSpeed is accepted in Manual while stopped"
    );
    assert_eq!(
        rig.real("M101.SpeedSetpoint"),
        0.0,
        "the speed reference is 0 % while stopped"
    );
    assert_eq!(rig.real("M101.ManualSpeed"), 45.0);
    rig.press("CmdStart");
    assert_eq!(
        rig.real("M101.SpeedSetpoint"),
        45.0,
        "the drive starts at the chosen speed"
    );
    rig.press("CmdStop");
    assert_eq!(rig.real("M101.SpeedSetpoint"), 0.0);
    assert_eq!(
        rig.real("M101.ManualSpeed"),
        0.0,
        "the manual speed returns to 0 % when the drive stops"
    );
    rig.press("CmdStart");
    assert!(rig.flag("M101.RunCommand"));
    assert_eq!(
        rig.real("M101.SpeedSetpoint"),
        0.0,
        "started again without a new speed, it runs at 0 %"
    );
}

#[test]
fn the_speed_setpoint_is_clamped_to_the_speed_range() {
    let mut rig = rig();
    rig.set_bool("RunRequest", true);
    rig.set_real("SpeedRequest", 95.0);
    rig.scan();
    assert_eq!(
        rig.real("M102.SpeedSetpoint"),
        80.0,
        "the program's speed is clamped to MaxSpeed"
    );
    rig.set_real("SpeedRequest", 5.0);
    rig.scan();
    assert_eq!(rig.real("M102.SpeedSetpoint"), 20.0, "and to MinSpeed");
    rig.press("CmdManual");
    rig.set_real("SpeedValue", 150.0);
    rig.press("CmdSetSpeed");
    assert_eq!(
        rig.real("M102.ManualSpeed"),
        80.0,
        "SetSpeed is clamped to MaxSpeed"
    );
    assert_eq!(rig.real("M102.SpeedSetpoint"), 80.0);
    rig.set_real("SpeedValue", -10.0);
    rig.press("CmdSetSpeed");
    assert_eq!(rig.real("M102.SpeedSetpoint"), 20.0, "and to MinSpeed");
}

#[test]
fn a_heater_gives_no_power_while_stopped_and_starts_at_zero_after_a_stop() {
    let mut rig = rig();
    rig.press("CmdManual");
    rig.set_real("PowerValue", 30.0);
    rig.press("CmdSetPower");
    assert!(
        rig.flag("PowerAccepted"),
        "SetPower is accepted in Manual while stopped"
    );
    rig.scans(3);
    assert_eq!(rig.real("H201.PowerOutput"), 0.0, "no power while stopped");
    assert!(!rig.flag("H201.PulseOutput"));
    rig.press("CmdStart");
    assert_eq!(
        rig.real("H201.PowerOutput"),
        30.0,
        "the heater starts at the chosen power"
    );
    rig.press("CmdStop");
    assert_eq!(rig.real("H201.PowerOutput"), 0.0);
    assert!(!rig.flag("H201.PulseOutput"));
    assert_eq!(
        rig.real("H201.ManualPower"),
        0.0,
        "the manual power returns to 0 % when the heater stops"
    );
    rig.press("CmdStart");
    assert_eq!(
        rig.real("H201.PowerOutput"),
        0.0,
        "started again without a new power, it gives 0 %"
    );
}

#[test]
fn a_heater_pulse_output_is_on_for_its_share_of_the_pulse_period() {
    let mut rig = Rig::new(PROGRAM);
    rig.set_bool("RunRequest", true);
    rig.set_real("PowerRequest", 30.0);
    rig.step_ms(1_000);
    assert_eq!(rig.real("H201.PowerOutput"), 30.0);
    let mut on = 0;
    for _ in 0..50 {
        rig.step_ms(1_000);
        if rig.flag("H201.PulseOutput") {
            on += 1;
        }
    }
    assert_eq!(
        on, 15,
        "30 % power is on for 3 s of every 10 s pulse period: 15 of 50 one-second scans"
    );
}
