TEST_PROGRAM ClosedLoopOopSubstitutePiAndPidBehindOneInterface
VAR
    Loop : IClosedLoopController;
    PiLoop : PiController;
    PidLoop : PidController;
    PiGainsValue : PiGains;
    PidGainsValue : PidGains;
    Limits : RealRange;
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

Loop := PiLoop;
ASSERT_NEAR(EXPECTED := REAL#42.0, ACTUAL := Loop.Update(Actual := REAL#4.0, Target := REAL#7.0), DELTA := REAL#0.001);
Loop := PidLoop;
ASSERT_NEAR(EXPECTED := REAL#42.0, ACTUAL := Loop.Update(Actual := REAL#4.0, Target := REAL#7.0), DELTA := REAL#0.001);
END_TEST_PROGRAM

(* Polymorphic dispatch sequence test: swap the slot back and forth
   across multiple Update calls. Verifies the interface call dispatches
   to whichever concrete controller is currently assigned, and that
   each controller maintains its own retained state independently. *)
TEST_PROGRAM ClosedLoopOopSwappingControllersDoesNotMixState
VAR
    Loop : IClosedLoopController;
    PiLoop : PiController;
    PidLoop : PidController;
    PiGainsValue : PiGains;
    PidGainsValue : PidGains;
    Limits : RealRange;
    PiOut1 : REAL;
    PidOut1 : REAL;
    PiOut2 : REAL;
    PidOut2 : REAL;
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
PidLoop.Initialize();
PidLoop.Configure(Gains := PidGainsValue, Limits := Limits, SupervisionBand := REAL#0.0, Offset := REAL#0.0);

(* Drive the PI loop, capture output *)
Loop := PiLoop;
PiOut1 := Loop.Update(Actual := REAL#4.0, Target := REAL#7.0);

(* Switch to PID, drive, capture output *)
Loop := PidLoop;
PidOut1 := Loop.Update(Actual := REAL#4.0, Target := REAL#7.0);

(* Switch back to PI - it must continue from its own retained state,
   not from PidLoop's state *)
Loop := PiLoop;
PiOut2 := Loop.Update(Actual := REAL#4.0, Target := REAL#7.0);

(* And PID retains its own state independently *)
Loop := PidLoop;
PidOut2 := Loop.Update(Actual := REAL#4.0, Target := REAL#7.0);

(* Each controller advanced its integral on its own calls only *)
ASSERT_TRUE(IN := PiOut2 >= PiOut1);
ASSERT_TRUE(IN := PidOut2 >= PidOut1);
END_TEST_PROGRAM
