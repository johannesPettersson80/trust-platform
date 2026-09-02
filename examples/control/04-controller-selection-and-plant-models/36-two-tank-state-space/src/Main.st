PROGRAM Main
VAR
    ExperimentEnabled : BOOL := FALSE;
    SafetyAbort : BOOL := FALSE;
    InputsValid : BOOL := TRUE;
    RequestedPumpFlow : LREAL := 0.0;
    MainPumpFlow : LREAL := 0.0;
    EmptyCasePumpFlow : LREAL := 0.0;
    ChargedCasePumpFlow : LREAL := 0.0;

    MainUpstreamLevel : LREAL := 0.0;
    MainDownstreamLevel : LREAL := 0.0;
    EmptyUpstreamLevel : LREAL := 0.0;
    EmptyDownstreamLevel : LREAL := 0.0;
    ChargedUpstreamLevel : LREAL := 0.2;
    ChargedDownstreamLevel : LREAL := 0.0;
END_VAR

IF ExperimentEnabled THEN
    RequestedPumpFlow := LREAL#0.1;
ELSE
    RequestedPumpFlow := LREAL#0.0;
END_IF;

InputsValid := MainUpstreamLevel >= LREAL#0.0
    AND MainUpstreamLevel <= LREAL#1.0
    AND MainDownstreamLevel >= LREAL#0.0
    AND MainDownstreamLevel <= LREAL#1.0
    AND EmptyUpstreamLevel >= LREAL#0.0
    AND EmptyUpstreamLevel <= LREAL#1.0
    AND EmptyDownstreamLevel >= LREAL#0.0
    AND EmptyDownstreamLevel <= LREAL#1.0
    AND ChargedUpstreamLevel >= LREAL#0.0
    AND ChargedUpstreamLevel <= LREAL#1.0
    AND ChargedDownstreamLevel >= LREAL#0.0
    AND ChargedDownstreamLevel <= LREAL#1.0;
IF SafetyAbort OR NOT InputsValid THEN
    RequestedPumpFlow := LREAL#0.0;
END_IF;

IF RequestedPumpFlow < LREAL#0.0 THEN
    MainPumpFlow := LREAL#0.0;
ELSIF RequestedPumpFlow > LREAL#0.2 THEN
    MainPumpFlow := LREAL#0.2;
ELSE
    MainPumpFlow := RequestedPumpFlow;
END_IF;

(* The hidden-state comparison uses identical zero pump inputs. *)
EmptyCasePumpFlow := LREAL#0.0;
ChargedCasePumpFlow := LREAL#0.0;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.ExperimentEnabled AT %MX8.0 : BOOL;
    P1.SafetyAbort AT %MX8.1 : BOOL;
    P1.InputsValid AT %MX8.2 : BOOL;
    P1.RequestedPumpFlow AT %ML0 : LREAL;
    P1.MainPumpFlow AT %QL0 : LREAL;
    P1.EmptyCasePumpFlow AT %QL8 : LREAL;
    P1.ChargedCasePumpFlow AT %QL16 : LREAL;
    P1.MainUpstreamLevel AT %IL0 : LREAL;
    P1.MainDownstreamLevel AT %IL8 : LREAL;
    P1.EmptyUpstreamLevel AT %IL16 : LREAL;
    P1.EmptyDownstreamLevel AT %IL24 : LREAL;
    P1.ChargedUpstreamLevel AT %IL32 : LREAL;
    P1.ChargedDownstreamLevel AT %IL40 : LREAL;
END_VAR
END_CONFIGURATION
