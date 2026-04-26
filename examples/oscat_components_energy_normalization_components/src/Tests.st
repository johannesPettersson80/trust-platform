TEST_PROGRAM EnergyNormalizationComponentsUseConverterService
VAR
    Converter : UnitConverter;
    ConverterObject : IUnitConverter;
    EnergyConversion : ENERGY;
    SpeedConversion : SPEED;
END_VAR

ConverterObject := Converter;
ConverterObject.Initialize();
EnergyConversion(J := REAL#3600.0);
SpeedConversion(ms := REAL#10.0);

ASSERT_NEAR(EXPECTED := EnergyConversion.YWh, ACTUAL := ConverterObject.WattHoursFromJoules(Joule := REAL#3600.0), DELTA := REAL#0.001);
ASSERT_NEAR(EXPECTED := SpeedConversion.Ykmh, ACTUAL := ConverterObject.KilometersPerHourFromMetersPerSecond(Mps := REAL#10.0), DELTA := REAL#0.001);
END_TEST_PROGRAM
