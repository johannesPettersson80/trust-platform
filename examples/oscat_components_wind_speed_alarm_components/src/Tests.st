TEST_PROGRAM WindSpeedAlarmComponentsAdaptConversionIntoAlarm
VAR
    Converter : UnitConverter;
    ConverterObject : IUnitConverter;
    WindAlarm : HysteresisSwitch;
    AlarmObject : IHysteresisSwitch;
    ClassicAlarm : HYST;
    AlarmLimits : RealRange;
END_VAR

ConverterObject := Converter;
ConverterObject.Initialize();
AlarmLimits.Low := REAL#3.0;
AlarmLimits.High := REAL#8.0;
AlarmObject := WindAlarm;
AlarmObject.Initialize();
AlarmObject.SetLimits(Limits := AlarmLimits);
ClassicAlarm(In := REAL#10.0, HIGH_LIMIT := REAL#8.0, LOW_LIMIT := REAL#3.0);

ASSERT_EQUAL(EXPECTED := MS_TO_BFT(MS := REAL#10.0), ACTUAL := ConverterObject.BeaufortFromMetersPerSecond(Mps := REAL#10.0));
ASSERT_TRUE(IN := AlarmObject.Update(MeasuredValue := REAL#10.0) = ClassicAlarm.Q);
END_TEST_PROGRAM
