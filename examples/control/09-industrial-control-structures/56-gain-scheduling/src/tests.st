(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. *)
TEST_PROGRAM Lesson56ScheduleAndAbortAuthorityFollowTheDerivedTankModel
ASSERT_FALSE(IN := Lesson56ControlAvailable(
    SchedulingAvailable := TRUE, InputsValid := TRUE, SafetyAbort := TRUE));
ASSERT_TRUE(IN := Lesson56ControlAvailable(
    SchedulingAvailable := TRUE, InputsValid := TRUE, SafetyAbort := FALSE));
ASSERT_NEAR(EXPECTED := LREAL#0.008,
    ACTUAL := Lesson56EquilibriumFlow(LevelMetres := LREAL#0.16),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#50.0,
    ACTUAL := Lesson56LocalGain(LevelMetres := LREAL#0.25),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.0025,
    ACTUAL := Lesson56LocalIntegralGain(LevelMetres := LREAL#0.16),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.0020,
    ACTUAL := Lesson56LocalIntegralGain(LevelMetres := LREAL#0.25),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.00166666666666667,
    ACTUAL := Lesson56LocalIntegralGain(LevelMetres := LREAL#0.36),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.25,
    ACTUAL := Lesson56ReviewedLevelSetpoint(
        ExperimentElapsedSeconds := LREAL#30.0,
        ExperimentEnabled := TRUE, InputsValid := TRUE, SafetyAbort := FALSE),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.36,
    ACTUAL := Lesson56ReviewedLevelSetpoint(
        ExperimentElapsedSeconds := LREAL#120.0,
        ExperimentEnabled := TRUE, InputsValid := TRUE, SafetyAbort := FALSE),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.16,
    ACTUAL := Lesson56ReviewedLevelSetpoint(
        ExperimentElapsedSeconds := LREAL#200.0,
        ExperimentEnabled := TRUE, InputsValid := TRUE, SafetyAbort := TRUE),
    DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM
