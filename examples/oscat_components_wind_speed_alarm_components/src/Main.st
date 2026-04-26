PROGRAM Main
VAR
    Converter : UnitConverter;
    ConverterObject : IUnitConverter;
    WindAlarm : HysteresisSwitch;
    AlarmObject : IHysteresisSwitch;
    AlarmLimits : RealRange;
    WindMetersPerSecond : REAL := REAL#10.0;
    BeaufortIndex : INT;
    AlarmActive : BOOL;
END_VAR

ConverterObject := Converter;
ConverterObject.Initialize();
BeaufortIndex := ConverterObject.BeaufortFromMetersPerSecond(Mps := WindMetersPerSecond);

AlarmLimits.Low := REAL#3.0;
AlarmLimits.High := REAL#8.0;
AlarmObject := WindAlarm;
AlarmObject.Initialize();
AlarmObject.SetLimits(Limits := AlarmLimits);
AlarmActive := AlarmObject.Update(MeasuredValue := WindMetersPerSecond);
END_PROGRAM
