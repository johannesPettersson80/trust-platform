PROGRAM Main
VAR
    UsePid : BOOL := FALSE;
    PiLoop : CTRL_PI;
    PidLoop : CTRL_PID;
    ActualPressure : REAL := REAL#4.0;
    TargetPressure : REAL := REAL#7.0;
    ValveCommand : REAL;
    CommandLimited : BOOL;
END_VAR

IF UsePid THEN
    PidLoop(
        ACT := ActualPressure,
        SET_POINT := TargetPressure,
        SUP := REAL#0.0,
        OFS := REAL#0.0,
        M_I := REAL#42.0,
        MAN := TRUE,
        RST := FALSE,
        KP := REAL#1.5,
        TN := REAL#1.0,
        TV := REAL#0.2,
        LL := REAL#0.0,
        LH := REAL#100.0
    );
    ValveCommand := PidLoop.Y;
    CommandLimited := PidLoop.LIM;
ELSE
    PiLoop(
        ACT := ActualPressure,
        SET_POINT := TargetPressure,
        SUP := REAL#0.0,
        OFS := REAL#0.0,
        M_I := REAL#42.0,
        MAN := TRUE,
        RST := FALSE,
        KP := REAL#1.5,
        KI := REAL#1.0,
        LL := REAL#0.0,
        LH := REAL#100.0
    );
    ValveCommand := PiLoop.Y;
    CommandLimited := PiLoop.LIM;
END_IF;
END_PROGRAM
