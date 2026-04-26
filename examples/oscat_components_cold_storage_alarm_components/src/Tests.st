TEST_PROGRAM ColdStorageAlarmComponentsDetectWarmChamber
VAR
    Converter : UnitConverter;
    ConverterObject : IUnitConverter;
    DoorAlarm : HysteresisSwitch;
    AlarmObject : IHysteresisSwitch;
    ClassicAlarm : HYST;
    AlarmLimits : RealRange;
END_VAR

ConverterObject := Converter;
ConverterObject.Initialize();
AlarmLimits.Low := REAL#0.0 - REAL#30.0;
AlarmLimits.High := REAL#0.0 - REAL#18.0;
AlarmObject := DoorAlarm;
AlarmObject.Initialize();
AlarmObject.SetLimits(Limits := AlarmLimits);
ClassicAlarm(
    In := REAL#0.0 - REAL#16.0,
    HIGH_LIMIT := REAL#0.0 - REAL#18.0,
    LOW_LIMIT := REAL#0.0 - REAL#30.0
);

ASSERT_NEAR(EXPECTED := REAL#257.15, ACTUAL := ConverterObject.KelvinFromCelsius(Celsius := REAL#0.0 - REAL#16.0), DELTA := REAL#0.01);
ASSERT_TRUE(IN := AlarmObject.Update(MeasuredValue := REAL#0.0 - REAL#16.0) = ClassicAlarm.Q);
END_TEST_PROGRAM
