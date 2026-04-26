TEST_PROGRAM WeatherStationConversionComponentsConvertWindData
VAR
    Context : AutomationContext;
    ContextObject : IAutomationContext;
    Converter : UnitConverter;
    ConverterObject : IUnitConverter;
END_VAR

ContextObject := Context;
ContextObject.Initialize();
ConverterObject := Converter;
ConverterObject.Initialize();

ASSERT_TRUE(IN := ContextObject.LoadConstants());
ASSERT_EQUAL(EXPECTED := MS_TO_BFT(MS := REAL#10.0), ACTUAL := ConverterObject.BeaufortFromMetersPerSecond(Mps := REAL#10.0));
ASSERT_EQUAL(EXPECTED := DEG_TO_DIR(DEG := INT#90, N := INT#4, L := INT#1), ACTUAL := ContextObject.DirectionName(Degrees := REAL#90.0));
END_TEST_PROGRAM
