(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. *)
TEST_PROGRAM Lesson70DerivedGainsScheduleAndSafeRequestsAreExplicit
ASSERT_NEAR(EXPECTED := LREAL#0.0,
    ACTUAL := Lesson70BearingReference(
        ExperimentElapsedSeconds := LREAL#79.9, ControlAuthority := TRUE),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.1,
    ACTUAL := Lesson70BearingReference(
        ExperimentElapsedSeconds := LREAL#80.0, ControlAuthority := TRUE),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.0,
    ACTUAL := Lesson70BearingReference(
        ExperimentElapsedSeconds := LREAL#100.0, ControlAuthority := FALSE),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#-1.6,
    ACTUAL := Lesson70BearingCurrentRequest(
        Reference := LREAL#0.0,
        Displacement := LREAL#0.1,
        Velocity := LREAL#0.0),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#1.2,
    ACTUAL := Lesson70BearingCurrentRequest(
        Reference := LREAL#0.1,
        Displacement := LREAL#0.0,
        Velocity := LREAL#0.0),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#1.5,
    ACTUAL := Lesson70MotorTorqueRequest(
        Reference := LREAL#1.0,
        EstimatedSpeed := LREAL#1.0,
        EstimatedLoad := LREAL#0.5),
    DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM
