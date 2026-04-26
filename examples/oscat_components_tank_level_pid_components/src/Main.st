PROGRAM Main
VAR
    TankController : PidController;
    ControllerObject : IPidController;
    Limits : RealRange;
    LevelPercent : REAL := REAL#42.0;
    TargetLevelPercent : REAL := REAL#75.0;
    ValveCommandPercent : REAL;
    CommandLimited : BOOL;
END_VAR

Limits.Low := REAL#0.0;
Limits.High := REAL#100.0;
ControllerObject := TankController;
ControllerObject.Initialize();
ControllerObject.SetKp(Kp := REAL#2.0);
ControllerObject.SetIntegralTime(Tn := REAL#1.0);
ControllerObject.SetDerivativeTime(Tv := REAL#0.0);
ControllerObject.SetNoiseBand(SupervisionBand := REAL#0.0);
ControllerObject.SetOffset(Offset := REAL#0.0);
ControllerObject.SetLimits(Limits := Limits);
ControllerObject.SetManual(Manual := TRUE, ManualInput := REAL#35.0);

ValveCommandPercent := ControllerObject.Update(
    Actual := LevelPercent,
    Target := TargetLevelPercent
);
CommandLimited := ControllerObject.Limited;
END_PROGRAM
