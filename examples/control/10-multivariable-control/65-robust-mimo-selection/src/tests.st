(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. *)

TEST_PROGRAM TheHoldingCommands
(* Vessel 1 at 37 °C with vessel 2 at 35 °C needs 75 + (10 + Hc) and 75 − Hc %: 91/69, 93/67 and 95/65 % at 6, 8 and
   10 kW/°C. A 10 kW load on vessel 1 at 35/35 °C needs 80/75 % at every coupling. *)
ASSERT_NEAR(EXPECTED := LREAL#91.0, ACTUAL := HoldingCommand1Pct(CouplingKwPerC := LREAL#6.0, X1C := LREAL#2.0, X2C := LREAL#0.0, D1Kw := LREAL#0.0), DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#69.0, ACTUAL := HoldingCommand2Pct(CouplingKwPerC := LREAL#6.0, X1C := LREAL#2.0, X2C := LREAL#0.0, D2Kw := LREAL#0.0), DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#93.0, ACTUAL := HoldingCommand1Pct(CouplingKwPerC := LREAL#8.0, X1C := LREAL#2.0, X2C := LREAL#0.0, D1Kw := LREAL#0.0), DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#67.0, ACTUAL := HoldingCommand2Pct(CouplingKwPerC := LREAL#8.0, X1C := LREAL#2.0, X2C := LREAL#0.0, D2Kw := LREAL#0.0), DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#95.0, ACTUAL := HoldingCommand1Pct(CouplingKwPerC := LREAL#10.0, X1C := LREAL#2.0, X2C := LREAL#0.0, D1Kw := LREAL#0.0), DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#65.0, ACTUAL := HoldingCommand2Pct(CouplingKwPerC := LREAL#10.0, X1C := LREAL#2.0, X2C := LREAL#0.0, D2Kw := LREAL#0.0), DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#80.0, ACTUAL := HoldingCommand1Pct(CouplingKwPerC := LREAL#10.0, X1C := LREAL#0.0, X2C := LREAL#0.0, D1Kw := LREAL#10.0), DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM

TEST_PROGRAM TheFrozenMap
(* The map designed for 8 kW/°C cancels the steady cross gain there; at 6 kW/°C it leaves −0.0101 °C per point (too much
   compensation), at 10 kW/°C +0.0074 (too little). Without a map the cross gain is 0.0545, 0.0615 and 0.0667. *)
ASSERT_NEAR(EXPECTED := LREAL#0.0, ACTUAL := MappedCrossGainCPerPct(CouplingKwPerC := LREAL#8.0, CrossGain := LREAL#4.0 / LREAL#9.0), DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#-0.0101010, ACTUAL := MappedCrossGainCPerPct(CouplingKwPerC := LREAL#6.0, CrossGain := LREAL#4.0 / LREAL#9.0), DELTA := LREAL#1.0e-7);
ASSERT_NEAR(EXPECTED := LREAL#0.0074074, ACTUAL := MappedCrossGainCPerPct(CouplingKwPerC := LREAL#10.0, CrossGain := LREAL#4.0 / LREAL#9.0), DELTA := LREAL#1.0e-7);
ASSERT_NEAR(EXPECTED := LREAL#0.0545455, ACTUAL := MappedCrossGainCPerPct(CouplingKwPerC := LREAL#6.0, CrossGain := LREAL#0.0), DELTA := LREAL#1.0e-7);
ASSERT_NEAR(EXPECTED := LREAL#0.0666667, ACTUAL := MappedCrossGainCPerPct(CouplingKwPerC := LREAL#10.0, CrossGain := LREAL#0.0), DELTA := LREAL#1.0e-7);
END_TEST_PROGRAM

TEST_PROGRAM OneBlockBothArchitectures
(* The first update of a vessel-1 target with Control Studio's designs: decentralized PI (Kp 6.75, Ki 1.73) gives
   75 + 6.75·2 + 1.73·0.2 = 88.846 % and 75 %; the decoupled group (Kp 5.42, Ki 1.74, D = (81/65)·[[1, −4/9], [−4/9, 1]])
   requests 11.188 points and gives 75 + (81/65)·11.188 = 88.9419692 % and 75 − (36/65)·11.188 = 68.8035692 %. *)
VAR
    Paired : TwoVesselPi;
    Decoupled : TwoVesselPi;
END_VAR
Paired(Setpoint1C := LREAL#37.0, Setpoint2C := LREAL#35.0, Vessel1C := LREAL#35.0, Vessel2C := LREAL#35.0, CrossGain := LREAL#0.0,
    ProportionalGain := LREAL#6.75, IntegralGain := LREAL#1.73);
Decoupled(Setpoint1C := LREAL#37.0, Setpoint2C := LREAL#35.0, Vessel1C := LREAL#35.0, Vessel2C := LREAL#35.0, CrossGain := LREAL#4.0 / LREAL#9.0,
    DecouplerScale := LREAL#81.0 / LREAL#65.0, ProportionalGain := LREAL#5.42, IntegralGain := LREAL#1.74);
ASSERT_NEAR(EXPECTED := LREAL#88.846, ACTUAL := Paired.Heater1Pct, DELTA := LREAL#1.0e-9);
ASSERT_NEAR(EXPECTED := LREAL#75.0, ACTUAL := Paired.Heater2Pct, DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#88.9419692, ACTUAL := Decoupled.Heater1Pct, DELTA := LREAL#1.0e-6);
ASSERT_NEAR(EXPECTED := LREAL#68.8035692, ACTUAL := Decoupled.Heater2Pct, DELTA := LREAL#1.0e-6);
END_TEST_PROGRAM

TEST_PROGRAM StudiosScaledDecoupler
(* Control Studio's D = (81/65)·[[1, −4/9], [−4/9, 1]]: a 5-point request on vessel 1 gives heaters 75 + 6.2308 = 81.2308 % and
   75 − 2.7692 = 72.2308 %; the scale multiplies the cross gain the frozen map leaves, −0.0101·81/65 = −0.0125874 °C per point
   at 6 kW/°C. *)
ASSERT_NEAR(EXPECTED := LREAL#81.2307692, ACTUAL := PhysicalRequest1Pct(V1Pct := LREAL#5.0, V2Pct := LREAL#0.0, CrossGain := LREAL#4.0 / LREAL#9.0, Scale := LREAL#81.0 / LREAL#65.0), DELTA := LREAL#1.0e-6);
ASSERT_NEAR(EXPECTED := LREAL#72.2307692, ACTUAL := PhysicalRequest2Pct(V1Pct := LREAL#5.0, V2Pct := LREAL#0.0, CrossGain := LREAL#4.0 / LREAL#9.0, Scale := LREAL#81.0 / LREAL#65.0), DELTA := LREAL#1.0e-6);
ASSERT_NEAR(EXPECTED := LREAL#-0.0125874, ACTUAL := MappedCrossGainCPerPct(CouplingKwPerC := LREAL#6.0, CrossGain := LREAL#4.0 / LREAL#9.0, Scale := LREAL#81.0 / LREAL#65.0), DELTA := LREAL#1.0e-6);
END_TEST_PROGRAM
