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
    conveyor_speed : REAL := 1.5;
    running : BOOL := FALSE;
    line_state : INT := 1;
END_VAR
RESOURCE Simulator ON PLC
    TASK MainTask (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM Main WITH MainTask : Main;
    TASK ScreenTask (INTERVAL := T#250ms, PRIORITY := 10);
    PROGRAM Screens WITH ScreenTask : Screens;
END_RESOURCE
END_CONFIGURATION
