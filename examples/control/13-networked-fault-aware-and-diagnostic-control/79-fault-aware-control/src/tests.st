(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. *)
TEST_PROGRAM BothValidAgreeingSensorsUseThePrimary
(* Both readings are fresh, in range and 0.1 °C apart, well within what two ±0.1 °C sensors can differ by: A is used. *)
VAR
    Selector : SensorSelector;
    A : SensorReading;
    B : SensorReading;
END_VAR
Selector.Reset();
A.ValueC := LREAL#35.0;
A.AcquiredMs := UDINT#100;
B.ValueC := LREAL#35.1;
B.AcquiredMs := UDINT#100;
Selector(NowMs := UDINT#100, A := A, B := B, AutoRequest := FALSE);
ASSERT_EQUAL(EXPECTED := INT#1, ACTUAL := Selector.Source);
ASSERT_NEAR(EXPECTED := LREAL#35.0, ACTUAL := Selector.SelectedC, DELTA := LREAL#0.0);
ASSERT_FALSE(IN := Selector.FaultLatched);
ASSERT_EQUAL(EXPECTED := UDINT#0, ACTUAL := Selector.DisagreementMs);
END_TEST_PROGRAM

TEST_PROGRAM AReadingIsUsableUntilItIsOlderThan500Milliseconds
(* A's last acquisition was at 0 ms. At 500 ms it is still valid; at 600 ms it is invalid and B takes over, reported as a
   switch in that update only. A reading acquired after the present time is not credible either. *)
VAR
    Selector : SensorSelector;
    A : SensorReading;
    B : SensorReading;
END_VAR
Selector.Reset();
A.ValueC := LREAL#35.0;
A.AcquiredMs := UDINT#0;
B.ValueC := LREAL#35.05;
B.AcquiredMs := UDINT#500;
Selector(NowMs := UDINT#500, A := A, B := B, AutoRequest := FALSE);
ASSERT_EQUAL(EXPECTED := INT#1, ACTUAL := Selector.Source);
B.AcquiredMs := UDINT#600;
Selector(NowMs := UDINT#600, A := A, B := B, AutoRequest := FALSE);
ASSERT_EQUAL(EXPECTED := INT#2, ACTUAL := Selector.Source);
ASSERT_TRUE(IN := Selector.Switched);
ASSERT_NEAR(EXPECTED := LREAL#35.05, ACTUAL := Selector.SelectedC, DELTA := LREAL#0.0);
B.AcquiredMs := UDINT#700;
Selector(NowMs := UDINT#700, A := A, B := B, AutoRequest := FALSE);
ASSERT_FALSE(IN := Selector.Switched);
A.AcquiredMs := UDINT#900;
A.ValueC := LREAL#35.05;
Selector(NowMs := UDINT#800, A := A, B := B, AutoRequest := FALSE);
ASSERT_EQUAL(EXPECTED := INT#2, ACTUAL := Selector.Source);
ASSERT_FALSE(IN := Selector.FaultLatched);
END_TEST_PROGRAM

TEST_PROGRAM AReadingOutside0To80DegreesIsInvalidAtOnce
VAR
    Selector : SensorSelector;
    A : SensorReading;
    B : SensorReading;
END_VAR
Selector.Reset();
A.ValueC := LREAL#85.0;
A.AcquiredMs := UDINT#100;
B.ValueC := LREAL#35.0;
B.AcquiredMs := UDINT#100;
Selector(NowMs := UDINT#100, A := A, B := B, AutoRequest := FALSE);
ASSERT_EQUAL(EXPECTED := INT#2, ACTUAL := Selector.Source);
A.ValueC := LREAL#-0.5;
A.AcquiredMs := UDINT#200;
B.AcquiredMs := UDINT#200;
Selector(NowMs := UDINT#200, A := A, B := B, AutoRequest := FALSE);
ASSERT_EQUAL(EXPECTED := INT#2, ACTUAL := Selector.Source);
ASSERT_FALSE(IN := Selector.FaultLatched);
END_TEST_PROGRAM

TEST_PROGRAM ASustainedDisagreementLatchesAt200Milliseconds
(* B reads 0.6 °C higher from 5.0 s. Both are fresh and in range, so neither can be chosen: the disagreement is timed 0, 100
   and 200 ms, and at 200 ms the fault latches with no source selected. *)
VAR
    Selector : SensorSelector;
    A : SensorReading;
    B : SensorReading;
    T : UDINT;
END_VAR
Selector.Reset();
FOR T := 5000 TO 5200 BY 100 DO
    A.ValueC := LREAL#35.4;
    A.AcquiredMs := T;
    B.ValueC := LREAL#36.0;
    B.AcquiredMs := T;
    Selector(NowMs := T, A := A, B := B, AutoRequest := FALSE);
    ASSERT_EQUAL(EXPECTED := T - UDINT#5000, ACTUAL := Selector.DisagreementMs);
END_FOR;
ASSERT_TRUE(IN := Selector.FaultLatched);
ASSERT_EQUAL(EXPECTED := INT#0, ACTUAL := Selector.Source);
END_TEST_PROGRAM

TEST_PROGRAM SeparateDisagreementsDoNotAccumulate
(* 0.6 °C apart at 0 and 100 ms, 0.2 °C at 200 ms, 0.6 °C again at 300 and 400 ms: two separate 100 ms intervals, no fault.
   A sensor becoming invalid also restarts the timer. *)
VAR
    Selector : SensorSelector;
    A : SensorReading;
    B : SensorReading;
END_VAR
Selector.Reset();
A.ValueC := LREAL#35.0;
B.ValueC := LREAL#35.6;
A.AcquiredMs := UDINT#0;
B.AcquiredMs := UDINT#0;
Selector(NowMs := UDINT#0, A := A, B := B, AutoRequest := FALSE);
A.AcquiredMs := UDINT#100;
B.AcquiredMs := UDINT#100;
Selector(NowMs := UDINT#100, A := A, B := B, AutoRequest := FALSE);
ASSERT_EQUAL(EXPECTED := UDINT#100, ACTUAL := Selector.DisagreementMs);
B.ValueC := LREAL#35.2;
A.AcquiredMs := UDINT#200;
B.AcquiredMs := UDINT#200;
Selector(NowMs := UDINT#200, A := A, B := B, AutoRequest := FALSE);
ASSERT_EQUAL(EXPECTED := UDINT#0, ACTUAL := Selector.DisagreementMs);
B.ValueC := LREAL#35.6;
A.AcquiredMs := UDINT#300;
B.AcquiredMs := UDINT#300;
Selector(NowMs := UDINT#300, A := A, B := B, AutoRequest := FALSE);
A.AcquiredMs := UDINT#400;
B.AcquiredMs := UDINT#400;
Selector(NowMs := UDINT#400, A := A, B := B, AutoRequest := FALSE);
ASSERT_EQUAL(EXPECTED := UDINT#100, ACTUAL := Selector.DisagreementMs);
ASSERT_FALSE(IN := Selector.FaultLatched);
A.ValueC := LREAL#85.0;
A.AcquiredMs := UDINT#500;
B.AcquiredMs := UDINT#500;
Selector(NowMs := UDINT#500, A := A, B := B, AutoRequest := FALSE);
ASSERT_EQUAL(EXPECTED := UDINT#0, ACTUAL := Selector.DisagreementMs);
ASSERT_EQUAL(EXPECTED := INT#2, ACTUAL := Selector.Source);
END_TEST_PROGRAM

TEST_PROGRAM NoValidSensorLatchesTheFault
VAR
    Selector : SensorSelector;
    A : SensorReading;
    B : SensorReading;
END_VAR
Selector.Reset();
A.ValueC := LREAL#85.0;
A.AcquiredMs := UDINT#100;
B.ValueC := LREAL#35.0;
B.AcquiredMs := UDINT#0;
Selector(NowMs := UDINT#600, A := A, B := B, AutoRequest := FALSE);
ASSERT_TRUE(IN := Selector.FaultLatched);
ASSERT_EQUAL(EXPECTED := INT#0, ACTUAL := Selector.Source);
END_TEST_PROGRAM

TEST_PROGRAM RecoveryNeedsThreeFreshAgreeingReadingsAndANewAutoRequest
(* After a latched disagreement, an Auto request is refused while the sensors disagree. Agreeing readings count only when both
   sensors have a new acquisition; after three the measurement is qualified, but only a new Auto request, not one that is
   still held, resumes control on A. *)
VAR
    Selector : SensorSelector;
    A : SensorReading;
    B : SensorReading;
    T : UDINT;
END_VAR
Selector.Reset();
A.ValueC := LREAL#35.0;
B.ValueC := LREAL#35.6;
FOR T := 0 TO 200 BY 100 DO
    A.AcquiredMs := T;
    B.AcquiredMs := T;
    Selector(NowMs := T, A := A, B := B, AutoRequest := FALSE);
END_FOR;
ASSERT_TRUE(IN := Selector.FaultLatched);
A.AcquiredMs := UDINT#300;
B.AcquiredMs := UDINT#300;
Selector(NowMs := UDINT#300, A := A, B := B, AutoRequest := TRUE);
ASSERT_TRUE(IN := Selector.FaultLatched);
ASSERT_EQUAL(EXPECTED := UDINT#0, ACTUAL := Selector.Agreeing);
B.ValueC := LREAL#35.05;
A.AcquiredMs := UDINT#400;
B.AcquiredMs := UDINT#400;
Selector(NowMs := UDINT#400, A := A, B := B, AutoRequest := TRUE);
ASSERT_EQUAL(EXPECTED := UDINT#1, ACTUAL := Selector.Agreeing);
Selector(NowMs := UDINT#500, A := A, B := B, AutoRequest := TRUE);
ASSERT_EQUAL(EXPECTED := UDINT#1, ACTUAL := Selector.Agreeing);
FOR T := 600 TO 700 BY 100 DO
    A.AcquiredMs := T;
    B.AcquiredMs := T;
    Selector(NowMs := T, A := A, B := B, AutoRequest := TRUE);
END_FOR;
ASSERT_EQUAL(EXPECTED := UDINT#3, ACTUAL := Selector.Agreeing);
ASSERT_TRUE(IN := Selector.Qualified);
ASSERT_TRUE(IN := Selector.FaultLatched);
A.AcquiredMs := UDINT#800;
B.AcquiredMs := UDINT#800;
Selector(NowMs := UDINT#800, A := A, B := B, AutoRequest := FALSE);
A.AcquiredMs := UDINT#900;
B.AcquiredMs := UDINT#900;
Selector(NowMs := UDINT#900, A := A, B := B, AutoRequest := TRUE);
ASSERT_FALSE(IN := Selector.FaultLatched);
ASSERT_EQUAL(EXPECTED := INT#1, ACTUAL := Selector.Source);
END_TEST_PROGRAM
