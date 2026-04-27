(* Compact showcase: Main keeps the component call sequence visible for the ST tests. *)
PROGRAM Main
VAR
    PressureController : PidController;
    PressureAlarm : HysteresisSwitch;
    ControllerLimits : RealRange;
    ControllerGains : PidGains;
    AlarmLimits : RealRange;
    PressureBar : REAL := REAL#12.0;
    TargetBar : REAL := REAL#8.0;
    PumpCommandPercent : REAL;
    AlarmActive : BOOL;
END_VAR

ControllerLimits.Low := REAL#0.0;
ControllerLimits.High := REAL#100.0;
ControllerGains.Kp := REAL#1.5;
ControllerGains.Tn := REAL#1.0;
ControllerGains.Tv := REAL#0.0;
PressureController.Initialize();
PressureController.Configure(
    Gains := ControllerGains,
    Limits := ControllerLimits,
    SupervisionBand := REAL#0.0,
    Offset := REAL#0.0
);
PressureController.SetManual(Manual := TRUE, ManualInput := REAL#60.0);

AlarmLimits.Low := REAL#3.0;
AlarmLimits.High := REAL#10.0;
PressureAlarm.Initialize();
PressureAlarm.SetLimits(Limits := AlarmLimits);

PumpCommandPercent := PressureController.Update(Actual := PressureBar, Target := TargetBar);
AlarmActive := PressureAlarm.Update(MeasuredValue := PressureBar);
END_PROGRAM
