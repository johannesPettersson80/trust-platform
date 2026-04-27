(* Compact showcase: Main keeps the component call sequence visible for the ST tests. *)
PROGRAM Main
VAR
    Converter : UnitConverter;
    WindAlarm : HysteresisSwitch;
    AlarmLimits : RealRange;
    WindMetersPerSecond : REAL := REAL#10.0;
    BeaufortIndex : INT;
    AlarmActive : BOOL;
END_VAR

Converter.Initialize();
BeaufortIndex := Converter.BeaufortFromMetersPerSecond(Mps := WindMetersPerSecond);

AlarmLimits.Low := REAL#3.0;
AlarmLimits.High := REAL#8.0;
WindAlarm.Initialize();
WindAlarm.SetLimits(Limits := AlarmLimits);
AlarmActive := WindAlarm.Update(MeasuredValue := WindMetersPerSecond);
END_PROGRAM
