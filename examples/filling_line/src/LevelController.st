FUNCTION_BLOCK LevelController
VAR_INPUT
    LevelPct : REAL;
    StartCmd : BOOL;
    StopCmd : BOOL;
END_VAR
VAR_OUTPUT
    FillCmd : BOOL;
    DrainCmd : BOOL;
    Mode : LevelMode;
END_VAR
VAR
    Target : REAL := 70.0;
    Hysteresis : REAL := 5.0;
END_VAR

IF StopCmd THEN
    Mode := LevelMode#Stopped;
ELSIF StartCmd THEN
    IF LevelPct < Target - Hysteresis THEN
        Mode := LevelMode#Filling;
    ELSIF LevelPct > Target + Hysteresis THEN
        Mode := LevelMode#Draining;
    ELSE
        Mode := LevelMode#Stopped;
    END_IF;
END_IF;

FillCmd := Mode = LevelMode#Filling;
DrainCmd := Mode = LevelMode#Draining;
END_FUNCTION_BLOCK
