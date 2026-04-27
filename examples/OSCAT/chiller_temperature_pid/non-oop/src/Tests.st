TEST_PROGRAM ChillerTemperaturePidClassicConvertsAndControls
VAR
    TemperatureConversion : TEMPERATURE;
    ChillerController : CTRL_PID;
END_VAR

TemperatureConversion(C := REAL#12.0);
ChillerController(
    ACT := REAL#12.0,
    SET_POINT := REAL#7.0,
    SUP := REAL#0.0,
    OFS := REAL#0.0,
    M_I := REAL#45.0,
    MAN := TRUE,
    RST := FALSE,
    KP := REAL#2.0,
    TN := REAL#1.0,
    TV := REAL#0.0,
    LL := REAL#0.0,
    LH := REAL#100.0
);

ASSERT_NEAR(EXPECTED := REAL#285.15, ACTUAL := TemperatureConversion.YK, DELTA := REAL#0.01);
ASSERT_NEAR(EXPECTED := REAL#45.0, ACTUAL := ChillerController.Y, DELTA := REAL#0.001);
END_TEST_PROGRAM
