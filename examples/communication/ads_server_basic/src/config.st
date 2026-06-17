CONFIGURATION Config
VAR_GLOBAL
    TankLevel : REAL := 42.5;
    PumpRunning : BOOL := FALSE;
    Setpoint : REAL := 10.0;
    StatusWord : WORD := 16#1234;
END_VAR
RESOURCE CommRes ON PLC
    TASK MainTask (INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM Main WITH MainTask : Main;
END_RESOURCE
END_CONFIGURATION
