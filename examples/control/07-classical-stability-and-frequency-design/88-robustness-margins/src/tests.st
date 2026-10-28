(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. *)

TEST_PROGRAM TheBalancedPiLoop
(* The heater, K = 0.2 °C/%, tau = 5 s, with Lesson 22's balanced PI, Kp = 5.769231 %/°C: a = 1/(lambda + L) = 0.230769 1/s. *)
ASSERT_NEAR(EXPECTED := LREAL#0.230769, ACTUAL := LoopCrossoverRate(ProcessGain := LREAL#0.2, ProportionalGain := LREAL#5.769231,
    TimeConstantS := LREAL#5.0), DELTA := LREAL#1.0e-6);
END_TEST_PROGRAM

TEST_PROGRAM ItsThreeMargins
(* With the 1 s delay: the gain may grow 6.81 times, the phase has 76.8 deg to spare, the delay may grow 5.81 s. *)
ASSERT_NEAR(EXPECTED := LREAL#6.806784, ACTUAL := GainMargin(CrossoverRate := LREAL#0.2307692308, DelayS := LREAL#1.0),
    DELTA := LREAL#1.0e-6);
ASSERT_NEAR(EXPECTED := LREAL#76.777897, ACTUAL := PhaseMarginDeg(CrossoverRate := LREAL#0.2307692308, DelayS := LREAL#1.0),
    DELTA := LREAL#1.0e-6);
ASSERT_NEAR(EXPECTED := LREAL#5.806784, ACTUAL := DelayMarginS(CrossoverRate := LREAL#0.2307692308, DelayS := LREAL#1.0),
    DELTA := LREAL#1.0e-6);
END_TEST_PROGRAM
