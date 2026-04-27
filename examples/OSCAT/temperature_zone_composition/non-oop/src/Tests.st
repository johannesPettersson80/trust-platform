TEST_PROGRAM TemperatureZoneClassicRepeatsTheThreeBlockPattern
VAR
    ZoneFilter : FT_PT1;
    ZoneController : CTRL_PID;
    ZoneAlarm : HYST;
END_VAR

ZoneFilter(in := REAL#21.0, T := T#1s, K := REAL#1.0);
ZoneController(ACT := ZoneFilter.out, SET_POINT := REAL#23.0, SUP := REAL#0.0, OFS := REAL#0.0, M_I := REAL#45.0, MAN := TRUE, RST := FALSE, KP := REAL#1.0, TN := REAL#1.0, TV := REAL#0.0, LL := REAL#0.0, LH := REAL#100.0);
ZoneAlarm(In := ZoneFilter.out, HIGH_LIMIT := REAL#30.0, LOW_LIMIT := REAL#5.0);

ASSERT_NEAR(EXPECTED := REAL#21.0, ACTUAL := ZoneFilter.out, DELTA := REAL#0.001);
ASSERT_NEAR(EXPECTED := REAL#45.0, ACTUAL := ZoneController.Y, DELTA := REAL#0.001);
ASSERT_FALSE(IN := ZoneAlarm.Q);
END_TEST_PROGRAM
