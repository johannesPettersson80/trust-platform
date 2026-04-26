PROGRAM MainProgram
VAR
    StartCmd : BOOL;
    StopCmd : BOOL;
    LevelRaw : INT;
    FillValveOut : BOOL;
    DrainValveOut : BOOL;
    PumpSpeedOut : INT;

    LevelPct : REAL;
    Ctrl : LevelController;
END_VAR

LevelPct := INT_TO_REAL(LevelRaw) * 0.1;

Ctrl(LevelPct := LevelPct, StartCmd := StartCmd, StopCmd := StopCmd);

IF Ctrl.FillCmd THEN
    FillValveOut := TRUE;
    DrainValveOut := FALSE;
    PumpSpeedOut := INT#800;
ELSIF Ctrl.DrainCmd THEN
    FillValveOut := FALSE;
    DrainValveOut := TRUE;
    PumpSpeedOut := INT#600;
ELSE
    FillValveOut := FALSE;
    DrainValveOut := FALSE;
    PumpSpeedOut := INT#0;
END_IF;
END_PROGRAM
