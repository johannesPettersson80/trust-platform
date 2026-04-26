TEST_PROGRAM PumpPressureClassicControlsAndRaisesAlarm
VAR
    PressureController : CTRL_PID;
    PressureAlarm : HYST;
END_VAR

PressureController(
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
PressureAlarm(In := REAL#12.0, HIGH_LIMIT := REAL#10.0, LOW_LIMIT := REAL#3.0);

ASSERT_NEAR(EXPECTED := REAL#60.0, ACTUAL := PressureController.Y, DELTA := REAL#0.001);
ASSERT_TRUE(IN := PressureAlarm.Q);
END_TEST_PROGRAM
