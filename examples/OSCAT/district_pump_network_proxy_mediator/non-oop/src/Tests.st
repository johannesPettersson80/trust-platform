TEST_PROGRAM ClassicDistrictBalancesHealthyDemand
VAR
    Network : ClassicDistrictPumpNetwork;
END_VAR

ASSERT_EQUAL(EXPECTED := REAL#90.0, ACTUAL := Network.Update(LocalDemandM3H := REAL#20.0, NorthDemandM3H := REAL#15.0, NorthQualityGood := TRUE, NorthAgeSeconds := INT#5, EastDemandM3H := REAL#10.0, EastQualityGood := TRUE, EastAgeSeconds := INT#5, SuctionHealthy := TRUE));
ASSERT_EQUAL(EXPECTED := REAL#45.0, ACTUAL := Network.TotalDemandM3H);
END_TEST_PROGRAM

TEST_PROGRAM ClassicDistrictDropsStaleDemand
VAR
    Network : ClassicDistrictPumpNetwork;
END_VAR

ASSERT_EQUAL(EXPECTED := REAL#40.0, ACTUAL := Network.Update(LocalDemandM3H := REAL#20.0, NorthDemandM3H := REAL#15.0, NorthQualityGood := TRUE, NorthAgeSeconds := INT#45, EastDemandM3H := REAL#10.0, EastQualityGood := FALSE, EastAgeSeconds := INT#5, SuctionHealthy := TRUE));
ASSERT_EQUAL(EXPECTED := INT#2, ACTUAL := Network.RemoteStaleCount);
END_TEST_PROGRAM
