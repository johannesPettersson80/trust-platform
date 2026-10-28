(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. *)

TEST_PROGRAM TheCrossoverAndTheCriticalDelay
(* K = 2, τ = 5 s: |L| = 1 at ω_c = √3/5 = 0.346410 rad/s for every delay. The plant's phase there is
   −arctan(√3) = −60°, so the delay reaches −180° at td = (2π/3)/ω_c = 6.0460 s. *)
ASSERT_NEAR(EXPECTED := LREAL#0.346410, ACTUAL := CrossoverRadS(LoopGain := LREAL#2.0, TimeConstantS := LREAL#5.0), DELTA := LREAL#1.0e-6);
ASSERT_NEAR(EXPECTED := LREAL#6.045998, ACTUAL := CriticalDelayS(LoopGain := LREAL#2.0, TimeConstantS := LREAL#5.0), DELTA := LREAL#1.0e-6);
ASSERT_NEAR(EXPECTED := LREAL#1.0, ACTUAL := SQRT(LoopReal(LoopGain := LREAL#2.0, TimeConstantS := LREAL#5.0, DelayS := LREAL#8.0, OmegaRadS := LREAL#0.3464102) ** LREAL#2.0
    + LoopImag(LoopGain := LREAL#2.0, TimeConstantS := LREAL#5.0, DelayS := LREAL#8.0, OmegaRadS := LREAL#0.3464102) ** LREAL#2.0), DELTA := LREAL#1.0e-6);
(* At the critical delay the curve passes through −1. *)
ASSERT_NEAR(EXPECTED := LREAL#-1.0, ACTUAL := LoopReal(LoopGain := LREAL#2.0, TimeConstantS := LREAL#5.0, DelayS := LREAL#6.045998, OmegaRadS := LREAL#0.3464102), DELTA := LREAL#1.0e-6);
ASSERT_NEAR(EXPECTED := LREAL#0.0, ACTUAL := LoopImag(LoopGain := LREAL#2.0, TimeConstantS := LREAL#5.0, DelayS := LREAL#6.045998, OmegaRadS := LREAL#0.3464102), DELTA := LREAL#1.0e-6);
END_TEST_PROGRAM

TEST_PROGRAM TheNegativeRealCrossings
(* Where the phase is −180° the curve crosses the negative real axis: at −0.2352 for td = 1 s (ω = 1.688683 rad/s),
   right of −1, and at −1.1765 for td = 8 s (ω = 0.274958 rad/s), left of −1. *)
ASSERT_NEAR(EXPECTED := LREAL#-0.235227, ACTUAL := LoopReal(LoopGain := LREAL#2.0, TimeConstantS := LREAL#5.0, DelayS := LREAL#1.0, OmegaRadS := LREAL#1.688683), DELTA := LREAL#1.0e-5);
ASSERT_NEAR(EXPECTED := LREAL#0.0, ACTUAL := LoopImag(LoopGain := LREAL#2.0, TimeConstantS := LREAL#5.0, DelayS := LREAL#1.0, OmegaRadS := LREAL#1.688683), DELTA := LREAL#1.0e-5);
ASSERT_NEAR(EXPECTED := LREAL#-1.176461, ACTUAL := LoopReal(LoopGain := LREAL#2.0, TimeConstantS := LREAL#5.0, DelayS := LREAL#8.0, OmegaRadS := LREAL#0.274958), DELTA := LREAL#1.0e-5);
ASSERT_NEAR(EXPECTED := LREAL#0.0, ACTUAL := LoopImag(LoopGain := LREAL#2.0, TimeConstantS := LREAL#5.0, DelayS := LREAL#8.0, OmegaRadS := LREAL#0.274958), DELTA := LREAL#1.0e-5);
END_TEST_PROGRAM

TEST_PROGRAM TheGrowingMode
(* For td = 8 s the closed-loop equation 1 + L(s) = 0 is 5s + 1 + 2·e^(−8s) = 0. With s = σ + jω and
   e^(−8s) = e^(−8σ)·(cos 8ω − j·sin 8ω), s = 0.0158047 ± j0.2787088 satisfies it: a mode that grows by
   e^(σ·2π/ω) = 1.428 in each period of 2π/ω = 22.54 s. *)
VAR
    Sigma : LREAL := 0.0158047;
    Omega : LREAL := 0.2787088;
END_VAR
ASSERT_NEAR(EXPECTED := LREAL#0.0, ACTUAL := LREAL#5.0 * Sigma + LREAL#1.0 + LREAL#2.0 * EXP(LREAL#-8.0 * Sigma) * COS(LREAL#8.0 * Omega), DELTA := LREAL#1.0e-5);
ASSERT_NEAR(EXPECTED := LREAL#0.0, ACTUAL := LREAL#5.0 * Omega - LREAL#2.0 * EXP(LREAL#-8.0 * Sigma) * SIN(LREAL#8.0 * Omega), DELTA := LREAL#1.0e-5);
ASSERT_NEAR(EXPECTED := LREAL#1.428, ACTUAL := EXP(Sigma * LREAL#2.0 * LREAL#3.14159265358979 / Omega), DELTA := LREAL#1.0e-3);
END_TEST_PROGRAM
