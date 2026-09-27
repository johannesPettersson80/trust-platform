(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. *)
TEST_PROGRAM Lesson57TuningPredictionScheduleAndAbortAreExplicit
ASSERT_FALSE(IN := Lesson57ControlAuthority(
    AutoMode := TRUE, InputsValid := TRUE, SafetyAbort := TRUE));
ASSERT_TRUE(IN := Lesson57ControlAuthority(
    AutoMode := TRUE, InputsValid := TRUE, SafetyAbort := FALSE));
ASSERT_NEAR(EXPECTED := LREAL#0.46875,
    ACTUAL := Lesson57ProportionalGain(
        TimeConstantSeconds := LREAL#12.0, ProcessGain := LREAL#0.8,
        DesignTimeSeconds := LREAL#16.0,
        DelayUsedForDesignSeconds := LREAL#16.0),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.0390625,
    ACTUAL := Lesson57IntegralGain(
        ProportionalGain := LREAL#0.46875,
        IntegralTimeSeconds := LREAL#12.0),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#23.0,
    ACTUAL := Lesson57PredictedTemperature(
        DelayFreeModel := LREAL#24.0,
        MeasuredTemperature := LREAL#21.0,
        DelayedModel := LREAL#22.0),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#35.0,
    ACTUAL := Lesson57ReviewedSetpoint(
        ExperimentElapsedSeconds := LREAL#10.0,
        ExperimentEnabled := TRUE, InputsValid := TRUE, SafetyAbort := FALSE),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#20.0,
    ACTUAL := Lesson57ReviewedSetpoint(
        ExperimentElapsedSeconds := LREAL#70.0,
        ExperimentEnabled := TRUE, InputsValid := TRUE, SafetyAbort := TRUE),
    DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM
