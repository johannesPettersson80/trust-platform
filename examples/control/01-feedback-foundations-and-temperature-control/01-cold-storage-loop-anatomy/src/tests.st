(* @trustlsp:runtime-ignore — execute these tests with trust-dev test, not as
   PLC instances when starting the lesson simulator. *)
TEST_PROGRAM Lesson1ManualDemandIsDirectAndBounded
ASSERT_TRUE(
    IN := ColdStorageManualDemand(
        Enabled := TRUE,
        InputsValid := TRUE,
        SafetyAbort := FALSE,
        ExperimentScan := ULINT#1
    ) = LREAL#100.0
);
ASSERT_TRUE(
    IN := ColdStorageManualDemand(
        Enabled := TRUE,
        InputsValid := TRUE,
        SafetyAbort := FALSE,
        ExperimentScan := ULINT#201
    ) = LREAL#0.0
);
ASSERT_TRUE(
    IN := ColdStorageManualDemand(
        Enabled := FALSE,
        InputsValid := TRUE,
        SafetyAbort := FALSE,
        ExperimentScan := ULINT#1
    ) = LREAL#0.0
);
END_TEST_PROGRAM
