PROGRAM Main
VAR
    ExperimentEnabled : BOOL := FALSE;
    PoorRequested : LREAL := 20.0;
    QualifiedRequested : LREAL := 20.0;
    ClosedRequested : LREAL := 20.0;
    PoorApplied : LREAL := 20.0;
    QualifiedApplied : LREAL := 20.0;
    ClosedApplied : LREAL := 20.0;
    PoorTemperature : LREAL := 50.0;
    QualifiedTemperature : LREAL := 50.0;
    ClosedTemperature : LREAL := 50.0;
    ClosedMeasurementUsed : LREAL := 50.0;
END_VAR

(* Retain the exact noisy temperature used by feedback. This makes the
   controller-created input/noise relationship visible to the learner. *)
ClosedMeasurementUsed := ClosedTemperature;

IF ExperimentEnabled THEN
    PoorRequested := LREAL#20.2;
    QualifiedRequested := LREAL#30.0;
    ClosedRequested := 20.0 + 2.0 * (50.0 - ClosedMeasurementUsed);
ELSE
    PoorRequested := LREAL#20.0;
    QualifiedRequested := LREAL#20.0;
    ClosedRequested := LREAL#20.0;
END_IF;

(* Requested and applied commands stay separate even though these simulated
   valves only impose a 0-to-100-percent position limit. *)
IF PoorRequested < LREAL#0.0 THEN
    PoorApplied := LREAL#0.0;
ELSIF PoorRequested > LREAL#100.0 THEN
    PoorApplied := LREAL#100.0;
ELSE
    PoorApplied := PoorRequested;
END_IF;

IF QualifiedRequested < LREAL#0.0 THEN
    QualifiedApplied := LREAL#0.0;
ELSIF QualifiedRequested > LREAL#100.0 THEN
    QualifiedApplied := LREAL#100.0;
ELSE
    QualifiedApplied := QualifiedRequested;
END_IF;

IF ClosedRequested < LREAL#0.0 THEN
    ClosedApplied := LREAL#0.0;
ELSIF ClosedRequested > LREAL#100.0 THEN
    ClosedApplied := LREAL#100.0;
ELSE
    ClosedApplied := ClosedRequested;
END_IF;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.ExperimentEnabled AT %MX64.0 : BOOL;
    P1.PoorRequested AT %ML0 : LREAL;
    P1.QualifiedRequested AT %ML8 : LREAL;
    P1.ClosedRequested AT %ML16 : LREAL;
    P1.ClosedMeasurementUsed AT %ML24 : LREAL;
    P1.PoorApplied AT %QL0 : LREAL;
    P1.QualifiedApplied AT %QL8 : LREAL;
    P1.ClosedApplied AT %QL16 : LREAL;
    P1.PoorTemperature AT %IL0 : LREAL;
    P1.QualifiedTemperature AT %IL8 : LREAL;
    P1.ClosedTemperature AT %IL16 : LREAL;
END_VAR
END_CONFIGURATION
