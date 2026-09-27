(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. *)
TEST_PROGRAM Lesson55SelectorDirectionScheduleAndAbortAreExplicit
ASSERT_FALSE(IN := Lesson55ControlAuthority(
    MarginValid := TRUE, InputsValid := TRUE, SafetyAbort := TRUE));
ASSERT_FALSE(IN := Lesson55ControlAuthority(
    MarginValid := FALSE, InputsValid := TRUE, SafetyAbort := FALSE));
ASSERT_TRUE(IN := Lesson55ControlAuthority(
    MarginValid := TRUE, InputsValid := TRUE, SafetyAbort := FALSE));
ASSERT_NEAR(EXPECTED := LREAL#42.0,
    ACTUAL := Lesson55HighSelect(
        NormalDemand := LREAL#31.0, ProtectiveDemand := LREAL#42.0),
    DELTA := LREAL#1.0e-12);
ASSERT_TRUE(IN := Lesson55TakeoverRequired(MarginPercent := LREAL#14.0));
ASSERT_FALSE(IN := Lesson55TakeoverRequired(MarginPercent := LREAL#14.1));
ASSERT_TRUE(IN := Lesson55ReleasePermitted(
    MarginPercent := LREAL#16.0,
    NormalDemand := LREAL#41.0,
    ProtectiveDemand := LREAL#40.0));
ASSERT_FALSE(IN := Lesson55ReleasePermitted(
    MarginPercent := LREAL#15.9,
    NormalDemand := LREAL#41.0,
    ProtectiveDemand := LREAL#40.0));
ASSERT_NEAR(EXPECTED := LREAL#9.0,
    ACTUAL := Lesson55ReviewedPressureSetpoint(
        ExperimentElapsedSeconds := LREAL#20.0,
        ExperimentEnabled := TRUE, InputsValid := TRUE, SafetyAbort := FALSE),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#8.0,
    ACTUAL := Lesson55ReviewedPressureSetpoint(
        ExperimentElapsedSeconds := LREAL#40.0,
        ExperimentEnabled := TRUE, InputsValid := TRUE, SafetyAbort := TRUE),
    DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM
