(* Compact showcase: Main keeps the component call sequence visible for the ST tests. *)
PROGRAM Main
VAR
    TankController : PidController;
    Limits : RealRange;
    Gains : PidGains;
    LevelPercent : REAL := REAL#42.0;
    TargetLevelPercent : REAL := REAL#75.0;
    ValveCommandPercent : REAL;
    CommandLimited : BOOL;
END_VAR

Limits.Low := REAL#0.0;
Limits.High := REAL#100.0;
Gains.Kp := REAL#2.0;
Gains.Tn := REAL#1.0;
Gains.Tv := REAL#0.0;
TankController.Initialize();
TankController.Configure(
    Gains := Gains,
    Limits := Limits,
    SupervisionBand := REAL#0.0,
    Offset := REAL#0.0
);
TankController.SetManual(Manual := TRUE, ManualInput := REAL#35.0);

ValveCommandPercent := TankController.Update(
    Actual := LevelPercent,
    Target := TargetLevelPercent
);
CommandLimited := TankController.Limited;
END_PROGRAM
