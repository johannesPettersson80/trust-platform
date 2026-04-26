PROGRAM Main
VAR
    Converter : UnitConverter;
    ConverterObject : IUnitConverter;
    DoorAlarm : HysteresisSwitch;
    AlarmObject : IHysteresisSwitch;
    AlarmLimits : RealRange;
    ChamberCelsius : REAL := REAL#0.0 - REAL#16.0;
    ChamberKelvin : REAL;
    AlarmActive : BOOL;
END_VAR

ConverterObject := Converter;
ConverterObject.Initialize();
ChamberKelvin := ConverterObject.KelvinFromCelsius(Celsius := ChamberCelsius);

AlarmLimits.Low := REAL#0.0 - REAL#30.0;
AlarmLimits.High := REAL#0.0 - REAL#18.0;
AlarmObject := DoorAlarm;
AlarmObject.Initialize();
AlarmObject.SetLimits(Limits := AlarmLimits);
AlarmActive := AlarmObject.Update(MeasuredValue := ChamberCelsius);
END_PROGRAM
