(* @trustlsp:runtime-ignore — run this arithmetic project with trust-dev test;
   these worked calculations do not drive the simulated heater. *)
FUNCTION Lesson7ProportionalCorrection : LREAL
VAR_INPUT
    ProportionalGain : LREAL;
    Error : LREAL;
END_VAR
Lesson7ProportionalCorrection := ProportionalGain * Error;
END_FUNCTION

FUNCTION Lesson7IntegralIncrement : LREAL
VAR_INPUT
    IntegralGain : LREAL;
    Error : LREAL;
    CycleTime : LREAL;
END_VAR
Lesson7IntegralIncrement := IntegralGain * Error * CycleTime;
END_FUNCTION

FUNCTION Lesson7ApplyHeaterRange : LREAL
VAR_INPUT
    RequestedPower : LREAL;
END_VAR
IF RequestedPower <= LREAL#0.0 THEN
    Lesson7ApplyHeaterRange := LREAL#0.0;
ELSIF RequestedPower >= LREAL#100.0 THEN
    Lesson7ApplyHeaterRange := LREAL#100.0;
ELSE
    Lesson7ApplyHeaterRange := RequestedPower;
END_IF;
END_FUNCTION

FUNCTION Lesson7TrackingCorrection : LREAL
VAR_INPUT
    RequestedPower : LREAL;
    AppliedPower : LREAL;
    CycleTime : LREAL;
    TrackingTime : LREAL;
END_VAR
Lesson7TrackingCorrection :=
    (CycleTime / TrackingTime) * (AppliedPower - RequestedPower);
END_FUNCTION

TEST_PROGRAM Lesson7StoredCorrectionMovesTowardAppliedPower
VAR
    Error : LREAL;
    ProportionalCorrection : LREAL;
    StoredBefore : LREAL;
    IntegralIncrement : LREAL;
    RequestedPower : LREAL;
    AppliedPower : LREAL;
    TrackingCorrection : LREAL;
    StoredAfter : LREAL;
END_VAR

Error := LREAL#5.0;
StoredBefore := LREAL#110.0;
ProportionalCorrection := Lesson7ProportionalCorrection(
    ProportionalGain := LREAL#2.0,
    Error := Error
);
IntegralIncrement := Lesson7IntegralIncrement(
    IntegralGain := LREAL#0.5,
    Error := Error,
    CycleTime := LREAL#0.1
);
RequestedPower := ProportionalCorrection + StoredBefore + IntegralIncrement;
AppliedPower := Lesson7ApplyHeaterRange(RequestedPower := RequestedPower);
TrackingCorrection := Lesson7TrackingCorrection(
    RequestedPower := RequestedPower,
    AppliedPower := AppliedPower,
    CycleTime := LREAL#0.1,
    TrackingTime := LREAL#1.0
);
StoredAfter := StoredBefore + IntegralIncrement + TrackingCorrection;

ASSERT_TRUE(IN := RequestedPower = LREAL#120.25);
ASSERT_TRUE(IN := AppliedPower = LREAL#100.0);
ASSERT_TRUE(IN := TrackingCorrection = LREAL#-2.025);
ASSERT_TRUE(IN := StoredAfter < StoredBefore);
END_TEST_PROGRAM

TEST_PROGRAM Lesson7TrackingCorrectionIsZeroWithinHeaterRange
VAR
    TrackingCorrection : LREAL;
END_VAR

TrackingCorrection := Lesson7TrackingCorrection(
    RequestedPower := LREAL#75.0,
    AppliedPower := LREAL#75.0,
    CycleTime := LREAL#0.1,
    TrackingTime := LREAL#1.0
);

ASSERT_TRUE(IN := TrackingCorrection = LREAL#0.0);
END_TEST_PROGRAM
