(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. *)
TEST_PROGRAM Lesson14ConservationPredictsTankLevel
VAR
    LevelRate : LREAL;
END_VAR
LevelRate := Lesson14LevelRateMetresPerSecond(
    InletFlowCubicMetresPerSecond := LREAL#0.012,
    OutletFlowCubicMetresPerSecond := LREAL#0.010,
    TankAreaSquareMetres := LREAL#0.5
);
ASSERT_NEAR(EXPECTED := LREAL#0.004, ACTUAL := LevelRate, DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.29, ACTUAL := Lesson14LevelAfterTime(
    InitialLevelMetres := LREAL#0.25,
    LevelRateMetresPerSecond := LevelRate,
    ElapsedSeconds := LREAL#10.0
), DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.0, ACTUAL := Lesson14LevelRateMetresPerSecond(
    InletFlowCubicMetresPerSecond := LREAL#0.010,
    OutletFlowCubicMetresPerSecond := LREAL#0.010,
    TankAreaSquareMetres := LREAL#0.5
), DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM
