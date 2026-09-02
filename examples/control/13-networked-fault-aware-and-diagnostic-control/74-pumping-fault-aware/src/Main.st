USING Control;

PROGRAM Main
VAR
    PressureASampleReceived : BOOL := TRUE;
    PressureBSampleReceived : BOOL := TRUE;
    LevelSampleReceived : BOOL := TRUE;
    DriveFeedbackReceived : BOOL := TRUE;
    PressureAQualityGood : BOOL := TRUE;
    PressureBQualityGood : BOOL := TRUE;
    LevelQualityGood : BOOL := TRUE;
    DriveFeedbackQualityGood : BOOL := TRUE;
    AcknowledgeRequest : BOOL := FALSE;
    ResetApproved : BOOL := FALSE;
    ExperimentEnabled : BOOL := FALSE;
    ExperimentWasEnabled : BOOL := FALSE;
    SafetyAbort : BOOL := FALSE;
    InputsValid : BOOL := TRUE;
    ExperimentScan : ULINT := ULINT#0;
    ExperimentElapsed : LREAL := 0.0;
    HeldDriveFeedback : LREAL := 50.0;
    Setpoint : LREAL := 6.0;
    LocalTimeSeconds : LREAL := 0.0;
    PressureASourceTime : LREAL := 0.0;
    PressureBSourceTime : LREAL := 0.0;
    LevelSourceTime : LREAL := 0.0;
    DriveFeedbackSourceTime : LREAL := 0.0;
    LevelRaw : LREAL := 35.0;
    NaivePressureA : LREAL := 6.0;
    PressureARaw : LREAL := 6.0;
    PressureBRaw : LREAL := 6.0;
    DriveFeedbackRaw : LREAL := 50.0;
    NaiveEvidencePressure : LREAL := 6.0;
    PressureAResidual : LREAL := 0.0;
    PressureBResidual : LREAL := 0.0;
    PressureDiscrepancy : LREAL := 0.0;
    PredictedPressure : LREAL := 6.0;
    ConservativeLevel : LREAL := 35.0;
    ActuatorTrackingResidual : LREAL := 0.0;
    SelectedPressure : LREAL := 6.0;
    SelectedPressureSource : INT := 0;
    OperatingMode : INT := 0;
    FaultCode : INT := 0;
    RecoveryHealthyCount : INT := 0;
    PressureAIsolated : BOOL := FALSE;
    PressureBIsolated : BOOL := FALSE;
    FaultLatched : BOOL := FALSE;
    FaultAcknowledged : BOOL := FALSE;
    RecoveryReady : BOOL := FALSE;
    NaiveRequested : LREAL := 50.0;
    ProtectedRequested : LREAL := 50.0;
    NaiveApplied : LREAL := 50.0;
    ProtectedApplied : LREAL := 50.0;
    Supervisor : FaultAwarePumpSupervisor;
END_VAR

(* Control Studio writes only ExperimentEnabled. The project owns the complete
   healthy, fault, acknowledgement, reset, and restoration record. Device
   pulses still follow their real 100/500/200 ms rates; held values do not gain
   new timestamps between pulses. *)
IF ExperimentEnabled AND InputsValid AND NOT SafetyAbort THEN
    IF NOT ExperimentWasEnabled THEN
        ExperimentScan := ULINT#0;
        LocalTimeSeconds := LREAL#0.0;
    END_IF;
    ExperimentScan := ExperimentScan + ULINT#1;
    ExperimentElapsed := ExperimentElapsed + LREAL#0.1;
    LocalTimeSeconds := ExperimentElapsed;

    PressureASampleReceived := TRUE;
    PressureBSampleReceived := TRUE;
    LevelSampleReceived := (ExperimentScan MOD ULINT#5) = ULINT#1;
    DriveFeedbackReceived := (ExperimentScan MOD ULINT#2) = ULINT#1;
    PressureAQualityGood := NOT ((ExperimentScan >= ULINT#116)
        AND (ExperimentScan <= ULINT#130));
    PressureBQualityGood := NOT ((ExperimentScan >= ULINT#121)
        AND (ExperimentScan <= ULINT#130));
    LevelQualityGood := TRUE;
    DriveFeedbackQualityGood := TRUE;
    IF (ExperimentScan >= ULINT#151) AND (ExperimentScan <= ULINT#170) THEN
        LevelSampleReceived := FALSE;
    END_IF;

    PressureASourceTime := LocalTimeSeconds;
    PressureBSourceTime := LocalTimeSeconds;
    IF LevelSampleReceived THEN
        LevelSourceTime := LocalTimeSeconds;
    END_IF;
    IF DriveFeedbackReceived THEN
        DriveFeedbackSourceTime := LocalTimeSeconds;
    END_IF;

    Setpoint := LREAL#6.0;
    LevelRaw := LREAL#35.0;
    PressureARaw := PressureBRaw;
    NaivePressureA := NaiveEvidencePressure;
    IF (ExperimentScan >= ULINT#51) AND (ExperimentScan <= ULINT#100) THEN
        PressureARaw := PressureBRaw + LREAL#1.0;
        NaivePressureA := NaiveEvidencePressure + LREAL#1.0;
    END_IF;
    IF (ExperimentScan >= ULINT#191) AND (ExperimentScan <= ULINT#200) THEN
        LevelRaw := LREAL#19.0;
    END_IF;
    IF (ExperimentScan >= ULINT#216) AND (ExperimentScan <= ULINT#240) THEN
        Setpoint := LREAL#7.0;
    END_IF;
    IF ExperimentScan = ULINT#221 THEN
        HeldDriveFeedback := DriveFeedbackRaw;
    END_IF;
    IF (ExperimentScan >= ULINT#221) AND (ExperimentScan <= ULINT#240) THEN
        DriveFeedbackRaw := HeldDriveFeedback;
    END_IF;

    AcknowledgeRequest := (ExperimentScan = ULINT#101)
        OR (ExperimentScan = ULINT#131)
        OR (ExperimentScan = ULINT#171)
        OR (ExperimentScan = ULINT#201)
        OR (ExperimentScan = ULINT#242);
    ResetApproved := (ExperimentScan = ULINT#111)
        OR (ExperimentScan = ULINT#141)
        OR (ExperimentScan = ULINT#181)
        OR (ExperimentScan = ULINT#211)
        OR (ExperimentScan = ULINT#253);
ELSIF ExperimentWasEnabled THEN
    ExperimentScan := ULINT#0;
    ExperimentElapsed := LREAL#0.0;
    LocalTimeSeconds := LREAL#0.0;
    PressureASampleReceived := TRUE;
    PressureBSampleReceived := TRUE;
    LevelSampleReceived := TRUE;
    DriveFeedbackReceived := TRUE;
    PressureAQualityGood := TRUE;
    PressureBQualityGood := TRUE;
    LevelQualityGood := TRUE;
    DriveFeedbackQualityGood := TRUE;
    AcknowledgeRequest := FALSE;
    ResetApproved := FALSE;
    Setpoint := LREAL#6.0;
    LevelRaw := LREAL#35.0;
    PressureARaw := PressureBRaw;
    NaivePressureA := NaiveEvidencePressure;
END_IF;

Supervisor(
    PressureASampleReceived := PressureASampleReceived,
    PressureBSampleReceived := PressureBSampleReceived,
    LevelSampleReceived := LevelSampleReceived,
    DriveFeedbackReceived := DriveFeedbackReceived,
    PressureAQualityGood := PressureAQualityGood,
    PressureBQualityGood := PressureBQualityGood,
    LevelQualityGood := LevelQualityGood,
    DriveFeedbackQualityGood := DriveFeedbackQualityGood,
    AcknowledgeRequest := AcknowledgeRequest,
    ResetApproved := ResetApproved,
    Setpoint := Setpoint,
    LocalTimeSeconds := LocalTimeSeconds,
    PressureASourceTime := PressureASourceTime,
    PressureBSourceTime := PressureBSourceTime,
    LevelSourceTime := LevelSourceTime,
    DriveFeedbackSourceTime := DriveFeedbackSourceTime,
    LevelRaw := LevelRaw,
    NaivePressureA := NaivePressureA,
    PressureARaw := PressureARaw,
    PressureBRaw := PressureBRaw,
    DriveFeedbackRaw := DriveFeedbackRaw,
    NaiveEvidencePressure := NaiveEvidencePressure
);
PressureAResidual := Supervisor.PressureAResidual;
PressureBResidual := Supervisor.PressureBResidual;
PressureDiscrepancy := Supervisor.PressureDiscrepancy;
PredictedPressure := Supervisor.PredictedPressure;
ConservativeLevel := Supervisor.ConservativeLevel;
ActuatorTrackingResidual := Supervisor.ActuatorTrackingResidual;
SelectedPressure := Supervisor.SelectedPressure;
SelectedPressureSource := Supervisor.SelectedPressureSource;
OperatingMode := Supervisor.OperatingMode;
FaultCode := Supervisor.FaultCode;
RecoveryHealthyCount := Supervisor.RecoveryHealthyCount;
PressureAIsolated := Supervisor.PressureAIsolated;
PressureBIsolated := Supervisor.PressureBIsolated;
FaultLatched := Supervisor.FaultLatched;
FaultAcknowledged := Supervisor.FaultAcknowledged;
RecoveryReady := Supervisor.RecoveryReady;
NaiveRequested := Supervisor.NaiveRequested;
ProtectedRequested := Supervisor.ProtectedRequested;
NaiveApplied := Supervisor.NaiveApplied;
ProtectedApplied := Supervisor.ProtectedApplied;
ExperimentWasEnabled := ExperimentEnabled;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.PressureASampleReceived AT %MX32.0 : BOOL;
    P1.PressureBSampleReceived AT %MX32.1 : BOOL;
    P1.LevelSampleReceived AT %MX32.2 : BOOL;
    P1.DriveFeedbackReceived AT %MX32.3 : BOOL;
    P1.PressureAQualityGood AT %MX32.4 : BOOL;
    P1.PressureBQualityGood AT %MX32.5 : BOOL;
    P1.LevelQualityGood AT %MX32.6 : BOOL;
    P1.DriveFeedbackQualityGood AT %MX32.7 : BOOL;
    P1.AcknowledgeRequest AT %MX33.0 : BOOL;
    P1.ResetApproved AT %MX33.1 : BOOL;
    P1.ExperimentEnabled AT %MX82.5 : BOOL;
    P1.SafetyAbort AT %MX82.6 : BOOL;
    P1.InputsValid AT %MX82.7 : BOOL;
    P1.Setpoint AT %ML40 : LREAL;
    P1.LocalTimeSeconds AT %ML48 : LREAL;
    P1.PressureASourceTime AT %ML56 : LREAL;
    P1.PressureBSourceTime AT %ML64 : LREAL;
    P1.LevelSourceTime AT %ML72 : LREAL;
    P1.DriveFeedbackSourceTime AT %ML80 : LREAL;
    P1.LevelRaw AT %ML88 : LREAL;
    P1.PressureAResidual AT %ML96 : LREAL;
    P1.PressureBResidual AT %ML104 : LREAL;
    P1.PressureDiscrepancy AT %ML112 : LREAL;
    P1.PredictedPressure AT %ML120 : LREAL;
    P1.ConservativeLevel AT %ML128 : LREAL;
    P1.ActuatorTrackingResidual AT %ML136 : LREAL;
    P1.SelectedPressure AT %ML144 : LREAL;
    P1.SelectedPressureSource AT %MW74 : INT;
    P1.OperatingMode AT %MW76 : INT;
    P1.FaultCode AT %MW78 : INT;
    P1.RecoveryHealthyCount AT %MW80 : INT;
    P1.PressureAIsolated AT %MX82.0 : BOOL;
    P1.PressureBIsolated AT %MX82.1 : BOOL;
    P1.FaultLatched AT %MX82.2 : BOOL;
    P1.FaultAcknowledged AT %MX82.3 : BOOL;
    P1.RecoveryReady AT %MX82.4 : BOOL;
    P1.NaiveRequested AT %ML152 : LREAL;
    P1.ProtectedRequested AT %ML160 : LREAL;
    P1.ExperimentElapsed AT %ML168 : LREAL;
    P1.NaiveApplied AT %QL0 : LREAL;
    P1.ProtectedApplied AT %QL8 : LREAL;
    P1.NaivePressureA AT %IL0 : LREAL;
    P1.PressureARaw AT %IL8 : LREAL;
    P1.PressureBRaw AT %IL16 : LREAL;
    P1.DriveFeedbackRaw AT %IL24 : LREAL;
    P1.NaiveEvidencePressure AT %IL40 : LREAL;
END_VAR
END_CONFIGURATION
