(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. *)

TEST_PROGRAM ThePrediction
(* Delay-free model 36 °C, delayed model 35.4 °C: c = 0.6 °C. With the measurement 35.2 °C the controller sees
   35.2 + 0.6 = 35.8 °C, which is 36 + (35.2 − 35.4). *)
ASSERT_NEAR(EXPECTED := LREAL#35.8, ACTUAL := PredictedTemperatureC(MeasuredC := LREAL#35.2, CorrectionC := LREAL#0.6), DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM

TEST_PROGRAM TheCorrectionUpdate
(* K 0.2 °C/%, T 5 s, one 0.1 s update: from c = 0 with the command 10 points above the command θ earlier,
   c = 0.2·10·(1 − e^(−0.02)) = 0.0396027 °C; with both commands equal, c = 0.5 °C decays to 0.5·e^(−0.02) = 0.4900993 °C;
   without a model (K = 0) it stays 0. *)
ASSERT_NEAR(EXPECTED := LREAL#0.0396027, ACTUAL := NextCorrectionC(CorrectionC := LREAL#0.0, CommandPct := LREAL#85.0, EarlierPct := LREAL#75.0, ModelGain := LREAL#0.2, ModelTimeConstantS := LREAL#5.0, UpdateS := LREAL#0.1), DELTA := LREAL#1.0e-7);
ASSERT_NEAR(EXPECTED := LREAL#0.4900993, ACTUAL := NextCorrectionC(CorrectionC := LREAL#0.5, CommandPct := LREAL#75.0, EarlierPct := LREAL#75.0, ModelGain := LREAL#0.2, ModelTimeConstantS := LREAL#5.0, UpdateS := LREAL#0.1), DELTA := LREAL#1.0e-7);
ASSERT_NEAR(EXPECTED := LREAL#0.0, ACTUAL := NextCorrectionC(CorrectionC := LREAL#0.0, CommandPct := LREAL#85.0, EarlierPct := LREAL#75.0, ModelGain := LREAL#0.0, ModelTimeConstantS := LREAL#5.0, UpdateS := LREAL#0.1), DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM
