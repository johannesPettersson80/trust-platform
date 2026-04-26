TEST_PROGRAM GreenhouseTemperatureClassicConvertsAndRequestsVentilation
VAR
    TemperatureConversion : TEMPERATURE;
    VentilationSwitch : HYST;
END_VAR

TemperatureConversion(C := REAL#29.0);
VentilationSwitch(In := REAL#29.0, HIGH_LIMIT := REAL#28.0, LOW_LIMIT := REAL#18.0);

ASSERT_NEAR(EXPECTED := REAL#302.15, ACTUAL := TemperatureConversion.YK, DELTA := REAL#0.01);
ASSERT_TRUE(IN := VentilationSwitch.Q);
END_TEST_PROGRAM
