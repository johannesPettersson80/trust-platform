// The screens: after the task that calls Hmi.Display, the runtime copies the values they show,
// applies their commands and reads the alarms.
PROGRAM Screens
VAR
    Display : Hmi.Display;
END_VAR
    Display();
END_PROGRAM

CONFIGURATION PlantDemo
TASK Fast (INTERVAL := T#100ms, PRIORITY := 1);
PROGRAM P1 WITH Fast : PlantProgram;
TASK ScreenTask (INTERVAL := T#250ms, PRIORITY := 10);
PROGRAM Screens WITH ScreenTask : Screens;
VAR_CONFIG
    P1.StartCmd AT %IX0.0 : BOOL;
    P1.SpeedRaw AT %IW0 : INT;
    P1.HaltReq AT %QX0.0 : BOOL;
END_VAR
END_CONFIGURATION
