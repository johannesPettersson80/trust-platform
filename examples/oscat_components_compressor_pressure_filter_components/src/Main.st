PROGRAM Main
VAR
    PressureFilter : Pt1Filter;
    FilterObject : IPt1Filter;
    PressureAlarm : HysteresisSwitch;
    AlarmObject : IHysteresisSwitch;
    AlarmLimits : RealRange;
    RawPressureBar : REAL := REAL#11.0;
    FilteredPressureBar : REAL;
    AlarmActive : BOOL;
END_VAR

FilterObject := PressureFilter;
FilterObject.Initialize();
FilterObject.Configure(TimeConstant := T#1s, Gain := REAL#1.0);
FilteredPressureBar := FilterObject.Update(Sample := RawPressureBar);

AlarmLimits.Low := REAL#4.0;
AlarmLimits.High := REAL#10.0;
AlarmObject := PressureAlarm;
AlarmObject.Initialize();
AlarmObject.SetLimits(Limits := AlarmLimits);
AlarmActive := AlarmObject.Update(MeasuredValue := FilteredPressureBar);
END_PROGRAM
