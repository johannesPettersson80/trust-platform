PROGRAM Main
VAR
    Enabled : BOOL := FALSE;
    ControllerValid : BOOL := TRUE;
    Saturated : BOOL := FALSE;
    DesignMode : DINT := 3;
    ForceRequested : LREAL := 0.0;
    EstimatedDeflection : LREAL := 0.0;
    EstimatedVelocity : LREAL := 0.0;
    Innovation : LREAL := 0.0;
    RoadAccelerationRequested : LREAL := 0.0;
    PreviousForce : LREAL := 0.0;
    PreviousRoad : LREAL := 0.0;
    ForceApplied : LREAL := 0.0;
    RoadAccelerationApplied : LREAL := 0.0;
    FullStateDeflectionMeasurement : LREAL := 0.0;
    FullStateVelocityMeasurement : LREAL := 0.0;
    PositionOnlyMeasurement : LREAL := 0.0;
    PositionOnlyVelocityTruth : LREAL := 0.0;
    PredictedDeflection : LREAL := 0.0;
    PredictedVelocity : LREAL := 0.0;
    FeedbackDeflection : LREAL := 0.0;
    FeedbackVelocity : LREAL := 0.0;
    GainDeflection : LREAL := 0.0;
    GainVelocity : LREAL := 0.0;
    ExperimentEnabled : BOOL := FALSE;
    SafetyAbort : BOOL := FALSE;
    InputsValid : BOOL := TRUE;
    ExperimentElapsed : LREAL := 0.0;
END_VAR

(* Control Studio grants only authority. The project owns the balanced LQG road
   pulse; the alternative regulators and faults remain native test records. *)
IF ExperimentEnabled AND InputsValid AND NOT SafetyAbort THEN
    ExperimentElapsed := ExperimentElapsed + LREAL#0.1;
    Enabled := TRUE;
    DesignMode := DINT#5;
    IF (ExperimentElapsed >= LREAL#1.0)
        AND (ExperimentElapsed < LREAL#1.5) THEN
        RoadAccelerationRequested := LREAL#0.5;
    ELSE
        RoadAccelerationRequested := LREAL#0.0;
    END_IF;
ELSE
    ExperimentElapsed := LREAL#0.0;
    Enabled := FALSE;
    DesignMode := DINT#5;
    RoadAccelerationRequested := LREAL#0.0;
END_IF;

ControllerValid := (DesignMode >= DINT#0)
    AND (DesignMode <= DINT#5)
    AND (RoadAccelerationRequested >= LREAL#-2.0)
    AND (RoadAccelerationRequested <= LREAL#2.0)
    AND (EstimatedDeflection >= LREAL#-0.08)
    AND (EstimatedDeflection <= LREAL#0.08)
    AND (EstimatedVelocity >= LREAL#-0.5)
    AND (EstimatedVelocity <= LREAL#0.5);

IF DesignMode = DINT#5 THEN
    ControllerValid := ControllerValid
        AND (PositionOnlyMeasurement >= LREAL#-0.10)
        AND (PositionOnlyMeasurement <= LREAL#0.10);
ELSE
    ControllerValid := ControllerValid
        AND (FullStateDeflectionMeasurement >= LREAL#-0.08)
        AND (FullStateDeflectionMeasurement <= LREAL#0.08)
        AND (FullStateVelocityMeasurement >= LREAL#-0.5)
        AND (FullStateVelocityMeasurement <= LREAL#0.5);
END_IF;

IF ControllerValid THEN
    // Exact 0.1 s zero-order-hold model used by the position-only estimator.
    PredictedDeflection := LREAL#0.98084004 * EstimatedDeflection
        + LREAL#0.09360616 * EstimatedVelocity
        + LREAL#0.00239499 * PreviousForce
        + LREAL#0.00478999 * PreviousRoad;
    PredictedVelocity := LREAL#-0.37442462 * EstimatedDeflection
        + LREAL#0.86851265 * EstimatedVelocity
        + LREAL#0.04680308 * PreviousForce
        + LREAL#0.09360616 * PreviousRoad;
    Innovation := PositionOnlyMeasurement - PredictedDeflection;
    // Steady-state Kalman gain is independently qualified by the lesson test.
    EstimatedDeflection := PredictedDeflection
        + LREAL#0.58488259 * Innovation;
    EstimatedVelocity := PredictedVelocity
        + LREAL#2.24682847 * Innovation;

    IF DesignMode = DINT#5 THEN
        FeedbackDeflection := EstimatedDeflection;
        FeedbackVelocity := EstimatedVelocity;
    ELSE
        FeedbackDeflection := FullStateDeflectionMeasurement;
        FeedbackVelocity := FullStateVelocityMeasurement;
    END_IF;

    IF DesignMode = DINT#0 THEN
        GainDeflection := LREAL#13.0;
        GainVelocity := LREAL#6.0;
    ELSIF DesignMode = DINT#1 THEN
        GainDeflection := LREAL#1.16515139;
        GainVelocity := LREAL#3.11548779;
    ELSIF DesignMode = DINT#2 THEN
        GainDeflection := LREAL#5.56465997;
        GainVelocity := LREAL#3.07892689;
    ELSIF (DesignMode = DINT#3) OR (DesignMode = DINT#5) THEN
        GainDeflection := LREAL#2.19803903;
        GainVelocity := LREAL#1.90722139;
    ELSE
        GainDeflection := LREAL#0.48528137;
        GainVelocity := LREAL#0.51566896;
    END_IF;

    ForceRequested := -GainDeflection * FeedbackDeflection
        - GainVelocity * FeedbackVelocity;
    Saturated := (ForceRequested < LREAL#-5.0)
        OR (ForceRequested > LREAL#5.0);
    RoadAccelerationApplied := LIMIT(
        LREAL#-2.0,
        RoadAccelerationRequested,
        LREAL#2.0
    );
    IF Enabled THEN
        ForceApplied := LIMIT(LREAL#-5.0, ForceRequested, LREAL#5.0);
    ELSE
        ForceApplied := LREAL#0.0;
    END_IF;
ELSE
    ForceRequested := LREAL#0.0;
    ForceApplied := LREAL#0.0;
    RoadAccelerationApplied := LREAL#0.0;
    Saturated := FALSE;
END_IF;

PreviousForce := ForceApplied;
PreviousRoad := RoadAccelerationApplied;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.DesignMode AT %MD0 : DINT;
    P1.ForceRequested AT %ML8 : LREAL;
    P1.EstimatedDeflection AT %ML16 : LREAL;
    P1.EstimatedVelocity AT %ML24 : LREAL;
    P1.Innovation AT %ML32 : LREAL;
    P1.RoadAccelerationRequested AT %ML40 : LREAL;
    P1.PreviousForce AT %ML48 : LREAL;
    P1.PreviousRoad AT %ML56 : LREAL;
    P1.Enabled AT %MX120.0 : BOOL;
    P1.ControllerValid AT %MX120.1 : BOOL;
    P1.Saturated AT %MX120.2 : BOOL;
    P1.ExperimentEnabled AT %MX120.3 : BOOL;
    P1.SafetyAbort AT %MX120.4 : BOOL;
    P1.InputsValid AT %MX120.5 : BOOL;
    P1.ForceApplied AT %QL0 : LREAL;
    P1.RoadAccelerationApplied AT %QL8 : LREAL;
    P1.FullStateDeflectionMeasurement AT %IL0 : LREAL;
    P1.FullStateVelocityMeasurement AT %IL8 : LREAL;
    P1.PositionOnlyMeasurement AT %IL16 : LREAL;
    P1.PositionOnlyVelocityTruth AT %IL24 : LREAL;
END_VAR
END_CONFIGURATION
