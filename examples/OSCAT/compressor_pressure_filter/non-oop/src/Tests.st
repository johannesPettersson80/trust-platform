TEST_PROGRAM CompressorPressureFilterClassicFiltersBeforeAlarm
VAR
    PressureFilter : FT_PT1;
    PressureAlarm : HYST;
END_VAR

PressureFilter(in := REAL#11.0, T := T#1s, K := REAL#1.0);
PressureAlarm(In := PressureFilter.out, HIGH_LIMIT := REAL#10.0, LOW_LIMIT := REAL#4.0);

ASSERT_NEAR(EXPECTED := REAL#11.0, ACTUAL := PressureFilter.out, DELTA := REAL#0.001);
ASSERT_TRUE(IN := PressureAlarm.Q);
END_TEST_PROGRAM
