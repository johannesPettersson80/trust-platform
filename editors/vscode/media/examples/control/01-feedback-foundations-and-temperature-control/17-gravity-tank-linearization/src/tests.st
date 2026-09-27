(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. *)
TEST_PROGRAM Lesson17TangentModelHasLocalValidity
VAR
    Gain : LREAL;
    TimeConstant : LREAL;
END_VAR
Gain := Lesson17LocalGain(
    OutletCoefficient := LREAL#0.02,
    OperatingLevel := LREAL#0.25
);
TimeConstant := Lesson17TimeConstant(
    TankArea := LREAL#0.5,
    OutletCoefficient := LREAL#0.02,
    OperatingLevel := LREAL#0.25
);
ASSERT_NEAR(EXPECTED := LREAL#50.0, ACTUAL := Gain, DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#25.0, ACTUAL := TimeConstant, DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.3025, ACTUAL := Lesson17NonlinearEquilibrium(
    InletFlow := LREAL#0.011,
    OutletCoefficient := LREAL#0.02
), DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.5625, ACTUAL := Lesson17NonlinearEquilibrium(
    InletFlow := LREAL#0.015,
    OutletCoefficient := LREAL#0.02
), DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM
