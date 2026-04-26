TEST_PROGRAM TankLevelPidComponentMatchesClassicPid
VAR
    TankController : PidController;
    ControllerObject : IPidController;
    ClassicController : CTRL_PID;
    Limits : RealRange;
    CommandPercent : REAL;
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

ClassicController(
    ACT := REAL#42.0,
    SET_POINT := REAL#75.0,
    SUP := REAL#0.0,
    OFS := REAL#0.0,
    M_I := REAL#35.0,
    MAN := TRUE,
    RST := FALSE,
    KP := REAL#2.0,
    TN := REAL#1.0,
    TV := REAL#0.0,
    LL := REAL#0.0,
    LH := REAL#100.0
);
CommandPercent := ControllerObject.Update(Actual := REAL#42.0, Target := REAL#75.0);

ASSERT_NEAR(EXPECTED := ClassicController.Y, ACTUAL := CommandPercent, DELTA := REAL#0.001);
ASSERT_TRUE(IN := ControllerObject.Limited = ClassicController.LIM);
END_TEST_PROGRAM
