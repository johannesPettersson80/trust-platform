TEST_PROGRAM ClassicColdStoreCalculatesDemandInline
VAR
    Plant : ClassicColdStorePlant;
END_VAR

ASSERT_EQUAL(EXPECTED := INT#2, ACTUAL := Plant.Update(Freezer1Temp := REAL#-20.0, Freezer1Door := FALSE, Freezer2Temp := REAL#-20.0, Freezer2Door := FALSE, Chiller1Temp := REAL#4.0, Chiller1Door := FALSE, Chiller2Temp := REAL#1.0, Chiller2Door := FALSE));
ASSERT_EQUAL(EXPECTED := INT#4, ACTUAL := Plant.FreezerDemand);
ASSERT_EQUAL(EXPECTED := INT#1, ACTUAL := Plant.ChillerDemand);
END_TEST_PROGRAM
