(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. *)

TEST_PROGRAM TheResonanceLiftsTheLoopGainAboveOne
(* Lesson 42's PID on the rigid axis has loop gain 0.386 at 3 Hz; the peak factor 1/(2·0.05) = 10 lifts it to 3.86. *)
ASSERT_NEAR(EXPECTED := LREAL#3.8616, ACTUAL := LoopGainAtResonance(ProportionalGain := LREAL#18.77, IntegralGain := LREAL#17.46,
    DerivativeGain := LREAL#7.27, Inertia := LREAL#1.0, Friction := LREAL#1.0, ResonanceRadS := LREAL#18.84955592153876,
    DampingRatio := LREAL#0.05), DELTA := LREAL#0.0001);
END_TEST_PROGRAM

TEST_PROGRAM TheNotchRemovesItsFrequencyAndPassesTheSteadyState
ASSERT_NEAR(EXPECTED := LREAL#0.0, ACTUAL := NotchGain(OmegaRadS := LREAL#18.84955592153876, NotchRadS := LREAL#18.84955592153876,
    Quality := LREAL#1.0), DELTA := LREAL#1.0e-9);
ASSERT_NEAR(EXPECTED := LREAL#1.0, ACTUAL := NotchGain(OmegaRadS := LREAL#0.001, NotchRadS := LREAL#18.84955592153876,
    Quality := LREAL#1.0), DELTA := LREAL#1.0e-6);
(* At 2 Hz the wide notch still passes only 0.64 of the signal, the narrow one 0.93. *)
ASSERT_NEAR(EXPECTED := LREAL#0.6402, ACTUAL := NotchGain(OmegaRadS := LREAL#12.566370614359172, NotchRadS := LREAL#18.84955592153876,
    Quality := LREAL#1.0), DELTA := LREAL#0.0001);
ASSERT_NEAR(EXPECTED := LREAL#0.9285, ACTUAL := NotchGain(OmegaRadS := LREAL#12.566370614359172, NotchRadS := LREAL#18.84955592153876,
    Quality := LREAL#3.0), DELTA := LREAL#0.0001);
END_TEST_PROGRAM

TEST_PROGRAM AWiderNotchCostsMorePhaseAtCrossover
(* At the rigid loop's crossover, 7.33 rad/s. *)
ASSERT_NEAR(EXPECTED := LREAL#24.63, ACTUAL := NotchPhaseLagDeg(OmegaRadS := LREAL#7.334, NotchRadS := LREAL#18.84955592153876,
    Quality := LREAL#1.0), DELTA := LREAL#0.01);
ASSERT_NEAR(EXPECTED := LREAL#8.69, ACTUAL := NotchPhaseLagDeg(OmegaRadS := LREAL#7.334, NotchRadS := LREAL#18.84955592153876,
    Quality := LREAL#3.0), DELTA := LREAL#0.01);
END_TEST_PROGRAM
