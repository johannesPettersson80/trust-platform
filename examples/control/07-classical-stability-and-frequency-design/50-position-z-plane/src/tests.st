(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. *)
TEST_PROGRAM Lesson50AuthorityAndExactHoldMatchTheSampledModel
ASSERT_NEAR(EXPECTED := LREAL#1.0, ACTUAL := Lesson50ReviewedSetpoint(
    ExperimentEnabled := TRUE, InputsValid := TRUE, SafetyAbort := FALSE),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.0, ACTUAL := Lesson50ReviewedSetpoint(
    ExperimentEnabled := FALSE, InputsValid := TRUE, SafetyAbort := FALSE),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.0, ACTUAL := Lesson50ReviewedSetpoint(
    ExperimentEnabled := TRUE, InputsValid := FALSE, SafetyAbort := FALSE),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.818730753077982,
    ACTUAL := Lesson50VelocityDecay(SampleSeconds := LREAL#0.1),
    DELTA := LREAL#1.0e-9);
ASSERT_NEAR(EXPECTED := LREAL#0.0468268826949546,
    ACTUAL := Lesson50PositionInputCoefficient(SampleSeconds := LREAL#0.1),
    DELTA := LREAL#1.0e-9);
ASSERT_NEAR(EXPECTED := LREAL#0.906346234610091,
    ACTUAL := Lesson50VelocityInputCoefficient(SampleSeconds := LREAL#0.1),
    DELTA := LREAL#1.0e-9);
ASSERT_NEAR(EXPECTED := LREAL#31.4159265358979,
    ACTUAL := Lesson50NyquistAngularFrequency(SampleSeconds := LREAL#0.1),
    DELTA := LREAL#1.0e-9);
END_TEST_PROGRAM
