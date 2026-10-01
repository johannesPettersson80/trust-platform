(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. *)

TEST_PROGRAM TheLoopAtTheTestRate
(* L(j0.2) = −j·e^(−j0.2) = −sin 0.2 − j·cos 0.2: |L| = 1 at −101.46°. *)
ASSERT_NEAR(EXPECTED := LREAL#-0.198669, ACTUAL := LoopReal(OmegaRadS := LREAL#0.2), DELTA := LREAL#1.0e-6);
ASSERT_NEAR(EXPECTED := LREAL#-0.980067, ACTUAL := LoopImag(OmegaRadS := LREAL#0.2), DELTA := LREAL#1.0e-6);
END_TEST_PROGRAM

TEST_PROGRAM SensitivityAndComplement
VAR
    A : LREAL;
    B : LREAL;
    SReal : LREAL;
    SImag : LREAL;
    TReal : LREAL;
    TImag : LREAL;
END_VAR
(* |1 + L| = √(2 − 2·sin 0.2) = 1.26596, so |S| = |T| = 0.78991. *)
ASSERT_NEAR(EXPECTED := LREAL#0.78991, ACTUAL := SensitivityMagnitude(LoopRealPart := LoopReal(OmegaRadS := LREAL#0.2),
    LoopImagPart := LoopImag(OmegaRadS := LREAL#0.2)), DELTA := LREAL#1.0e-5);
ASSERT_NEAR(EXPECTED := LREAL#0.78991, ACTUAL := ComplementaryMagnitude(LoopRealPart := LoopReal(OmegaRadS := LREAL#0.2),
    LoopImagPart := LoopImag(OmegaRadS := LREAL#0.2)), DELTA := LREAL#1.0e-5);
(* S + T = 1 as complex numbers, yet |S| + |T| = 1.58. *)
A := LREAL#1.0 + LoopReal(OmegaRadS := LREAL#0.2);
B := LoopImag(OmegaRadS := LREAL#0.2);
SReal := A / (A * A + B * B);
SImag := -B / (A * A + B * B);
(* T = L/(1 + L) = L·(A − jB)/(A² + B²). *)
TReal := (LoopReal(OmegaRadS := LREAL#0.2) * A + LoopImag(OmegaRadS := LREAL#0.2) * B) / (A * A + B * B);
TImag := (LoopImag(OmegaRadS := LREAL#0.2) * A - LoopReal(OmegaRadS := LREAL#0.2) * B) / (A * A + B * B);
ASSERT_NEAR(EXPECTED := LREAL#1.0, ACTUAL := SReal + TReal, DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.0, ACTUAL := SImag + TImag, DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#1.57982, ACTUAL := SQRT(SReal * SReal + SImag * SImag) + SQRT(TReal * TReal + TImag * TImag), DELTA := LREAL#1.0e-5);
ASSERT_NEAR(EXPECTED := SQRT(SReal * SReal + SImag * SImag), ACTUAL := SensitivityMagnitude(LoopRealPart := LoopReal(OmegaRadS := LREAL#0.2),
    LoopImagPart := LoopImag(OmegaRadS := LREAL#0.2)), DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM

TEST_PROGRAM ThePredictedAmplitudes
(* 10 kW load: 10·(0.1/√2)·|S| = 0.5586 °C. Noise 0.05 °C: water |T|·0.05 = 0.0395 °C, command |C|·|S|·0.05 = 0.2793 %. *)
ASSERT_NEAR(EXPECTED := LREAL#0.5586, ACTUAL := LREAL#10.0 * LREAL#0.1 / SQRT(LREAL#2.0)
    * SensitivityMagnitude(LoopRealPart := LoopReal(OmegaRadS := LREAL#0.2), LoopImagPart := LoopImag(OmegaRadS := LREAL#0.2)), DELTA := LREAL#1.0e-4);
ASSERT_NEAR(EXPECTED := LREAL#0.0395, ACTUAL := LREAL#0.05
    * ComplementaryMagnitude(LoopRealPart := LoopReal(OmegaRadS := LREAL#0.2), LoopImagPart := LoopImag(OmegaRadS := LREAL#0.2)), DELTA := LREAL#1.0e-4);
ASSERT_NEAR(EXPECTED := LREAL#0.2793, ACTUAL := LREAL#5.0 * SQRT(LREAL#2.0) * LREAL#0.05
    * SensitivityMagnitude(LoopRealPart := LoopReal(OmegaRadS := LREAL#0.2), LoopImagPart := LoopImag(OmegaRadS := LREAL#0.2)), DELTA := LREAL#1.0e-4);
END_TEST_PROGRAM
