TEST_PROGRAM ClosedLoopClassicBranchesBetweenPiAndPid
VAR
    PiLoop : CTRL_PI;
    PidLoop : CTRL_PID;
END_VAR

PiLoop(
    ACT := REAL#4.0,
    SET_POINT := REAL#7.0,
    SUP := REAL#0.0,
    OFS := REAL#0.0,
    M_I := REAL#42.0,
    MAN := TRUE,
    RST := FALSE,
    KP := REAL#1.5,
    KI := REAL#1.0,
    LL := REAL#0.0,
    LH := REAL#100.0
);
PidLoop(
    ACT := REAL#4.0,
    SET_POINT := REAL#7.0,
    SUP := REAL#0.0,
    OFS := REAL#0.0,
    M_I := REAL#42.0,
    MAN := TRUE,
    RST := FALSE,
    KP := REAL#1.5,
    TN := REAL#1.0,
    TV := REAL#0.2,
    LL := REAL#0.0,
    LH := REAL#100.0
);

ASSERT_NEAR(EXPECTED := REAL#42.0, ACTUAL := PiLoop.Y, DELTA := REAL#0.001);
ASSERT_NEAR(EXPECTED := REAL#42.0, ACTUAL := PidLoop.Y, DELTA := REAL#0.001);
END_TEST_PROGRAM
