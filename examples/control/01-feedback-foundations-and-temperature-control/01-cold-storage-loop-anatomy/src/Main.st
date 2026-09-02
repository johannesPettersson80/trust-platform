PROGRAM Main
VAR
    Setpoint : LREAL := 2.0;
    ManualDemand : LREAL := 0.0;
    ProcessValue : LREAL := 4.0;
    ManipulatedValue : LREAL := 0.0;
    DoorOpenIndication : LREAL := 0.0;
    EffectiveSurroundings : LREAL := 4.0;
END_VAR

(* Manual demand is intentionally the only control action in Lesson 1. The
   clamp demonstrates that an operator command and an applied MV are distinct. *)
ManipulatedValue := MIN(MAX(ManualDemand, LREAL#0.0), LREAL#100.0);
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.Setpoint AT %ML0 : LREAL;
    P1.ManualDemand AT %ML8 : LREAL;
    P1.ManipulatedValue AT %QL0 : LREAL;
    P1.ProcessValue AT %IL0 : LREAL;
    P1.DoorOpenIndication AT %IL8 : LREAL;
    P1.EffectiveSurroundings AT %IL16 : LREAL;
END_VAR
END_CONFIGURATION
