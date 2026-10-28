PROGRAM Main
VAR
    AutoMode : BOOL := TRUE;
    Setpoint : LREAL := 30.0;
    ProcessValue : LREAL := 20.0;
    ManipulatedValue : LREAL := 25.0;
END_VAR

IF AutoMode THEN
    ManipulatedValue := ManipulatedValue;
ELSE
    ManipulatedValue := LREAL#0.0;
END_IF;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#20ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.AutoMode AT %MX8.0 : BOOL;
    P1.Setpoint AT %ML0 : LREAL;
    P1.ManipulatedValue AT %QL0 : LREAL;
    P1.ProcessValue AT %IL0 : LREAL;
END_VAR
END_CONFIGURATION
