(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. *)
TEST_PROGRAM Lesson59InstalledCurveProbeAndSafeClosureAreExplicit
ASSERT_NEAR(EXPECTED := LREAL#16.0,
    ACTUAL := Lesson59InstalledTargetFlow(StemPercent := LREAL#40.0),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#72.5,
    ACTUAL := Lesson59ReviewedManualDemand(
        ExperimentElapsedSeconds := LREAL#74.0,
        ExperimentEnabled := TRUE, InputsValid := TRUE, SafetyAbort := FALSE),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.0,
    ACTUAL := Lesson59ReviewedManualDemand(
        ExperimentElapsedSeconds := LREAL#74.0,
        ExperimentEnabled := TRUE, InputsValid := TRUE, SafetyAbort := TRUE),
    DELTA := LREAL#1.0e-12);
ASSERT_TRUE(IN := Lesson59ReviewedAutomaticMode(
    ExperimentElapsedSeconds := LREAL#110.0,
    ExperimentEnabled := TRUE, InputsValid := TRUE, SafetyAbort := FALSE));
ASSERT_FALSE(IN := Lesson59ReviewedAutomaticMode(
    ExperimentElapsedSeconds := LREAL#110.0,
    ExperimentEnabled := TRUE, InputsValid := FALSE, SafetyAbort := FALSE));
ASSERT_NEAR(EXPECTED := LREAL#70.0,
    ACTUAL := Lesson59ReviewedFlowSetpoint(AutomaticMode := TRUE),
    DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM
