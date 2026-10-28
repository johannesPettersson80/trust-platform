PROGRAM Main
VAR
    TankCommand : LREAL := 2.5;
    OvenCommand : LREAL := 0.0;
    TankFeedback : LREAL := 1.5;
    OvenFeedback : LREAL := 0.0;
    CouplingCommand : BOOL := FALSE;
    CouplingFeedback : BOOL := FALSE;
END_VAR

TankFeedback := TankCommand;
OvenFeedback := OvenCommand;
CouplingFeedback := CouplingCommand;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.TankCommand AT %QL0 : LREAL;
    P1.OvenCommand AT %QL8 : LREAL;
    P1.TankFeedback AT %IL0 : LREAL;
    P1.OvenFeedback AT %IL8 : LREAL;
    P1.CouplingCommand AT %QX16.0 : BOOL;
    P1.CouplingFeedback AT %IX16.0 : BOOL;
END_VAR
END_CONFIGURATION
