TEST_PROGRAM BoosterClassicSelectsLeadWithDirectBranches
VAR
    Station : BoosterStationClassic;
END_VAR

ASSERT_EQUAL(EXPECTED := INT#2, ACTUAL := Station.Update(StartCommand := TRUE, SuctionOk := TRUE, Pump1Fault := FALSE, Pump2Fault := FALSE, Pump3Fault := FALSE));
ASSERT_EQUAL(EXPECTED := INT#3, ACTUAL := Station.Update(StartCommand := TRUE, SuctionOk := TRUE, Pump1Fault := FALSE, Pump2Fault := TRUE, Pump3Fault := FALSE));
ASSERT_EQUAL(EXPECTED := INT#0, ACTUAL := Station.Update(StartCommand := TRUE, SuctionOk := TRUE, Pump1Fault := TRUE, Pump2Fault := TRUE, Pump3Fault := TRUE));
ASSERT_TRUE(IN := Station.AlarmActive);
ASSERT_EQUAL(EXPECTED := INT#1, ACTUAL := Station.AlarmCount);
END_TEST_PROGRAM
