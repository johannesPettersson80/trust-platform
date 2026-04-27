TEST_PROGRAM TankFarmClassicStopsAreaWithHardCodedBranches
VAR
    Farm : TankFarmClassic;
END_VAR

ASSERT_EQUAL(EXPECTED := INT#2, ACTUAL := Farm.AlarmCount);
Farm.EmergencyStopArea(AreaIndex := INT#2);
ASSERT_EQUAL(EXPECTED := INT#2, ACTUAL := Farm.StoppedCount);
Farm.AcknowledgeAll();
ASSERT_EQUAL(EXPECTED := INT#0, ACTUAL := Farm.AlarmCount);
END_TEST_PROGRAM
