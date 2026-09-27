FUNCTION Lesson50ReviewedSetpoint : LREAL
VAR_INPUT
    ExperimentEnabled : BOOL;
    InputsValid : BOOL;
    SafetyAbort : BOOL;
END_VAR
IF ExperimentEnabled AND InputsValid AND NOT SafetyAbort THEN
    Lesson50ReviewedSetpoint := LREAL#1.0;
ELSE
    Lesson50ReviewedSetpoint := LREAL#0.0;
END_IF;
END_FUNCTION

FUNCTION Lesson50VelocityDecay : LREAL
VAR_INPUT
    SampleSeconds : LREAL;
END_VAR
Lesson50VelocityDecay := EXP(LREAL#-2.0 * SampleSeconds);
END_FUNCTION

FUNCTION Lesson50PositionInputCoefficient : LREAL
VAR_INPUT
    SampleSeconds : LREAL;
END_VAR
Lesson50PositionInputCoefficient := LREAL#5.0 * SampleSeconds
    - LREAL#2.5 * (LREAL#1.0
        - Lesson50VelocityDecay(SampleSeconds := SampleSeconds));
END_FUNCTION

FUNCTION Lesson50VelocityInputCoefficient : LREAL
VAR_INPUT
    SampleSeconds : LREAL;
END_VAR
Lesson50VelocityInputCoefficient := LREAL#5.0
    * (LREAL#1.0 - Lesson50VelocityDecay(SampleSeconds := SampleSeconds));
END_FUNCTION

FUNCTION Lesson50NyquistAngularFrequency : LREAL
VAR_INPUT
    SampleSeconds : LREAL;
END_VAR
Lesson50NyquistAngularFrequency := LREAL#3.141592653589793 / SampleSeconds;
END_FUNCTION

PROGRAM Main
VAR
    ExperimentEnabled : BOOL := FALSE;
    SafetyAbort : BOOL := FALSE;
    InputsValid : BOOL := TRUE;
    FineAborted : BOOL := FALSE;
    CoarseAborted : BOOL := FALSE;
    CorrectedAborted : BOOL := FALSE;
    FineSampleSeconds : LREAL := 0.01;
    CoarseSampleSeconds : LREAL := 0.1;
    ContinuousProportionalGain : LREAL := 4.0;
    ContinuousDerivativeGain : LREAL := 0.4;
    CorrectedProportionalGain : LREAL := 1.0;
    CorrectedDerivativeGain : LREAL := 0.4;
    ForceMinimum : LREAL := -10.0;
    ForceMaximum : LREAL := 10.0;
    PositionSetpoint : LREAL := 0.0;
    FinePosition : LREAL := 0.0;
    FineVelocity : LREAL := 0.0;
    CoarsePosition : LREAL := 0.0;
    CoarseVelocity : LREAL := 0.0;
    CorrectedPosition : LREAL := 0.0;
    CorrectedVelocity : LREAL := 0.0;
    FineRequestedForce : LREAL := 0.0;
    FinePendingForce : LREAL := 0.0;
    FineAppliedForce : LREAL := 0.0;
    CoarseRequestedForce : LREAL := 0.0;
    CoarsePendingForce : LREAL := 0.0;
    CoarseAppliedForce : LREAL := 0.0;
    CorrectedRequestedForce : LREAL := 0.0;
    CorrectedPendingForce : LREAL := 0.0;
    CorrectedAppliedForce : LREAL := 0.0;
    CoarseCounter : DINT := 0;
    CorrectedCounter : DINT := 0;
END_VAR

PositionSetpoint := Lesson50ReviewedSetpoint(
    ExperimentEnabled := ExperimentEnabled,
    InputsValid := InputsValid,
    SafetyAbort := SafetyAbort
);

IF ExperimentEnabled AND (NOT SafetyAbort) AND InputsValid THEN
    IF FineAborted THEN
        FineAppliedForce := MAX(MIN(-LREAL#0.8 * FineVelocity, ForceMaximum), ForceMinimum);
    ELSE
        // Apply the preceding request before calculating the next one. This is
        // the explicit one-sample computation and actuation delay.
        FineAppliedForce := FinePendingForce;
        FineRequestedForce := ContinuousProportionalGain * (PositionSetpoint - FinePosition)
            - ContinuousDerivativeGain * FineVelocity;
        IF (FineRequestedForce < ForceMinimum) OR (FineRequestedForce > ForceMaximum)
            OR (FinePosition < LREAL#-5.0) OR (FinePosition > LREAL#5.0) THEN
            FineAborted := TRUE;
            FinePendingForce := LREAL#0.0;
        ELSE
            FinePendingForce := FineRequestedForce;
        END_IF;
    END_IF;

    CoarseCounter := CoarseCounter + 1;
    IF CoarseAborted THEN
        CoarseAppliedForce := MAX(MIN(-LREAL#0.8 * CoarseVelocity, ForceMaximum), ForceMinimum);
    ELSIF CoarseCounter >= 10 THEN
        CoarseCounter := 0;
        CoarseAppliedForce := CoarsePendingForce;
        CoarseRequestedForce := ContinuousProportionalGain * (PositionSetpoint - CoarsePosition)
            - ContinuousDerivativeGain * CoarseVelocity;
        IF (CoarseRequestedForce < ForceMinimum) OR (CoarseRequestedForce > ForceMaximum)
            OR (CoarsePosition < LREAL#-5.0) OR (CoarsePosition > LREAL#5.0) THEN
            CoarseAborted := TRUE;
            CoarsePendingForce := LREAL#0.0;
            CoarseAppliedForce := MAX(MIN(-LREAL#0.8 * CoarseVelocity, ForceMaximum), ForceMinimum);
        ELSE
            CoarsePendingForce := CoarseRequestedForce;
        END_IF;
    END_IF;

    CorrectedCounter := CorrectedCounter + 1;
    IF CorrectedAborted THEN
        CorrectedAppliedForce := MAX(MIN(-LREAL#0.8 * CorrectedVelocity, ForceMaximum), ForceMinimum);
    ELSIF CorrectedCounter >= 10 THEN
        CorrectedCounter := 0;
        CorrectedAppliedForce := CorrectedPendingForce;
        CorrectedRequestedForce := CorrectedProportionalGain * (PositionSetpoint - CorrectedPosition)
            - CorrectedDerivativeGain * CorrectedVelocity;
        IF (CorrectedRequestedForce < ForceMinimum) OR (CorrectedRequestedForce > ForceMaximum)
            OR (CorrectedPosition < LREAL#-5.0) OR (CorrectedPosition > LREAL#5.0) THEN
            CorrectedAborted := TRUE;
            CorrectedPendingForce := LREAL#0.0;
        ELSE
            CorrectedPendingForce := CorrectedRequestedForce;
        END_IF;
    END_IF;
ELSE
    FineAborted := FALSE;
    CoarseAborted := FALSE;
    CorrectedAborted := FALSE;
    FineRequestedForce := LREAL#0.0;
    FinePendingForce := LREAL#0.0;
    FineAppliedForce := LREAL#0.0;
    CoarseRequestedForce := LREAL#0.0;
    CoarsePendingForce := LREAL#0.0;
    CoarseAppliedForce := LREAL#0.0;
    CorrectedRequestedForce := LREAL#0.0;
    CorrectedPendingForce := LREAL#0.0;
    CorrectedAppliedForce := LREAL#0.0;
    CoarseCounter := 0;
    CorrectedCounter := 0;
END_IF;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.ExperimentEnabled AT %MX96.0 : BOOL;
    P1.SafetyAbort AT %MX96.1 : BOOL;
    P1.InputsValid AT %MX96.2 : BOOL;
    P1.FineAborted AT %MX97.0 : BOOL;
    P1.CoarseAborted AT %MX97.1 : BOOL;
    P1.CorrectedAborted AT %MX97.2 : BOOL;
    P1.FineRequestedForce AT %ML0 : LREAL;
    P1.FinePendingForce AT %ML8 : LREAL;
    P1.CoarseRequestedForce AT %ML16 : LREAL;
    P1.CoarsePendingForce AT %ML24 : LREAL;
    P1.CorrectedRequestedForce AT %ML32 : LREAL;
    P1.CorrectedPendingForce AT %ML40 : LREAL;
    P1.PositionSetpoint AT %ML48 : LREAL;
    P1.FineAppliedForce AT %QL0 : LREAL;
    P1.CoarseAppliedForce AT %QL8 : LREAL;
    P1.CorrectedAppliedForce AT %QL16 : LREAL;
    P1.FinePosition AT %IL0 : LREAL;
    P1.FineVelocity AT %IL8 : LREAL;
    P1.CoarsePosition AT %IL16 : LREAL;
    P1.CoarseVelocity AT %IL24 : LREAL;
    P1.CorrectedPosition AT %IL32 : LREAL;
    P1.CorrectedVelocity AT %IL40 : LREAL;
END_VAR
END_CONFIGURATION
