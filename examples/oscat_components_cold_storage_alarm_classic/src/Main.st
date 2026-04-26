PROGRAM Main
VAR
    TemperatureConversion : TEMPERATURE;
    DoorAlarm : HYST;
    ChamberCelsius : REAL := REAL#0.0 - REAL#16.0;
    ChamberKelvin : REAL;
    AlarmActive : BOOL;
END_VAR

TemperatureConversion(C := ChamberCelsius);
DoorAlarm(
    In := ChamberCelsius,
    HIGH_LIMIT := REAL#0.0 - REAL#18.0,
    LOW_LIMIT := REAL#0.0 - REAL#30.0
);
ChamberKelvin := TemperatureConversion.YK;
AlarmActive := DoorAlarm.Q;
END_PROGRAM
