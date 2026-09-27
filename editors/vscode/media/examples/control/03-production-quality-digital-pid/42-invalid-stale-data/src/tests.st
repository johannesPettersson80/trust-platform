(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. *)
TEST_PROGRAM Lesson42InvalidInputsAreRejectedBeforeControllerMutation
ASSERT_FALSE(IN := Lesson42NumericRangeIsValid(
    Setpoint := LREAL#6.0,
    ProcessValue := LREAL#13.0,
    ActualValvePosition := LREAL#40.0,
    FeedForward := LREAL#0.0
));
ASSERT_FALSE(IN := Lesson42MeasurementIsFresh(
    QualityGood := TRUE,
    AgeSeconds := LREAL#0.6
));
ASSERT_FALSE(IN := Lesson42TimingIsValid(CycleTime := LREAL#0.0));
ASSERT_FALSE(IN := Lesson42CandidateIsValid(
    ProportionalGain := LREAL#5.0,
    IntegralGain := LREAL#1.0,
    TrackingTime := LREAL#1.0,
    OutputMinimum := LREAL#80.0,
    OutputMaximum := LREAL#20.0,
    RateLimit := LREAL#20.0
));
ASSERT_FALSE(IN := Lesson42ControllerMutationAllowed(
    NumericRangeValid := FALSE,
    MeasurementFresh := TRUE,
    TimingValid := TRUE,
    DeadlineMissed := FALSE,
    ConfigurationValid := TRUE
));
ASSERT_FALSE(IN := Lesson42ControllerMutationAllowed(
    NumericRangeValid := TRUE,
    MeasurementFresh := TRUE,
    TimingValid := TRUE,
    DeadlineMissed := FALSE,
    ConfigurationValid := FALSE
));
ASSERT_TRUE(IN := Lesson42ControllerMutationAllowed(
    NumericRangeValid := TRUE,
    MeasurementFresh := TRUE,
    TimingValid := TRUE,
    DeadlineMissed := FALSE,
    ConfigurationValid := TRUE
));
ASSERT_EQUAL(EXPECTED := INT#1, ACTUAL := Lesson42DetectedFault(
    NumericRangeValid := FALSE,
    MeasurementFresh := FALSE,
    TimingValid := FALSE,
    DeadlineMissed := TRUE,
    ApplyConfiguration := TRUE,
    CandidateConfigurationValid := FALSE
));
ASSERT_EQUAL(EXPECTED := INT#2, ACTUAL := Lesson42DetectedFault(
    NumericRangeValid := TRUE,
    MeasurementFresh := FALSE,
    TimingValid := FALSE,
    DeadlineMissed := TRUE,
    ApplyConfiguration := TRUE,
    CandidateConfigurationValid := FALSE
));
ASSERT_EQUAL(EXPECTED := INT#3, ACTUAL := Lesson42DetectedFault(
    NumericRangeValid := TRUE,
    MeasurementFresh := TRUE,
    TimingValid := FALSE,
    DeadlineMissed := TRUE,
    ApplyConfiguration := TRUE,
    CandidateConfigurationValid := FALSE
));
ASSERT_EQUAL(EXPECTED := INT#4, ACTUAL := Lesson42DetectedFault(
    NumericRangeValid := TRUE,
    MeasurementFresh := TRUE,
    TimingValid := TRUE,
    DeadlineMissed := TRUE,
    ApplyConfiguration := TRUE,
    CandidateConfigurationValid := FALSE
));
ASSERT_EQUAL(EXPECTED := INT#5, ACTUAL := Lesson42DetectedFault(
    NumericRangeValid := TRUE,
    MeasurementFresh := TRUE,
    TimingValid := TRUE,
    DeadlineMissed := FALSE,
    ApplyConfiguration := TRUE,
    CandidateConfigurationValid := FALSE
));
END_TEST_PROGRAM
