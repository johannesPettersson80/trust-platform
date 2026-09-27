PROGRAM Main
VAR
    Command : LREAL := 0.0;
    Feedback : LREAL := 0.0;
    Feedback2 : LREAL := 0.0;
END_VAR

Feedback := Command;
Feedback2 := Command;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.Command AT %QL0 : LREAL;
    P1.Feedback AT %IL0 : LREAL;
    P1.Feedback2 AT %IL8 : LREAL;
END_VAR
END_CONFIGURATION
