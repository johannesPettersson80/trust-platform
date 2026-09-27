(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. *)
TEST_PROGRAM Lesson19PolesAndZeroPredictResponseClass
ASSERT_NEAR(EXPECTED := LREAL#-0.04, ACTUAL := Lesson19StablePole(
    TimeConstant := LREAL#25.0
), DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#-0.3, ACTUAL := Lesson19OscillatingPoleRealPart(
    NaturalFrequency := LREAL#1.5,
    DampingRatio := LREAL#0.2
), DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#1.469693845669907,
    ACTUAL := Lesson19OscillatingPoleImaginaryPart(
        NaturalFrequency := LREAL#1.5,
        DampingRatio := LREAL#0.2
    ), DELTA := LREAL#1.0e-12);
ASSERT_TRUE(IN := Lesson19InverseResponse(ElapsedTime := LREAL#1.0) < LREAL#0.0);
ASSERT_TRUE(IN := Lesson19InverseResponse(ElapsedTime := LREAL#40.0) > LREAL#0.0);
END_TEST_PROGRAM
