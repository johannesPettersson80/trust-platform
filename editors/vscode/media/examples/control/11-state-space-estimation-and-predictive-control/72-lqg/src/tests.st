(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. *)
TEST_PROGRAM Lesson72RoadScheduleAndDerivedForceRequestsAreExplicit
ASSERT_NEAR(EXPECTED := LREAL#0.0,
    ACTUAL := Lesson72ReviewedRoadAcceleration(
        ExperimentElapsedSeconds := LREAL#0.9, ControlAuthority := TRUE),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.5,
    ACTUAL := Lesson72ReviewedRoadAcceleration(
        ExperimentElapsedSeconds := LREAL#1.0, ControlAuthority := TRUE),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.0,
    ACTUAL := Lesson72ReviewedRoadAcceleration(
        ExperimentElapsedSeconds := LREAL#1.2, ControlAuthority := FALSE),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#-0.390000,
    ACTUAL := Lesson72ForceRequest(
        GainDeflection := LREAL#13.0,
        GainVelocity := LREAL#6.0,
        Deflection := LREAL#0.03,
        Velocity := LREAL#0.0),
    DELTA := LREAL#1.0e-9);
ASSERT_NEAR(EXPECTED := LREAL#-0.0659411709,
    ACTUAL := Lesson72ForceRequest(
        GainDeflection := LREAL#2.19803903,
        GainVelocity := LREAL#1.90722139,
        Deflection := LREAL#0.03,
        Velocity := LREAL#0.0),
    DELTA := LREAL#1.0e-9);
END_TEST_PROGRAM
