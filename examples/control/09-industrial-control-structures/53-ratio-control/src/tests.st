(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. *)
TEST_PROGRAM Lesson53RatioDemandUsesValidFlowAndAbortRemovesAuthority
ASSERT_TRUE(IN := Lesson53RatioAuthority(
    WildFlowValid := TRUE, InputsValid := TRUE, SafetyAbort := FALSE,
    MeasuredWildFlow := LREAL#120.0));
ASSERT_FALSE(IN := Lesson53RatioAuthority(
    WildFlowValid := TRUE, InputsValid := TRUE, SafetyAbort := FALSE,
    MeasuredWildFlow := LREAL#5.0));
ASSERT_FALSE(IN := Lesson53RatioAuthority(
    WildFlowValid := TRUE, InputsValid := FALSE, SafetyAbort := FALSE,
    MeasuredWildFlow := LREAL#120.0));
ASSERT_FALSE(IN := Lesson53RatioAuthority(
    WildFlowValid := TRUE, InputsValid := TRUE, SafetyAbort := TRUE,
    MeasuredWildFlow := LREAL#120.0));
ASSERT_NEAR(EXPECTED := LREAL#12.0,
    ACTUAL := Lesson53RatioDemand(
        RatioSetpoint := Lesson53ConfiguredRatioSetpoint(), MeasuredWildFlow := LREAL#120.0,
        RatioAuthority := TRUE),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.10,
    ACTUAL := Lesson53ConfiguredRatioSetpoint(),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.0,
    ACTUAL := Lesson53RatioDemand(
        RatioSetpoint := Lesson53ConfiguredRatioSetpoint(), MeasuredWildFlow := LREAL#120.0,
        RatioAuthority := FALSE),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#120.0,
    ACTUAL := Lesson53ReviewedWildFlow(
        ExperimentElapsedSeconds := LREAL#30.0,
        ExperimentEnabled := TRUE, InputsValid := TRUE,
        SafetyAbort := FALSE),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#100.0,
    ACTUAL := Lesson53ReviewedWildFlow(
        ExperimentElapsedSeconds := LREAL#30.0,
        ExperimentEnabled := TRUE, InputsValid := TRUE,
        SafetyAbort := TRUE),
    DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM
