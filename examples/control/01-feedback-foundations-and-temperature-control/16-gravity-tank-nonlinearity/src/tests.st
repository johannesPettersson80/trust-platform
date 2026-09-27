(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. *)
TEST_PROGRAM Lesson16GravityBalancePredictsFlowAndEquilibrium
VAR
    OutletFlow : LREAL;
END_VAR
OutletFlow := Lesson16GravityOutflow(
    OutletCoefficient := LREAL#0.02,
    LevelMetres := LREAL#0.25
);
ASSERT_NEAR(EXPECTED := LREAL#0.01, ACTUAL := OutletFlow, DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.002, ACTUAL := Lesson16InitialLevelRate(
    InletFlow := LREAL#0.011,
    OutletFlow := OutletFlow,
    TankArea := LREAL#0.5
), DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.3025, ACTUAL := Lesson16EquilibriumLevel(
    InletFlow := LREAL#0.011,
    OutletCoefficient := LREAL#0.02
), DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM
