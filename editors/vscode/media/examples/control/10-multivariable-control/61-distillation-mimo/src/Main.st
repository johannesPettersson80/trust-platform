PROGRAM Main
VAR
    AutoMode : BOOL := FALSE;
    ExperimentEnabled : BOOL := FALSE;
    SafetyAbort : BOOL := FALSE;
    InputsValid : BOOL := TRUE;
    ExperimentElapsed : LREAL := 0.0;
    RefluxRequested : LREAL := 0.0;
    ReboilerRequested : LREAL := 0.0;
    RefluxApplied : LREAL := 0.0;
    ReboilerApplied : LREAL := 0.0;
    OverheadPurityDeviation : LREAL := 0.0;
    BottomsPurityDeviation : LREAL := 0.0;
END_VAR

(* One observation contains two independently excited windows separated by a
   restored baseline. Control Studio grants only Boolean authority. *)
IF ExperimentEnabled AND InputsValid AND NOT SafetyAbort THEN
    ExperimentElapsed := ExperimentElapsed + LREAL#0.1;
    AutoMode := FALSE;
    IF ExperimentElapsed < LREAL#20.0 THEN
        RefluxRequested := LREAL#0.0;
        ReboilerRequested := LREAL#0.0;
    ELSIF ExperimentElapsed < LREAL#200.0 THEN
        RefluxRequested := LREAL#5.0;
        ReboilerRequested := LREAL#0.0;
    ELSIF ExperimentElapsed < LREAL#220.0 THEN
        RefluxRequested := LREAL#0.0;
        ReboilerRequested := LREAL#0.0;
    ELSE
        RefluxRequested := LREAL#0.0;
        ReboilerRequested := LREAL#5.0;
    END_IF;
ELSE
    ExperimentElapsed := LREAL#0.0;
    AutoMode := FALSE;
    RefluxRequested := LREAL#0.0;
    ReboilerRequested := LREAL#0.0;
END_IF;

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
    P1.ExperimentEnabled AT %MX16.1 : BOOL;
    P1.SafetyAbort AT %MX16.2 : BOOL;
    P1.InputsValid AT %MX16.3 : BOOL;
    P1.RefluxApplied AT %QL0 : LREAL;
    P1.ReboilerApplied AT %QL8 : LREAL;
    P1.OverheadPurityDeviation AT %IL0 : LREAL;
    P1.BottomsPurityDeviation AT %IL8 : LREAL;
END_VAR
END_CONFIGURATION
