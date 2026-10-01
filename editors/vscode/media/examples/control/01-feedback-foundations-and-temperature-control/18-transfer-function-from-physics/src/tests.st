(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. *)

TEST_PROGRAM TheClosedLoopOfTheHeater
(* H(s) = 0.2·Kp/(5s + 1 + 0.2·Kp): gain 2/7 and time constant 25/7 s for Kp = 2; 0.5 and 2.5 s for Kp = 5. *)
ASSERT_NEAR(EXPECTED := LREAL#2.0 / LREAL#7.0, ACTUAL := ClosedLoopGain(ProportionalGain := LREAL#2.0), DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#25.0 / LREAL#7.0, ACTUAL := ClosedLoopTimeS(ProportionalGain := LREAL#2.0), DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.5, ACTUAL := ClosedLoopGain(ProportionalGain := LREAL#5.0), DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#2.5, ACTUAL := ClosedLoopTimeS(ProportionalGain := LREAL#5.0), DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM

TEST_PROGRAM TheInitialStateResponds
(* (5s + 1)·X = 0.2·V + 5·x(0): with V = 0 and x(0) = 1 °C, X = 5/(5s + 1) and x(t) = e^(−t/5): 0.368 °C after 5 s. *)
ASSERT_NEAR(EXPECTED := LREAL#0.36788, ACTUAL := EXP(LREAL#-5.0 / LREAL#5.0), DELTA := LREAL#1.0e-5);
END_TEST_PROGRAM

TEST_PROGRAM ThePiLoopCancelsAFactor
VAR
    S : LREAL;
    Loop : LREAL;
END_VAR
(* C(s) = 2.5 + 0.5/s and G(s) = 0.2/(5s + 1): CG/(1 + CG) = 0.1/(s + 0.1), checked at s = 0.5 and s = 1. *)
S := LREAL#0.5;
Loop := (LREAL#2.5 + LREAL#0.5 / S) * LREAL#0.2 / (LREAL#5.0 * S + LREAL#1.0);
ASSERT_NEAR(EXPECTED := LREAL#0.1 / (S + LREAL#0.1), ACTUAL := Loop / (LREAL#1.0 + Loop), DELTA := LREAL#1.0e-12);
S := LREAL#1.0;
Loop := (LREAL#2.5 + LREAL#0.5 / S) * LREAL#0.2 / (LREAL#5.0 * S + LREAL#1.0);
ASSERT_NEAR(EXPECTED := LREAL#0.1 / (S + LREAL#0.1), ACTUAL := Loop / (LREAL#1.0 + Loop), DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM
