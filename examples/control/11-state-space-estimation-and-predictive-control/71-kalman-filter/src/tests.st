(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. *)
TEST_PROGRAM Lesson71ScheduleCovarianceAndFallbackInputsAreExplicit
VAR
    PriorCovariance : LREAL;
    Gain : LREAL;
END_VAR
ASSERT_NEAR(EXPECTED := LREAL#5.0,
    ACTUAL := Lesson71ReviewedCommand(
        ExperimentElapsedSeconds := LREAL#0.1, ControlAuthority := TRUE),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#10.0,
    ACTUAL := Lesson71ReviewedCommand(
        ExperimentElapsedSeconds := LREAL#5.0, ControlAuthority := TRUE),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.0,
    ACTUAL := Lesson71ReviewedCommand(
        ExperimentElapsedSeconds := LREAL#10.0, ControlAuthority := FALSE),
    DELTA := LREAL#1.0e-12);
ASSERT_TRUE(IN := Lesson71CovarianceInputsValid(
    ProcessVariance := LREAL#0.1,
    MeasurementVariance := LREAL#3.0,
    CorrectedCovariance := LREAL#1.9));
ASSERT_FALSE(IN := Lesson71CovarianceInputsValid(
    ProcessVariance := LREAL#-0.1,
    MeasurementVariance := LREAL#3.0,
    CorrectedCovariance := LREAL#1.9));
PriorCovariance := Lesson71PredictedCovariance(
    CorrectedCovariance := LREAL#1.9,
    ProcessVariance := LREAL#0.1);
Gain := Lesson71KalmanGain(
    PredictedCovariance := PriorCovariance,
    MeasurementVariance := LREAL#3.0);
ASSERT_NEAR(EXPECTED := LREAL#1.655588430848166,
    ACTUAL := PriorCovariance, DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.3556131422352871,
    ACTUAL := Gain, DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM
