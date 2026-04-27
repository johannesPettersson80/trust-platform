(* Compact showcase: Main keeps the component call sequence visible for the ST tests. *)
PROGRAM Main
VAR
    PressureFilter : Pt1Filter;
    PressureAlarm : HysteresisSwitch;
    AlarmLimits : RealRange;
    RawPressureBar : REAL := REAL#11.0;
    FilteredPressureBar : REAL;
    AlarmActive : BOOL;
END_VAR

PressureFilter.Initialize();
PressureFilter.Configure(TimeConstant := T#1s, Gain := REAL#1.0);
FilteredPressureBar := PressureFilter.Update(Sample := RawPressureBar);

AlarmLimits.Low := REAL#4.0;
AlarmLimits.High := REAL#10.0;
PressureAlarm.Initialize();
PressureAlarm.SetLimits(Limits := AlarmLimits);
AlarmActive := PressureAlarm.Update(MeasuredValue := FilteredPressureBar);
END_PROGRAM
