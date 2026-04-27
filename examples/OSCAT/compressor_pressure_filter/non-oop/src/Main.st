PROGRAM Main
VAR
    PressureFilter : FT_PT1;
    PressureAlarm : HYST;
    RawPressureBar : REAL := REAL#11.0;
    FilteredPressureBar : REAL;
    AlarmActive : BOOL;
END_VAR

PressureFilter(in := RawPressureBar, T := T#1s, K := REAL#1.0);
FilteredPressureBar := PressureFilter.out;
PressureAlarm(In := FilteredPressureBar, HIGH_LIMIT := REAL#10.0, LOW_LIMIT := REAL#4.0);
AlarmActive := PressureAlarm.Q;
END_PROGRAM
