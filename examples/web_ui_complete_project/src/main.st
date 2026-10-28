PROGRAM Main
VAR
  StartBtn : BOOL := FALSE;
  StopBtn : BOOL := FALSE;
  FaultReset : BOOL := FALSE;
  Overload : BOOL := FALSE;
  MotorRun : BOOL := FALSE;
  FaultLatched : BOOL := FALSE;
  RunHours : REAL := 0.0;
  CycleSeconds : REAL := 0.02;
END_VAR

IF StopBtn THEN
  MotorRun := FALSE;
END_IF;

IF Overload THEN
  FaultLatched := TRUE;
  MotorRun := FALSE;
END_IF;

IF FaultReset THEN
  FaultLatched := FALSE;
END_IF;

IF StartBtn AND NOT FaultLatched THEN
  MotorRun := TRUE;
END_IF;

IF MotorRun THEN
  RunHours := RunHours + (CycleSeconds / 3600.0);
END_IF;
END_PROGRAM

// The screens: after the task that calls Hmi.Display, the runtime copies the values they show,
// applies their commands and reads the alarms.
PROGRAM Screens
VAR
    Display : Hmi.Display;
END_VAR
    Display();
END_PROGRAM

CONFIGURATION Config
RESOURCE Cpu ON PLC
    TASK MainTask (INTERVAL := T#20ms, PRIORITY := 1);
    PROGRAM Main WITH MainTask : Main;
    TASK ScreenTask (INTERVAL := T#250ms, PRIORITY := 10);
    PROGRAM Screens WITH ScreenTask : Screens;
END_RESOURCE
END_CONFIGURATION
