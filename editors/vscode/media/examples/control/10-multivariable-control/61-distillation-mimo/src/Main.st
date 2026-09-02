PROGRAM Main
VAR
    AutoMode : BOOL := FALSE;
    RefluxRequested : LREAL := 0.0;
    ReboilerRequested : LREAL := 0.0;
    RefluxApplied : LREAL := 0.0;
    ReboilerApplied : LREAL := 0.0;
    OverheadPurityDeviation : LREAL := 0.0;
    BottomsPurityDeviation : LREAL := 0.0;
END_VAR

// This lesson identifies plant interaction, so no feedback controller is hidden in the test path.
IF NOT AutoMode THEN
    // Separate requested and applied values make the experiment's input limits observable.
    RefluxApplied := LIMIT(LREAL#-10.0, RefluxRequested, LREAL#10.0);
    ReboilerApplied := LIMIT(LREAL#-10.0, ReboilerRequested, LREAL#10.0);
ELSE
    RefluxApplied := LREAL#0.0;
    ReboilerApplied := LREAL#0.0;
END_IF;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.RefluxRequested AT %ML0 : LREAL;
    P1.ReboilerRequested AT %ML8 : LREAL;
    P1.AutoMode AT %MX16.0 : BOOL;
    P1.RefluxApplied AT %QL0 : LREAL;
    P1.ReboilerApplied AT %QL8 : LREAL;
    P1.OverheadPurityDeviation AT %IL0 : LREAL;
    P1.BottomsPurityDeviation AT %IL8 : LREAL;
END_VAR
END_CONFIGURATION
