(* Scan role: Main wires the example objects and exposes the PLC call path under test. *)
PROGRAM Main
VAR
    UsePid : BOOL := FALSE;
    Loop : IClosedLoopController;
    PiLoop : PiController;
    PidLoop : PidController;
    PiGainsValue : PiGains;
    PidGainsValue : PidGains;
    Limits : RealRange;
    ActualPressure : REAL := REAL#4.0;
    TargetPressure : REAL := REAL#7.0;
    ValveCommand : REAL;
    CommandLimited : BOOL;
END_VAR

Limits.Low := REAL#0.0;
Limits.High := REAL#100.0;
PiGainsValue.Kp := REAL#1.5;
PiGainsValue.Ki := REAL#1.0;
PidGainsValue.Kp := REAL#1.5;
PidGainsValue.Tn := REAL#1.0;
PidGainsValue.Tv := REAL#0.2;

PiLoop.Initialize();
PiLoop.Configure(Gains := PiGainsValue, Limits := Limits, SupervisionBand := REAL#0.0, Offset := REAL#0.0);
PiLoop.SetManual(Manual := TRUE, ManualInput := REAL#42.0);
PidLoop.Initialize();
PidLoop.Configure(Gains := PidGainsValue, Limits := Limits, SupervisionBand := REAL#0.0, Offset := REAL#0.0);
PidLoop.SetManual(Manual := TRUE, ManualInput := REAL#42.0);

IF UsePid THEN
    Loop := PidLoop;
ELSE
    Loop := PiLoop;
END_IF;

ValveCommand := Loop.Update(Actual := ActualPressure, Target := TargetPressure);
CommandLimited := Loop.Limited;
END_PROGRAM
