(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. *)
TEST_PROGRAM Lesson43RouthColumnDerivesTheCompleteGainInterval
ASSERT_NEAR(EXPECTED := LREAL#1.0,
    ACTUAL := Lesson43RouthCubicS1Element(
        CubicCoefficient := LREAL#1.0,
        QuadraticCoefficient := LREAL#2.0,
        LinearCoefficient := LREAL#3.0,
        ConstantCoefficient := LREAL#4.0
    ), DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.0,
    ACTUAL := Lesson43RouthCubicS1Element(
        CubicCoefficient := LREAL#1.0,
        QuadraticCoefficient := LREAL#2.0,
        LinearCoefficient := LREAL#3.0,
        ConstantCoefficient := LREAL#6.0
    ), DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#-1.0,
    ACTUAL := Lesson43RouthCubicS1Element(
        CubicCoefficient := LREAL#1.0,
        QuadraticCoefficient := LREAL#2.0,
        LinearCoefficient := LREAL#3.0,
        ConstantCoefficient := LREAL#8.0
    ), DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#-0.4,
    ACTUAL := Lesson43ProportionalRequest(
        Gain := LREAL#4.0, Setpoint := LREAL#0.0,
        Position := LREAL#0.1
    ), DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#100.0,
    ACTUAL := Lesson43ForceClamp(RequestedForce := LREAL#120.0),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#-100.0,
    ACTUAL := Lesson43ForceClamp(RequestedForce := LREAL#-120.0),
    DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM
