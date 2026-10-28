(* @trustlsp:runtime-ignore — run this arithmetic project with trust-dev test;
   these worked calculations do not drive the simulated heater. *)
FUNCTION Lesson6ProportionalCorrection : LREAL
VAR_INPUT
    ProportionalGain : LREAL;
    Error : LREAL;
END_VAR
Lesson6ProportionalCorrection := ProportionalGain * Error;
END_FUNCTION

FUNCTION Lesson6IntegralIncrement : LREAL
VAR_INPUT
    IntegralGain : LREAL;
    Error : LREAL;
    CycleTime : LREAL;
END_VAR
Lesson6IntegralIncrement := IntegralGain * Error * CycleTime;
END_FUNCTION

FUNCTION Lesson6ApplyHeaterRange : LREAL
VAR_INPUT
    RequestedPower : LREAL;
END_VAR
IF RequestedPower <= LREAL#0.0 THEN
    Lesson6ApplyHeaterRange := LREAL#0.0;
ELSIF RequestedPower >= LREAL#100.0 THEN
    Lesson6ApplyHeaterRange := LREAL#100.0;
ELSE
    Lesson6ApplyHeaterRange := RequestedPower;
END_IF;
END_FUNCTION

TEST_PROGRAM Lesson6SeparatesRequestedAndAppliedPower
ASSERT_TRUE(IN := Lesson6ApplyHeaterRange(RequestedPower := LREAL#95.0) = LREAL#95.0);
ASSERT_TRUE(IN := Lesson6ApplyHeaterRange(RequestedPower := LREAL#125.0) = LREAL#100.0);
ASSERT_TRUE(IN := Lesson6ApplyHeaterRange(RequestedPower := LREAL#-5.0) = LREAL#0.0);
END_TEST_PROGRAM

TEST_PROGRAM Lesson6IntegralKeepsGrowingDuringSaturation
VAR
    Error : LREAL;
    P : LREAL;
    IBefore : LREAL;
    IIncrement : LREAL;
    IAfter : LREAL;
    Requested : LREAL;
    Applied : LREAL;
END_VAR

Error := LREAL#5.0;
IBefore := LREAL#110.0;
P := Lesson6ProportionalCorrection(ProportionalGain := LREAL#2.0, Error := Error);
IIncrement := Lesson6IntegralIncrement(IntegralGain := LREAL#0.5, Error := Error, CycleTime := LREAL#0.1);
IAfter := IBefore + IIncrement;
Requested := P + IAfter;
Applied := Lesson6ApplyHeaterRange(RequestedPower := Requested);

ASSERT_TRUE(IN := IIncrement = LREAL#0.25);
ASSERT_TRUE(IN := IAfter > IBefore);
ASSERT_TRUE(IN := Requested = LREAL#120.25);
ASSERT_TRUE(IN := Applied = LREAL#100.0);
ASSERT_TRUE(IN := Requested - Applied = LREAL#20.25);
END_TEST_PROGRAM
