// The screens: after the task that calls Hmi.Display, the runtime copies the values they show,
// applies their commands and reads the alarms.
PROGRAM Screens
VAR
    Display : Hmi.Display;
END_VAR
    Display();
END_PROGRAM

CONFIGURATION Config
VAR_GLOBAL
    TankLevel : REAL := 42.5;
    PumpRunning : BOOL := FALSE;
    Setpoint : REAL := 10.0;
    StatusWord : WORD := 16#1234;
END_VAR
RESOURCE truST_ADS ON PLC
    TASK MainTask (INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM Main WITH MainTask : Main;
    TASK ScreenTask (INTERVAL := T#250ms, PRIORITY := 10);
    PROGRAM Screens WITH ScreenTask : Screens;
END_RESOURCE
END_CONFIGURATION
