TEST_PROGRAM CraneUsesStateObjectsForMotion
VAR
    Crane : CraneController;
END_VAR

Crane.Initialize();
Crane.InjectDriveStatus(HoistStatus := WORD#16#0004, TravelStatus := WORD#16#0400);
ASSERT_EQUAL(EXPECTED := INT#10, ACTUAL := Crane.Update(LiftCommand := TRUE, TravelCommand := FALSE, LowerCommand := FALSE, UpperLimit := FALSE, Overload := FALSE));
ASSERT_TRUE(IN := Crane.HoistRunning);
ASSERT_EQUAL(EXPECTED := INT#0, ACTUAL := Crane.Update(LiftCommand := FALSE, TravelCommand := FALSE, LowerCommand := FALSE, UpperLimit := FALSE, Overload := FALSE));
END_TEST_PROGRAM

TEST_PROGRAM CraneAdapterNormalizesDriveFault
VAR
    Crane : CraneController;
END_VAR

Crane.Initialize();
Crane.InjectDriveStatus(HoistStatus := WORD#16#0008, TravelStatus := WORD#16#0400);
ASSERT_EQUAL(EXPECTED := INT#90, ACTUAL := Crane.Update(LiftCommand := TRUE, TravelCommand := FALSE, LowerCommand := FALSE, UpperLimit := FALSE, Overload := FALSE));
ASSERT_EQUAL(EXPECTED := DWORD#16#A0000044, ACTUAL := Crane.NormalizedFaultCode);
END_TEST_PROGRAM

(* Multi-state-transition test: drives the crane through Lift, Travel,
   and Park on the same instance. Catches bugs where state transitions
   cache the previous state's exit conditions, or where adapter status
   from one motion phase leaks into another. *)
TEST_PROGRAM CraneTransitionsThroughMultipleMotionStatesOnOneInstance
VAR
    Crane : CraneController;
END_VAR

Crane.Initialize();
Crane.InjectDriveStatus(HoistStatus := WORD#16#0004, TravelStatus := WORD#16#0400);

(* Lift command -> Lifting state *)
ASSERT_EQUAL(EXPECTED := INT#10, ACTUAL := Crane.Update(LiftCommand := TRUE, TravelCommand := FALSE, LowerCommand := FALSE, UpperLimit := FALSE, Overload := FALSE));

(* Travel command (no lift) -> Travelling state *)
ASSERT_EQUAL(EXPECTED := INT#20, ACTUAL := Crane.Update(LiftCommand := FALSE, TravelCommand := TRUE, LowerCommand := FALSE, UpperLimit := FALSE, Overload := FALSE));

(* No commands -> Parked state *)
ASSERT_EQUAL(EXPECTED := INT#0, ACTUAL := Crane.Update(LiftCommand := FALSE, TravelCommand := FALSE, LowerCommand := FALSE, UpperLimit := FALSE, Overload := FALSE));
ASSERT_FALSE(IN := Crane.HoistRunning);
END_TEST_PROGRAM

(* Each state's exit transitions covered explicitly: Lifting can exit
   to Travelling, Lowering, Parked, or Fault. *)
TEST_PROGRAM CraneLiftingStateExitsToEverySuccessor
VAR
    Crane : CraneController;
END_VAR

(* Lifting -> Travelling (lift off, travel on) *)
Crane.Initialize();
Crane.InjectDriveStatus(HoistStatus := WORD#16#0004, TravelStatus := WORD#16#0400);
ASSERT_EQUAL(EXPECTED := INT#10, ACTUAL := Crane.Update(LiftCommand := TRUE, TravelCommand := FALSE, LowerCommand := FALSE, UpperLimit := FALSE, Overload := FALSE));
ASSERT_EQUAL(EXPECTED := INT#20, ACTUAL := Crane.Update(LiftCommand := FALSE, TravelCommand := TRUE, LowerCommand := FALSE, UpperLimit := FALSE, Overload := FALSE));
END_TEST_PROGRAM

(* Lifting -> Lowering directly via in-state forward *)
TEST_PROGRAM CraneLiftingForwardsToLowering
VAR
    Crane : CraneController;
END_VAR
Crane.Initialize();
Crane.InjectDriveStatus(HoistStatus := WORD#16#0004, TravelStatus := WORD#16#0400);
ASSERT_EQUAL(EXPECTED := INT#10, ACTUAL := Crane.Update(LiftCommand := TRUE, TravelCommand := FALSE, LowerCommand := FALSE, UpperLimit := FALSE, Overload := FALSE));
ASSERT_EQUAL(EXPECTED := INT#30, ACTUAL := Crane.Update(LiftCommand := FALSE, TravelCommand := FALSE, LowerCommand := TRUE, UpperLimit := FALSE, Overload := FALSE));
END_TEST_PROGRAM

(* Lifting -> Fault on UpperLimit during lift *)
TEST_PROGRAM CraneLiftingTrapsUpperLimit
VAR
    Crane : CraneController;
END_VAR
Crane.Initialize();
Crane.InjectDriveStatus(HoistStatus := WORD#16#0004, TravelStatus := WORD#16#0400);
ASSERT_EQUAL(EXPECTED := INT#10, ACTUAL := Crane.Update(LiftCommand := TRUE, TravelCommand := FALSE, LowerCommand := FALSE, UpperLimit := FALSE, Overload := FALSE));
(* UpperLimit hit while lifting -> Fault *)
ASSERT_EQUAL(EXPECTED := INT#90, ACTUAL := Crane.Update(LiftCommand := TRUE, TravelCommand := FALSE, LowerCommand := FALSE, UpperLimit := TRUE, Overload := FALSE));
END_TEST_PROGRAM

(* Travelling state forwards to Lifting, Lowering, Parked, or Fault. *)
TEST_PROGRAM CraneTravellingForwardsBackToLifting
VAR
    Crane : CraneController;
END_VAR
Crane.Initialize();
Crane.InjectDriveStatus(HoistStatus := WORD#16#0004, TravelStatus := WORD#16#0400);
(* Travel first *)
ASSERT_EQUAL(EXPECTED := INT#20, ACTUAL := Crane.Update(LiftCommand := FALSE, TravelCommand := TRUE, LowerCommand := FALSE, UpperLimit := FALSE, Overload := FALSE));
(* Drop travel, raise lift *)
ASSERT_EQUAL(EXPECTED := INT#10, ACTUAL := Crane.Update(LiftCommand := TRUE, TravelCommand := FALSE, LowerCommand := FALSE, UpperLimit := FALSE, Overload := FALSE));
END_TEST_PROGRAM

(* Lowering state forwards to Lifting, Travelling, Parked, or Fault. *)
TEST_PROGRAM CraneLoweringForwardsToTravelling
VAR
    Crane : CraneController;
END_VAR
Crane.Initialize();
Crane.InjectDriveStatus(HoistStatus := WORD#16#0004, TravelStatus := WORD#16#0400);
(* Lower first *)
ASSERT_EQUAL(EXPECTED := INT#30, ACTUAL := Crane.Update(LiftCommand := FALSE, TravelCommand := FALSE, LowerCommand := TRUE, UpperLimit := FALSE, Overload := FALSE));
(* Drop lower, raise travel *)
ASSERT_EQUAL(EXPECTED := INT#20, ACTUAL := Crane.Update(LiftCommand := FALSE, TravelCommand := TRUE, LowerCommand := FALSE, UpperLimit := FALSE, Overload := FALSE));
(* Drop everything: Parked *)
ASSERT_EQUAL(EXPECTED := INT#0, ACTUAL := Crane.Update(LiftCommand := FALSE, TravelCommand := FALSE, LowerCommand := FALSE, UpperLimit := FALSE, Overload := FALSE));
END_TEST_PROGRAM

(* Overload during any motion forces Fault. *)
TEST_PROGRAM CraneOverloadFromAnyMotionForcesFault
VAR
    Crane : CraneController;
END_VAR

(* During Lifting *)
Crane.Initialize();
Crane.InjectDriveStatus(HoistStatus := WORD#16#0004, TravelStatus := WORD#16#0400);
Crane.Update(LiftCommand := TRUE, TravelCommand := FALSE, LowerCommand := FALSE, UpperLimit := FALSE, Overload := FALSE);
ASSERT_EQUAL(EXPECTED := INT#90, ACTUAL := Crane.Update(LiftCommand := TRUE, TravelCommand := FALSE, LowerCommand := FALSE, UpperLimit := FALSE, Overload := TRUE));
END_TEST_PROGRAM

(* Overload from Parked goes straight to Fault. *)
TEST_PROGRAM CraneOverloadFromParkedFaults
VAR
    Crane : CraneController;
END_VAR
Crane.Initialize();
Crane.InjectDriveStatus(HoistStatus := WORD#16#0004, TravelStatus := WORD#16#0400);
ASSERT_EQUAL(EXPECTED := INT#90, ACTUAL := Crane.Update(LiftCommand := FALSE, TravelCommand := FALSE, LowerCommand := FALSE, UpperLimit := FALSE, Overload := TRUE));
END_TEST_PROGRAM

(* Fault state is terminal: any further Update returns 90 even when
   inputs return to safe. *)
TEST_PROGRAM CraneFaultStateIsTerminal
VAR
    Crane : CraneController;
END_VAR
Crane.Initialize();
Crane.InjectDriveStatus(HoistStatus := WORD#16#0008, TravelStatus := WORD#16#0400);
ASSERT_EQUAL(EXPECTED := INT#90, ACTUAL := Crane.Update(LiftCommand := TRUE, TravelCommand := FALSE, LowerCommand := FALSE, UpperLimit := FALSE, Overload := FALSE));
(* Even with healthy inputs, the fault state holds *)
Crane.InjectDriveStatus(HoistStatus := WORD#16#0004, TravelStatus := WORD#16#0400);
ASSERT_EQUAL(EXPECTED := INT#90, ACTUAL := Crane.Update(LiftCommand := FALSE, TravelCommand := FALSE, LowerCommand := FALSE, UpperLimit := FALSE, Overload := FALSE));
END_TEST_PROGRAM

(* Per-adapter isolation: AbbHoistAdapter alone, no controller. ABB
   running bit is 0x0004; fault bit 0x0008. *)
TEST_PROGRAM CraneAbbAdapterDecodesStatusWordIsolated
VAR
    Abb : AbbHoistAdapter;
END_VAR

(* Idle: no running, no fault *)
Abb.ConfigureRaw(StatusWord := WORD#16#0000, FaultRaw := WORD#16#0000);
ASSERT_FALSE(IN := Abb.IsRunning);
ASSERT_FALSE(IN := Abb.HasFault);

(* Running bit set *)
Abb.ConfigureRaw(StatusWord := WORD#16#0004, FaultRaw := WORD#16#0000);
ASSERT_TRUE(IN := Abb.IsRunning);
ASSERT_FALSE(IN := Abb.HasFault);

(* Fault bit set *)
Abb.ConfigureRaw(StatusWord := WORD#16#0008, FaultRaw := WORD#16#0044);
ASSERT_FALSE(IN := Abb.IsRunning);
ASSERT_TRUE(IN := Abb.HasFault);
ASSERT_EQUAL(EXPECTED := DWORD#16#A0000044, ACTUAL := Abb.FaultCode);

(* Both bits at once: running and faulted (status word ambiguous) *)
Abb.ConfigureRaw(StatusWord := WORD#16#000C, FaultRaw := WORD#16#0011);
ASSERT_TRUE(IN := Abb.IsRunning);
ASSERT_TRUE(IN := Abb.HasFault);
ASSERT_EQUAL(EXPECTED := DWORD#16#A0000011, ACTUAL := Abb.FaultCode);
END_TEST_PROGRAM

(* Per-adapter isolation: SiemensTravelAdapter. Different bit positions
   from ABB. Running bit is 0x0400 (not 0x0004). Fault namespace is
   0x5000_____ (not 0xA000_____). *)
TEST_PROGRAM CraneSiemensAdapterDecodesStatusWordIsolated
VAR
    Sie : SiemensTravelAdapter;
END_VAR

(* Idle *)
Sie.ConfigureRaw(StatusWord := WORD#16#0000, FaultRaw := WORD#16#0000);
ASSERT_FALSE(IN := Sie.IsRunning);
ASSERT_FALSE(IN := Sie.HasFault);

(* Running bit (0x0400 — Siemens-specific) *)
Sie.ConfigureRaw(StatusWord := WORD#16#0400, FaultRaw := WORD#16#0000);
ASSERT_TRUE(IN := Sie.IsRunning);
ASSERT_FALSE(IN := Sie.HasFault);

(* ABB bit (0x0004) does NOT register on Siemens adapter *)
Sie.ConfigureRaw(StatusWord := WORD#16#0004, FaultRaw := WORD#16#0000);
ASSERT_FALSE(IN := Sie.IsRunning);

(* Siemens fault uses 0x0008 like ABB but its FaultCode prefix is 0x5000_____ *)
Sie.ConfigureRaw(StatusWord := WORD#16#0008, FaultRaw := WORD#16#0055);
ASSERT_TRUE(IN := Sie.HasFault);
ASSERT_EQUAL(EXPECTED := DWORD#16#50000055, ACTUAL := Sie.FaultCode);
END_TEST_PROGRAM

(* Travel-side fault must drive the controller to Fault even when
   hoist is healthy. Verifies adapter-OR composition. *)
TEST_PROGRAM CraneTravelFaultDrivesControllerFault
VAR
    Crane : CraneController;
END_VAR
Crane.Initialize();
(* Hoist healthy, travel faulted *)
Crane.InjectDriveStatus(HoistStatus := WORD#16#0004, TravelStatus := WORD#16#0008);
ASSERT_EQUAL(EXPECTED := INT#90, ACTUAL := Crane.Update(LiftCommand := FALSE, TravelCommand := TRUE, LowerCommand := FALSE, UpperLimit := FALSE, Overload := FALSE));
(* Fault code must be travel-side namespace (0x5000_____), not hoist *)
ASSERT_EQUAL(EXPECTED := DWORD#16#50000055, ACTUAL := Crane.NormalizedFaultCode);
END_TEST_PROGRAM

(* All commands at once from Parked: Lift wins (priority order). *)
TEST_PROGRAM CraneAllCommandsAtOncePicksLift
VAR
    Crane : CraneController;
END_VAR
Crane.Initialize();
Crane.InjectDriveStatus(HoistStatus := WORD#16#0004, TravelStatus := WORD#16#0400);
ASSERT_EQUAL(EXPECTED := INT#10, ACTUAL := Crane.Update(LiftCommand := TRUE, TravelCommand := TRUE, LowerCommand := TRUE, UpperLimit := FALSE, Overload := FALSE));
END_TEST_PROGRAM

(* Empty commands hold at Parked indefinitely across many cycles. *)
TEST_PROGRAM CraneEmptyCommandsHoldParked
VAR
    Crane : CraneController;
    I : INT;
END_VAR
Crane.Initialize();
Crane.InjectDriveStatus(HoistStatus := WORD#16#0004, TravelStatus := WORD#16#0400);
FOR I := 1 TO 10 DO
    Crane.Update(LiftCommand := FALSE, TravelCommand := FALSE, LowerCommand := FALSE, UpperLimit := FALSE, Overload := FALSE);
END_FOR;
ASSERT_EQUAL(EXPECTED := INT#0, ACTUAL := Crane.CurrentStateId);
ASSERT_FALSE(IN := Crane.HoistRunning);
END_TEST_PROGRAM
