(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. *)

TEST_PROGRAM TheSplit
(* +40 kW: heater 20 %, cooler 0 %. −40 kW: heater 0 %, cooler 40 %. 30 kW of heating needs 15 %, 30 kW of cooling 30 %. *)
ASSERT_NEAR(EXPECTED := LREAL#20.0, ACTUAL := HeaterPct(RequestKw := LREAL#40.0), DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.0, ACTUAL := CoolerPct(RequestKw := LREAL#40.0), DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.0, ACTUAL := HeaterPct(RequestKw := LREAL#-40.0), DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#40.0, ACTUAL := CoolerPct(RequestKw := LREAL#-40.0), DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#15.0, ACTUAL := HeaterPct(RequestKw := LREAL#30.0), DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#30.0, ACTUAL := CoolerPct(RequestKw := LREAL#-30.0), DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM

TEST_PROGRAM TheNetHeat
(* Percentages do not add: heater 15 % is +30 kW, cooler 30 % is −30 kW. The limits give −100…200 kW. *)
ASSERT_NEAR(EXPECTED := LREAL#30.0, ACTUAL := NetHeatKw(HeaterPct := LREAL#15.0, CoolerPct := LREAL#0.0), DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#-30.0, ACTUAL := NetHeatKw(HeaterPct := LREAL#0.0, CoolerPct := LREAL#30.0), DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#200.0, ACTUAL := NetHeatKw(HeaterPct := HeaterPct(RequestKw := LREAL#300.0), CoolerPct := CoolerPct(RequestKw := LREAL#300.0)), DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#-100.0, ACTUAL := NetHeatKw(HeaterPct := HeaterPct(RequestKw := LREAL#-150.0), CoolerPct := CoolerPct(RequestKw := LREAL#-150.0)), DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM
