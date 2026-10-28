(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. *)

TEST_PROGRAM TrackingMovesTheIntegralTowardTheStem
(* Stem at 75 %, request 80.1 %, dt = 0.1 s, Tt = 1 s: the integral moves by -0.51 point. *)
ASSERT_NEAR(EXPECTED := LREAL#-0.51, ACTUAL := MeasuredTrackingCorrection(StemPosition := LREAL#75.0,
    Request := LREAL#80.1, Elapsed := LREAL#0.1, TrackingTime := LREAL#1.0), DELTA := LREAL#1.0e-9);
(* A stem that has reached the request needs no correction. *)
ASSERT_NEAR(EXPECTED := LREAL#0.0, ACTUAL := MeasuredTrackingCorrection(StemPosition := LREAL#80.0,
    Request := LREAL#80.0, Elapsed := LREAL#0.1, TrackingTime := LREAL#1.0), DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM

TEST_PROGRAM RateLimitSetsTheTravelTime
VAR
    Seconds : LREAL;
END_VAR
(* At 0.5 % per second, 75 → 85 % takes 10 points / 0.5 %/s = 20 s. *)
Seconds := (LREAL#85.0 - LREAL#75.0) / LREAL#0.5;
ASSERT_NEAR(EXPECTED := LREAL#20.0, ACTUAL := Seconds, DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM
