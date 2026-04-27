TEST_PROGRAM CipStateObjectsAdvanceThroughSequence
VAR
    Cip : CipController;
END_VAR

Cip.Initialize();
ASSERT_EQUAL(EXPECTED := INT#10, ACTUAL := Cip.Update(StartCommand := TRUE, StepComplete := FALSE, ConductivityUs := REAL#200.0, CausticTempC := REAL#75.0, FlowOk := TRUE, EStopHealthy := TRUE));
ASSERT_EQUAL(EXPECTED := INT#20, ACTUAL := Cip.Update(StartCommand := FALSE, StepComplete := TRUE, ConductivityUs := REAL#200.0, CausticTempC := REAL#75.0, FlowOk := TRUE, EStopHealthy := TRUE));
ASSERT_EQUAL(EXPECTED := INT#30, ACTUAL := Cip.Update(StartCommand := FALSE, StepComplete := TRUE, ConductivityUs := REAL#200.0, CausticTempC := REAL#75.0, FlowOk := TRUE, EStopHealthy := TRUE));
ASSERT_EQUAL(EXPECTED := INT#40, ACTUAL := Cip.Update(StartCommand := FALSE, StepComplete := TRUE, ConductivityUs := REAL#200.0, CausticTempC := REAL#75.0, FlowOk := TRUE, EStopHealthy := TRUE));
ASSERT_EQUAL(EXPECTED := INT#50, ACTUAL := Cip.Update(StartCommand := FALSE, StepComplete := TRUE, ConductivityUs := REAL#200.0, CausticTempC := REAL#75.0, FlowOk := TRUE, EStopHealthy := TRUE));
ASSERT_EQUAL(EXPECTED := INT#100, ACTUAL := Cip.Update(StartCommand := FALSE, StepComplete := FALSE, ConductivityUs := REAL#20.0, CausticTempC := REAL#75.0, FlowOk := TRUE, EStopHealthy := TRUE));
ASSERT_EQUAL(EXPECTED := DWORD#1, ACTUAL := Cip.CycleCount);
END_TEST_PROGRAM

TEST_PROGRAM CipStateObjectsAbortFromActiveStep
VAR
    Cip : CipController;
END_VAR

Cip.Initialize();
Cip.Update(StartCommand := TRUE, StepComplete := FALSE, ConductivityUs := REAL#200.0, CausticTempC := REAL#75.0, FlowOk := TRUE, EStopHealthy := TRUE);
ASSERT_EQUAL(EXPECTED := INT#90, ACTUAL := Cip.Update(StartCommand := FALSE, StepComplete := FALSE, ConductivityUs := REAL#200.0, CausticTempC := REAL#75.0, FlowOk := FALSE, EStopHealthy := TRUE));
ASSERT_TRUE(IN := Cip.Aborted);
END_TEST_PROGRAM

(* Integration test: simulates the PROGRAM Main scan loop with the
   Init-once guard. Catches the "Initialize every cycle" bug — without
   the guard, the state machine resets to Idle on every iteration and
   never reaches Done. *)
TEST_PROGRAM CipScanLoopWithInitGuardProgressesThroughSequence
VAR
    Cip : CipController;
    Initialized : BOOL;
    I : INT;
    StateValue : INT;
END_VAR

FOR I := 1 TO 6 DO
    IF NOT Initialized THEN
        Cip.Initialize();
        Initialized := TRUE;
    END_IF;
    CASE I OF
        INT#1: StateValue := Cip.Update(StartCommand := TRUE,  StepComplete := FALSE, ConductivityUs := REAL#200.0, CausticTempC := REAL#75.0, FlowOk := TRUE, EStopHealthy := TRUE);
        INT#2: StateValue := Cip.Update(StartCommand := FALSE, StepComplete := TRUE,  ConductivityUs := REAL#200.0, CausticTempC := REAL#75.0, FlowOk := TRUE, EStopHealthy := TRUE);
        INT#3: StateValue := Cip.Update(StartCommand := FALSE, StepComplete := TRUE,  ConductivityUs := REAL#200.0, CausticTempC := REAL#75.0, FlowOk := TRUE, EStopHealthy := TRUE);
        INT#4: StateValue := Cip.Update(StartCommand := FALSE, StepComplete := TRUE,  ConductivityUs := REAL#200.0, CausticTempC := REAL#75.0, FlowOk := TRUE, EStopHealthy := TRUE);
        INT#5: StateValue := Cip.Update(StartCommand := FALSE, StepComplete := TRUE,  ConductivityUs := REAL#200.0, CausticTempC := REAL#75.0, FlowOk := TRUE, EStopHealthy := TRUE);
        INT#6: StateValue := Cip.Update(StartCommand := FALSE, StepComplete := FALSE, ConductivityUs := REAL#20.0,  CausticTempC := REAL#75.0, FlowOk := TRUE, EStopHealthy := TRUE);
    END_CASE;
END_FOR;
ASSERT_EQUAL(EXPECTED := INT#100, ACTUAL := StateValue);
ASSERT_EQUAL(EXPECTED := DWORD#1, ACTUAL := Cip.CycleCount);
END_TEST_PROGRAM

(* Anti-pattern test: explicitly demonstrates that calling Initialize on
   every cycle resets the state machine. If a future change removes the
   Init-once guard from Main, this test pins the contract: re-init each
   cycle cannot reach the wash-complete state. *)
TEST_PROGRAM CipReinitializingEveryCycleResetsStateMachine
VAR
    Cip : CipController;
    I : INT;
    StateValue : INT;
END_VAR

FOR I := 1 TO 6 DO
    Cip.Initialize();
    StateValue := Cip.Update(StartCommand := TRUE, StepComplete := TRUE, ConductivityUs := REAL#20.0, CausticTempC := REAL#75.0, FlowOk := TRUE, EStopHealthy := TRUE);
END_FOR;
(* Re-initializing every cycle: state machine cannot get past PreRinse *)
ASSERT_TRUE(IN := StateValue <= INT#10);
ASSERT_EQUAL(EXPECTED := DWORD#0, ACTUAL := Cip.CycleCount);
END_TEST_PROGRAM

(* Idle state edge cases: e-stop loss aborts before start; no command holds at 0. *)
TEST_PROGRAM CipIdleStateBranchesCovered
VAR
    Cip : CipController;
END_VAR
Cip.Initialize();
(* No start command: state stays at 0 *)
ASSERT_EQUAL(EXPECTED := INT#0, ACTUAL := Cip.Update(StartCommand := FALSE, StepComplete := FALSE, ConductivityUs := REAL#200.0, CausticTempC := REAL#75.0, FlowOk := TRUE, EStopHealthy := TRUE));
(* E-stop loss while idle: state goes to abort (90) *)
ASSERT_EQUAL(EXPECTED := INT#90, ACTUAL := Cip.Update(StartCommand := TRUE, StepComplete := FALSE, ConductivityUs := REAL#200.0, CausticTempC := REAL#75.0, FlowOk := TRUE, EStopHealthy := FALSE));
ASSERT_TRUE(IN := Cip.Aborted);
END_TEST_PROGRAM

(* CausticWash temperature gate: must reach 70 C before stepping forward. *)
TEST_PROGRAM CipCausticWashHoldsBelowTemperatureThreshold
VAR
    Cip : CipController;
END_VAR
Cip.Initialize();
(* Drive into caustic wash *)
Cip.Update(StartCommand := TRUE, StepComplete := FALSE, ConductivityUs := REAL#200.0, CausticTempC := REAL#75.0, FlowOk := TRUE, EStopHealthy := TRUE);
Cip.Update(StartCommand := FALSE, StepComplete := TRUE, ConductivityUs := REAL#200.0, CausticTempC := REAL#75.0, FlowOk := TRUE, EStopHealthy := TRUE);
ASSERT_EQUAL(EXPECTED := INT#20, ACTUAL := Cip.CurrentStateId);
(* Below threshold even with StepComplete - stays in caustic *)
ASSERT_EQUAL(EXPECTED := INT#20, ACTUAL := Cip.Update(StartCommand := FALSE, StepComplete := TRUE, ConductivityUs := REAL#200.0, CausticTempC := REAL#69.9, FlowOk := TRUE, EStopHealthy := TRUE));
(* Below 70 without StepComplete - stays *)
ASSERT_EQUAL(EXPECTED := INT#20, ACTUAL := Cip.Update(StartCommand := FALSE, StepComplete := FALSE, ConductivityUs := REAL#200.0, CausticTempC := REAL#65.0, FlowOk := TRUE, EStopHealthy := TRUE));
(* At 70 + StepComplete - advances to intermediate rinse (30) *)
ASSERT_EQUAL(EXPECTED := INT#30, ACTUAL := Cip.Update(StartCommand := FALSE, StepComplete := TRUE, ConductivityUs := REAL#200.0, CausticTempC := REAL#70.0, FlowOk := TRUE, EStopHealthy := TRUE));
END_TEST_PROGRAM

(* FinalRinse conductivity gate: must drop below 50 uS/cm to complete. *)
TEST_PROGRAM CipFinalRinseHoldsAboveConductivityThreshold
VAR
    Cip : CipController;
END_VAR
Cip.Initialize();
(* Drive through to FinalRinse *)
Cip.Update(StartCommand := TRUE, StepComplete := FALSE, ConductivityUs := REAL#200.0, CausticTempC := REAL#75.0, FlowOk := TRUE, EStopHealthy := TRUE);
Cip.Update(StartCommand := FALSE, StepComplete := TRUE, ConductivityUs := REAL#200.0, CausticTempC := REAL#75.0, FlowOk := TRUE, EStopHealthy := TRUE);
Cip.Update(StartCommand := FALSE, StepComplete := TRUE, ConductivityUs := REAL#200.0, CausticTempC := REAL#75.0, FlowOk := TRUE, EStopHealthy := TRUE);
Cip.Update(StartCommand := FALSE, StepComplete := TRUE, ConductivityUs := REAL#200.0, CausticTempC := REAL#75.0, FlowOk := TRUE, EStopHealthy := TRUE);
Cip.Update(StartCommand := FALSE, StepComplete := TRUE, ConductivityUs := REAL#200.0, CausticTempC := REAL#75.0, FlowOk := TRUE, EStopHealthy := TRUE);
ASSERT_EQUAL(EXPECTED := INT#50, ACTUAL := Cip.CurrentStateId);
(* Conductivity above 50: stays in FinalRinse *)
ASSERT_EQUAL(EXPECTED := INT#50, ACTUAL := Cip.Update(StartCommand := FALSE, StepComplete := FALSE, ConductivityUs := REAL#100.0, CausticTempC := REAL#75.0, FlowOk := TRUE, EStopHealthy := TRUE));
(* Just at 50: stays (strict less-than required) *)
ASSERT_EQUAL(EXPECTED := INT#50, ACTUAL := Cip.Update(StartCommand := FALSE, StepComplete := FALSE, ConductivityUs := REAL#50.0, CausticTempC := REAL#75.0, FlowOk := TRUE, EStopHealthy := TRUE));
(* Just below 50: completes *)
ASSERT_EQUAL(EXPECTED := INT#100, ACTUAL := Cip.Update(StartCommand := FALSE, StepComplete := FALSE, ConductivityUs := REAL#49.9, CausticTempC := REAL#75.0, FlowOk := TRUE, EStopHealthy := TRUE));
END_TEST_PROGRAM

(* Done state is terminal: any further Update returns 100. *)
TEST_PROGRAM CipDoneStateIsTerminal
VAR
    Cip : CipController;
END_VAR
Cip.Initialize();
(* Drive to done *)
Cip.Update(StartCommand := TRUE, StepComplete := FALSE, ConductivityUs := REAL#200.0, CausticTempC := REAL#75.0, FlowOk := TRUE, EStopHealthy := TRUE);
Cip.Update(StartCommand := FALSE, StepComplete := TRUE, ConductivityUs := REAL#200.0, CausticTempC := REAL#75.0, FlowOk := TRUE, EStopHealthy := TRUE);
Cip.Update(StartCommand := FALSE, StepComplete := TRUE, ConductivityUs := REAL#200.0, CausticTempC := REAL#75.0, FlowOk := TRUE, EStopHealthy := TRUE);
Cip.Update(StartCommand := FALSE, StepComplete := TRUE, ConductivityUs := REAL#200.0, CausticTempC := REAL#75.0, FlowOk := TRUE, EStopHealthy := TRUE);
Cip.Update(StartCommand := FALSE, StepComplete := TRUE, ConductivityUs := REAL#200.0, CausticTempC := REAL#75.0, FlowOk := TRUE, EStopHealthy := TRUE);
Cip.Update(StartCommand := FALSE, StepComplete := FALSE, ConductivityUs := REAL#20.0, CausticTempC := REAL#75.0, FlowOk := TRUE, EStopHealthy := TRUE);
ASSERT_EQUAL(EXPECTED := INT#100, ACTUAL := Cip.CurrentStateId);
(* Subsequent Updates with any inputs keep state at 100 *)
ASSERT_EQUAL(EXPECTED := INT#100, ACTUAL := Cip.Update(StartCommand := TRUE, StepComplete := TRUE, ConductivityUs := REAL#5.0, CausticTempC := REAL#80.0, FlowOk := TRUE, EStopHealthy := TRUE));
ASSERT_EQUAL(EXPECTED := INT#100, ACTUAL := Cip.Update(StartCommand := FALSE, StepComplete := FALSE, ConductivityUs := REAL#200.0, CausticTempC := REAL#0.0, FlowOk := FALSE, EStopHealthy := FALSE));
END_TEST_PROGRAM

(* Abort state is terminal: any further Update returns 90 even with all inputs healthy. *)
TEST_PROGRAM CipAbortStateIsTerminal
VAR
    Cip : CipController;
END_VAR
Cip.Initialize();
Cip.Update(StartCommand := TRUE, StepComplete := FALSE, ConductivityUs := REAL#200.0, CausticTempC := REAL#75.0, FlowOk := TRUE, EStopHealthy := TRUE);
Cip.Update(StartCommand := FALSE, StepComplete := FALSE, ConductivityUs := REAL#200.0, CausticTempC := REAL#75.0, FlowOk := FALSE, EStopHealthy := TRUE);
ASSERT_EQUAL(EXPECTED := INT#90, ACTUAL := Cip.CurrentStateId);
(* Healthy inputs after abort: still at 90 *)
ASSERT_EQUAL(EXPECTED := INT#90, ACTUAL := Cip.Update(StartCommand := TRUE, StepComplete := TRUE, ConductivityUs := REAL#20.0, CausticTempC := REAL#75.0, FlowOk := TRUE, EStopHealthy := TRUE));
ASSERT_TRUE(IN := Cip.Aborted);
END_TEST_PROGRAM

(* CycleCount increments only on Done transitions, not on Abort. *)
TEST_PROGRAM CipCycleCountTracksOnlyCompletions
VAR
    Cip : CipController;
END_VAR
Cip.Initialize();
(* First wash: complete cycle - cycle count := 1 *)
Cip.Update(StartCommand := TRUE, StepComplete := FALSE, ConductivityUs := REAL#200.0, CausticTempC := REAL#75.0, FlowOk := TRUE, EStopHealthy := TRUE);
Cip.Update(StartCommand := FALSE, StepComplete := TRUE, ConductivityUs := REAL#200.0, CausticTempC := REAL#75.0, FlowOk := TRUE, EStopHealthy := TRUE);
Cip.Update(StartCommand := FALSE, StepComplete := TRUE, ConductivityUs := REAL#200.0, CausticTempC := REAL#75.0, FlowOk := TRUE, EStopHealthy := TRUE);
Cip.Update(StartCommand := FALSE, StepComplete := TRUE, ConductivityUs := REAL#200.0, CausticTempC := REAL#75.0, FlowOk := TRUE, EStopHealthy := TRUE);
Cip.Update(StartCommand := FALSE, StepComplete := TRUE, ConductivityUs := REAL#200.0, CausticTempC := REAL#75.0, FlowOk := TRUE, EStopHealthy := TRUE);
Cip.Update(StartCommand := FALSE, StepComplete := FALSE, ConductivityUs := REAL#20.0, CausticTempC := REAL#75.0, FlowOk := TRUE, EStopHealthy := TRUE);
ASSERT_EQUAL(EXPECTED := DWORD#1, ACTUAL := Cip.CycleCount);

(* Second wash starts after Done is terminal - CycleCount unchanged *)
Cip.Update(StartCommand := TRUE, StepComplete := TRUE, ConductivityUs := REAL#20.0, CausticTempC := REAL#75.0, FlowOk := TRUE, EStopHealthy := TRUE);
ASSERT_EQUAL(EXPECTED := DWORD#1, ACTUAL := Cip.CycleCount);
END_TEST_PROGRAM

(* PreRinse and IntermediateRinse: flow loss aborts at any moment. *)
TEST_PROGRAM CipFlowLossAbortsRinseStates
VAR
    Cip : CipController;
END_VAR
Cip.Initialize();
(* Enter PreRinse, lose flow *)
Cip.Update(StartCommand := TRUE, StepComplete := FALSE, ConductivityUs := REAL#200.0, CausticTempC := REAL#75.0, FlowOk := TRUE, EStopHealthy := TRUE);
ASSERT_EQUAL(EXPECTED := INT#90, ACTUAL := Cip.Update(StartCommand := FALSE, StepComplete := FALSE, ConductivityUs := REAL#200.0, CausticTempC := REAL#75.0, FlowOk := FALSE, EStopHealthy := TRUE));
ASSERT_TRUE(IN := Cip.Aborted);
END_TEST_PROGRAM

(* AcidWash flow loss aborts. *)
TEST_PROGRAM CipFlowLossAbortsAcidWash
VAR
    Cip : CipController;
END_VAR
Cip.Initialize();
(* Drive to AcidWash *)
Cip.Update(StartCommand := TRUE, StepComplete := FALSE, ConductivityUs := REAL#200.0, CausticTempC := REAL#75.0, FlowOk := TRUE, EStopHealthy := TRUE);
Cip.Update(StartCommand := FALSE, StepComplete := TRUE, ConductivityUs := REAL#200.0, CausticTempC := REAL#75.0, FlowOk := TRUE, EStopHealthy := TRUE);
Cip.Update(StartCommand := FALSE, StepComplete := TRUE, ConductivityUs := REAL#200.0, CausticTempC := REAL#75.0, FlowOk := TRUE, EStopHealthy := TRUE);
Cip.Update(StartCommand := FALSE, StepComplete := TRUE, ConductivityUs := REAL#200.0, CausticTempC := REAL#75.0, FlowOk := TRUE, EStopHealthy := TRUE);
ASSERT_EQUAL(EXPECTED := INT#40, ACTUAL := Cip.CurrentStateId);
(* Flow loss while in AcidWash *)
ASSERT_EQUAL(EXPECTED := INT#90, ACTUAL := Cip.Update(StartCommand := FALSE, StepComplete := FALSE, ConductivityUs := REAL#200.0, CausticTempC := REAL#75.0, FlowOk := FALSE, EStopHealthy := TRUE));
END_TEST_PROGRAM

(* TransitionCount accumulates exactly one increment per state change. *)
TEST_PROGRAM CipTransitionCountMatchesStateChanges
VAR
    Cip : CipController;
END_VAR
Cip.Initialize();
(* Each StepComplete = one transition *)
Cip.Update(StartCommand := TRUE, StepComplete := FALSE, ConductivityUs := REAL#200.0, CausticTempC := REAL#75.0, FlowOk := TRUE, EStopHealthy := TRUE);
ASSERT_EQUAL(EXPECTED := INT#1, ACTUAL := Cip.TransitionCount);
(* Stay in PreRinse - no transition *)
Cip.Update(StartCommand := FALSE, StepComplete := FALSE, ConductivityUs := REAL#200.0, CausticTempC := REAL#75.0, FlowOk := TRUE, EStopHealthy := TRUE);
ASSERT_EQUAL(EXPECTED := INT#1, ACTUAL := Cip.TransitionCount);
(* Step complete - transit to caustic *)
Cip.Update(StartCommand := FALSE, StepComplete := TRUE, ConductivityUs := REAL#200.0, CausticTempC := REAL#75.0, FlowOk := TRUE, EStopHealthy := TRUE);
ASSERT_EQUAL(EXPECTED := INT#2, ACTUAL := Cip.TransitionCount);
END_TEST_PROGRAM
