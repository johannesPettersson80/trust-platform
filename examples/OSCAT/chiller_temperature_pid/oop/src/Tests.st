TEST_PROGRAM ChillerTemperaturePidOopConvertAndControl
VAR
    Converter : UnitConverter;
    ChillerController : PidController;
    ClassicController : CTRL_PID;
    Limits : RealRange;
    Gains : PidGains;
END_VAR

Converter.Initialize();
Limits.Low := REAL#0.0;
Limits.High := REAL#100.0;
Gains.Kp := REAL#2.0;
Gains.Tn := REAL#1.0;
Gains.Tv := REAL#0.0;
ChillerController.Initialize();
ChillerController.Configure(
    Gains := Gains,
    Limits := Limits,
    SupervisionBand := REAL#0.0,
    Offset := REAL#0.0
);
ChillerController.SetManual(Manual := TRUE, ManualInput := REAL#45.0);
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

ASSERT_NEAR(EXPECTED := REAL#285.15, ACTUAL := Converter.KelvinFromCelsius(Celsius := REAL#12.0), DELTA := REAL#0.01);
ASSERT_NEAR(EXPECTED := ClassicController.Y, ACTUAL := ChillerController.Update(Actual := REAL#12.0, Target := REAL#7.0), DELTA := REAL#0.001);
END_TEST_PROGRAM

(* Multi-call sequence: drives Converter and PID through three
   different return temperatures on the same instances. Verifies the
   converter never caches its previous result and the PID maintains
   manual-mode output across consecutive Update calls. *)
TEST_PROGRAM ChillerComponentsHandleSequentialReturnTemperatures
VAR
    Converter : UnitConverter;
    ChillerController : PidController;
    Limits : RealRange;
    Gains : PidGains;
    Cmd1 : REAL;
    Cmd2 : REAL;
END_VAR

Converter.Initialize();
Limits.Low := REAL#0.0;
Limits.High := REAL#100.0;
Gains.Kp := REAL#2.0;
Gains.Tn := REAL#1.0;
Gains.Tv := REAL#0.0;
ChillerController.Initialize();
ChillerController.Configure(Gains := Gains, Limits := Limits, SupervisionBand := REAL#0.0, Offset := REAL#0.0);
ChillerController.SetManual(Manual := TRUE, ManualInput := REAL#45.0);

(* Three sequential conversions return distinct results *)
ASSERT_NEAR(EXPECTED := REAL#285.15, ACTUAL := Converter.KelvinFromCelsius(Celsius := REAL#12.0), DELTA := REAL#0.01);
ASSERT_NEAR(EXPECTED := REAL#290.15, ACTUAL := Converter.KelvinFromCelsius(Celsius := REAL#17.0), DELTA := REAL#0.01);
ASSERT_NEAR(EXPECTED := REAL#293.15, ACTUAL := Converter.KelvinFromCelsius(Celsius := REAL#20.0), DELTA := REAL#0.01);

(* PID in manual mode returns manual input on every call *)
Cmd1 := ChillerController.Update(Actual := REAL#12.0, Target := REAL#7.0);
Cmd2 := ChillerController.Update(Actual := REAL#15.0, Target := REAL#7.0);
ASSERT_NEAR(EXPECTED := REAL#45.0, ACTUAL := Cmd1, DELTA := REAL#0.001);
ASSERT_NEAR(EXPECTED := REAL#45.0, ACTUAL := Cmd2, DELTA := REAL#0.001);
END_TEST_PROGRAM

(* Round-trip conversion: Celsius -> Kelvin -> Celsius keeps the value.
   Verifies no accumulated drift across consecutive converter calls. *)
TEST_PROGRAM ChillerConverterRoundTripsAcrossUnits
VAR
    Converter : UnitConverter;
    Original : REAL;
    Kelvin : REAL;
    Restored : REAL;
END_VAR

Converter.Initialize();
Original := REAL#23.5;
Kelvin := Converter.KelvinFromCelsius(Celsius := Original);
Restored := Converter.CelsiusFromKelvin(Kelvin := Kelvin);
ASSERT_NEAR(EXPECTED := Original, ACTUAL := Restored, DELTA := REAL#0.001);
END_TEST_PROGRAM

(* Manual-mode clamping at output limits: ManualInput beyond HighLimit
   must clamp to HighLimit (100). The classic CTRL_PID enforces the
   same bound and the OOP wrapper must not weaken it. *)
TEST_PROGRAM ChillerManualInputClampsToHighLimit
VAR
    ChillerController : PidController;
    Limits : RealRange;
    Gains : PidGains;
    Cmd : REAL;
END_VAR

Limits.Low := REAL#0.0;
Limits.High := REAL#100.0;
Gains.Kp := REAL#2.0;
Gains.Tn := REAL#1.0;
Gains.Tv := REAL#0.0;
ChillerController.Initialize();
ChillerController.Configure(Gains := Gains, Limits := Limits, SupervisionBand := REAL#0.0, Offset := REAL#0.0);
ChillerController.SetManual(Manual := TRUE, ManualInput := REAL#150.0);
Cmd := ChillerController.Update(Actual := REAL#12.0, Target := REAL#7.0);
ASSERT_NEAR(EXPECTED := REAL#100.0, ACTUAL := Cmd, DELTA := REAL#0.001);
ASSERT_TRUE(IN := ChillerController.Limited);
END_TEST_PROGRAM

(* Configure rejects inverted limits. A real commissioning slip pasted
   Low > High would otherwise silently drive the loop with bogus bounds. *)
TEST_PROGRAM ChillerConfigureRejectsInvertedLimits
VAR
    ChillerController : PidController;
    Limits : RealRange;
    Gains : PidGains;
END_VAR

Limits.Low := REAL#100.0;
Limits.High := REAL#0.0;
Gains.Kp := REAL#2.0;
Gains.Tn := REAL#1.0;
Gains.Tv := REAL#0.0;
ChillerController.Initialize();
ASSERT_FALSE(IN := ChillerController.Error);
ChillerController.Configure(Gains := Gains, Limits := Limits, SupervisionBand := REAL#0.0, Offset := REAL#0.0);
ASSERT_TRUE(IN := ChillerController.Error);
END_TEST_PROGRAM

(* Per-FB isolation: two PidController instances must not share state.
   Manual flag on one must not leak into the other's Update call. *)
TEST_PROGRAM ChillerPidInstancesAreIsolated
VAR
    LoopA : PidController;
    LoopB : PidController;
    Limits : RealRange;
    Gains : PidGains;
    OutA : REAL;
    OutB : REAL;
END_VAR

Limits.Low := REAL#0.0;
Limits.High := REAL#100.0;
Gains.Kp := REAL#2.0;
Gains.Tn := REAL#1.0;
Gains.Tv := REAL#0.0;
LoopA.Initialize();
LoopB.Initialize();
LoopA.Configure(Gains := Gains, Limits := Limits, SupervisionBand := REAL#0.0, Offset := REAL#0.0);
LoopB.Configure(Gains := Gains, Limits := Limits, SupervisionBand := REAL#0.0, Offset := REAL#0.0);
LoopA.SetManual(Manual := TRUE, ManualInput := REAL#80.0);
LoopB.SetManual(Manual := TRUE, ManualInput := REAL#20.0);
OutA := LoopA.Update(Actual := REAL#12.0, Target := REAL#7.0);
OutB := LoopB.Update(Actual := REAL#12.0, Target := REAL#7.0);
ASSERT_NEAR(EXPECTED := REAL#80.0, ACTUAL := OutA, DELTA := REAL#0.001);
ASSERT_NEAR(EXPECTED := REAL#20.0, ACTUAL := OutB, DELTA := REAL#0.001);
END_TEST_PROGRAM
