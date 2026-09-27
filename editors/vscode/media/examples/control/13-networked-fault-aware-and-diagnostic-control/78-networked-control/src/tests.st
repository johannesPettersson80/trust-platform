(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. *)
TEST_PROGRAM Lesson78ProtectedReceiverAcceptsFreshAndExpiresStaleCommands
VAR
    Supervisor : NetworkedPumpSupervisor;
END_VAR
Supervisor(
    RemoteEnabled := TRUE,
    RemoteSampleReceived := TRUE,
    RemoteQualityGood := TRUE,
    ClockSynchronized := TRUE,
    RecoveryApproved := FALSE,
    LocalTimeSeconds := LREAL#0.40,
    RemoteSourceTimeSeconds := LREAL#0.35,
    ClockUncertaintySeconds := LREAL#0.05,
    RemoteDemand := LREAL#50.0,
    RemoteSequence := DINT#1,
    ProtectedPressure := LREAL#6.0);
ASSERT_TRUE(IN := Supervisor.NewSampleEligible);
ASSERT_TRUE(IN := Supervisor.ProtectedRemoteOwned);
ASSERT_NEAR(EXPECTED := LREAL#50.0,
    ACTUAL := Supervisor.ProtectedApplied, DELTA := LREAL#1.0e-12);

Supervisor(
    RemoteEnabled := TRUE,
    RemoteSampleReceived := FALSE,
    RemoteQualityGood := TRUE,
    ClockSynchronized := TRUE,
    RecoveryApproved := FALSE,
    LocalTimeSeconds := LREAL#0.90,
    RemoteSourceTimeSeconds := LREAL#0.35,
    ClockUncertaintySeconds := LREAL#0.05,
    RemoteDemand := LREAL#50.0,
    RemoteSequence := DINT#1,
    ProtectedPressure := LREAL#6.0);
ASSERT_FALSE(IN := Supervisor.ProtectedRemoteOwned);
ASSERT_TRUE(IN := Supervisor.CommunicationFaultLatched);
ASSERT_TRUE(IN := Supervisor.RejectionReason = INT#5);
ASSERT_NEAR(EXPECTED := LREAL#50.0,
    ACTUAL := Supervisor.ProtectedApplied, DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM
