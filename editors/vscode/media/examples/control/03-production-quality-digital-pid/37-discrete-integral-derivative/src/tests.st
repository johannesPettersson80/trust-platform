(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. *)
TEST_PROGRAM Lesson37IntegralAndDerivativeUseElapsedTime
ASSERT_NEAR(EXPECTED := LREAL#1.008,
    ACTUAL := Lesson37IntegralTorque(
        PreviousIntegralTorque := LREAL#1.0,
        IntegralGain := LREAL#4.0,
        Error := LREAL#0.1,
        ElapsedSeconds := LREAL#0.02),
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.7,
    ACTUAL := Lesson37DerivativeTorque(
        DerivativeGain := LREAL#7.0,
        PreviousPosition := LREAL#0.100,
        CurrentPosition := LREAL#0.102,
        ElapsedSeconds := LREAL#0.02),
    DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM
