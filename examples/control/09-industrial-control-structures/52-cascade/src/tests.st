(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. *)
TEST_PROGRAM Lesson52PhysicalPredictionsAndCommissioningOrderAreExplicit
ASSERT_NEAR(EXPECTED := LREAL#3.57142857142857,
    ACTUAL := Lesson52RequiredValveDeviation(
        JacketDeviationDegC := LREAL#5.0,
        UtilityDisturbanceDegC := LREAL#0.0,
        ValveToJacketGain := LREAL#1.4),
    DELTA := LREAL#1.0e-9);
ASSERT_NEAR(EXPECTED := LREAL#7.14285714285714,
    ACTUAL := Lesson52RequiredValveDeviation(
        JacketDeviationDegC := LREAL#0.0,
        UtilityDisturbanceDegC := LREAL#-10.0,
        ValveToJacketGain := LREAL#1.4),
    DELTA := LREAL#1.0e-9);
ASSERT_TRUE(IN := Lesson52CommissioningActive(
    ExperimentElapsedSeconds := LREAL#39.9,
    ExperimentEnabled := TRUE, InputsValid := TRUE, SafetyAbort := FALSE));
ASSERT_FALSE(IN := Lesson52CommissioningActive(
    ExperimentElapsedSeconds := LREAL#40.0,
    ExperimentEnabled := TRUE, InputsValid := TRUE, SafetyAbort := FALSE));
ASSERT_NEAR(EXPECTED := LREAL#75.0,
    ACTUAL := Lesson52JacketTestSetpoint(
        ExperimentElapsedSeconds := LREAL#10.0,
        ExperimentEnabled := TRUE, InputsValid := TRUE, SafetyAbort := FALSE),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#70.0,
    ACTUAL := Lesson52JacketTestSetpoint(
        ExperimentElapsedSeconds := LREAL#30.0,
        ExperimentEnabled := TRUE, InputsValid := TRUE, SafetyAbort := FALSE),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#-10.0,
    ACTUAL := Lesson52UtilityDisturbance(
        ExperimentElapsedSeconds := LREAL#40.0,
        ExperimentEnabled := TRUE, InputsValid := TRUE, SafetyAbort := FALSE),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.0,
    ACTUAL := Lesson52UtilityDisturbance(
        ExperimentElapsedSeconds := LREAL#40.0,
        ExperimentEnabled := TRUE, InputsValid := FALSE, SafetyAbort := FALSE),
    DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM
