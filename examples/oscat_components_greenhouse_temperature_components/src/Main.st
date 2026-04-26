PROGRAM Main
VAR
    Converter : UnitConverter;
    ConverterObject : IUnitConverter;
    VentilationSwitch : HysteresisSwitch;
    SwitchObject : IHysteresisSwitch;
    Limits : RealRange;
    TemperatureCelsius : REAL := REAL#29.0;
    TemperatureKelvin : REAL;
    VentilationRequest : BOOL;
END_VAR

ConverterObject := Converter;
ConverterObject.Initialize();
TemperatureKelvin := ConverterObject.KelvinFromCelsius(Celsius := TemperatureCelsius);

Limits.Low := REAL#18.0;
Limits.High := REAL#28.0;
SwitchObject := VentilationSwitch;
SwitchObject.Initialize();
SwitchObject.SetLimits(Limits := Limits);
VentilationRequest := SwitchObject.Update(MeasuredValue := TemperatureCelsius);
END_PROGRAM
