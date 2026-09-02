PROGRAM Main
VAR
    ExperimentEnabled : BOOL := FALSE;
    SafetyAbort : BOOL := FALSE;
    InputsValid : BOOL := TRUE;
    LocalRequested : LREAL := 20.0;
    LoadRequested : LREAL := 20.0;
    HighFlowRequested : LREAL := 20.0;
    LocalApplied : LREAL := 20.0;
    LoadApplied : LREAL := 20.0;
    HighFlowApplied : LREAL := 20.0;
    LocalTemperature : LREAL := 50.0;
    LoadTemperature : LREAL := 50.0;
    HighFlowTemperature : LREAL := 50.0;
END_VAR

(* All three holdout records receive the same reviewed input. Their different
   outcomes therefore come from the declared plant condition, not the command. *)
InputsValid := LocalTemperature >= LREAL#45.0
    AND LocalTemperature <= LREAL#56.0
    AND LoadTemperature >= LREAL#45.0 AND LoadTemperature <= LREAL#56.0
    AND HighFlowTemperature >= LREAL#45.0
    AND HighFlowTemperature <= LREAL#56.0;

IF ExperimentEnabled AND InputsValid AND NOT SafetyAbort THEN
    LocalRequested := LREAL#25.0;
    LoadRequested := LREAL#25.0;
    HighFlowRequested := LREAL#25.0;
ELSE
    LocalRequested := LREAL#20.0;
    LoadRequested := LREAL#20.0;
    HighFlowRequested := LREAL#20.0;
END_IF;

(* Requested and applied positions stay distinct so later valve constraints
   cannot be mistaken for model error. *)
IF LocalRequested < LREAL#0.0 THEN
    LocalApplied := LREAL#0.0;
ELSIF LocalRequested > LREAL#100.0 THEN
    LocalApplied := LREAL#100.0;
ELSE
    LocalApplied := LocalRequested;
END_IF;

IF LoadRequested < LREAL#0.0 THEN
    LoadApplied := LREAL#0.0;
ELSIF LoadRequested > LREAL#100.0 THEN
    LoadApplied := LREAL#100.0;
ELSE
    LoadApplied := LoadRequested;
END_IF;

IF HighFlowRequested < LREAL#0.0 THEN
    HighFlowApplied := LREAL#0.0;
ELSIF HighFlowRequested > LREAL#100.0 THEN
    HighFlowApplied := LREAL#100.0;
ELSE
    HighFlowApplied := HighFlowRequested;
END_IF;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.ExperimentEnabled AT %MX64.0 : BOOL;
    P1.SafetyAbort AT %MX64.1 : BOOL;
    P1.InputsValid AT %MX64.2 : BOOL;
    P1.LocalRequested AT %ML0 : LREAL;
    P1.LoadRequested AT %ML8 : LREAL;
    P1.HighFlowRequested AT %ML16 : LREAL;
    P1.LocalApplied AT %QL0 : LREAL;
    P1.LoadApplied AT %QL8 : LREAL;
    P1.HighFlowApplied AT %QL16 : LREAL;
    P1.LocalTemperature AT %IL0 : LREAL;
    P1.LoadTemperature AT %IL8 : LREAL;
    P1.HighFlowTemperature AT %IL16 : LREAL;
END_VAR
END_CONFIGURATION
