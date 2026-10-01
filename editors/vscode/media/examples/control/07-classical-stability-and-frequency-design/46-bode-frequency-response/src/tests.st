(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. *)

TEST_PROGRAM TheHeaterFrequencyResponse
(* M = 0.2/√(1 + (5ω)²) and φ = −ω − arctan(5ω): 0.141421 °C per point and −0.985398 rad at 0.2 rad/s. *)
ASSERT_NEAR(EXPECTED := LREAL#0.141421, ACTUAL := HeaterAmplitudeRatio(OmegaRadS := LREAL#0.2), DELTA := LREAL#1.0e-6);
ASSERT_NEAR(EXPECTED := LREAL#-0.985398, ACTUAL := HeaterPhaseRad(OmegaRadS := LREAL#0.2), DELTA := LREAL#1.0e-6);
(* The other two rates of Lesson 53. *)
ASSERT_NEAR(EXPECTED := LREAL#0.178885, ACTUAL := HeaterAmplitudeRatio(OmegaRadS := LREAL#0.1), DELTA := LREAL#1.0e-6);
ASSERT_NEAR(EXPECTED := LREAL#-0.563648, ACTUAL := HeaterPhaseRad(OmegaRadS := LREAL#0.1), DELTA := LREAL#1.0e-6);
ASSERT_NEAR(EXPECTED := LREAL#0.089443, ACTUAL := HeaterAmplitudeRatio(OmegaRadS := LREAL#0.4), DELTA := LREAL#1.0e-6);
ASSERT_NEAR(EXPECTED := LREAL#-1.507149, ACTUAL := HeaterPhaseRad(OmegaRadS := LREAL#0.4), DELTA := LREAL#1.0e-6);
END_TEST_PROGRAM

TEST_PROGRAM ReadingThePrediction
(* A 5-point sine gives 5·0.141421 = 0.7071 °C; the peaks lag by −φ/ω = 4.927 s; 20·log10(0.141421) = −16.99 dB. *)
ASSERT_NEAR(EXPECTED := LREAL#0.70711, ACTUAL := LREAL#5.0 * HeaterAmplitudeRatio(OmegaRadS := LREAL#0.2), DELTA := LREAL#1.0e-5);
ASSERT_NEAR(EXPECTED := LREAL#4.92699, ACTUAL := -HeaterPhaseRad(OmegaRadS := LREAL#0.2) / LREAL#0.2, DELTA := LREAL#1.0e-5);
ASSERT_NEAR(EXPECTED := LREAL#-16.9897, ACTUAL := LREAL#20.0 * LOG(HeaterAmplitudeRatio(OmegaRadS := LREAL#0.2)), DELTA := LREAL#1.0e-4);
END_TEST_PROGRAM
