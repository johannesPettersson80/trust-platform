// The screens: after the task that calls Hmi.Display, the runtime copies the values they show,
// applies their commands and reads the alarms.
PROGRAM Screens
VAR
    Display : Hmi.Display;
END_VAR
    Display();
END_PROGRAM

CONFIGURATION MarkerCounter
TASK Cycle (INTERVAL := T#50ms, PRIORITY := 1);
PROGRAM P1 WITH Cycle : MainProgram;
TASK ScreenTask (INTERVAL := T#250ms, PRIORITY := 10);
PROGRAM Screens WITH ScreenTask : Screens;
VAR_CONFIG
    P1.Counter AT %MW0 : INT;
    P1.CounterLatched AT %QW0 : INT;
END_VAR
END_CONFIGURATION
