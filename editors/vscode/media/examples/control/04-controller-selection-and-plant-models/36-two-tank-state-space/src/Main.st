PROGRAM Main
VAR
    RequestedPumpFlow : LREAL := 0.1;
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
