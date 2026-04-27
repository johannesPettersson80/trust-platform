(* Compact showcase: Main keeps the component call sequence visible for the ST tests. *)
PROGRAM Main
VAR
    Converter : UnitConverter;
    DoorAlarm : HysteresisSwitch;
    AlarmLimits : RealRange;
    ChamberCelsius : REAL := REAL#0.0 - REAL#16.0;
    ChamberKelvin : REAL;
    AlarmActive : BOOL;
END_VAR

Converter.Initialize();
ChamberKelvin := Converter.KelvinFromCelsius(Celsius := ChamberCelsius);

AlarmLimits.Low := REAL#0.0 - REAL#30.0;
AlarmLimits.High := REAL#0.0 - REAL#18.0;
DoorAlarm.Initialize();
DoorAlarm.SetLimits(Limits := AlarmLimits);
AlarmActive := DoorAlarm.Update(MeasuredValue := ChamberCelsius);
END_PROGRAM
