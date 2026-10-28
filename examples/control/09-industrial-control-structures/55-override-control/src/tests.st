(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. *)

TEST_PROGRAM TheLowSelect
(* For a heating constraint the smaller request wins: 60 % and 40 % give 40 %. *)
ASSERT_NEAR(EXPECTED := LREAL#40.0, ACTUAL := LowSelect(NormalPct := LREAL#60.0, LimitPct := LREAL#40.0), DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#40.0, ACTUAL := LowSelect(NormalPct := LREAL#40.0, LimitPct := LREAL#60.0), DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM

TEST_PROGRAM TheFeasibleVessel
(* A 37 °C vessel needs a 40.4 °C jacket; 40 °C would need 44 °C. A 42 °C jacket allows at most 38.333 °C, the 41.8 °C
   setpoint of the jacket controller 38.167 °C. *)
ASSERT_NEAR(EXPECTED := LREAL#40.4, ACTUAL := HoldingJacketC(VesselC := LREAL#37.0), DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#44.0, ACTUAL := HoldingJacketC(VesselC := LREAL#40.0), DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#38.333333, ACTUAL := MaximumVesselC(JacketLimitC := LREAL#42.0), DELTA := LREAL#1.0e-6);
ASSERT_NEAR(EXPECTED := LREAL#38.166667, ACTUAL := MaximumVesselC(JacketLimitC := LREAL#41.8), DELTA := LREAL#1.0e-6);
END_TEST_PROGRAM
