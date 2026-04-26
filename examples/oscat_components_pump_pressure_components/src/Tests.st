TEST_PROGRAM PumpPressureComponentsComposeControllerAndAlarm
VAR
    PressureController : PidController;
    ControllerObject : IPidController;
    PressureAlarm : HysteresisSwitch;
    AlarmObject : IHysteresisSwitch;
    ClassicController : CTRL_PID;
    ClassicAlarm : HYST;
    ControllerLimits : RealRange;
    AlarmLimits : RealRange;
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

ClassicController(
    ACT := REAL#12.0,
    SET_POINT := REAL#8.0,
    SUP := REAL#0.0,
    OFS := REAL#0.0,
    M_I := REAL#60.0,
    MAN := TRUE,
    RST := FALSE,
    KP := REAL#1.5,
    TN := REAL#1.0,
    TV := REAL#0.0,
    LL := REAL#0.0,
    LH := REAL#100.0
);
ClassicAlarm(In := REAL#12.0, HIGH_LIMIT := REAL#10.0, LOW_LIMIT := REAL#3.0);

ASSERT_NEAR(EXPECTED := ClassicController.Y, ACTUAL := ControllerObject.Update(Actual := REAL#12.0, Target := REAL#8.0), DELTA := REAL#0.001);
ASSERT_TRUE(IN := AlarmObject.Update(MeasuredValue := REAL#12.0) = ClassicAlarm.Q);
END_TEST_PROGRAM
