(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. *)
TEST_PROGRAM Lesson48FeedbackPathsPreserveDcAuthorityAndPhysicalEquilibria
ASSERT_NEAR(EXPECTED := LREAL#0.4, ACTUAL := Lesson48ProportionalTorque(
    ControllerGain := LREAL#0.4, SpeedSetpoint := LREAL#1.0,
    MeasuredSpeed := LREAL#0.0), DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.8, ACTUAL := Lesson48ClosedLoopEquilibrium(
    PlantGain := LREAL#10.0, ControllerGain := LREAL#0.4,
    SpeedSetpoint := LREAL#1.0, OpposingLoadTorque := LREAL#0.0),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.6, ACTUAL := Lesson48ClosedLoopEquilibrium(
    PlantGain := LREAL#10.0, ControllerGain := LREAL#0.4,
    SpeedSetpoint := LREAL#1.0, OpposingLoadTorque := LREAL#0.1),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#1.0, ACTUAL := Lesson48FactorSteadyOutput(
    InputValue := LREAL#1.0, ZeroRate := LREAL#0.3,
    PoleRate := LREAL#3.0), DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#1.0, ACTUAL := Lesson48FactorSteadyOutput(
    InputValue := LREAL#1.0, ZeroRate := LREAL#0.8,
    PoleRate := LREAL#0.08), DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM
