(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. *)

TEST_PROGRAM TheFeedforwardCommand
(* 2 kW per command point: 30 kW needs 15 points, from 75 % to 90 %; an estimate of 36 kW asks for 18. *)
ASSERT_NEAR(EXPECTED := LREAL#15.0, ACTUAL := LoadFeedforwardPct(MeasuredLoadKw := LREAL#30.0, KwPerPoint := LREAL#2.0), DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#18.0, ACTUAL := LoadFeedforwardPct(MeasuredLoadKw := LREAL#36.0, KwPerPoint := LREAL#2.0), DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM

TEST_PROGRAM TheUnavoidableDrop
(* 30 kW with H = 10 kW/°C and τ = 5 s: in the 1 s before the extra heat arrives the water falls by
   3·(1 − e^(−1/5)) = 0.5438 °C. *)
ASSERT_NEAR(EXPECTED := LREAL#0.5438077, ACTUAL := UnavoidableDropC(LoadKw := LREAL#30.0, LossKwPerC := LREAL#10.0, TimeConstantS := LREAL#5.0, DelayS := LREAL#1.0), DELTA := LREAL#1.0e-7);
END_TEST_PROGRAM
