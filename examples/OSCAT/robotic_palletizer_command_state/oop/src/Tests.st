TEST_PROGRAM PalletizerCompletesThroughCommandObjects
VAR
    Cell : PalletizerController;
END_VAR

Cell.Initialize();
ASSERT_EQUAL(EXPECTED := INT#10, ACTUAL := Cell.Update(BoxPresent := TRUE, PalletReady := TRUE, RobotHealthy := TRUE, PickVacuumOk := TRUE));
ASSERT_EQUAL(EXPECTED := INT#20, ACTUAL := Cell.Update(BoxPresent := TRUE, PalletReady := TRUE, RobotHealthy := TRUE, PickVacuumOk := TRUE));
ASSERT_EQUAL(EXPECTED := INT#30, ACTUAL := Cell.Update(BoxPresent := TRUE, PalletReady := TRUE, RobotHealthy := TRUE, PickVacuumOk := TRUE));
ASSERT_EQUAL(EXPECTED := INT#100, ACTUAL := Cell.Update(BoxPresent := TRUE, PalletReady := TRUE, RobotHealthy := TRUE, PickVacuumOk := TRUE));
ASSERT_EQUAL(EXPECTED := DWORD#1, ACTUAL := Cell.CompletedPallets);
END_TEST_PROGRAM

TEST_PROGRAM PalletizerRejectsWhenPickCommandFails
VAR
    Cell : PalletizerController;
END_VAR

Cell.Initialize();
Cell.Update(BoxPresent := TRUE, PalletReady := TRUE, RobotHealthy := TRUE, PickVacuumOk := TRUE);
ASSERT_EQUAL(EXPECTED := INT#40, ACTUAL := Cell.Update(BoxPresent := TRUE, PalletReady := TRUE, RobotHealthy := TRUE, PickVacuumOk := FALSE));
ASSERT_EQUAL(EXPECTED := INT#1, ACTUAL := Cell.LastCommandId);
END_TEST_PROGRAM

(* Multi-pallet integration test: drives the same Cell through two
   complete pallet cycles. Catches bugs where the per-pallet state
   sticks at the previous Place/Index state, or where CompletedPallets
   fails to grow on subsequent successful cycles. *)
TEST_PROGRAM PalletizerCompletesMultiplePalletsOnOneInstance
VAR
    Cell : PalletizerController;
END_VAR

Cell.Initialize();

(* Pallet 1: complete the pick/place/index cycle *)
Cell.Update(BoxPresent := TRUE, PalletReady := TRUE, RobotHealthy := TRUE, PickVacuumOk := TRUE);
Cell.Update(BoxPresent := TRUE, PalletReady := TRUE, RobotHealthy := TRUE, PickVacuumOk := TRUE);
Cell.Update(BoxPresent := TRUE, PalletReady := TRUE, RobotHealthy := TRUE, PickVacuumOk := TRUE);
ASSERT_EQUAL(EXPECTED := INT#100, ACTUAL := Cell.Update(BoxPresent := TRUE, PalletReady := TRUE, RobotHealthy := TRUE, PickVacuumOk := TRUE));
ASSERT_EQUAL(EXPECTED := DWORD#1, ACTUAL := Cell.CompletedPallets);

(* Pallet 2: same instance, drive a second complete cycle *)
Cell.Update(BoxPresent := TRUE, PalletReady := TRUE, RobotHealthy := TRUE, PickVacuumOk := TRUE);
Cell.Update(BoxPresent := TRUE, PalletReady := TRUE, RobotHealthy := TRUE, PickVacuumOk := TRUE);
Cell.Update(BoxPresent := TRUE, PalletReady := TRUE, RobotHealthy := TRUE, PickVacuumOk := TRUE);
Cell.Update(BoxPresent := TRUE, PalletReady := TRUE, RobotHealthy := TRUE, PickVacuumOk := TRUE);
ASSERT_TRUE(IN := Cell.CompletedPallets >= DWORD#2);
END_TEST_PROGRAM

(* WaitBox idle: with no box present, the controller stays at WaitBox
   and never starts a command. LastCommandId must remain 0. *)
TEST_PROGRAM PalletizerWaitsForBoxAndPallet
VAR
    Cell : PalletizerController;
END_VAR

Cell.Initialize();
(* No box: stays at WaitBox *)
ASSERT_EQUAL(EXPECTED := INT#0, ACTUAL := Cell.Update(BoxPresent := FALSE, PalletReady := TRUE, RobotHealthy := TRUE, PickVacuumOk := TRUE));
ASSERT_EQUAL(EXPECTED := INT#0, ACTUAL := Cell.LastCommandId);
(* Box present but pallet not ready: stays at WaitBox *)
ASSERT_EQUAL(EXPECTED := INT#0, ACTUAL := Cell.Update(BoxPresent := TRUE, PalletReady := FALSE, RobotHealthy := TRUE, PickVacuumOk := TRUE));
ASSERT_EQUAL(EXPECTED := INT#0, ACTUAL := Cell.LastCommandId);
(* Both ready: advances to Pick on next cycle *)
ASSERT_EQUAL(EXPECTED := INT#10, ACTUAL := Cell.Update(BoxPresent := TRUE, PalletReady := TRUE, RobotHealthy := TRUE, PickVacuumOk := TRUE));
END_TEST_PROGRAM

(* PlaceCommand failure routes to fault, not reject. RejectState is
   for vacuum-loss at pick; PlaceCommand failure is a hard fault that
   needs operator intervention. Verifies the per-command failure
   semantics are distinct. *)
TEST_PROGRAM PalletizerPlaceFailureGoesToFault
VAR
    Cell : PalletizerController;
END_VAR

Cell.Initialize();
(* Drive into Place state *)
Cell.Update(BoxPresent := TRUE, PalletReady := TRUE, RobotHealthy := TRUE, PickVacuumOk := TRUE);
Cell.Update(BoxPresent := TRUE, PalletReady := TRUE, RobotHealthy := TRUE, PickVacuumOk := TRUE);
ASSERT_EQUAL(EXPECTED := INT#20, ACTUAL := Cell.CurrentStateId);
(* In Place, robot dies: must go to Fault (90), not Reject (40) *)
ASSERT_EQUAL(EXPECTED := INT#90, ACTUAL := Cell.Update(BoxPresent := TRUE, PalletReady := TRUE, RobotHealthy := FALSE, PickVacuumOk := TRUE));
ASSERT_TRUE(IN := Cell.Faulted);
ASSERT_EQUAL(EXPECTED := INT#2, ACTUAL := Cell.LastCommandId);
END_TEST_PROGRAM

(* Reject path drives back to WaitBox: after a vacuum-loss reject,
   the cell ejects the box and returns to idle ready for the next box. *)
TEST_PROGRAM PalletizerRejectClearsToWaitBox
VAR
    Cell : PalletizerController;
END_VAR

Cell.Initialize();
Cell.Update(BoxPresent := TRUE, PalletReady := TRUE, RobotHealthy := TRUE, PickVacuumOk := TRUE);
ASSERT_EQUAL(EXPECTED := INT#10, ACTUAL := Cell.CurrentStateId);
(* Vacuum loss: reject *)
Cell.Update(BoxPresent := TRUE, PalletReady := TRUE, RobotHealthy := TRUE, PickVacuumOk := FALSE);
ASSERT_EQUAL(EXPECTED := INT#40, ACTUAL := Cell.CurrentStateId);
(* RejectCommand returns Done := TRUE; reject state advances back to WaitBox *)
ASSERT_EQUAL(EXPECTED := INT#0, ACTUAL := Cell.Update(BoxPresent := FALSE, PalletReady := TRUE, RobotHealthy := TRUE, PickVacuumOk := TRUE));
(* No completed pallet from a reject *)
ASSERT_EQUAL(EXPECTED := DWORD#0, ACTUAL := Cell.CompletedPallets);
END_TEST_PROGRAM

(* Per-FB isolation: two PalletizerController instances must keep
   independent state and counters. *)
TEST_PROGRAM PalletizerInstancesAreIndependent
VAR
    CellA : PalletizerController;
    CellB : PalletizerController;
END_VAR

CellA.Initialize();
CellB.Initialize();

(* Drive CellA to Pick *)
CellA.Update(BoxPresent := TRUE, PalletReady := TRUE, RobotHealthy := TRUE, PickVacuumOk := TRUE);
ASSERT_EQUAL(EXPECTED := INT#10, ACTUAL := CellA.CurrentStateId);

(* CellB stays at WaitBox - independent of CellA *)
ASSERT_EQUAL(EXPECTED := INT#0, ACTUAL := CellB.CurrentStateId);

(* Drive CellB independently to Place *)
CellB.Update(BoxPresent := TRUE, PalletReady := TRUE, RobotHealthy := TRUE, PickVacuumOk := TRUE);
CellB.Update(BoxPresent := TRUE, PalletReady := TRUE, RobotHealthy := TRUE, PickVacuumOk := TRUE);
ASSERT_EQUAL(EXPECTED := INT#20, ACTUAL := CellB.CurrentStateId);

(* CellA still at Pick - the LastCommandId on each is independent *)
ASSERT_EQUAL(EXPECTED := INT#10, ACTUAL := CellA.CurrentStateId);
END_TEST_PROGRAM

(* Complete state restart-friendly: when a new box and pallet appear
   in the same scan that completes the previous pallet, the controller
   must advance straight from Complete (100) into Pick (10), not lose
   the trigger by returning to WaitBox (0) for one wasted cycle. *)
TEST_PROGRAM PalletizerCompleteStateForwardsTriggerToPick
VAR
    Cell : PalletizerController;
END_VAR

Cell.Initialize();
(* Drive through to Complete *)
Cell.Update(BoxPresent := TRUE, PalletReady := TRUE, RobotHealthy := TRUE, PickVacuumOk := TRUE);
Cell.Update(BoxPresent := TRUE, PalletReady := TRUE, RobotHealthy := TRUE, PickVacuumOk := TRUE);
Cell.Update(BoxPresent := TRUE, PalletReady := TRUE, RobotHealthy := TRUE, PickVacuumOk := TRUE);
ASSERT_EQUAL(EXPECTED := INT#100, ACTUAL := Cell.Update(BoxPresent := TRUE, PalletReady := TRUE, RobotHealthy := TRUE, PickVacuumOk := TRUE));
ASSERT_EQUAL(EXPECTED := DWORD#1, ACTUAL := Cell.CompletedPallets);
(* Next box and pallet ready in same scan as Complete - forward to Pick (10) *)
ASSERT_EQUAL(EXPECTED := INT#10, ACTUAL := Cell.Update(BoxPresent := TRUE, PalletReady := TRUE, RobotHealthy := TRUE, PickVacuumOk := TRUE));
END_TEST_PROGRAM
