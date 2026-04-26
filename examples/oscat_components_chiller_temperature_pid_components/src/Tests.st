TEST_PROGRAM ChillerTemperaturePidComponentsConvertAndControl
VAR
    Converter : UnitConverter;
    ConverterObject : IUnitConverter;
    ChillerController : PidController;
    ControllerObject : IPidController;
    ClassicController : CTRL_PID;
    Limits : RealRange;
END_VAR

ConverterObject := Converter;
ConverterObject.Initialize();
Limits.Low := REAL#0.0;
Limits.High := REAL#100.0;
ControllerObject := ChillerController;
ControllerObject.Initialize();
ControllerObject.SetKp(Kp := REAL#2.0);
ControllerObject.SetIntegralTime(Tn := REAL#1.0);
ControllerObject.SetDerivativeTime(Tv := REAL#0.0);
ControllerObject.SetLimits(Limits := Limits);
ControllerObject.SetManual(Manual := TRUE, ManualInput := REAL#45.0);
ClassicController(
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

ASSERT_NEAR(EXPECTED := REAL#285.15, ACTUAL := ConverterObject.KelvinFromCelsius(Celsius := REAL#12.0), DELTA := REAL#0.01);
ASSERT_NEAR(EXPECTED := ClassicController.Y, ACTUAL := ControllerObject.Update(Actual := REAL#12.0, Target := REAL#7.0), DELTA := REAL#0.001);
END_TEST_PROGRAM
