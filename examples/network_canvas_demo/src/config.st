CONFIGURATION Config
VAR_GLOBAL
    conveyor_speed : REAL := 1.5;
    running : BOOL := FALSE;
    line_state : INT := 1;
END_VAR
RESOURCE CommRes ON PLC
    TASK MainTask (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM Main WITH MainTask : Main;
END_RESOURCE
END_CONFIGURATION
