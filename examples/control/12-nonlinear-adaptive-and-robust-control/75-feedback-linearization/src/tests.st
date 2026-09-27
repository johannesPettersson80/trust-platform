(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. *)
TEST_PROGRAM Lesson75ScheduleGainAndGravityModelAreExplicit
ASSERT_NEAR(EXPECTED := LREAL#1.2,
    ACTUAL := Lesson75ReviewedSetpoint(
        ExperimentElapsedSeconds := LREAL#1.0, ControlAuthority := TRUE),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#-1.2,
    ACTUAL := Lesson75ReviewedSetpoint(
        ExperimentElapsedSeconds := LREAL#16.0, ControlAuthority := TRUE),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.0,
    ACTUAL := Lesson75ReviewedSetpoint(
        ExperimentElapsedSeconds := LREAL#20.0, ControlAuthority := FALSE),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#15.19,
    ACTUAL := Lesson75ScheduledProportionalGain(SetpointAngle := LREAL#0.0),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#16.903457618,
    ACTUAL := Lesson75ScheduledProportionalGain(SetpointAngle := LREAL#0.6),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#21.445270429,
    ACTUAL := Lesson75ScheduledProportionalGain(SetpointAngle := LREAL#-1.2),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#4.703164533707231,
    ACTUAL := Lesson75GravityCompensation(
        EstimatedGravityCoefficient := LREAL#9.81,
        JointAngle := LREAL#0.5),
    DELTA := LREAL#1.0e-9);
END_TEST_PROGRAM
