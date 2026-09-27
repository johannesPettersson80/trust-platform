(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. *)
TEST_PROGRAM Lesson82MechanicalPredictionsAndScheduleAreExplicit
ASSERT_NEAR(EXPECTED := LREAL#11.180339887498949,
    ACTUAL := Lesson82FlexibleModeFrequency(
        ShaftStiffness := LREAL#20.0, MotorInertia := LREAL#0.2,
        PayloadInertia := LREAL#0.8), DELTA := LREAL#1.0e-9);
ASSERT_NEAR(EXPECTED := LREAL#0.2142857142857143,
    ACTUAL := Lesson82PDSteadyLoadError(
        LoadTorque := LREAL#1.5, ProportionalGain := LREAL#7.0),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.0,
    ACTUAL := Lesson82ReviewedPositionSetpoint(
        ExperimentScan := DINT#100, ControlAuthority := TRUE),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.5,
    ACTUAL := Lesson82ReviewedPositionSetpoint(
        ExperimentScan := DINT#101, ControlAuthority := TRUE),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.0,
    ACTUAL := Lesson82ReviewedPositionSetpoint(
        ExperimentScan := DINT#101, ControlAuthority := FALSE),
    DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM
