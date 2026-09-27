(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. *)
TEST_PROGRAM Lesson76ScheduleAndDerivedGainsAreExplicit
ASSERT_NEAR(EXPECTED := LREAL#0.0,
    ACTUAL := Lesson76ReviewedPressureSetpoint(
        ExperimentElapsedSeconds := LREAL#0.0, ControlAuthority := FALSE),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#40.0,
    ACTUAL := Lesson76ReviewedPressureSetpoint(
        ExperimentElapsedSeconds := LREAL#0.0, ControlAuthority := TRUE),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#50.0,
    ACTUAL := Lesson76ReviewedPressureSetpoint(
        ExperimentElapsedSeconds := LREAL#30.0, ControlAuthority := TRUE),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#60.0,
    ACTUAL := Lesson76ReviewedPressureSetpoint(
        ExperimentElapsedSeconds := LREAL#90.0, ControlAuthority := TRUE),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#1.0,
    ACTUAL := Lesson76RecipeGain(ResinGrade := INT#0),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.75,
    ACTUAL := Lesson76RecipeGain(ResinGrade := INT#1),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#1.666666666666667,
    ACTUAL := Lesson76ProportionalGain(DesignGain := LREAL#1.0),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.444444444444444,
    ACTUAL := Lesson76IntegralGain(DesignGain := LREAL#0.75),
    DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM
