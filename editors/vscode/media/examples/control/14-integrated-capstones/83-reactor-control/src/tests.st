(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. *)

TEST_PROGRAM TheHeatBalance
(* At 35 °C the vessel loses 30 kW: jacket 38 °C, heater 15 %. At 37 °C: jacket 40.4 °C, heater 17 %. With the 30 kW jacket
   loss at 35 °C the heater must supply 60 kW: 30 %. *)
ASSERT_NEAR(EXPECTED := LREAL#38.0, ACTUAL := HoldingJacketC(VesselC := LREAL#35.0), DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#40.4, ACTUAL := HoldingJacketC(VesselC := LREAL#37.0), DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#15.0, ACTUAL := HoldingCommandPct(VesselC := LREAL#35.0, JacketLossKw := LREAL#0.0), DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#17.0, ACTUAL := HoldingCommandPct(VesselC := LREAL#37.0, JacketLossKw := LREAL#0.0), DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#30.0, ACTUAL := HoldingCommandPct(VesselC := LREAL#35.0, JacketLossKw := LREAL#30.0), DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM

TEST_PROGRAM TheReadingCheck
(* Usable: vessel 30–40 °C and jacket 20–55 °C. *)
ASSERT_TRUE(ReadingsValid(VesselC := LREAL#35.0, JacketC := LREAL#38.0));
ASSERT_TRUE(ReadingsValid(VesselC := LREAL#30.0, JacketC := LREAL#20.0));
ASSERT_FALSE(ReadingsValid(VesselC := LREAL#0.0, JacketC := LREAL#38.0));
ASSERT_FALSE(ReadingsValid(VesselC := LREAL#35.0, JacketC := LREAL#56.0));
END_TEST_PROGRAM

TEST_PROGRAM TheTransferUpdate
(* Cascade, setpoint 37 °C, vessel 35 °C, jacket 38 °C. Manual holds 15 %; the transfer update keeps 15 %. The next update
   adds only integral steps of Lesson 63's design: the jacket setpoint 38 + 0.21·2·0.1 = 38.042 °C, the heater
   15 + 25.25·0.042 + 13.59·0.042·0.1 = 16.117578 %. *)
VAR
    Train : ReactorTrain;
END_VAR
Train.Configure();
Train.Update(SetpointC := LREAL#37.0, VesselC := LREAL#35.0, JacketC := LREAL#38.0, UseCascade := TRUE, Automatic := FALSE);
ASSERT_NEAR(EXPECTED := LREAL#15.0, ACTUAL := Train.AppliedPowerPct, DELTA := LREAL#1.0e-12);
Train.Update(SetpointC := LREAL#37.0, VesselC := LREAL#35.0, JacketC := LREAL#38.0, UseCascade := TRUE, Automatic := TRUE);
ASSERT_NEAR(EXPECTED := LREAL#15.0, ACTUAL := Train.AppliedPowerPct, DELTA := LREAL#1.0e-12);
Train.Update(SetpointC := LREAL#37.0, VesselC := LREAL#35.0, JacketC := LREAL#38.0, UseCascade := TRUE, Automatic := TRUE);
ASSERT_NEAR(EXPECTED := LREAL#38.042, ACTUAL := Train.SelectedJacketC, DELTA := LREAL#1.0e-9);
ASSERT_NEAR(EXPECTED := LREAL#16.117578, ACTUAL := Train.AppliedPowerPct, DELTA := LREAL#1.0e-9);
END_TEST_PROGRAM

TEST_PROGRAM TheFaultLatch
(* A 0 °C vessel reading removes heat in the same update; the latch keeps it off when the reading returns to 35 °C. *)
VAR
    Train : ReactorTrain;
END_VAR
Train.Configure();
Train.Update(SetpointC := LREAL#35.0, VesselC := LREAL#35.0, JacketC := LREAL#38.0, UseCascade := TRUE, Automatic := TRUE);
Train.Update(SetpointC := LREAL#35.0, VesselC := LREAL#0.0, JacketC := LREAL#38.0, UseCascade := TRUE, Automatic := TRUE);
ASSERT_TRUE(Train.Faulted);
ASSERT_NEAR(EXPECTED := LREAL#0.0, ACTUAL := Train.AppliedPowerPct, DELTA := LREAL#1.0e-12);
Train.Update(SetpointC := LREAL#35.0, VesselC := LREAL#35.0, JacketC := LREAL#38.0, UseCascade := TRUE, Automatic := TRUE);
ASSERT_TRUE(Train.Faulted);
ASSERT_NEAR(EXPECTED := LREAL#0.0, ACTUAL := Train.AppliedPowerPct, DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM
