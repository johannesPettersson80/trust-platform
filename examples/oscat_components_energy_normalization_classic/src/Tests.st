TEST_PROGRAM EnergyNormalizationClassicUsesConversionBlocks
VAR
    EnergyConversion : ENERGY;
    SpeedConversion : SPEED;
END_VAR

EnergyConversion(J := REAL#3600.0);
SpeedConversion(ms := REAL#10.0);

ASSERT_NEAR(EXPECTED := REAL#1.0, ACTUAL := EnergyConversion.YWh, DELTA := REAL#0.001);
ASSERT_NEAR(EXPECTED := REAL#36.0, ACTUAL := SpeedConversion.Ykmh, DELTA := REAL#0.001);
END_TEST_PROGRAM
