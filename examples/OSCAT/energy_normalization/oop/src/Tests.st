TEST_PROGRAM EnergyNormalizationOopUseConverterService
VAR
    Converter : UnitConverter;
    EnergyConversion : ENERGY;
    SpeedConversion : SPEED;
END_VAR

Converter.Initialize();
EnergyConversion(J := REAL#3600.0);
SpeedConversion(ms := REAL#10.0);

ASSERT_NEAR(EXPECTED := EnergyConversion.YWh, ACTUAL := Converter.WattHoursFromJoules(Joule := REAL#3600.0), DELTA := REAL#0.001);
ASSERT_NEAR(EXPECTED := SpeedConversion.Ykmh, ACTUAL := Converter.KilometersPerHourFromMetersPerSecond(Mps := REAL#10.0), DELTA := REAL#0.001);
END_TEST_PROGRAM

(* Multi-call converter test: drives the same Converter through
   several different inputs across both energy and speed methods.
   Verifies the converter never caches its previous result. *)
TEST_PROGRAM EnergyNormalizationConverterDoesNotCacheBetweenCalls
VAR
    Converter : UnitConverter;
END_VAR

Converter.Initialize();

(* Three different Joule inputs -> three different WattHour outputs *)
ASSERT_NEAR(EXPECTED := REAL#1.0, ACTUAL := Converter.WattHoursFromJoules(Joule := REAL#3600.0), DELTA := REAL#0.001);
ASSERT_NEAR(EXPECTED := REAL#2.0, ACTUAL := Converter.WattHoursFromJoules(Joule := REAL#7200.0), DELTA := REAL#0.001);
ASSERT_NEAR(EXPECTED := REAL#0.5, ACTUAL := Converter.WattHoursFromJoules(Joule := REAL#1800.0), DELTA := REAL#0.001);

(* Three different m/s inputs -> three different km/h outputs *)
ASSERT_NEAR(EXPECTED := REAL#36.0, ACTUAL := Converter.KilometersPerHourFromMetersPerSecond(Mps := REAL#10.0), DELTA := REAL#0.001);
ASSERT_NEAR(EXPECTED := REAL#72.0, ACTUAL := Converter.KilometersPerHourFromMetersPerSecond(Mps := REAL#20.0), DELTA := REAL#0.001);
ASSERT_NEAR(EXPECTED := REAL#18.0, ACTUAL := Converter.KilometersPerHourFromMetersPerSecond(Mps := REAL#5.0), DELTA := REAL#0.001);
END_TEST_PROGRAM

(* Round-trip conversion: Joules -> WattHours -> Joules must restore
   the value. The two methods are inverses; any drift is a bug. *)
TEST_PROGRAM EnergyConverterJouleRoundTripIsLossless
VAR
    Converter : UnitConverter;
    Original : REAL;
    Wh : REAL;
    Restored : REAL;
END_VAR

Converter.Initialize();
Original := REAL#7200.0;
Wh := Converter.WattHoursFromJoules(Joule := Original);
Restored := Converter.JoulesFromWattHours(WattHour := Wh);
ASSERT_NEAR(EXPECTED := Original, ACTUAL := Restored, DELTA := REAL#0.001);
END_TEST_PROGRAM

(* Speed round-trip: km/h -> m/s -> km/h must restore. Verifies the
   3.6 factor is consistent across both directions. *)
TEST_PROGRAM EnergyConverterSpeedRoundTripIsLossless
VAR
    Converter : UnitConverter;
    Original : REAL;
    Mps : REAL;
    Restored : REAL;
END_VAR

Converter.Initialize();
Original := REAL#90.0;
Mps := Converter.MetersPerSecondFromKilometersPerHour(Kmh := Original);
Restored := Converter.KilometersPerHourFromMetersPerSecond(Mps := Mps);
ASSERT_NEAR(EXPECTED := Original, ACTUAL := Restored, DELTA := REAL#0.001);
ASSERT_NEAR(EXPECTED := REAL#25.0, ACTUAL := Mps, DELTA := REAL#0.001);
END_TEST_PROGRAM

(* Multi-family on one instance: the same Converter handles energy AND
   speed AND a third family in one scan, with no cross-talk between
   the methods. *)
TEST_PROGRAM EnergyConverterHandlesMultipleFamiliesOnOneInstance
VAR
    Converter : UnitConverter;
    EnergyResult : REAL;
    SpeedResult : REAL;
    CalorieResult : REAL;
END_VAR

Converter.Initialize();
EnergyResult := Converter.WattHoursFromJoules(Joule := REAL#3600.0);
SpeedResult := Converter.KilometersPerHourFromMetersPerSecond(Mps := REAL#10.0);
CalorieResult := Converter.CaloriesFromJoules(Joule := REAL#4184.0);

ASSERT_NEAR(EXPECTED := REAL#1.0, ACTUAL := EnergyResult, DELTA := REAL#0.001);
ASSERT_NEAR(EXPECTED := REAL#36.0, ACTUAL := SpeedResult, DELTA := REAL#0.001);
(* 4184 J = 1000 calories (4.184 J per cal) *)
ASSERT_NEAR(EXPECTED := REAL#1000.0, ACTUAL := CalorieResult, DELTA := REAL#1.0);
END_TEST_PROGRAM

(* Edge: zero input must yield zero output (no offset bias). *)
TEST_PROGRAM EnergyConverterZeroInputProducesZeroOutput
VAR
    Converter : UnitConverter;
END_VAR

Converter.Initialize();
ASSERT_NEAR(EXPECTED := REAL#0.0, ACTUAL := Converter.WattHoursFromJoules(Joule := REAL#0.0), DELTA := REAL#0.0001);
ASSERT_NEAR(EXPECTED := REAL#0.0, ACTUAL := Converter.KilometersPerHourFromMetersPerSecond(Mps := REAL#0.0), DELTA := REAL#0.0001);
ASSERT_NEAR(EXPECTED := REAL#0.0, ACTUAL := Converter.CaloriesFromJoules(Joule := REAL#0.0), DELTA := REAL#0.0001);
END_TEST_PROGRAM

(* Two converter instances must be functionally identical: stateless
   service, so isolation is trivially true but still worth pinning. *)
TEST_PROGRAM EnergyConverterTwoInstancesAgree
VAR
    ConvA : UnitConverter;
    ConvB : UnitConverter;
END_VAR

ConvA.Initialize();
ConvB.Initialize();
ASSERT_NEAR(EXPECTED := ConvA.WattHoursFromJoules(Joule := REAL#3600.0),
    ACTUAL := ConvB.WattHoursFromJoules(Joule := REAL#3600.0), DELTA := REAL#0.0001);
ASSERT_NEAR(EXPECTED := ConvA.KilometersPerHourFromMetersPerSecond(Mps := REAL#42.0),
    ACTUAL := ConvB.KilometersPerHourFromMetersPerSecond(Mps := REAL#42.0), DELTA := REAL#0.0001);
END_TEST_PROGRAM
