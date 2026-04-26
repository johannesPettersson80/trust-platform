TEST_PROGRAM CompressorPressureFilterComponentsFilterBeforeAlarm
VAR
    PressureFilter : Pt1Filter;
    FilterObject : IPt1Filter;
    PressureAlarm : HysteresisSwitch;
    AlarmObject : IHysteresisSwitch;
    AlarmLimits : RealRange;
    ClassicFilter : FT_PT1;
    ClassicAlarm : HYST;
    FilteredPressureBar : REAL;
END_VAR

FilterObject := PressureFilter;
FilterObject.Initialize();
FilterObject.Configure(TimeConstant := T#1s, Gain := REAL#1.0);
AlarmLimits.Low := REAL#4.0;
AlarmLimits.High := REAL#10.0;
AlarmObject := PressureAlarm;
AlarmObject.Initialize();
AlarmObject.SetLimits(Limits := AlarmLimits);

ClassicFilter(in := REAL#11.0, T := T#1s, K := REAL#1.0);
FilteredPressureBar := FilterObject.Update(Sample := REAL#11.0);
ClassicAlarm(In := ClassicFilter.out, HIGH_LIMIT := REAL#10.0, LOW_LIMIT := REAL#4.0);

ASSERT_NEAR(EXPECTED := ClassicFilter.out, ACTUAL := FilteredPressureBar, DELTA := REAL#0.001);
ASSERT_TRUE(IN := AlarmObject.Update(MeasuredValue := FilteredPressureBar) = ClassicAlarm.Q);
END_TEST_PROGRAM
