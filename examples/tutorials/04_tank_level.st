PROGRAM TankLevel
VAR
    InletValveCmd : BOOL := FALSE;
    OutletValveCmd : BOOL := TRUE;
    LevelPercent : REAL := 45.0;
    LowSetpoint : REAL := 30.0;
    HighSetpoint : REAL := 70.0;
END_VAR

IF LevelPercent <= LowSetpoint THEN
    InletValveCmd := TRUE;
    OutletValveCmd := FALSE;
ELSIF LevelPercent >= HighSetpoint THEN
    InletValveCmd := FALSE;
    OutletValveCmd := TRUE;
END_IF;

IF InletValveCmd THEN
    LevelPercent := LevelPercent + 2.5;
END_IF;

IF OutletValveCmd THEN
    LevelPercent := LevelPercent - 1.5;
END_IF;
END_PROGRAM
