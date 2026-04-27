PROGRAM Main
VAR
    PressureController : CTRL_PID;
    PressureAlarm : HYST;
    PressureBar : REAL := REAL#12.0;
    TargetBar : REAL := REAL#8.0;
    PumpCommandPercent : REAL;
    AlarmActive : BOOL;
END_VAR

PressureController(
    ACT := PressureBar,
    SET_POINT := TargetBar,
    SUP := REAL#0.0,
    OFS := REAL#0.0,
    M_I := REAL#60.0,
    MAN := TRUE,
    RST := FALSE,
    KP := REAL#1.5,
    TN := REAL#1.0,
    TV := REAL#0.0,
    LL := REAL#0.0,
    LH := REAL#100.0
);
PressureAlarm(In := PressureBar, HIGH_LIMIT := REAL#10.0, LOW_LIMIT := REAL#3.0);
PumpCommandPercent := PressureController.Y;
AlarmActive := PressureAlarm.Q;
END_PROGRAM
