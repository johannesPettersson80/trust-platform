FUNCTION_BLOCK FB_TANK_CONTROL
VAR_INPUT
    StartCmd : BOOL;
    StopCmd : BOOL;
    LevelRaw : INT;
END_VAR
VAR_OUTPUT
    FillValveCmd : BOOL;
    DrainValveCmd : BOOL;
    PumpSpeedCmd : INT;
END_VAR

IF StopCmd THEN
    FillValveCmd := FALSE;
    DrainValveCmd := FALSE;
    PumpSpeedCmd := INT#0;
ELSIF StartCmd THEN
    IF LevelRaw < INT#300 THEN
        FillValveCmd := TRUE;
        DrainValveCmd := FALSE;
        PumpSpeedCmd := INT#800;
    ELSIF LevelRaw > INT#700 THEN
        FillValveCmd := FALSE;
        DrainValveCmd := TRUE;
        PumpSpeedCmd := INT#600;
    ELSE
        FillValveCmd := FALSE;
        DrainValveCmd := FALSE;
        PumpSpeedCmd := INT#500;
    END_IF;
ELSE
    FillValveCmd := FALSE;
    DrainValveCmd := FALSE;
    PumpSpeedCmd := INT#0;
END_IF;
END_FUNCTION_BLOCK

PROGRAM TankProgram
VAR
    StartCmd : BOOL;
    StopCmd : BOOL;
    LevelRaw : INT;
    FillValveOut : BOOL;
    DrainValveOut : BOOL;
    PumpSpeedOut : INT;

    Ctrl : FB_TANK_CONTROL;
END_VAR

Ctrl(StartCmd := StartCmd, StopCmd := StopCmd, LevelRaw := LevelRaw);
FillValveOut := Ctrl.FillValveCmd;
DrainValveOut := Ctrl.DrainValveCmd;
PumpSpeedOut := Ctrl.PumpSpeedCmd;
END_PROGRAM

CONFIGURATION TankControlConfig
TASK Cycle (INTERVAL := T#100ms, PRIORITY := 1);
PROGRAM P1 WITH Cycle : TankProgram;
VAR_CONFIG
    P1.StartCmd AT %IX0.0 : BOOL;
    P1.StopCmd AT %IX0.1 : BOOL;
    P1.LevelRaw AT %IW0 : INT;
    P1.FillValveOut AT %QX0.0 : BOOL;
    P1.DrainValveOut AT %QX0.1 : BOOL;
    P1.PumpSpeedOut AT %QW0 : INT;
END_VAR
END_CONFIGURATION
