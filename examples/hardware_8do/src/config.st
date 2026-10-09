// The screens: after the task that calls Hmi.Display, the runtime copies the values they show,
// applies their commands and reads the alarms.
PROGRAM Screens
VAR
    Display : Hmi.Display;
END_VAR
    Display();
END_PROGRAM

CONFIGURATION Config
RESOURCE MainResource ON PLC
    TASK MainTask (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
    TASK ScreenTask (INTERVAL := T#250ms, PRIORITY := 10);
    PROGRAM Screens WITH ScreenTask : Screens;
END_RESOURCE
VAR_CONFIG
    P1.DO0 AT %QX0.0 : BOOL;
    P1.DO1 AT %QX0.1 : BOOL;
    P1.DO2 AT %QX0.2 : BOOL;
    P1.DO3 AT %QX0.3 : BOOL;
    P1.DO4 AT %QX0.4 : BOOL;
    P1.DO5 AT %QX0.5 : BOOL;
    P1.DO6 AT %QX0.6 : BOOL;
    P1.DO7 AT %QX0.7 : BOOL;
END_VAR
END_CONFIGURATION
