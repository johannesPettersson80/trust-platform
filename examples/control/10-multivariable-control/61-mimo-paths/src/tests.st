(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. *)
TEST_PROGRAM Lesson61IndependentExcitationAndSafeLimitsAreExplicit
ASSERT_NEAR(EXPECTED := LREAL#5.0,
    ACTUAL := Lesson61RefluxRequest(
        ExperimentElapsedSeconds := LREAL#20.0,
        ExperimentEnabled := TRUE, InputsValid := TRUE, SafetyAbort := FALSE),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.0,
    ACTUAL := Lesson61ReboilerRequest(
        ExperimentElapsedSeconds := LREAL#20.0,
        ExperimentEnabled := TRUE, InputsValid := TRUE, SafetyAbort := FALSE),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.0,
    ACTUAL := Lesson61RefluxRequest(
        ExperimentElapsedSeconds := LREAL#220.0,
        ExperimentEnabled := TRUE, InputsValid := TRUE, SafetyAbort := FALSE),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#5.0,
    ACTUAL := Lesson61ReboilerRequest(
        ExperimentElapsedSeconds := LREAL#220.0,
        ExperimentEnabled := TRUE, InputsValid := TRUE, SafetyAbort := FALSE),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.0,
    ACTUAL := Lesson61ReboilerRequest(
        ExperimentElapsedSeconds := LREAL#250.0,
        ExperimentEnabled := TRUE, InputsValid := TRUE, SafetyAbort := TRUE),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#10.0,
    ACTUAL := Lesson61AppliedInput(
        RequestedChange := LREAL#12.0, ManualMode := TRUE),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.0,
    ACTUAL := Lesson61AppliedInput(
        RequestedChange := LREAL#5.0, ManualMode := FALSE),
    DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM
