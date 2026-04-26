TEST_PROGRAM GreenhouseTemperatureComponentsMatchClassicConversionAndSwitch
VAR
    Converter : UnitConverter;
    ConverterObject : IUnitConverter;
    VentilationSwitch : HysteresisSwitch;
    SwitchObject : IHysteresisSwitch;
    ClassicSwitch : HYST;
    Limits : RealRange;
END_VAR

ConverterObject := Converter;
ConverterObject.Initialize();
Limits.Low := REAL#18.0;
Limits.High := REAL#28.0;
SwitchObject := VentilationSwitch;
SwitchObject.Initialize();
SwitchObject.SetLimits(Limits := Limits);
ClassicSwitch(In := REAL#29.0, HIGH_LIMIT := REAL#28.0, LOW_LIMIT := REAL#18.0);

ASSERT_NEAR(EXPECTED := REAL#302.15, ACTUAL := ConverterObject.KelvinFromCelsius(Celsius := REAL#29.0), DELTA := REAL#0.01);
ASSERT_TRUE(IN := SwitchObject.Update(MeasuredValue := REAL#29.0) = ClassicSwitch.Q);
END_TEST_PROGRAM
