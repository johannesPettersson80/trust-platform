(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. *)
TEST_PROGRAM Lesson65FeasibilityScheduleScalingAndAbortAreExplicit
ASSERT_FALSE(IN := Lesson65ControlAuthority(
    AutomaticMode := TRUE, InputsValid := TRUE, SafetyAbort := TRUE));
ASSERT_NEAR(EXPECTED := LREAL#1.0,
    ACTUAL := Lesson65ReviewedSetpoint(
        ExperimentElapsedSeconds := LREAL#20.0,
        ExperimentEnabled := TRUE, InputsValid := TRUE, SafetyAbort := FALSE),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#2.0,
    ACTUAL := Lesson65ReviewedSetpoint(
        ExperimentElapsedSeconds := LREAL#480.0,
        ExperimentEnabled := TRUE, InputsValid := TRUE, SafetyAbort := FALSE),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.0,
    ACTUAL := Lesson65ReviewedSetpoint(
        ExperimentElapsedSeconds := LREAL#500.0,
        ExperimentEnabled := TRUE, InputsValid := TRUE, SafetyAbort := TRUE),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#1.0,
    ACTUAL := Lesson65CommonScale(RequestedPeak := LREAL#8.0),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.8,
    ACTUAL := Lesson65CommonScale(RequestedPeak := LREAL#12.5),
    DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM
