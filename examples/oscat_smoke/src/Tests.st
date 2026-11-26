(* The OSCAT helpers that Main uses, checked against values worked out by hand. trust-dev test runs
   each TEST_PROGRAM once, after a cold restart: `trust-dev test --project examples/oscat_smoke`. *)

TEST_PROGRAM OscatConvertsTemperaturesAndSpeeds
VAR
    Converter : TEMPERATURE;
END_VAR

(* The scalar constants, PHYS.T0 among them, have their values from the start, so these conversions
   need no OSCAT_BASIC_Constants(). 25 degrees Celsius are 25 + 273.15 = 298.15 kelvin and
   25 * 1.8 + 32 = 77 degrees Fahrenheit. *)
ASSERT_NEAR(EXPECTED := REAL#298.15, ACTUAL := C_TO_K(Celsius := REAL#25.0), DELTA := REAL#0.01);
ASSERT_NEAR(EXPECTED := REAL#77.0, ACTUAL := C_TO_F(celsius := REAL#25.0), DELTA := REAL#0.01);
Converter(C := REAL#25.0);
ASSERT_NEAR(EXPECTED := REAL#77.0, ACTUAL := Converter.YF, DELTA := REAL#0.01);
(* 36 km/h are 36,000 m in 3,600 s, 10 m/s; 10 m/s is Beaufort 5, fresh breeze, 8.0 to 10.7 m/s. *)
ASSERT_NEAR(EXPECTED := REAL#10.0, ACTUAL := KMH_TO_MS(kmh := REAL#36.0), DELTA := REAL#0.001);
ASSERT_NEAR(EXPECTED := REAL#36.0, ACTUAL := MS_TO_KMH(ms := REAL#10.0), DELTA := REAL#0.001);
ASSERT_EQUAL(EXPECTED := INT#5, ACTUAL := MS_TO_BFT(MS := REAL#10.0));
END_TEST_PROGRAM

TEST_PROGRAM OscatNamesMonthsAndDirectionsOnceItsConstantsAreLoaded
VAR
    Ready : BOOL;
END_VAR

(* The tables of names stay empty until OSCAT_BASIC_Constants() fills them. Language 1 is English;
   with four compass points (N := 1), 90 degrees are east, and east is 90 degrees. *)
Ready := OSCAT_BASIC_Constants();
ASSERT_TRUE(IN := Ready);
ASSERT_EQUAL(EXPECTED := 'April', ACTUAL := MONTH_TO_STRING(MTH := INT#4, LANG := INT#1, LX := INT#0));
ASSERT_EQUAL(EXPECTED := 'E', ACTUAL := DEG_TO_DIR(DEG := INT#90, N := INT#1, L := INT#1));
ASSERT_EQUAL(EXPECTED := INT#90, ACTUAL := DIR_TO_DEG(DIR := 'E', L := INT#1));
END_TEST_PROGRAM

TEST_PROGRAM OscatReadsCalendarDates
(* 29 February 2024 is day 31 + 29 = 60 of a leap year, and Easter Sunday 2026 is 5 April. *)
ASSERT_EQUAL(EXPECTED := INT#60, ACTUAL := DAY_OF_YEAR(IDATE := DATE#2024-02-29));
ASSERT_TRUE(IN := LEAP_DAY(IDATE := DATE#2024-02-29));
ASSERT_FALSE(IN := LEAP_DAY(IDATE := DATE#2024-02-28));
ASSERT_EQUAL(EXPECTED := DATE#2026-04-05, ACTUAL := EASTER(year := INT#2026));
END_TEST_PROGRAM

TEST_PROGRAM OscatFormatsBytesAndText
(* 165 is 16#A5, 2#10100101. *)
ASSERT_EQUAL(EXPECTED := 'A5', ACTUAL := BYTE_TO_STRH(IN := BYTE#165));
ASSERT_EQUAL(EXPECTED := '10100101', ACTUAL := BYTE_TO_STRB(IN := BYTE#165));
ASSERT_EQUAL(EXPECTED := 'OSCAT', ACTUAL := UPPERCASE(STR := 'oscat'));
END_TEST_PROGRAM
