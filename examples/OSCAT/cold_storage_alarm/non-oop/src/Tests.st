TEST_PROGRAM ColdStorageAlarmClassicDetectsWarmChamber
VAR
    TemperatureConversion : TEMPERATURE;
    DoorAlarm : HYST;
END_VAR

TemperatureConversion(C := REAL#0.0 - REAL#16.0);
DoorAlarm(
    In := REAL#0.0 - REAL#16.0,
    HIGH_LIMIT := REAL#0.0 - REAL#18.0,
    LOW_LIMIT := REAL#0.0 - REAL#30.0
);

ASSERT_NEAR(EXPECTED := REAL#257.15, ACTUAL := TemperatureConversion.YK, DELTA := REAL#0.01);
ASSERT_TRUE(IN := DoorAlarm.Q);
END_TEST_PROGRAM
