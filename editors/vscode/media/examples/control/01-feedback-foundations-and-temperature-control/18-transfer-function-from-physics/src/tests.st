(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. *)
TEST_PROGRAM Lesson18TransferFunctionMatchesLocalStepResponse
VAR
    ResponseAtOneTimeConstant : LREAL;
END_VAR
ResponseAtOneTimeConstant := Lesson18TransferStepResponse(
    InputDeviation := LREAL#0.001,
    Gain := LREAL#50.0,
    TimeConstant := LREAL#25.0,
    ElapsedTime := LREAL#25.0
);
ASSERT_NEAR(EXPECTED := LREAL#0.03160602794142788,
    ACTUAL := ResponseAtOneTimeConstant,
    DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.0002, ACTUAL := Lesson18LocalLevelNext(
    CurrentDeviation := LREAL#0.0,
    InputDeviation := LREAL#0.001,
    Gain := LREAL#50.0,
    TimeConstant := LREAL#25.0,
    ElapsedTime := LREAL#0.1
), DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM
