//! Plant library specification §4.7, §4.8, §10, §11 and §14.7: settings, counters and the mode
//! survive a power loss through the runtime's retain store; manual run commands and drive speeds
//! chosen in Manual do not. (A PID loop is code Create PLC code writes, with its values in a
//! `VAR RETAIN` block: specification 39, §10.5, tested with the generator.)

mod plant_support;

use plant_support::Rig;

const PROGRAM: &str = r#"
USING Plant;

PROGRAM Main
VAR
    Arm : BOOL;
    CmdManual : BOOL;
    CmdStart : BOOL;
    CmdSetSpeed : BOOL;
    P101 : Plant.FB_Pump := (HasFeedback := FALSE);
    P102 : Plant.FB_Pump := (HasFeedback := FALSE, StartTimeout := T#10s);
    M103 : Plant.FB_VariableSpeedPump := (HasFeedback := FALSE);
END_VAR
    IF Arm THEN
        (* An engineer's settings, written once. *)
        P101.StartTimeout := T#12s;
        Arm := FALSE;
    END_IF;
    IF CmdManual THEN
        P101.SetMode(Value := E_Mode#Manual);
        M103.SetMode(Value := E_Mode#Manual);
        CmdManual := FALSE;
    END_IF;
    IF CmdStart THEN
        P101.Start();
        M103.Start();
        CmdStart := FALSE;
    END_IF;
    IF CmdSetSpeed THEN
        M103.SetSpeed(Value := 70.0);
        CmdSetSpeed := FALSE;
    END_IF;
    P101();
    P102();
    M103();
END_PROGRAM
"#;

fn rig() -> Rig {
    let mut rig = Rig::new(PROGRAM);
    rig.scan();
    rig
}

#[test]
fn settings_survive_a_power_loss() {
    let mut rig = rig();
    rig.press("Arm");
    assert_eq!(rig.millis("P101.StartTimeout"), 12_000);
    let mut after = rig.power_loss();
    assert_eq!(
        after.millis("P101.StartTimeout"),
        12_000,
        "a setting the program changed comes back from the retain store, not the 5 s default"
    );
    assert_eq!(
        after.millis("P102.StartTimeout"),
        10_000,
        "a setting from the instance initializer stays"
    );
    after.scan();
    assert_eq!(after.millis("P101.StartTimeout"), 12_000);
}

#[test]
fn counters_and_the_mode_survive_a_power_loss_but_the_manual_run_command_does_not() {
    let mut rig = rig();
    rig.press("CmdManual");
    rig.press("CmdStart");
    assert!(rig.flag("P101.Running"));
    for _ in 0..3 {
        rig.step_ms(360_000);
    }
    let starts = rig.count("P101.Starts");
    let hours = rig.real("P101.RunHours");
    let since_service = rig.real("P101.HoursSinceService");
    assert_eq!(starts, 1);
    assert!(hours > 0.29, "18 minutes of running: {hours} h");
    let mut after = rig.power_loss();
    assert_eq!(after.state("P101.Mode"), "Manual", "the mode is retained");
    assert_eq!(
        after.count("P101.Starts"),
        starts,
        "the start counter is retained"
    );
    assert_eq!(
        after.real("P101.RunHours"),
        hours,
        "the run hours are retained"
    );
    assert_eq!(after.real("P101.HoursSinceService"), since_service);
    after.scan();
    assert!(
        !after.flag("P101.RunCommand") && !after.flag("P101.Running"),
        "the manual run command is not retained: the pump is stopped after the power loss"
    );
    assert_eq!(after.state("P101.Mode"), "Manual");
    after.scans(5);
    assert_eq!(
        after.real("P101.RunHours"),
        hours,
        "a stopped pump adds no hours"
    );
}

#[test]
fn a_drive_speed_chosen_in_manual_is_not_retained() {
    let mut rig = rig();
    rig.press("CmdManual");
    rig.press("CmdSetSpeed");
    rig.press("CmdStart");
    assert_eq!(rig.real("M103.SpeedSetpoint"), 70.0);
    let mut after = rig.power_loss();
    after.scan();
    assert_eq!(after.state("M103.Mode"), "Manual");
    assert!(!after.flag("M103.RunCommand"), "the drive is stopped");
    assert_eq!(
        after.real("M103.ManualSpeed"),
        0.0,
        "the manual speed is 0 % after the power loss"
    );
    assert_eq!(after.real("M103.SpeedSetpoint"), 0.0);
}

/// A PID loop as Create PLC code writes it (specification 39, §10.5): ordinary code around the
/// real Control.PID, its gains, mode, setpoint, manual output and output in `VAR RETAIN`.
const LOOP_PROGRAM: &str = r#"
USING Plant;
USING Control;

PROGRAM Main
VAR
    Arm : BOOL;
    LevelSignal : LREAL := 40.0;
    LT104 : Plant.FB_LevelInput := (RawLow := 0.0, RawHigh := 100.0, BadBelow := -1.0, BadAbove := 101.0);
    LIC104 : Control.PID;
    LIC104_Input : Control.PIDInput;
    LIC104_Start : Control.PIDInitialState;
    LIC104_Initialized : BOOL;
END_VAR
VAR RETAIN
    LIC104_Parameters : Control.PIDParameters := (ProportionalGain := 1.0, IntegralGain := 0.1,
        DerivativeGain := 0.0, ProportionalSetpointWeight := 1.0, DerivativeSetpointWeight := 0.0,
        Bias := 0.0, OutputMinimum := 0.0, OutputMaximum := 100.0, RateMinimum := -1000000000.0,
        RateMaximum := 1000000000.0, TrackingTime := 1.0, AntiWindup := PIDAntiWindup#BackCalculation);
    LIC104_Manual : BOOL := TRUE;
    LIC104_Setpoint : LREAL := 42.0;
    LIC104_ManualOutput : LREAL;
    LIC104_Output : LREAL;
END_VAR
    IF Arm THEN
        (* An engineer's tuning and the operator's Auto, written once; the setpoint is 42. *)
        LIC104_Parameters.ProportionalGain := 3.5;
        LIC104_Parameters.IntegralGain := 0.25;
        LIC104_Manual := FALSE;
        Arm := FALSE;
    END_IF;
    LT104(Raw := LevelSignal);
    LIC104.Configure(Parameters := LIC104_Parameters);
    LIC104_Input.Setpoint := LIC104_Setpoint;
    LIC104_Input.ManualOutput := LIC104_ManualOutput;
    LIC104_Input.CycleTime := 0.1;
    IF LT104.GetQuality() = E_Quality#Good THEN
        LIC104_Input.ProcessValue := LT104.GetValue();
        IF NOT LIC104_Initialized THEN
            LIC104_Start.Setpoint := LIC104_Setpoint;
            LIC104_Start.ProcessValue := LIC104_Input.ProcessValue;
            LIC104_Start.NominalOutput := LIC104_Output;
            LIC104_Start.SaturatedOutput := LIC104_Output;
            LIC104.Initialize(State := LIC104_Start);
            LIC104_Initialized := TRUE;
        END_IF;
        IF LIC104_Manual THEN
            LIC104_Input.Mode := PIDMode#Manual;
        ELSE
            LIC104_Input.Mode := PIDMode#Auto;
        END_IF;
    ELSE
        LIC104_Input.Mode := PIDMode#Manual;
        LIC104_Initialized := FALSE;
    END_IF;
    LIC104_Output := LIC104.Update(Input := LIC104_Input);
    IF LIC104_Input.Mode = PIDMode#Auto THEN
        LIC104_ManualOutput := LIC104_Output;
    END_IF;
END_PROGRAM
"#;

#[test]
fn a_pid_loop_keeps_its_tuning_setpoint_mode_and_output_across_a_power_loss() {
    let mut rig = Rig::new(LOOP_PROGRAM);
    rig.scan();
    rig.press("Arm");
    rig.scans(10);
    let output = rig.real("LIC104_Output");
    assert!(
        output > 0.0 && output < 100.0,
        "the loop regulates inside its output range: {output}"
    );
    let mut after = rig.power_loss();
    assert_eq!(after.real("LIC104_Parameters.ProportionalGain"), 3.5);
    assert_eq!(after.real("LIC104_Parameters.IntegralGain"), 0.25);
    assert_eq!(after.real("LIC104_Setpoint"), 42.0);
    assert!(!after.flag("LIC104_Manual"), "the mode is retained: Auto");
    assert_eq!(
        after.real("LIC104_Output"),
        output,
        "the output is retained"
    );
    after.scan();
    let first = after.real("LIC104_Output");
    // Initialised from the retained output, setpoint and measurement: only Ki·dt·e acts.
    assert!(
        (first - (output + 0.25 * 0.1 * 2.0)).abs() < 1e-9,
        "the loop resumes without a bump: {output} -> {first}"
    );
}
