USING Control;

(* NetworkedPumpSupervisor validates remote pump commands by source time,
   quality, sequence, and recovery approval. It owns the deterministic transfer
   to a local pressure controller so communication loss cannot leave an
   unbounded or stale actuator command in authority. *)
FUNCTION_BLOCK NetworkedPumpSupervisor
VAR_INPUT
    RemoteEnabled : BOOL;
    RemoteSampleReceived : BOOL;
    RemoteQualityGood : BOOL;
    ClockSynchronized : BOOL;
    RecoveryApproved : BOOL;
    LocalTimeSeconds : LREAL;
    RemoteSourceTimeSeconds : LREAL;
    ClockUncertaintySeconds : LREAL;
    RemoteDemand : LREAL;
    RemoteSequence : DINT;
    ProtectedPressure : LREAL;
END_VAR
VAR_OUTPUT
    NaiveRequested : LREAL := 50.0;
    NaiveApplied : LREAL := 50.0;
    ProtectedRequested : LREAL := 50.0;
    ProtectedApplied : LREAL := 50.0;
    NominalAgeSeconds : LREAL := 0.0;
    ConservativeAgeSeconds : LREAL := 0.05;
    LastAcceptedSequence : DINT := 0;
    RejectionReason : INT := 0;
    CommunicationFaultLatched : BOOL := FALSE;
    ProtectedRemoteValid : BOOL := FALSE;
    ProtectedRemoteOwned : BOOL := FALSE;
    NewSampleEligible : BOOL := FALSE;
END_VAR
VAR
    AcceptedSourceTimeSeconds : LREAL := 0.0;
    AcceptedDemand : LREAL := 50.0;
    AcceptedNominalAgeSeconds : LREAL := 0.0;
    AcceptedConservativeAgeSeconds : LREAL := 0.05;
    HasAcceptedRemoteCommand : BOOL := FALSE;
    LocalControllerOutput : LREAL := 50.0;
    ControllerInitialized : BOOL := FALSE;
    LocalController : Control.PID;
    LocalParameters : Control.PIDParameters;
    LocalInitialState : Control.PIDInitialState;
    LocalInput : Control.PIDInput;
END_VAR

IF NOT ControllerInitialized THEN
    LocalParameters.ProportionalGain := LREAL#10.0;
    LocalParameters.IntegralGain := LREAL#2.0;
    LocalParameters.DerivativeGain := LREAL#0.0;
    LocalParameters.Bias := LREAL#50.0;
    LocalParameters.ProportionalSetpointWeight := LREAL#1.0;
    LocalParameters.DerivativeSetpointWeight := LREAL#0.0;
    LocalParameters.OutputMinimum := LREAL#0.0;
    LocalParameters.OutputMaximum := LREAL#100.0;
    LocalParameters.RateMinimum := LREAL#-1000.0;
    LocalParameters.RateMaximum := LREAL#1000.0;
    LocalParameters.TrackingTime := LREAL#0.5;
    LocalParameters.AntiWindup := PIDAntiWindup#Combined;
    LocalInitialState.Setpoint := LREAL#6.0;
    LocalInitialState.ProcessValue := ProtectedPressure;
    LocalInitialState.NominalOutput := LREAL#50.0;
    LocalInitialState.SaturatedOutput := LREAL#50.0;
    LocalController.Configure(Parameters := LocalParameters);
    LocalController.Initialize(State := LocalInitialState);
    ControllerInitialized := TRUE;
END_IF;

(* The naive path deliberately demonstrates the unsafe arrival-order rule. *)
IF RemoteSampleReceived THEN
    NaiveRequested := RemoteDemand;
