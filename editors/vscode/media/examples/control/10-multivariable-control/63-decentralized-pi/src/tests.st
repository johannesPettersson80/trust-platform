(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. *)
TEST_PROGRAM Lesson63TuningMappingScheduleAndAbortAreExplicit
ASSERT_FALSE(IN := Lesson63ControlAuthority(
    AutomaticMode := TRUE, InputsValid := TRUE, SafetyAbort := TRUE));
ASSERT_NEAR(EXPECTED := LREAL#1.66666666666667,
    ACTUAL := Lesson63ProportionalGain(
        TimeConstantSeconds := LREAL#20.0, ProcessGain := LREAL#0.8,
        DesignTimeSeconds := LREAL#15.0), DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.0833333333333333,
    ACTUAL := Lesson63IntegralGain(
        ProportionalGain := LREAL#1.66666666666667,
        IntegralTimeSeconds := LREAL#20.0), DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#2.0,
    ACTUAL := Lesson63ReviewedOverheadSetpoint(
        ExperimentElapsedSeconds := LREAL#20.0,
        ExperimentEnabled := TRUE, InputsValid := TRUE, SafetyAbort := FALSE),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.0,
    ACTUAL := Lesson63ReviewedOverheadSetpoint(
        ExperimentElapsedSeconds := LREAL#20.0,
        ExperimentEnabled := TRUE, InputsValid := TRUE, SafetyAbort := TRUE),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#1.5,
    ACTUAL := Lesson63DynamicRefluxRequest(
        OverheadCorrection := LREAL#1.0,
        BottomsCorrection := LREAL#1.0,
        FilteredBottomsCorrection := LREAL#1.0),
    DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM
