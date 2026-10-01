(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. *)

TEST_PROGRAM TheHeaterPole
(* 5s + 1 = 0 at s = −0.2 s⁻¹: a 1 °C deviation decays to e^(−1) = 0.368 °C in 5 s. *)
ASSERT_NEAR(EXPECTED := LREAL#-0.2, ACTUAL := FirstOrderPole(TimeConstantS := LREAL#5.0), DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.36788,
    ACTUAL := RealPoleResponse(Pole := FirstOrderPole(TimeConstantS := LREAL#5.0), Seconds := LREAL#5.0), DELTA := LREAL#1.0e-5);
END_TEST_PROGRAM

TEST_PROGRAM TheShaftPoles
VAR
    Sigma : LREAL;
    Omega : LREAL;
END_VAR
(* s² + s + 4 = 0 at s = −0.5 ± j1.9365: a swing every 2π/ω = 3.245 s; the first peak at π/ω = 1.622 s reaches
   1 + e^(−0.5·1.622) = 1.444 rad. *)
Sigma := ShaftPoleRealPart(Inertia := LREAL#1.0, Friction := LREAL#1.0);
Omega := ShaftPoleImaginaryPart(Inertia := LREAL#1.0, Friction := LREAL#1.0, ProportionalGain := LREAL#4.0);
ASSERT_NEAR(EXPECTED := LREAL#-0.5, ACTUAL := Sigma, DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#1.93649, ACTUAL := Omega, DELTA := LREAL#1.0e-5);
ASSERT_NEAR(EXPECTED := LREAL#3.24462, ACTUAL := LREAL#2.0 * LREAL#3.14159265358979 / Omega, DELTA := LREAL#1.0e-5);
ASSERT_NEAR(EXPECTED := LREAL#1.44434,
    ACTUAL := ComplexPoleStep(Sigma := Sigma, Omega := Omega, Seconds := LREAL#3.14159265358979 / Omega), DELTA := LREAL#1.0e-5);
(* Kp = 16: s = −0.5 ± j3.969, a swing every 1.583 s inside the same envelope e^(−0.5t). *)
ASSERT_NEAR(EXPECTED := LREAL#3.96863,
    ACTUAL := ShaftPoleImaginaryPart(Inertia := LREAL#1.0, Friction := LREAL#1.0, ProportionalGain := LREAL#16.0), DELTA := LREAL#1.0e-5);
END_TEST_PROGRAM

TEST_PROGRAM ThePiLoopPolesAndZeros
VAR
    S : LREAL;
END_VAR
(* PI on the heater without delay, C = 2.5 + 0.5/s: 1 + CG = 0 gives 5s² + 1.5s + 0.1 = 0, the poles −0.1 and −0.2.
   The setpoint numerator 0.5s + 0.1 has its zero at −0.2; the disturbance numerator 0.2s at 0. *)
S := LREAL#-0.1;
ASSERT_NEAR(EXPECTED := LREAL#0.0, ACTUAL := LREAL#5.0 * S * S + LREAL#1.5 * S + LREAL#0.1, DELTA := LREAL#1.0e-12);
S := LREAL#-0.2;
ASSERT_NEAR(EXPECTED := LREAL#0.0, ACTUAL := LREAL#5.0 * S * S + LREAL#1.5 * S + LREAL#0.1, DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.0, ACTUAL := LREAL#0.5 * S + LREAL#0.1, DELTA := LREAL#1.0e-12);
(* A 1 % disturbance step: x(t) = 0.4·(e^(−0.1t) − e^(−0.2t)) °C, largest at t = ln 2/0.1 = 6.93 s with 0.1 °C. *)
ASSERT_NEAR(EXPECTED := LREAL#0.1,
    ACTUAL := LREAL#0.4 * (RealPoleResponse(Pole := LREAL#-0.1, Seconds := LN(LREAL#2.0) / LREAL#0.1)
        - RealPoleResponse(Pole := LREAL#-0.2, Seconds := LN(LREAL#2.0) / LREAL#0.1)), DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM
