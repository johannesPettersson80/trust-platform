(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. *)
TEST_PROGRAM Lesson51FeedforwardComesFromBothPathsAndFailsClosed
ASSERT_NEAR(EXPECTED := LREAL#10.0,
    ACTUAL := Lesson51StaticFeedforward(
        HeaterPathGain := LREAL#2.0,
        DisturbancePathGain := LREAL#-20.0,
        DisturbanceDeviation := LREAL#1.0,
        SpeedValid := TRUE),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#20.0,
    ACTUAL := Lesson51DynamicFeedforward(
        DisturbanceDeviation := LREAL#1.0,
        FilteredDisturbance := LREAL#0.0,
        Scale := LREAL#0.8,
        SpeedValid := TRUE),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#25.0,
    ACTUAL := Lesson51DynamicFeedforward(
        DisturbanceDeviation := LREAL#1.0,
        FilteredDisturbance := LREAL#0.0,
        Scale := LREAL#1.0,
        SpeedValid := TRUE),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#10.0,
    ACTUAL := Lesson51DynamicFeedforward(
        DisturbanceDeviation := LREAL#1.0,
        FilteredDisturbance := LREAL#1.0,
        Scale := LREAL#1.0,
        SpeedValid := TRUE),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.0,
    ACTUAL := Lesson51StaticFeedforward(
        HeaterPathGain := LREAL#2.0,
        DisturbancePathGain := LREAL#-20.0,
        DisturbanceDeviation := LREAL#1.0,
        SpeedValid := FALSE),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.0,
    ACTUAL := Lesson51DynamicFeedforward(
        DisturbanceDeviation := LREAL#1.0,
        FilteredDisturbance := LREAL#0.0,
        Scale := LREAL#1.0,
        SpeedValid := FALSE),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#1.0,
    ACTUAL := Lesson51ReviewedSpeed(
        ExperimentElapsedSeconds := LREAL#29.9,
        ExperimentEnabled := TRUE, InputsValid := TRUE,
        SafetyAbort := FALSE),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#2.0,
    ACTUAL := Lesson51ReviewedSpeed(
        ExperimentElapsedSeconds := LREAL#30.0,
        ExperimentEnabled := TRUE, InputsValid := TRUE,
        SafetyAbort := FALSE),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#1.0,
    ACTUAL := Lesson51ReviewedSpeed(
        ExperimentElapsedSeconds := LREAL#30.0,
        ExperimentEnabled := TRUE, InputsValid := FALSE,
        SafetyAbort := FALSE),
    DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM
