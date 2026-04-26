TEST_PROGRAM TankLevelPidClassicUsesOscatPidDirectly
VAR
    TankController : CTRL_PID;
END_VAR

TankController(
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

ASSERT_NEAR(EXPECTED := REAL#35.0, ACTUAL := TankController.Y, DELTA := REAL#0.001);
ASSERT_FALSE(IN := TankController.LIM);
END_TEST_PROGRAM
