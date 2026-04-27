TEST_PROGRAM WindSpeedAlarmOopAdaptConversionIntoAlarm
VAR
    Converter : UnitConverter;
    WindAlarm : HysteresisSwitch;
    ClassicAlarm : HYST;
    AlarmLimits : RealRange;
END_VAR

Converter.Initialize();
AlarmLimits.Low := REAL#3.0;
AlarmLimits.High := REAL#8.0;
WindAlarm.Initialize();
WindAlarm.SetLimits(Limits := AlarmLimits);
ClassicAlarm(In := REAL#10.0, HIGH_LIMIT := REAL#8.0, LOW_LIMIT := REAL#3.0);

ASSERT_EQUAL(EXPECTED := MS_TO_BFT(MS := REAL#10.0), ACTUAL := Converter.BeaufortFromMetersPerSecond(Mps := REAL#10.0));
ASSERT_TRUE(IN := WindAlarm.Update(MeasuredValue := REAL#10.0) = ClassicAlarm.Q);
END_TEST_PROGRAM

(* Multi-wind-speed alarm cycle: drives the same alarm switch through
   high -> in-window -> low wind speeds. Verifies Q transitions
   instead of latching on the first high reading. *)
TEST_PROGRAM WindSpeedAlarmTracksWindSpeedTransitions
VAR
    WindAlarm : HysteresisSwitch;
    AlarmLimits : RealRange;
END_VAR

AlarmLimits.Low := REAL#3.0;
AlarmLimits.High := REAL#8.0;
WindAlarm.Initialize();
WindAlarm.SetLimits(Limits := AlarmLimits);

(* High wind (10 m/s above HIGH 8) -> trip *)
ASSERT_TRUE(IN := WindAlarm.Update(MeasuredValue := REAL#10.0));

(* In window (5 m/s) -> retain, Window TRUE *)
WindAlarm.Update(MeasuredValue := REAL#5.0);
ASSERT_TRUE(IN := WindAlarm.Window);

(* Low wind (1 m/s below LOW 3) -> clear *)
ASSERT_FALSE(IN := WindAlarm.Update(MeasuredValue := REAL#1.0));
END_TEST_PROGRAM

(* Multi-call regression: a long burst of updates with the same
   measurement keeps the alarm latch stable. Catches bugs where the
   hysteresis would oscillate on repeated reads of the same value. *)
TEST_PROGRAM WindSpeedAlarmStaysLatchedOnRepeatedHighReads
VAR
    WindAlarm : HysteresisSwitch;
    AlarmLimits : RealRange;
    I : INT;
END_VAR

AlarmLimits.Low := REAL#3.0;
AlarmLimits.High := REAL#8.0;
WindAlarm.Initialize();
WindAlarm.SetLimits(Limits := AlarmLimits);

(* Cross HIGH once *)
WindAlarm.Update(MeasuredValue := REAL#10.0);

(* Hold above for 25 reads -> Q stays TRUE *)
FOR I := 1 TO 25 DO
    ASSERT_TRUE(IN := WindAlarm.Update(MeasuredValue := REAL#10.0));
END_FOR;
END_TEST_PROGRAM

(* Boundary test: the exact LOW and HIGH limit values land in the
   window. Below LOW clears Q; above HIGH latches Q TRUE. *)
TEST_PROGRAM WindSpeedAlarmExactLimitsLandInWindow
VAR
    WindAlarm : HysteresisSwitch;
    AlarmLimits : RealRange;
END_VAR

AlarmLimits.Low := REAL#3.0;
AlarmLimits.High := REAL#8.0;
WindAlarm.Initialize();
WindAlarm.SetLimits(Limits := AlarmLimits);

(* Exactly at HIGH -> not above -> stays in default Q (FALSE), Window TRUE *)
WindAlarm.Update(MeasuredValue := REAL#8.0);
ASSERT_TRUE(IN := WindAlarm.Window);
ASSERT_FALSE(IN := WindAlarm.Q);

(* Above HIGH -> Q latches TRUE, Window FALSE *)
WindAlarm.Update(MeasuredValue := REAL#9.0);
ASSERT_TRUE(IN := WindAlarm.Q);
ASSERT_FALSE(IN := WindAlarm.Window);

(* At LOW -> Window again, Q holds latched TRUE *)
WindAlarm.Update(MeasuredValue := REAL#3.0);
ASSERT_TRUE(IN := WindAlarm.Window);
ASSERT_TRUE(IN := WindAlarm.Q);
END_TEST_PROGRAM

(* Per-FB isolation: drive a UnitConverter on its own. The Beaufort
   number for several wind speeds matches the classic helper. *)
TEST_PROGRAM WindSpeedAlarmConverterMatchesClassicHelper
VAR
    Converter : UnitConverter;
END_VAR

Converter.Initialize();
ASSERT_EQUAL(EXPECTED := MS_TO_BFT(MS := REAL#0.0), ACTUAL := Converter.BeaufortFromMetersPerSecond(Mps := REAL#0.0));
ASSERT_EQUAL(EXPECTED := MS_TO_BFT(MS := REAL#5.0), ACTUAL := Converter.BeaufortFromMetersPerSecond(Mps := REAL#5.0));
ASSERT_EQUAL(EXPECTED := MS_TO_BFT(MS := REAL#15.0), ACTUAL := Converter.BeaufortFromMetersPerSecond(Mps := REAL#15.0));
ASSERT_EQUAL(EXPECTED := MS_TO_BFT(MS := REAL#30.0), ACTUAL := Converter.BeaufortFromMetersPerSecond(Mps := REAL#30.0));
END_TEST_PROGRAM

(* Edge input: zero wind. Beaufort returns 0 and the alarm stays clear
   from initial state. *)
TEST_PROGRAM WindSpeedAlarmCalmWindReturnsZeroBeaufortAndNoAlarm
VAR
    Converter : UnitConverter;
    WindAlarm : HysteresisSwitch;
    AlarmLimits : RealRange;
END_VAR

Converter.Initialize();
AlarmLimits.Low := REAL#3.0;
AlarmLimits.High := REAL#8.0;
WindAlarm.Initialize();
WindAlarm.SetLimits(Limits := AlarmLimits);

ASSERT_EQUAL(EXPECTED := INT#0, ACTUAL := Converter.BeaufortFromMetersPerSecond(Mps := REAL#0.0));
ASSERT_FALSE(IN := WindAlarm.Update(MeasuredValue := REAL#0.0));
END_TEST_PROGRAM

(* Edge input: hurricane-force wind. The hysteresis stays latched TRUE
   even after the wind drops back into the band, until it falls below
   the low limit. *)
TEST_PROGRAM WindSpeedAlarmHurricaneForceLatchesAndPersistsAcrossWindow
VAR
    WindAlarm : HysteresisSwitch;
    AlarmLimits : RealRange;
END_VAR

AlarmLimits.Low := REAL#3.0;
AlarmLimits.High := REAL#8.0;
WindAlarm.Initialize();
WindAlarm.SetLimits(Limits := AlarmLimits);

ASSERT_TRUE(IN := WindAlarm.Update(MeasuredValue := REAL#50.0));

(* Drop into window: alarm latches *)
WindAlarm.Update(MeasuredValue := REAL#5.0);
ASSERT_TRUE(IN := WindAlarm.Q);
ASSERT_TRUE(IN := WindAlarm.Window);

(* Drop below low limit: alarm clears *)
ASSERT_FALSE(IN := WindAlarm.Update(MeasuredValue := REAL#1.0));
END_TEST_PROGRAM
