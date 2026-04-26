PROGRAM Main
VAR
    Converter : UnitConverter;
    ConverterObject : IUnitConverter;
    ChillerController : PidController;
    ControllerObject : IPidController;
    Limits : RealRange;
    ReturnTemperatureCelsius : REAL := REAL#12.0;
    ReturnTemperatureKelvin : REAL;
    ValveCommandPercent : REAL;
END_VAR

ConverterObject := Converter;
ConverterObject.Initialize();
ReturnTemperatureKelvin := ConverterObject.KelvinFromCelsius(Celsius := ReturnTemperatureCelsius);

Limits.Low := REAL#0.0;
Limits.High := REAL#100.0;
ControllerObject := ChillerController;
ControllerObject.Initialize();
ControllerObject.SetKp(Kp := REAL#2.0);
ControllerObject.SetIntegralTime(Tn := REAL#1.0);
ControllerObject.SetDerivativeTime(Tv := REAL#0.0);
ControllerObject.SetLimits(Limits := Limits);
ControllerObject.SetManual(Manual := TRUE, ManualInput := REAL#45.0);
ValveCommandPercent := ControllerObject.Update(Actual := ReturnTemperatureCelsius, Target := REAL#7.0);
END_PROGRAM
