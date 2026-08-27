PROGRAM Main
VAR
    Setpoint : LREAL := 50.0;
    ProcessValue : LREAL := 20.0;
    ManipulatedValue : LREAL := 0.0;
    OptionalFeedback : LREAL := 0.0;
END_VAR
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.ManipulatedValue AT %QL0 : LREAL;
    P1.ProcessValue AT %IL0 : LREAL;
    P1.OptionalFeedback AT %IL8 : LREAL;
END_VAR
END_CONFIGURATION
