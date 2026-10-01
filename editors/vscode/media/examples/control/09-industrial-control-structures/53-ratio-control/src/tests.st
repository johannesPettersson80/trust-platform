(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. *)

TEST_PROGRAM TheRatioTarget
(* R = 0.1: 10 L/min of additive at 100 L/min of carrier, 15 at 150; 10 in 110 litres is 9.09 % of the mixture. *)
ASSERT_NEAR(EXPECTED := LREAL#10.0, ACTUAL := AdditiveSetpointLMin(Ratio := LREAL#0.1, CarrierLMin := LREAL#100.0), DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#15.0, ACTUAL := AdditiveSetpointLMin(Ratio := LREAL#0.1, CarrierLMin := LREAL#150.0), DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.0909091, ACTUAL := MixtureFraction(Ratio := LREAL#0.1), DELTA := LREAL#1.0e-7);
END_TEST_PROGRAM

TEST_PROGRAM TheStopTail
(* From 10 L/min with τ = 1 s the pump still delivers 10·1/60 = 0.1667 L after a zero command. *)
ASSERT_NEAR(EXPECTED := LREAL#0.1666667, ACTUAL := TailVolumeL(FlowLMin := LREAL#10.0, TimeConstantS := LREAL#1.0), DELTA := LREAL#1.0e-7);
END_TEST_PROGRAM
