(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. *)
TEST_PROGRAM Lesson23StepMarkersProduceModelValues
ASSERT_NEAR(Lesson23ProcessGain(
    InitialInput := LREAL#60.0,
    FinalInput := LREAL#70.0,
    InitialOutput := LREAL#50.0,
    FinalOutput := LREAL#55.0
), LREAL#0.5, LREAL#0.000001);
ASSERT_NEAR(Lesson23Delay(
    StepTime := LREAL#0.0,
    ResponseStartTime := LREAL#3.0
), LREAL#3.0, LREAL#0.000001);
ASSERT_NEAR(Lesson23TimeConstant(
    StepTime := LREAL#0.0,
    ResponseStartTime := LREAL#3.0,
    SixtyThreePercentTime := LREAL#23.0
), LREAL#20.0, LREAL#0.000001);
END_TEST_PROGRAM

TEST_PROGRAM Lesson23ZeroStepDoesNotDivide
VAR
    Gain : LREAL;
    Valid : BOOL := TRUE;
END_VAR
Gain := Lesson23ProcessGain(
    InitialInput := LREAL#75.0,
    FinalInput := LREAL#75.0,
    InitialOutput := LREAL#35.0,
    FinalOutput := LREAL#37.0,
    ENO => Valid
);
(* An invalid step must return normally with a neutral unused result. *)
ASSERT_NEAR(Gain, LREAL#0.0, LREAL#0.000001);
ASSERT_FALSE(Valid);
END_TEST_PROGRAM

TEST_PROGRAM Lesson23SignedAndZeroOutputGainsAreValid
VAR
    Gain : LREAL;
    Valid : BOOL := FALSE;
END_VAR
Gain := Lesson23ProcessGain(InitialInput := LREAL#75.0, FinalInput := LREAL#65.0,
    InitialOutput := LREAL#35.0, FinalOutput := LREAL#33.0, ENO => Valid);
ASSERT_TRUE(Valid);
ASSERT_NEAR(Gain, LREAL#0.2, LREAL#0.000001);
Gain := Lesson23ProcessGain(InitialInput := LREAL#75.0, FinalInput := LREAL#85.0,
    InitialOutput := LREAL#35.0, FinalOutput := LREAL#33.0, ENO => Valid);
ASSERT_TRUE(Valid);
ASSERT_NEAR(Gain, LREAL#-0.2, LREAL#0.000001);
Gain := Lesson23ProcessGain(InitialInput := LREAL#75.0, FinalInput := LREAL#85.0,
    InitialOutput := LREAL#35.0, FinalOutput := LREAL#35.0, ENO => Valid);
ASSERT_TRUE(Valid);
ASSERT_NEAR(Gain, LREAL#0.0, LREAL#0.000001);
END_TEST_PROGRAM
