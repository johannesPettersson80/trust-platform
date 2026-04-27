TEST_PROGRAM WeatherStationConversionOopConvertWindData
VAR
    Context : AutomationContext;
    Converter : UnitConverter;
END_VAR

Context.Initialize();
Converter.Initialize();

ASSERT_TRUE(IN := Context.LoadConstants());
ASSERT_EQUAL(EXPECTED := MS_TO_BFT(MS := REAL#10.0), ACTUAL := Converter.BeaufortFromMetersPerSecond(Mps := REAL#10.0));
ASSERT_EQUAL(EXPECTED := DEG_TO_DIR(DEG := INT#90, N := INT#4, L := INT#1), ACTUAL := Context.DirectionName(Degrees := REAL#90.0));
END_TEST_PROGRAM

(* Multi-call converter+context test: drives the same Context and
   Converter through several wind speeds and compass headings.
   Verifies neither service caches its last input. *)
TEST_PROGRAM WeatherStationConvertsMultipleWindReadingsOnSameInstance
VAR
    Context : AutomationContext;
    Converter : UnitConverter;
END_VAR

Context.Initialize();
Converter.Initialize();
Context.LoadConstants();

(* Three wind speeds -> three Beaufort values *)
ASSERT_EQUAL(EXPECTED := MS_TO_BFT(MS := REAL#3.0), ACTUAL := Converter.BeaufortFromMetersPerSecond(Mps := REAL#3.0));
ASSERT_EQUAL(EXPECTED := MS_TO_BFT(MS := REAL#10.0), ACTUAL := Converter.BeaufortFromMetersPerSecond(Mps := REAL#10.0));
ASSERT_EQUAL(EXPECTED := MS_TO_BFT(MS := REAL#20.0), ACTUAL := Converter.BeaufortFromMetersPerSecond(Mps := REAL#20.0));

(* Three compass headings -> three direction strings *)
ASSERT_EQUAL(EXPECTED := DEG_TO_DIR(DEG := INT#0, N := INT#4, L := INT#1), ACTUAL := Context.DirectionName(Degrees := REAL#0.0));
ASSERT_EQUAL(EXPECTED := DEG_TO_DIR(DEG := INT#90, N := INT#4, L := INT#1), ACTUAL := Context.DirectionName(Degrees := REAL#90.0));
ASSERT_EQUAL(EXPECTED := DEG_TO_DIR(DEG := INT#180, N := INT#4, L := INT#1), ACTUAL := Context.DirectionName(Degrees := REAL#180.0));
END_TEST_PROGRAM

(* Multi-call regression: a long burst of converter calls returns
   stable Beaufort values for the same input across many invocations.
   Catches caching bugs where a converter would memoize the last
   Mps and return it on repeated calls. *)
TEST_PROGRAM WeatherStationConverterReturnsStableValuesOnRepeatedInput
VAR
    Converter : UnitConverter;
    I : INT;
    First : INT;
    Reading : INT;
END_VAR

Converter.Initialize();
First := Converter.BeaufortFromMetersPerSecond(Mps := REAL#10.0);
FOR I := 1 TO 20 DO
    Reading := Converter.BeaufortFromMetersPerSecond(Mps := REAL#10.0);
    ASSERT_EQUAL(EXPECTED := First, ACTUAL := Reading);
END_FOR;
END_TEST_PROGRAM

(* Boundary test: temperature conversion round-trips. C -> K -> C and
   C -> F -> C must return the original temperature within float
   tolerance. *)
TEST_PROGRAM WeatherStationConverterTemperatureRoundTrips
VAR
    Converter : UnitConverter;
    Kelvin : REAL;
    Fahrenheit : REAL;
    Celsius : REAL;
END_VAR

Converter.Initialize();

(* C -> K -> C *)
Kelvin := Converter.KelvinFromCelsius(Celsius := REAL#25.0);
Celsius := Converter.CelsiusFromKelvin(Kelvin := Kelvin);
ASSERT_NEAR(EXPECTED := REAL#25.0, ACTUAL := Celsius, DELTA := REAL#0.01);

(* C -> F -> C *)
Fahrenheit := Converter.FahrenheitFromCelsius(Celsius := REAL#100.0);
Celsius := Converter.CelsiusFromFahrenheit(Fahrenheit := Fahrenheit);
ASSERT_NEAR(EXPECTED := REAL#100.0, ACTUAL := Celsius, DELTA := REAL#0.01);
END_TEST_PROGRAM

(* Per-FB isolation: drive a bare AutomationContext and verify
   Initialize loads constants and exposes them as Ready + Loaded.
   The global constants cache persists across resets, so this asserts
   the positive contract only. *)
TEST_PROGRAM WeatherStationContextInitializeLoadsConstants
VAR
    Context : AutomationContext;
END_VAR

Context.Initialize();
ASSERT_TRUE(IN := Context.ConstantsLoaded);
ASSERT_TRUE(IN := Context.LoadConstants());
ASSERT_TRUE(IN := Context.ConstantsLoaded);
END_TEST_PROGRAM

(* Edge input: zero and very high m/s. Beaufort 0 is calm; a wind
   above 32.6 m/s is hurricane (Beaufort 12). *)
TEST_PROGRAM WeatherStationConverterHandlesEdgeWindSpeeds
VAR
    Converter : UnitConverter;
END_VAR

Converter.Initialize();

(* Calm: 0 m/s -> Beaufort 0 *)
ASSERT_EQUAL(EXPECTED := MS_TO_BFT(MS := REAL#0.0), ACTUAL := Converter.BeaufortFromMetersPerSecond(Mps := REAL#0.0));

(* Hurricane: 50 m/s -> Beaufort 12 (saturated) *)
ASSERT_EQUAL(EXPECTED := MS_TO_BFT(MS := REAL#50.0), ACTUAL := Converter.BeaufortFromMetersPerSecond(Mps := REAL#50.0));
END_TEST_PROGRAM

(* Edge input: a km/h to m/s round-trip via the converter must match
   within float tolerance. *)
TEST_PROGRAM WeatherStationConverterKmhRoundTrips
VAR
    Converter : UnitConverter;
    Mps : REAL;
    KmhRoundTrip : REAL;
END_VAR

Converter.Initialize();
Mps := Converter.MetersPerSecondFromKilometersPerHour(Kmh := REAL#36.0);
ASSERT_NEAR(EXPECTED := REAL#10.0, ACTUAL := Mps, DELTA := REAL#0.01);

KmhRoundTrip := Converter.KilometersPerHourFromMetersPerSecond(Mps := Mps);
ASSERT_NEAR(EXPECTED := REAL#36.0, ACTUAL := KmhRoundTrip, DELTA := REAL#0.01);
END_TEST_PROGRAM
