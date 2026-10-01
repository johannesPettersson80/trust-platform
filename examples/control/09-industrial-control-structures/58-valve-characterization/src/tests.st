(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. *)

TEST_PROGRAM RegulatedPressureGivesALinearMap
(* At 100 kPa, 75 → 80 % changes q from 1.5 to 1.6 kg/s: a slope of 0.1/5 = 0.02 kg/(s·point). *)
ASSERT_NEAR(EXPECTED := LREAL#1.5, ACTUAL := ValveFlow(OpeningPct := LREAL#75.0, PressureDropKPa := LREAL#100.0), DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#1.6, ACTUAL := ValveFlow(OpeningPct := LREAL#80.0, PressureDropKPa := LREAL#100.0), DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.02, ACTUAL := LocalSlope(FlowLow := LREAL#1.5, FlowHigh := LREAL#1.6,
    StemLow := LREAL#75.0, StemHigh := LREAL#80.0), DELTA := LREAL#1.0e-12);
(* Four times the pressure drop doubles the flow: q grows with the square root of dp. *)
ASSERT_NEAR(EXPECTED := LREAL#3.0, ACTUAL := ValveFlow(OpeningPct := LREAL#75.0, PressureDropKPa := LREAL#400.0), DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM

TEST_PROGRAM PipeLossFlattensTheInstalledMap
(* Fixed 128.125 kPa supply: the operating point is unchanged, but 85 % gives 1.649 kg/s instead of 1.7. *)
ASSERT_NEAR(EXPECTED := LREAL#1.5, ACTUAL := FixedSupplyFlow(OpeningPct := LREAL#75.0), DELTA := LREAL#1.0e-9);
ASSERT_NEAR(EXPECTED := LREAL#1.64929, ACTUAL := FixedSupplyFlow(OpeningPct := LREAL#85.0), DELTA := LREAL#1.0e-5);
(* The valve then sees 128.125 - 12.5·1.5² = 100 kPa at 75 %. *)
ASSERT_NEAR(EXPECTED := LREAL#1.5, ACTUAL := ValveFlow(OpeningPct := LREAL#75.0,
    PressureDropKPa := LREAL#128.125 - LREAL#12.5 * LREAL#1.5 * LREAL#1.5), DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM

TEST_PROGRAM FlowBecomesHeat
(* 1.5 kg/s of 60 °C water into 35 °C water: 4 · 1.5 · 25 = 150 kW, and 0.02 kg/(s·point) is 2 kW per point. *)
ASSERT_NEAR(EXPECTED := LREAL#150.0, ACTUAL := DeliveredHeatKW(FlowKgS := LREAL#1.5, SupplyC := LREAL#60.0, WaterC := LREAL#35.0), DELTA := LREAL#1.0e-9);
ASSERT_NEAR(EXPECTED := LREAL#2.0, ACTUAL := DeliveredHeatKW(FlowKgS := LREAL#0.02, SupplyC := LREAL#60.0, WaterC := LREAL#35.0), DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM
