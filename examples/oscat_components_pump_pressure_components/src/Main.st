PROGRAM Main
VAR
    PressureController : PidController;
    ControllerObject : IPidController;
    PressureAlarm : HysteresisSwitch;
    AlarmObject : IHysteresisSwitch;
    ControllerLimits : RealRange;
    AlarmLimits : RealRange;
    PressureBar : REAL := REAL#12.0;
    TargetBar : REAL := REAL#8.0;
    PumpCommandPercent : REAL;
    AlarmActive : BOOL;
END_VAR

ControllerLimits.Low := REAL#0.0;
ControllerLimits.High := REAL#100.0;
ControllerObject := PressureController;
ControllerObject.Initialize();
ControllerObject.SetKp(Kp := REAL#1.5);
ControllerObject.SetIntegralTime(Tn := REAL#1.0);
ControllerObject.SetDerivativeTime(Tv := REAL#0.0);
ControllerObject.SetNoiseBand(SupervisionBand := REAL#0.0);
ControllerObject.SetLimits(Limits := ControllerLimits);
ControllerObject.SetManual(Manual := TRUE, ManualInput := REAL#60.0);

AlarmLimits.Low := REAL#3.0;
AlarmLimits.High := REAL#10.0;
AlarmObject := PressureAlarm;
AlarmObject.Initialize();
AlarmObject.SetLimits(Limits := AlarmLimits);

PumpCommandPercent := ControllerObject.Update(Actual := PressureBar, Target := TargetBar);
AlarmActive := AlarmObject.Update(MeasuredValue := PressureBar);
END_PROGRAM
