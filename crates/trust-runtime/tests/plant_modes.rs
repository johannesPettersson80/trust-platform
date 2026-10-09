//! Plant library specification §3 and §14.3–§14.4: who commands a device is decided by its mode.
//! Auto follows the program's `Auto…` inputs, Manual the operator's commands, Out of service holds
//! the safe state; changing between Auto and Manual is bumpless for a motor, a drive and a control
//! valve; a refused command has no effect and leaves nothing pending.

mod plant_support;

use plant_support::Rig;

const PROGRAM: &str = r#"
USING Plant;

PROGRAM Main
VAR
    RunRequest : BOOL;
    SpeedRequest : LREAL;
    PositionRequest : LREAL;
    Blocking : BOOL;
    CmdAuto : BOOL;
    CmdManual : BOOL;
    CmdOutOfService : BOOL;
    CmdStart : BOOL;
    CmdStop : BOOL;
    ModeAccepted : BOOL;
    StartAccepted : BOOL;
    StopAccepted : BOOL;
    P101 : Plant.FB_Pump := (HasFeedback := FALSE);
    M201 : Plant.FB_VariableSpeedMotor := (HasFeedback := FALSE);
    FV301 : Plant.FB_ControlValve;
END_VAR
    IF CmdAuto THEN
        ModeAccepted := P101.SetMode(Value := E_Mode#Auto);
        M201.SetMode(Value := E_Mode#Auto);
        FV301.SetMode(Value := E_Mode#Auto);
        CmdAuto := FALSE;
    END_IF;
    IF CmdManual THEN
        ModeAccepted := P101.SetMode(Value := E_Mode#Manual);
        M201.SetMode(Value := E_Mode#Manual);
        FV301.SetMode(Value := E_Mode#Manual);
        CmdManual := FALSE;
    END_IF;
    IF CmdOutOfService THEN
        ModeAccepted := P101.SetMode(Value := E_Mode#OutOfService);
        M201.SetMode(Value := E_Mode#OutOfService);
        FV301.SetMode(Value := E_Mode#OutOfService);
        CmdOutOfService := FALSE;
    END_IF;
    IF CmdStart THEN
        StartAccepted := P101.Start();
        M201.Start();
        CmdStart := FALSE;
    END_IF;
    IF CmdStop THEN
        StopAccepted := P101.Stop();
        M201.Stop();
        CmdStop := FALSE;
    END_IF;
    P101(AutoRun := RunRequest, Interlock := Blocking, InterlockText := 'Tank low');
    M201(AutoRun := RunRequest, AutoSpeed := SpeedRequest);
    FV301(AutoPosition := PositionRequest);
END_PROGRAM
"#;

fn rig() -> Rig {
    let mut rig = Rig::new(PROGRAM);
    rig.scan();
    rig
}

#[test]
fn in_auto_a_motor_follows_the_program_run_demand() {
    let mut rig = rig();
    assert_eq!(rig.state("P101.Mode"), "Auto", "Auto is the default mode");
    assert!(!rig.flag("P101.RunCommand"), "nothing demands a run yet");
    rig.set_bool("RunRequest", true);
    rig.scan();
    assert!(rig.flag("P101.RunCommand"), "AutoRun starts the motor");
    assert!(
        rig.flag("P101.Running"),
        "without feedback Running follows the command"
    );
    rig.set_bool("RunRequest", false);
    rig.scan();
    assert!(!rig.flag("P101.RunCommand"), "dropping AutoRun stops it");
}

#[test]
fn in_manual_a_motor_follows_start_and_stop_and_ignores_the_program() {
    let mut rig = rig();
    rig.press("CmdManual");
    assert!(
        rig.flag("ModeAccepted"),
        "SetMode returns TRUE when the mode is in force"
    );
    assert_eq!(rig.state("P101.Mode"), "Manual");
    assert!(rig.flag("P101.Manual") && rig.flag("P101.InService"));
    rig.set_bool("RunRequest", true);
    rig.scans(3);
    assert!(
        !rig.flag("P101.RunCommand"),
        "the program's AutoRun is ignored in Manual"
    );
    rig.press("CmdStart");
    assert!(rig.flag("StartAccepted"), "Start is accepted in Manual");
    assert!(rig.flag("P101.RunCommand"), "the operator's Start runs it");
    rig.set_bool("RunRequest", false);
    rig.scans(3);
    assert!(
        rig.flag("P101.RunCommand"),
        "dropping AutoRun does not stop a motor in Manual"
    );
    rig.press("CmdStop");
    assert!(rig.flag("StopAccepted"), "Stop is accepted in Manual");
    assert!(!rig.flag("P101.RunCommand"), "the operator's Stop stops it");
}

#[test]
fn a_start_in_auto_is_refused_with_its_reason_and_leaves_nothing_pending() {
    let mut rig = rig();
    rig.press("CmdStart");
    assert!(
        !rig.flag("StartAccepted"),
        "an operator command is refused in Auto"
    );
    assert!(
        !rig.flag("P101.RunCommand"),
        "the refused start has no effect"
    );
    assert_eq!(rig.text("P101.CommandText"), "Auto: switch to Manual");
    assert!(!rig.flag("P101.StartPermit"), "no start permit in Auto");
    rig.press("CmdManual");
    rig.scans(3);
    assert!(
        !rig.flag("P101.RunCommand"),
        "the refused start does not start the motor later, in Manual"
    );
    assert_eq!(
        rig.text("P101.CommandText"),
        "",
        "in Manual and unblocked a command is accepted"
    );
    assert!(rig.flag("P101.StartPermit"));
}

#[test]
fn changing_from_auto_to_manual_keeps_a_running_motor_running() {
    let mut rig = rig();
    rig.set_bool("RunRequest", true);
    rig.scan();
    assert!(rig.flag("P101.RunCommand"));
    rig.press("CmdManual");
    assert!(
        rig.flag("P101.RunCommand"),
        "the change to Manual does not stop the motor"
    );
    rig.set_bool("RunRequest", false);
    rig.scans(3);
    assert!(
        rig.flag("P101.RunCommand"),
        "in Manual it runs on until the operator stops it"
    );
    assert_eq!(
        rig.count("P101.Starts"),
        1,
        "the motor was never stopped and started again"
    );
}

#[test]
fn changing_from_auto_to_manual_keeps_a_drive_at_its_speed() {
    let mut rig = rig();
    rig.set_bool("RunRequest", true);
    rig.set_real("SpeedRequest", 65.0);
    rig.scan();
    assert_eq!(rig.real("M201.SpeedSetpoint"), 65.0);
    rig.press("CmdManual");
    assert!(rig.flag("M201.RunCommand"), "the drive keeps running");
    assert_eq!(
        rig.real("M201.SpeedSetpoint"),
        65.0,
        "Manual continues at the speed Auto had"
    );
    assert_eq!(rig.real("M201.ManualSpeed"), 65.0);
    rig.set_real("SpeedRequest", 20.0);
    rig.set_bool("RunRequest", false);
    rig.scans(3);
    assert!(rig.flag("M201.RunCommand"));
    assert_eq!(
        rig.real("M201.SpeedSetpoint"),
        65.0,
        "the program's speed is ignored in Manual"
    );
}

#[test]
fn changing_from_auto_to_manual_holds_a_control_valve_where_it_is() {
    let mut rig = rig();
    rig.set_real("PositionRequest", 40.0);
    rig.scan();
    assert_eq!(rig.real("FV301.PositionSetpoint"), 40.0);
    rig.press("CmdManual");
    assert_eq!(
        rig.real("FV301.PositionSetpoint"),
        40.0,
        "the change to Manual does not move the valve"
    );
    rig.set_real("PositionRequest", 80.0);
    rig.scans(3);
    assert_eq!(
        rig.real("FV301.PositionSetpoint"),
        40.0,
        "the program's position is ignored in Manual"
    );
}

#[test]
fn out_of_service_holds_the_safe_state_and_refuses_commands() {
    let mut rig = rig();
    rig.set_real("PositionRequest", 40.0);
    rig.scan();
    rig.press("CmdManual");
    rig.press("CmdStart");
    assert!(rig.flag("P101.RunCommand"));
    assert_eq!(rig.real("FV301.PositionSetpoint"), 40.0);
    rig.press("CmdOutOfService");
    assert!(rig.flag("ModeAccepted"));
    assert!(rig.flag("P101.OutOfService") && !rig.flag("P101.InService"));
    assert!(
        !rig.flag("P101.RunCommand"),
        "a motor out of service is stopped"
    );
    assert_eq!(
        rig.real("FV301.PositionSetpoint"),
        0.0,
        "a control valve out of service is at its fail position"
    );
    rig.press("CmdStart");
    assert!(
        !rig.flag("StartAccepted"),
        "commands are refused out of service"
    );
    assert!(!rig.flag("P101.RunCommand"));
    assert!(rig.flag("P101.Blocked"));
    assert_eq!(rig.text("P101.BlockReason"), "Out of service");
    assert_eq!(rig.text("P101.CommandText"), "Out of service");
    rig.set_bool("RunRequest", true);
    rig.scans(3);
    assert!(
        !rig.flag("P101.RunCommand"),
        "the program cannot run it out of service either"
    );
}

#[test]
fn leaving_out_of_service_starts_from_the_safe_state() {
    let mut rig = rig();
    rig.set_real("PositionRequest", 40.0);
    rig.scan();
    rig.press("CmdManual");
    rig.press("CmdStart");
    rig.press("CmdOutOfService");
    rig.press("CmdManual");
    rig.scans(3);
    assert!(
        !rig.flag("P101.RunCommand"),
        "back in Manual the motor stays stopped: the earlier manual run is gone"
    );
    assert_eq!(
        rig.real("FV301.PositionSetpoint"),
        0.0,
        "back in Manual the valve stays at its fail position"
    );
    rig.press("CmdStart");
    assert!(
        rig.flag("StartAccepted") && rig.flag("P101.RunCommand"),
        "the operator starts it again"
    );
}

/// Plant §4.2 item 4: the message line names why the device is blocked, also in Auto where the
/// command text only says to switch to Manual; else it is the command text.
#[test]
fn the_message_line_names_the_block_before_the_command_text() {
    let mut rig = rig();
    rig.scan();
    assert_eq!(
        rig.text("P101.ReasonText"),
        "Auto: switch to Manual",
        "in Auto and unblocked"
    );
    rig.set_bool("Blocking", true);
    rig.scan();
    assert_eq!(rig.text("P101.CommandText"), "Auto: switch to Manual");
    assert_eq!(
        rig.text("P101.ReasonText"),
        "Interlocked: Tank low",
        "the block is named in Auto too"
    );
    rig.press("CmdManual");
    rig.set_bool("Blocking", false);
    rig.scans(2);
    assert_eq!(
        rig.text("P101.ReasonText"),
        "",
        "in Manual and unblocked there is nothing to say"
    );
}

#[test]
fn a_start_refused_while_interlocked_does_not_start_when_the_interlock_clears() {
    let mut rig = rig();
    rig.press("CmdManual");
    rig.set_bool("Blocking", true);
    rig.scan();
    rig.press("CmdStart");
    assert!(
        !rig.flag("StartAccepted"),
        "a start is refused while the device is blocked"
    );
    assert_eq!(rig.text("P101.CommandText"), "Interlocked: Tank low");
    rig.set_bool("Blocking", false);
    rig.scans(5);
    assert!(
        !rig.flag("P101.RunCommand"),
        "a refused start leaves nothing pending"
    );
}
