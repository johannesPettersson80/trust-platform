PROGRAM Main
VAR
    Setpoint : LREAL := 2.0;
    ManualDemand : LREAL := 0.0;
    ProcessValue : LREAL := 4.0;
    ManipulatedValue : LREAL := 0.0;
    DoorOpenIndication : LREAL := 0.0;
    EffectiveSurroundings : LREAL := 4.0;
    ExperimentEnabled : BOOL := FALSE;
    SafetyAbort : BOOL := FALSE;
    InputsValid : BOOL := TRUE;
    ExperimentScan : ULINT := ULINT#0;
    ExperimentElapsedSeconds : LREAL := 0.0;
END_VAR

(* Control Studio grants one Boolean authority. The project owns the bounded
   cooling interval and its restoration; the simulator owns the door event. *)
IF ExperimentEnabled AND InputsValid AND NOT SafetyAbort THEN
    ExperimentScan := ExperimentScan + ULINT#1;
    ExperimentElapsedSeconds := ULINT_TO_LREAL(ExperimentScan) * LREAL#0.1;
    IF ExperimentScan <= ULINT#200 THEN
        ManualDemand := LREAL#150.0;
    ELSE
        ManualDemand := LREAL#0.0;
    END_IF;
ELSE
    ExperimentScan := ULINT#0;
    ExperimentElapsedSeconds := LREAL#0.0;
    ManualDemand := LREAL#0.0;
END_IF;

(* The clamp demonstrates that a calculated request and an applied MV are distinct. *)
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
    P1.ExperimentEnabled AT %MX24.0 : BOOL;
    P1.SafetyAbort AT %MX24.1 : BOOL;
    P1.InputsValid AT %MX24.2 : BOOL;
    P1.ExperimentElapsedSeconds AT %ML32 : LREAL;
END_VAR
END_CONFIGURATION
