(* @trustlsp:runtime-ignore *)
TEST_PROGRAM Lesson80ThresholdsAndTimingFallbackAreExplicit
ASSERT_NEAR(EXPECTED := LREAL#2.5,
    ACTUAL := Lesson80OnThreshold(
        Setpoint := LREAL#2.0, HysteresisWidth := LREAL#1.0),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#1.5,
    ACTUAL := Lesson80OffThreshold(
        Setpoint := LREAL#2.0, HysteresisWidth := LREAL#1.0),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.1,
    ACTUAL := Lesson80QualifiedCycleTime(MeasuredCycleTime := LREAL#0.0),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.2,
    ACTUAL := Lesson80QualifiedCycleTime(MeasuredCycleTime := LREAL#0.2),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#4.0,
    ACTUAL := Lesson80DoorEquilibriumTemperature(ExperimentScan := DINT#600),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#6.0,
    ACTUAL := Lesson80DoorEquilibriumTemperature(ExperimentScan := DINT#601),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#6.0,
    ACTUAL := Lesson80DoorEquilibriumTemperature(ExperimentScan := DINT#900),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#4.0,
    ACTUAL := Lesson80DoorEquilibriumTemperature(ExperimentScan := DINT#901),
    DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM
