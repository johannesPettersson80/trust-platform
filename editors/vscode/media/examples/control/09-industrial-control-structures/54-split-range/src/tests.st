(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. *)
TEST_PROGRAM Lesson54MappingsAndAbortAuthorityFollowPhysicalSigns
ASSERT_FALSE(IN := Lesson54ControlAuthority(
    AutomaticAvailable := TRUE, InputsValid := TRUE, SafetyAbort := TRUE));
ASSERT_NEAR(EXPECTED := LREAL#26.0,
    ACTUAL := Lesson54DeadZoneHeating(Demand := LREAL#30.0),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#16.0,
    ACTUAL := Lesson54DeadZoneCooling(Demand := LREAL#-20.0),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#4.0,
    ACTUAL := Lesson54OverlapHeating(Demand := LREAL#0.0),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#4.0,
    ACTUAL := Lesson54OverlapCooling(Demand := LREAL#0.0),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#30.0,
    ACTUAL := Lesson54CharacterizedHeating(Demand := LREAL#30.0),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#13.3333333333333,
    ACTUAL := Lesson54CharacterizedCooling(Demand := LREAL#-20.0),
    DELTA := LREAL#1.0e-9);
ASSERT_NEAR(EXPECTED := LREAL#0.0,
    ACTUAL := Lesson54CharacterizedHeating(Demand := LREAL#0.0),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#84.0,
    ACTUAL := Lesson54ReviewedSetpoint(
        ExperimentElapsedSeconds := LREAL#20.0,
        ExperimentEnabled := TRUE, InputsValid := TRUE, SafetyAbort := FALSE),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#76.0,
    ACTUAL := Lesson54ReviewedSetpoint(
        ExperimentElapsedSeconds := LREAL#80.0,
        ExperimentEnabled := TRUE, InputsValid := TRUE, SafetyAbort := FALSE),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#80.0,
    ACTUAL := Lesson54ReviewedSetpoint(
        ExperimentElapsedSeconds := LREAL#80.0,
        ExperimentEnabled := TRUE, InputsValid := TRUE, SafetyAbort := TRUE),
    DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM
