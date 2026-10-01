(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. *)

TEST_PROGRAM TheGainRows
(* A 5-point heater-1 step raises vessel 1 by 45/65 = 0.6923 °C and vessel 2 by 20/65 = 0.3077 °C; heater 2 swaps them.
   Both rises satisfy the steady balance 18·x1 − 8·x2 = 2·v1. *)
ASSERT_NEAR(EXPECTED := LREAL#0.6923077, ACTUAL := SteadyRise1C(V1Pct := LREAL#5.0, V2Pct := LREAL#0.0), DELTA := LREAL#1.0e-7);
ASSERT_NEAR(EXPECTED := LREAL#0.3076923, ACTUAL := SteadyRise2C(V1Pct := LREAL#5.0, V2Pct := LREAL#0.0), DELTA := LREAL#1.0e-7);
ASSERT_NEAR(EXPECTED := LREAL#0.3076923, ACTUAL := SteadyRise1C(V1Pct := LREAL#0.0, V2Pct := LREAL#5.0), DELTA := LREAL#1.0e-7);
ASSERT_NEAR(EXPECTED := LREAL#0.6923077, ACTUAL := SteadyRise2C(V1Pct := LREAL#0.0, V2Pct := LREAL#5.0), DELTA := LREAL#1.0e-7);
ASSERT_NEAR(EXPECTED := LREAL#10.0, ACTUAL := LREAL#18.0 * SteadyRise1C(V1Pct := LREAL#5.0, V2Pct := LREAL#0.0)
    - LREAL#8.0 * SteadyRise2C(V1Pct := LREAL#5.0, V2Pct := LREAL#0.0), DELTA := LREAL#1.0e-9);
END_TEST_PROGRAM

TEST_PROGRAM TheModelUsesOldTemperatures
(* From 35/35 °C with heater 1 at 80 %: the first step moves only vessel 1, to 35.02 °C, because the wall flow is
   calculated from the old, equal temperatures. The second carries 8·0.02 = 0.16 kW: 35.03928 and 35.00032 °C. *)
VAR
    Model : CoupledVesselModel;
END_VAR
Model(Heater1Pct := LREAL#80.0, Heater2Pct := LREAL#75.0);
ASSERT_NEAR(EXPECTED := LREAL#35.02, ACTUAL := Model.Vessel1C, DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#35.0, ACTUAL := Model.Vessel2C, DELTA := LREAL#1.0e-12);
Model(Heater1Pct := LREAL#80.0, Heater2Pct := LREAL#75.0);
ASSERT_NEAR(EXPECTED := LREAL#0.16, ACTUAL := Model.TransferKw, DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#35.03928, ACTUAL := Model.Vessel1C, DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#35.00032, ACTUAL := Model.Vessel2C, DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM

TEST_PROGRAM TheModelSettlesAtTheGainMatrix
(* After 100 s with heater 1 at 80 % the model has reached 35 + 5·9/65 and 35 + 5·4/65 °C. *)
VAR
    Model : CoupledVesselModel;
    Count : INT;
END_VAR
FOR Count := 1 TO 1000 DO
    Model(Heater1Pct := LREAL#80.0, Heater2Pct := LREAL#75.0);
END_FOR;
ASSERT_NEAR(EXPECTED := LREAL#35.6923077, ACTUAL := Model.Vessel1C, DELTA := LREAL#1.0e-6);
ASSERT_NEAR(EXPECTED := LREAL#35.3076923, ACTUAL := Model.Vessel2C, DELTA := LREAL#1.0e-6);
END_TEST_PROGRAM
