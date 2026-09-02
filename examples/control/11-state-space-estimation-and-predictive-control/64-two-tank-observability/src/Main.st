PROGRAM Main
VAR
    ActuatorLayout : INT := 0;
    SensorLayout : INT := 0;
    ExperimentEnabled : BOOL := FALSE;
    SafetyAbort : BOOL := FALSE;
    InputsValid : BOOL := TRUE;
    ExperimentElapsed : LREAL := 0.0;
    FlowRequested : LREAL := 0.0;
    FlowApplied : LREAL := 0.0;
    UpstreamPumpApplied : LREAL := 0.0;
    BypassPumpApplied : LREAL := 0.0;
    Tank1Truth : LREAL := 0.0;
    Tank2Truth : LREAL := 0.0;
    SelectedMeasurement : LREAL := 0.0;
END_VAR

(* The visible case checks the qualified upstream-pump/downstream-sensor
   layout. Control Studio grants only authority; the project owns the step. *)
IF ExperimentEnabled AND InputsValid AND NOT SafetyAbort THEN
    ExperimentElapsed := ExperimentElapsed + LREAL#0.1;
    ActuatorLayout := 0;
    SensorLayout := 0;
    IF ExperimentElapsed < LREAL#20.0 THEN
        FlowRequested := LREAL#0.0;
    ELSIF ExperimentElapsed < LREAL#200.0 THEN
        FlowRequested := LREAL#1.0;
    ELSE
        FlowRequested := LREAL#0.0;
    END_IF;
ELSE
    ExperimentElapsed := LREAL#0.0;
    ActuatorLayout := 0;
    SensorLayout := 0;
    FlowRequested := LREAL#0.0;
END_IF;

FlowApplied := LIMIT(LREAL#-2.0, FlowRequested, LREAL#2.0);
(* Layout 0 drives the upstream pump; layout 1 drives the downstream bypass.
   The unselected output stays at zero so one actuator owns the flow request. *)
IF ActuatorLayout = 1 THEN
    UpstreamPumpApplied := LREAL#0.0;
    BypassPumpApplied := FlowApplied;
ELSE
    UpstreamPumpApplied := FlowApplied;
    BypassPumpApplied := LREAL#0.0;
END_IF;

(* Sensor layout 0 exposes Tank 2; layout 1 exposes Tank 1. Only the selected
   measurement is controller-facing; both truth channels remain diagnostics. *)
IF SensorLayout = 1 THEN
    SelectedMeasurement := Tank1Truth;
ELSE
    SelectedMeasurement := Tank2Truth;
END_IF;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.ActuatorLayout AT %MW0 : INT;
    P1.SensorLayout AT %MW2 : INT;
    P1.ExperimentEnabled AT %MX32.0 : BOOL;
    P1.SafetyAbort AT %MX32.1 : BOOL;
    P1.InputsValid AT %MX32.2 : BOOL;
    P1.FlowRequested AT %ML8 : LREAL;
    P1.SelectedMeasurement AT %ML16 : LREAL;
    P1.FlowApplied AT %ML24 : LREAL;
    P1.UpstreamPumpApplied AT %QL0 : LREAL;
    P1.BypassPumpApplied AT %QL8 : LREAL;
    P1.Tank1Truth AT %IL0 : LREAL;
    P1.Tank2Truth AT %IL8 : LREAL;
END_VAR
END_CONFIGURATION
