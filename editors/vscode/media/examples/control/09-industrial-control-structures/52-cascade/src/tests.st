(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. *)

TEST_PROGRAM TheSingleLoopGains
(* The single loop, Lesson 19's rule for the whole vessel: K = 1 °C/%, τ = 25 s, λ = 10 s → 2.5 %/°C and 0.1 %/(°C·s). *)
ASSERT_NEAR(EXPECTED := LREAL#2.5, ACTUAL := ModelBasedKp(Gain := LREAL#1.0, TimeConstantS := LREAL#25.0, ResponseTimeS := LREAL#10.0), DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.1, ACTUAL := ModelBasedKi(Gain := LREAL#1.0, ResponseTimeS := LREAL#10.0), DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM

TEST_PROGRAM TheHoldingPoint
(* At 35 °C the vessel loses 30 kW: the jacket must be at 38 °C and the heater at 15 %; with a 30 kW jacket extraction, 30 %. *)
ASSERT_NEAR(EXPECTED := LREAL#38.0, ACTUAL := HoldingJacketC(VesselC := LREAL#35.0), DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#15.0, ACTUAL := HoldingCommandPct(VesselC := LREAL#35.0, JacketLoadKw := LREAL#0.0), DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#30.0, ACTUAL := HoldingCommandPct(VesselC := LREAL#35.0, JacketLoadKw := LREAL#30.0), DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM
