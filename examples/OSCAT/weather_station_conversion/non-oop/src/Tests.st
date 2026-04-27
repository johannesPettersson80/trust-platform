TEST_PROGRAM WeatherStationConversionClassicConvertsWindData
VAR
    ConstantsReady : BOOL;
END_VAR

ConstantsReady := OSCAT_BASIC_Constants();

ASSERT_TRUE(IN := ConstantsReady);
ASSERT_EQUAL(EXPECTED := MS_TO_BFT(MS := REAL#10.0), ACTUAL := MS_TO_BFT(MS := REAL#10.0));
ASSERT_EQUAL(EXPECTED := DEG_TO_DIR(DEG := INT#90, N := INT#4, L := INT#1), ACTUAL := DEG_TO_DIR(DEG := INT#90, N := INT#4, L := INT#1));
END_TEST_PROGRAM
