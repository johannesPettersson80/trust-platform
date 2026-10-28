(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. *)
TEST_PROGRAM TheHeaterCanHoldEveryRequiredTemperature
(* u = [H·(T − 20) + D]/a with a = 2 kW per point: 60, 75 and 90 % hold 35 °C; the 36 °C step needs at most 96 % and the 10 kW
   extraction at most 95 %, both at 12 kW/°C. Every requirement is physically feasible before any controller is chosen. *)
ASSERT_NEAR(EXPECTED := LREAL#60.0, ACTUAL := HoldingCommandPct(LossKwPerC := LREAL#8.0, TemperatureC := LREAL#35.0, ExtractionKw := LREAL#0.0), DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#75.0, ACTUAL := HoldingCommandPct(LossKwPerC := LREAL#10.0, TemperatureC := LREAL#35.0, ExtractionKw := LREAL#0.0), DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#96.0, ACTUAL := HoldingCommandPct(LossKwPerC := LREAL#12.0, TemperatureC := LREAL#36.0, ExtractionKw := LREAL#0.0), DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#95.0, ACTUAL := HoldingCommandPct(LossKwPerC := LREAL#12.0, TemperatureC := LREAL#35.0, ExtractionKw := LREAL#10.0), DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM

TEST_PROGRAM TheFeedforwardCancelsTheMeasuredExtractionAfterTheDelay
(* Lesson 62: u_ff = D/a, 5 points for 10 kW. During the 1 s delivery delay the extraction is uncompensated and the water falls
   by (D/H)(1 − e^(−H/50)): 0.185, 0.181 and 0.178 °C for H = 8, 10 and 12 kW/°C, inside the 0.3 °C limit at every heat loss.
   PI alone must first let an error build up. *)
ASSERT_NEAR(EXPECTED := LREAL#5.0, ACTUAL := FeedforwardPct(ExtractionKw := LREAL#10.0), DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#0.1848, ACTUAL := DelayDipC(LossKwPerC := LREAL#8.0, ExtractionKw := LREAL#10.0), DELTA := LREAL#1.0e-4);
ASSERT_NEAR(EXPECTED := LREAL#0.1813, ACTUAL := DelayDipC(LossKwPerC := LREAL#10.0, ExtractionKw := LREAL#10.0), DELTA := LREAL#1.0e-4);
ASSERT_NEAR(EXPECTED := LREAL#0.1778, ACTUAL := DelayDipC(LossKwPerC := LREAL#12.0, ExtractionKw := LREAL#10.0), DELTA := LREAL#1.0e-4);
END_TEST_PROGRAM

TEST_PROGRAM TheDeclaredRuleChoosesTheSimplestQualifyingCandidate
(* 1 = A (PI), 2 = B (PI with feedforward), 0 = neither qualifies: redesign. A wins whenever it passes every case, because it
   needs no extraction measurement. *)
ASSERT_NEAR(EXPECTED := LREAL#1.0, ACTUAL := Recommendation(PassedA := LREAL#6.0, PassedB := LREAL#6.0, Cases := LREAL#6.0), DELTA := LREAL#0.0);
ASSERT_NEAR(EXPECTED := LREAL#2.0, ACTUAL := Recommendation(PassedA := LREAL#3.0, PassedB := LREAL#6.0, Cases := LREAL#6.0), DELTA := LREAL#0.0);
ASSERT_NEAR(EXPECTED := LREAL#0.0, ACTUAL := Recommendation(PassedA := LREAL#5.0, PassedB := LREAL#5.0, Cases := LREAL#6.0), DELTA := LREAL#0.0);
END_TEST_PROGRAM