END_IF;
IF RemoteEnabled THEN
    NaiveApplied := LIMIT(LREAL#0.0, NaiveRequested, LREAL#100.0);
ELSE
    NaiveApplied := LREAL#50.0;
END_IF;

(* Freshness is calculated from locally generated time and the packet's source
   timestamp. Clock uncertainty is added so acceptance uses the worst age. *)
NominalAgeSeconds := LocalTimeSeconds - RemoteSourceTimeSeconds;
ConservativeAgeSeconds := NominalAgeSeconds + ClockUncertaintySeconds;
NewSampleEligible := FALSE;
RejectionReason := INT#0;

IF RemoteSampleReceived THEN
    IF NOT RemoteQualityGood THEN
        RejectionReason := INT#1;
    ELSIF NOT ClockSynchronized THEN
        RejectionReason := INT#2;
    ELSIF (ClockUncertaintySeconds < LREAL#0.0)
        OR (ClockUncertaintySeconds > LREAL#0.05) THEN
        RejectionReason := INT#3;
    ELSIF NominalAgeSeconds < -ClockUncertaintySeconds THEN
        RejectionReason := INT#4;
    ELSIF ConservativeAgeSeconds > LREAL#0.50 THEN
        RejectionReason := INT#5;
    ELSIF RemoteSequence <= LastAcceptedSequence THEN
        RejectionReason := INT#6;
    ELSIF (RemoteDemand <> RemoteDemand)
        OR (RemoteDemand < LREAL#0.0)
        OR (RemoteDemand > LREAL#100.0) THEN
        RejectionReason := INT#7;
    ELSIF CommunicationFaultLatched
        AND ((NOT RecoveryApproved)
            OR (ABS(RemoteDemand - ProtectedApplied) > LREAL#2.0)) THEN
        RejectionReason := INT#8;
    ELSE
        NewSampleEligible := RemoteEnabled;
    END_IF;

    IF NewSampleEligible THEN
        AcceptedDemand := RemoteDemand;
        AcceptedSourceTimeSeconds := RemoteSourceTimeSeconds;
        LastAcceptedSequence := RemoteSequence;
        HasAcceptedRemoteCommand := TRUE;
        CommunicationFaultLatched := FALSE;
    ELSE
        CommunicationFaultLatched := TRUE;
    END_IF;
END_IF;

AcceptedNominalAgeSeconds := LocalTimeSeconds - AcceptedSourceTimeSeconds;
AcceptedConservativeAgeSeconds := AcceptedNominalAgeSeconds
    + ClockUncertaintySeconds;

IF HasAcceptedRemoteCommand THEN
    IF NOT RemoteQualityGood THEN
        CommunicationFaultLatched := TRUE;
        IF RejectionReason = INT#0 THEN RejectionReason := INT#1; END_IF;
    ELSIF NOT ClockSynchronized THEN
        CommunicationFaultLatched := TRUE;
        IF RejectionReason = INT#0 THEN RejectionReason := INT#2; END_IF;
    ELSIF (ClockUncertaintySeconds < LREAL#0.0)
        OR (ClockUncertaintySeconds > LREAL#0.05) THEN
        CommunicationFaultLatched := TRUE;
        IF RejectionReason = INT#0 THEN RejectionReason := INT#3; END_IF;
    ELSIF AcceptedNominalAgeSeconds < -ClockUncertaintySeconds THEN
        CommunicationFaultLatched := TRUE;
        IF RejectionReason = INT#0 THEN RejectionReason := INT#4; END_IF;
    ELSIF AcceptedConservativeAgeSeconds > LREAL#0.50 THEN
        CommunicationFaultLatched := TRUE;
        IF RejectionReason = INT#0 THEN RejectionReason := INT#5; END_IF;
    END_IF;
END_IF;

ProtectedRemoteValid := RemoteEnabled
    AND HasAcceptedRemoteCommand
    AND NOT CommunicationFaultLatched
    AND RemoteQualityGood
    AND ClockSynchronized
    AND (ClockUncertaintySeconds >= LREAL#0.0)
    AND (ClockUncertaintySeconds <= LREAL#0.05)
    AND (AcceptedNominalAgeSeconds >= -ClockUncertaintySeconds)
    AND (AcceptedConservativeAgeSeconds <= LREAL#0.50);
ProtectedRemoteOwned := ProtectedRemoteValid;

LocalInput.Setpoint := LREAL#6.0;
LocalInput.ProcessValue := ProtectedPressure;
LocalInput.CycleTime := LREAL#0.1;
LocalInput.FeedForward := LREAL#0.0;
IF ProtectedRemoteOwned THEN
    (* Track the actuator owner so local takeover starts from the live command. *)
    LocalInput.Mode := PIDMode#Track;
    LocalInput.TrackingOutput := AcceptedDemand;
ELSE
    LocalInput.Mode := PIDMode#Auto;
END_IF;
LocalControllerOutput := LocalController.Update(Input := LocalInput);

IF ProtectedRemoteOwned THEN
    ProtectedRequested := AcceptedDemand;
ELSE
    ProtectedRequested := LocalControllerOutput;
END_IF;
ProtectedApplied := LIMIT(LREAL#0.0, ProtectedRequested, LREAL#100.0);
END_FUNCTION_BLOCK
