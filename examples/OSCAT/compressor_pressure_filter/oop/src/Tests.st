TEST_PROGRAM CompressorPressureFilterOopFilterBeforeAlarm
VAR
    PressureFilter : Pt1Filter;
    PressureAlarm : HysteresisSwitch;
    AlarmLimits : RealRange;
    ClassicFilter : FT_PT1;
    ClassicAlarm : HYST;
    FilteredPressureBar : REAL;
END_VAR

PressureFilter.Initialize();
PressureFilter.Configure(TimeConstant := T#1s, Gain := REAL#1.0);
AlarmLimits.Low := REAL#4.0;
AlarmLimits.High := REAL#10.0;
PressureAlarm.Initialize();
PressureAlarm.SetLimits(Limits := AlarmLimits);

ClassicFilter(in := REAL#11.0, T := T#1s, K := REAL#1.0);
FilteredPressureBar := PressureFilter.Update(Sample := REAL#11.0);
ClassicAlarm(In := ClassicFilter.out, HIGH_LIMIT := REAL#10.0, LOW_LIMIT := REAL#4.0);

ASSERT_NEAR(EXPECTED := ClassicFilter.out, ACTUAL := FilteredPressureBar, DELTA := REAL#0.001);
ASSERT_TRUE(IN := PressureAlarm.Update(MeasuredValue := FilteredPressureBar) = ClassicAlarm.Q);
END_TEST_PROGRAM

(* Multi-call alarm-transit test: drives the same HysteresisSwitch
   through high -> in-window -> low pressures bypassing the PT1 filter
   (filter dynamics depend on real-time and would make a multi-scan
   filter test brittle). Verifies the alarm Q reacts to transitions
   instead of latching after the first trip. *)
TEST_PROGRAM CompressorAlarmTracksMultiplePressureReadings
VAR
    PressureAlarm : HysteresisSwitch;
    AlarmLimits : RealRange;
END_VAR

AlarmLimits.Low := REAL#4.0;
AlarmLimits.High := REAL#10.0;
PressureAlarm.Initialize();
PressureAlarm.SetLimits(Limits := AlarmLimits);

(* High pressure -> trip *)
ASSERT_TRUE(IN := PressureAlarm.Update(MeasuredValue := REAL#11.0));

(* In window -> Q retains, Window TRUE *)
PressureAlarm.Update(MeasuredValue := REAL#7.0);
ASSERT_TRUE(IN := PressureAlarm.Window);

(* Low pressure -> clear *)
ASSERT_FALSE(IN := PressureAlarm.Update(MeasuredValue := REAL#2.0));
END_TEST_PROGRAM

(* Deadband holds: once tripped at high, an in-window reading must
   keep Q latched. Pt1Filter is bypassed in this test because its
   wall-clock dependency makes multi-step assertions on filtered
   pressure too fragile in a microsecond test cycle. *)
TEST_PROGRAM CompressorAlarmHoldsThroughDeadband
VAR
    PressureAlarm : HysteresisSwitch;
    AlarmLimits : RealRange;
END_VAR

AlarmLimits.Low := REAL#4.0;
AlarmLimits.High := REAL#10.0;
PressureAlarm.Initialize();
PressureAlarm.SetLimits(Limits := AlarmLimits);

(* Trip at 12 bar *)
ASSERT_TRUE(IN := PressureAlarm.Update(MeasuredValue := REAL#12.0));
(* Drop to 8 bar (in window): held *)
ASSERT_TRUE(IN := PressureAlarm.Update(MeasuredValue := REAL#8.0));
(* Drop to 5 bar (still in window): still held *)
ASSERT_TRUE(IN := PressureAlarm.Update(MeasuredValue := REAL#5.0));
(* Drop to 3 bar (below LowLimit): cleared *)
ASSERT_FALSE(IN := PressureAlarm.Update(MeasuredValue := REAL#3.0));
(* Re-enter window from below: stay cleared *)
ASSERT_FALSE(IN := PressureAlarm.Update(MeasuredValue := REAL#7.0));
END_TEST_PROGRAM

(* Two compressors with shared limits but separate FBs: per-FB isolation.
   Each FB's latched Q must not leak into the other instance. *)
TEST_PROGRAM CompressorTwoTagsAreIsolated
VAR
    AlarmA : HysteresisSwitch;
    AlarmB : HysteresisSwitch;
    AlarmLimits : RealRange;
END_VAR

AlarmLimits.Low := REAL#4.0;
AlarmLimits.High := REAL#10.0;
AlarmA.Initialize();
AlarmB.Initialize();
AlarmA.SetLimits(Limits := AlarmLimits);
AlarmB.SetLimits(Limits := AlarmLimits);

(* Compressor A trips, Compressor B stays low *)
ASSERT_TRUE(IN := AlarmA.Update(MeasuredValue := REAL#12.0));
ASSERT_FALSE(IN := AlarmB.Update(MeasuredValue := REAL#3.0));
(* Same in-window reading goes to both: each holds its own state *)
ASSERT_TRUE(IN := AlarmA.Update(MeasuredValue := REAL#7.0));
ASSERT_FALSE(IN := AlarmB.Update(MeasuredValue := REAL#7.0));
END_TEST_PROGRAM

(* Pt1Filter unity gain: with Gain=1.0, a steady-state input of X
   eventually settles at X. We do NOT assert on a single Update call
   because of the wall-clock dependency, but we can verify that
   passing a constant value through Update returns a non-NaN REAL
   inside the closed band [0, Sample]. *)
TEST_PROGRAM CompressorFilterReturnsRealValueWithinBand
VAR
    PressureFilter : Pt1Filter;
    Sample : REAL;
    Out : REAL;
END_VAR

Sample := REAL#11.0;
PressureFilter.Initialize();
PressureFilter.Configure(TimeConstant := T#1s, Gain := REAL#1.0);
Out := PressureFilter.Update(Sample := Sample);
(* Without an assumption on dt, the filter output is in [0, Sample]
   for a positive step input from rest *)
ASSERT_TRUE(IN := Out >= REAL#0.0);
ASSERT_TRUE(IN := Out <= Sample);
END_TEST_PROGRAM

(* Inverted limits raise Error. Commissioning slip guard. *)
TEST_PROGRAM CompressorInvertedLimitsRaiseError
VAR
    PressureAlarm : HysteresisSwitch;
    BadLimits : RealRange;
END_VAR

BadLimits.Low := REAL#10.0;
BadLimits.High := REAL#4.0;
PressureAlarm.Initialize();
ASSERT_FALSE(IN := PressureAlarm.Error);
PressureAlarm.SetLimits(Limits := BadLimits);
ASSERT_TRUE(IN := PressureAlarm.Error);
END_TEST_PROGRAM
