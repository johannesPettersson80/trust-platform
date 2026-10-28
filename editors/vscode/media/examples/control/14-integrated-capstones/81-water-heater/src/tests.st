(* @trustlsp:runtime-ignore — run these assertions with trust-dev test;
   they are not scheduled PLC instances in the lesson simulator. *)

TEST_PROGRAM CapacityCheck
(* H = 10 kW/°C, Ta = 20 °C, a = 2 kW per point: 75 % at 35 °C, 90 % with 30 kW, 90 % at 38 °C,
   and 105 % at 38 °C with 30 kW, beyond the heater. *)
ASSERT_NEAR(EXPECTED := LREAL#75.0, ACTUAL := HoldingCommand(LossCoefficient := LREAL#10.0, Temperature := LREAL#35.0,
    Ambient := LREAL#20.0, Extraction := LREAL#0.0, HeaterPerPoint := LREAL#2.0), DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#90.0, ACTUAL := HoldingCommand(LossCoefficient := LREAL#10.0, Temperature := LREAL#35.0,
    Ambient := LREAL#20.0, Extraction := LREAL#30.0, HeaterPerPoint := LREAL#2.0), DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#90.0, ACTUAL := HoldingCommand(LossCoefficient := LREAL#10.0, Temperature := LREAL#38.0,
    Ambient := LREAL#20.0, Extraction := LREAL#0.0, HeaterPerPoint := LREAL#2.0), DELTA := LREAL#1.0e-12);
ASSERT_NEAR(EXPECTED := LREAL#105.0, ACTUAL := HoldingCommand(LossCoefficient := LREAL#10.0, Temperature := LREAL#38.0,
    Ambient := LREAL#20.0, Extraction := LREAL#30.0, HeaterPerPoint := LREAL#2.0), DELTA := LREAL#1.0e-12);
END_TEST_PROGRAM

TEST_PROGRAM StaleAndInvalidReadingsAreRejected
ASSERT_TRUE(ReadingUsable(QualityGood := TRUE, Value := LREAL#35.0, Age := LREAL#0.5, MaximumAge := LREAL#0.5));
ASSERT_FALSE(ReadingUsable(QualityGood := TRUE, Value := LREAL#35.0, Age := LREAL#0.6, MaximumAge := LREAL#0.5));
ASSERT_FALSE(ReadingUsable(QualityGood := FALSE, Value := LREAL#35.0, Age := LREAL#0.0, MaximumAge := LREAL#0.5));
ASSERT_FALSE(ReadingUsable(QualityGood := TRUE, Value := LREAL#85.0, Age := LREAL#0.0, MaximumAge := LREAL#0.5));
END_TEST_PROGRAM
