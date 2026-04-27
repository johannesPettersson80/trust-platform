TEST_PROGRAM ClassicFillingRejectsBadWeightInline
VAR
    Filler : ClassicFillingController;
END_VAR

Filler.Initialize();
ASSERT_TRUE(IN := Filler.ConfigureRecipe(VolumeMl := REAL#10.0, PumpSpeedPct := REAL#45.0, LowWeightG := REAL#9.8, HighWeightG := REAL#10.2));
Filler.Update(StartCommand := TRUE, VialPresent := TRUE, FillDone := FALSE, WeightG := REAL#0.0, LowWeightG := REAL#9.8, HighWeightG := REAL#10.2);
Filler.Update(StartCommand := FALSE, VialPresent := TRUE, FillDone := FALSE, WeightG := REAL#0.0, LowWeightG := REAL#9.8, HighWeightG := REAL#10.2);
Filler.Update(StartCommand := FALSE, VialPresent := TRUE, FillDone := TRUE, WeightG := REAL#0.0, LowWeightG := REAL#9.8, HighWeightG := REAL#10.2);
ASSERT_EQUAL(EXPECTED := INT#40, ACTUAL := Filler.Update(StartCommand := FALSE, VialPresent := TRUE, FillDone := FALSE, WeightG := REAL#10.5, LowWeightG := REAL#9.8, HighWeightG := REAL#10.2));
ASSERT_EQUAL(EXPECTED := DWORD#1, ACTUAL := Filler.RejectCount);
END_TEST_PROGRAM
