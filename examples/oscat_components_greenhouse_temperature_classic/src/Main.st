PROGRAM Main
VAR
    TemperatureConversion : TEMPERATURE;
    VentilationSwitch : HYST;
    TemperatureCelsius : REAL := REAL#29.0;
    TemperatureKelvin : REAL;
    TemperatureFahrenheit : REAL;
    VentilationRequest : BOOL;
END_VAR

TemperatureConversion(C := TemperatureCelsius);
TemperatureKelvin := TemperatureConversion.YK;
TemperatureFahrenheit := TemperatureConversion.YF;
VentilationSwitch(In := TemperatureCelsius, HIGH_LIMIT := REAL#28.0, LOW_LIMIT := REAL#18.0);
VentilationRequest := VentilationSwitch.Q;
END_PROGRAM
