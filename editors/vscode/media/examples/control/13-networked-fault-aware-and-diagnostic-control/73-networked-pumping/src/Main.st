USING Control;

PROGRAM Main
VAR
    RemoteEnabled : BOOL := TRUE;
    RemoteSampleReceived : BOOL := FALSE;
    RemoteQualityGood : BOOL := TRUE;
    ClockSynchronized : BOOL := TRUE;
    RecoveryApproved : BOOL := FALSE;
    LocalTimeSeconds : LREAL := 0.0;
    RemoteSourceTimeSeconds : LREAL := 0.0;
    ClockUncertaintySeconds : LREAL := 0.05;
    RemoteDemand : LREAL := 50.0;
    RemoteSequence : DINT := 0;
    NaivePressure : LREAL := 6.0;
    ProtectedPressure : LREAL := 6.0;
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
    Supervisor : NetworkedPumpSupervisor;
END_VAR

Supervisor(
    RemoteEnabled := RemoteEnabled,
    RemoteSampleReceived := RemoteSampleReceived,
    RemoteQualityGood := RemoteQualityGood,
    ClockSynchronized := ClockSynchronized,
    RecoveryApproved := RecoveryApproved,
    LocalTimeSeconds := LocalTimeSeconds,
    RemoteSourceTimeSeconds := RemoteSourceTimeSeconds,
    ClockUncertaintySeconds := ClockUncertaintySeconds,
    RemoteDemand := RemoteDemand,
    RemoteSequence := RemoteSequence,
    ProtectedPressure := ProtectedPressure
);
NaiveRequested := Supervisor.NaiveRequested;
NaiveApplied := Supervisor.NaiveApplied;
ProtectedRequested := Supervisor.ProtectedRequested;
ProtectedApplied := Supervisor.ProtectedApplied;
NominalAgeSeconds := Supervisor.NominalAgeSeconds;
ConservativeAgeSeconds := Supervisor.ConservativeAgeSeconds;
LastAcceptedSequence := Supervisor.LastAcceptedSequence;
RejectionReason := Supervisor.RejectionReason;
CommunicationFaultLatched := Supervisor.CommunicationFaultLatched;
ProtectedRemoteValid := Supervisor.ProtectedRemoteValid;
ProtectedRemoteOwned := Supervisor.ProtectedRemoteOwned;
NewSampleEligible := Supervisor.NewSampleEligible;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.RemoteEnabled AT %MX16.0 : BOOL;
    P1.RemoteSampleReceived AT %MX16.1 : BOOL;
    P1.RemoteQualityGood AT %MX96.2 : BOOL;
    P1.ClockSynchronized AT %MX96.3 : BOOL;
    P1.RecoveryApproved AT %MX16.4 : BOOL;
    P1.CommunicationFaultLatched AT %MX96.5 : BOOL;
    P1.ProtectedRemoteValid AT %MX16.6 : BOOL;
    P1.ProtectedRemoteOwned AT %MX96.7 : BOOL;
    P1.NewSampleEligible AT %MX17.0 : BOOL;
    P1.LocalTimeSeconds AT %ML16 : LREAL;
    P1.RemoteSourceTimeSeconds AT %ML24 : LREAL;
    P1.ClockUncertaintySeconds AT %ML32 : LREAL;
    P1.RemoteDemand AT %ML40 : LREAL;
    P1.RemoteSequence AT %MD48 : DINT;
    P1.LastAcceptedSequence AT %MD52 : DINT;
    P1.NominalAgeSeconds AT %ML56 : LREAL;
    P1.ConservativeAgeSeconds AT %ML64 : LREAL;
    P1.RejectionReason AT %MW72 : INT;
    P1.NaiveRequested AT %ML80 : LREAL;
    P1.ProtectedRequested AT %ML88 : LREAL;
    P1.NaiveApplied AT %QL0 : LREAL;
    P1.ProtectedApplied AT %QL8 : LREAL;
    P1.NaivePressure AT %IL0 : LREAL;
    P1.ProtectedPressure AT %IL8 : LREAL;
END_VAR
END_CONFIGURATION
