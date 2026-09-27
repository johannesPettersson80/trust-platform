USING Control;

(* FaultAwarePumpSupervisor qualifies redundant pressure, level, and drive
   feedback before selecting controller authority. It latches deterministic
   degraded modes and requires acknowledgement plus reviewed healthy evidence
   before recovery; no single diagnostic residual is treated as plant truth. *)
FUNCTION_BLOCK FaultAwarePumpSupervisor
VAR_INPUT
    PressureASampleReceived : BOOL;
    PressureBSampleReceived : BOOL;
    LevelSampleReceived : BOOL;
    DriveFeedbackReceived : BOOL;
    PressureAQualityGood : BOOL;
    PressureBQualityGood : BOOL;
    LevelQualityGood : BOOL;
    DriveFeedbackQualityGood : BOOL;
    AcknowledgeRequest : BOOL;
    ResetApproved : BOOL;
    Setpoint : LREAL;
    LocalTimeSeconds : LREAL;
    PressureASourceTime : LREAL;
    PressureBSourceTime : LREAL;
    LevelSourceTime : LREAL;
    DriveFeedbackSourceTime : LREAL;
    LevelRaw : LREAL;
    NaivePressureA : LREAL;
    PressureARaw : LREAL;
    PressureBRaw : LREAL;
    DriveFeedbackRaw : LREAL;
    NaiveEvidencePressure : LREAL;
END_VAR
VAR_OUTPUT
    PressureAResidual : LREAL;
    PressureBResidual : LREAL;
    PressureDiscrepancy : LREAL;
    PredictedPressure : LREAL := 6.0;
    ConservativeLevel : LREAL := 35.0;
    ActuatorTrackingResidual : LREAL;
    SelectedPressure : LREAL := 6.0;
    SelectedPressureSource : INT;
    OperatingMode : INT;
    FaultCode : INT;
    RecoveryHealthyCount : INT;
    PressureAIsolated : BOOL;
    PressureBIsolated : BOOL;
    FaultLatched : BOOL;
    FaultAcknowledged : BOOL;
    RecoveryReady : BOOL;
    NaiveRequested : LREAL := 50.0;
    ProtectedRequested : LREAL := 50.0;
    NaiveApplied : LREAL := 50.0;
    ProtectedApplied : LREAL := 50.0;
END_VAR
VAR
    PressureA : LREAL := 6.0;
    PressureB : LREAL := 6.0;
    LevelSample : LREAL := 35.0;
    DriveSpeedFeedback : LREAL := 50.0;
    HasPressureA : BOOL;
    HasPressureB : BOOL;
    HasLevel : BOOL;
    HasDriveFeedback : BOOL;
    PressureAAge : LREAL;
    PressureBAge : LREAL;
    LevelAge : LREAL;
    DriveFeedbackAge : LREAL;
    PressureAEligible : BOOL;
    PressureBEligible : BOOL;
    LevelEligible : BOOL;
    DriveFeedbackEligible : BOOL;
    SelectedPressureValid : BOOL := TRUE;
    DisagreementCount : INT;
    ActuatorMismatchCount : INT;
    DetectedFaultCode : INT;
    RecoveryInputValid : BOOL;
    PreviousProtectedApplied : LREAL := 50.0;
    LowerAppliedLimit : LREAL := 49.0;
    UpperAppliedLimit : LREAL := 51.0;
    ControllerInitialized : BOOL;
    NaiveController : Control.PID;
    ProtectedController : Control.PID;
    Parameters : Control.PIDParameters;
    InitialState : Control.PIDInitialState;
    NaiveInput : Control.PIDInput;
    ProtectedInput : Control.PIDInput;
END_VAR

IF NOT ControllerInitialized THEN
    Parameters.ProportionalGain := LREAL#10.0;
    Parameters.IntegralGain := LREAL#2.0;
    Parameters.DerivativeGain := LREAL#0.0;
    Parameters.Bias := LREAL#50.0;
    Parameters.ProportionalSetpointWeight := LREAL#1.0;
    Parameters.DerivativeSetpointWeight := LREAL#0.0;
    Parameters.OutputMinimum := LREAL#0.0;
    Parameters.OutputMaximum := LREAL#100.0;
    Parameters.RateMinimum := LREAL#-1000.0;
    Parameters.RateMaximum := LREAL#1000.0;
    Parameters.TrackingTime := LREAL#0.5;
    Parameters.AntiWindup := PIDAntiWindup#Combined;
    InitialState.Setpoint := Setpoint;
    InitialState.ProcessValue := LREAL#6.0;
    InitialState.NominalOutput := LREAL#50.0;
    InitialState.SaturatedOutput := LREAL#50.0;
    NaiveController.Configure(Parameters := Parameters);
    NaiveController.Initialize(State := InitialState);
    ProtectedController.Configure(Parameters := Parameters);
    ProtectedController.Initialize(State := InitialState);
    PredictedPressure := LREAL#6.0;
    PreviousProtectedApplied := LREAL#50.0;
    NaiveRequested := LREAL#50.0;
    NaiveApplied := LREAL#50.0;
    ProtectedRequested := LREAL#50.0;
    ProtectedApplied := LREAL#50.0;
    ControllerInitialized := TRUE;
END_IF;

(* Each device keeps its own sample time. Holding a value does not reset age. *)
IF PressureASampleReceived THEN
    PressureA := PressureARaw;
    HasPressureA := TRUE;
END_IF;
IF PressureBSampleReceived THEN
    PressureB := PressureBRaw;
    HasPressureB := TRUE;
END_IF;
IF LevelSampleReceived THEN
    LevelSample := LevelRaw;
    HasLevel := TRUE;
END_IF;
IF DriveFeedbackReceived THEN
    DriveSpeedFeedback := DriveFeedbackRaw;
    HasDriveFeedback := TRUE;
END_IF;

PressureAAge := LocalTimeSeconds - PressureASourceTime;
PressureBAge := LocalTimeSeconds - PressureBSourceTime;
LevelAge := LocalTimeSeconds - LevelSourceTime;
DriveFeedbackAge := LocalTimeSeconds - DriveFeedbackSourceTime;

PressureAEligible := HasPressureA AND PressureAQualityGood
    AND (PressureAAge >= LREAL#0.0) AND (PressureAAge <= LREAL#0.25)
    AND (PressureA >= LREAL#0.0) AND (PressureA <= LREAL#12.0);
PressureBEligible := HasPressureB AND PressureBQualityGood
    AND (PressureBAge >= LREAL#0.0) AND (PressureBAge <= LREAL#0.25)
    AND (PressureB >= LREAL#0.0) AND (PressureB <= LREAL#12.0);
LevelEligible := HasLevel AND LevelQualityGood
    AND (LevelAge >= LREAL#0.0) AND (LevelAge <= LREAL#1.0)
    AND (LevelSample >= LREAL#0.0) AND (LevelSample <= LREAL#100.0);
DriveFeedbackEligible := HasDriveFeedback AND DriveFeedbackQualityGood
    AND (DriveFeedbackAge >= LREAL#0.0) AND (DriveFeedbackAge <= LREAL#0.40)
    AND (DriveSpeedFeedback >= LREAL#0.0)
    AND (DriveSpeedFeedback <= LREAL#100.0);

(* This model-based relation is diagnostic redundancy, not a truth signal. *)
PredictedPressure := PredictedPressure + LREAL#0.025
    * (LREAL#4.0 + LREAL#0.04 * PreviousProtectedApplied
        - PredictedPressure);
PressureDiscrepancy := ABS(PressureA - PressureB);
PressureAResidual := ABS(PressureA - PredictedPressure);
PressureBResidual := ABS(PressureB - PredictedPressure);
ConservativeLevel := LevelSample - LREAL#0.5 * LevelAge;
ActuatorTrackingResidual := ABS(PreviousProtectedApplied - DriveSpeedFeedback);

IF PressureASampleReceived AND PressureBSampleReceived
    AND PressureAEligible AND PressureBEligible
    AND (PressureDiscrepancy > LREAL#0.30) THEN
    DisagreementCount := DisagreementCount + INT#1;
ELSE
    DisagreementCount := INT#0;
END_IF;

IF DisagreementCount >= INT#3 THEN
    IF (PressureAResidual > LREAL#0.50)
        AND (PressureBResidual <= LREAL#0.50) THEN
        PressureAIsolated := TRUE;
    ELSIF (PressureBResidual > LREAL#0.50)
        AND (PressureAResidual <= LREAL#0.50) THEN
        PressureBIsolated := TRUE;
    END_IF;
END_IF;

IF DriveFeedbackReceived AND DriveFeedbackEligible
    AND (ActuatorTrackingResidual > LREAL#5.0) THEN
    ActuatorMismatchCount := ActuatorMismatchCount + INT#1;
ELSIF DriveFeedbackReceived THEN
    ActuatorMismatchCount := INT#0;
END_IF;

SelectedPressureValid := FALSE;
SelectedPressureSource := INT#0;
IF PressureAEligible AND NOT PressureAIsolated THEN
    SelectedPressure := PressureA;
    SelectedPressureValid := TRUE;
    SelectedPressureSource := INT#1;
ELSIF PressureBEligible AND NOT PressureBIsolated THEN
    SelectedPressure := PressureB;
    SelectedPressureValid := TRUE;
    SelectedPressureSource := INT#2;
END_IF;

(* Stable priority keeps simultaneous failures reproducible and reviewable. *)
IF LevelEligible AND (ConservativeLevel < LREAL#20.0) THEN
    DetectedFaultCode := INT#5;
ELSIF NOT LevelEligible THEN
    DetectedFaultCode := INT#4;
ELSIF NOT SelectedPressureValid
    OR ((DisagreementCount >= INT#3)
        AND NOT PressureAIsolated AND NOT PressureBIsolated) THEN
    DetectedFaultCode := INT#3;
ELSIF ActuatorMismatchCount >= INT#3 THEN
    DetectedFaultCode := INT#6;
ELSIF PressureAIsolated OR PressureBIsolated THEN
    DetectedFaultCode := INT#1;
ELSE
    DetectedFaultCode := INT#0;
END_IF;

IF (DetectedFaultCode <> INT#0) AND NOT FaultLatched THEN
    FaultLatched := TRUE;
    FaultCode := DetectedFaultCode;
    FaultAcknowledged := FALSE;
    RecoveryHealthyCount := INT#0;
    RecoveryReady := FALSE;
END_IF;
IF AcknowledgeRequest AND FaultLatched THEN
    FaultAcknowledged := TRUE;
END_IF;

RecoveryInputValid := FALSE;
IF FaultCode = INT#1 THEN
    RecoveryInputValid := PressureAEligible AND PressureBEligible
        AND (PressureDiscrepancy <= LREAL#0.30)
        AND (PressureAResidual <= LREAL#0.50)
        AND (PressureBResidual <= LREAL#0.50);
ELSIF FaultCode = INT#3 THEN
    RecoveryInputValid := PressureAEligible AND PressureBEligible
        AND (PressureDiscrepancy <= LREAL#0.30);
ELSIF (FaultCode = INT#4) OR (FaultCode = INT#5) THEN
    RecoveryInputValid := LevelEligible
        AND (ConservativeLevel >= LREAL#30.0);
ELSIF FaultCode = INT#6 THEN
    RecoveryInputValid := DriveFeedbackEligible
        AND (ActuatorTrackingResidual <= LREAL#5.0);
END_IF;

IF FaultLatched AND RecoveryInputValid THEN
    IF RecoveryHealthyCount < INT#10 THEN
        RecoveryHealthyCount := RecoveryHealthyCount + INT#1;
    END_IF;
ELSE
    RecoveryHealthyCount := INT#0;
END_IF;
RecoveryReady := FaultLatched AND FaultAcknowledged
    AND (RecoveryHealthyCount >= INT#10);

IF ResetApproved AND RecoveryReady THEN
    FaultLatched := FALSE;
    FaultCode := INT#0;
    FaultAcknowledged := FALSE;
    RecoveryHealthyCount := INT#0;
    RecoveryReady := FALSE;
    PressureAIsolated := FALSE;
    PressureBIsolated := FALSE;
    DisagreementCount := INT#0;
    ActuatorMismatchCount := INT#0;
END_IF;

IF FaultLatched AND (FaultCode <> INT#1) THEN
    OperatingMode := INT#2;
ELSIF PressureAIsolated OR PressureBIsolated
    OR (PressureAEligible <> PressureBEligible) THEN
    OperatingMode := INT#1;
ELSE
    OperatingMode := INT#0;
END_IF;

NaiveInput.Setpoint := Setpoint;
NaiveInput.ProcessValue := NaivePressureA;
NaiveInput.CycleTime := LREAL#0.1;
NaiveInput.Mode := PIDMode#Auto;
NaiveRequested := NaiveController.Update(Input := NaiveInput);
NaiveApplied := LIMIT(LREAL#0.0, NaiveRequested, LREAL#100.0);

ProtectedInput.Setpoint := Setpoint;
ProtectedInput.ProcessValue := SelectedPressure;
ProtectedInput.CycleTime := LREAL#0.1;
IF OperatingMode = INT#2 THEN
    ProtectedInput.Mode := PIDMode#Track;
    ProtectedInput.TrackingOutput := LREAL#0.0;
ELSE
    ProtectedInput.Mode := PIDMode#Auto;
END_IF;
ProtectedRequested := ProtectedController.Update(Input := ProtectedInput);

IF OperatingMode = INT#2 THEN
    ProtectedRequested := LREAL#0.0;
    ProtectedApplied := LREAL#0.0;
ELSE
    LowerAppliedLimit := PreviousProtectedApplied - LREAL#1.0;
    UpperAppliedLimit := PreviousProtectedApplied + LREAL#1.0;
    ProtectedApplied := LIMIT(LowerAppliedLimit,
        LIMIT(LREAL#0.0, ProtectedRequested, LREAL#100.0),
        UpperAppliedLimit);
    ProtectedApplied := LIMIT(LREAL#0.0, ProtectedApplied, LREAL#100.0);
END_IF;
PreviousProtectedApplied := ProtectedApplied;
END_FUNCTION_BLOCK
