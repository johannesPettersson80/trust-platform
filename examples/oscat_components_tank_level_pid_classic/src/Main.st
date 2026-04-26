PROGRAM Main
VAR
    TankController : CTRL_PID;
    LevelPercent : REAL := REAL#42.0;
    TargetLevelPercent : REAL := REAL#75.0;
    ValveCommandPercent : REAL;
    CommandLimited : BOOL;
END_VAR

TankController(
    ACT := LevelPercent,
    SET_POINT := TargetLevelPercent,
    SUP := REAL#0.0,
    OFS := REAL#0.0,
    M_I := REAL#35.0,
    MAN := TRUE,
    RST := FALSE,
    KP := REAL#2.0,
    TN := REAL#1.0,
    TV := REAL#0.0,
    LL := REAL#0.0,
    LH := REAL#100.0
);
ValveCommandPercent := TankController.Y;
CommandLimited := TankController.LIM;
END_PROGRAM
