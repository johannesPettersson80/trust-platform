TEST_PROGRAM ClassicCoolingTowerBranchesOnSeason
VAR
    Tower : ClassicCoolingTower;
END_VAR
ASSERT_EQUAL(EXPECTED := INT#2, ACTUAL := Tower.Update(SeasonRequest := INT#1, WaterTempC := REAL#32.0, OutdoorTempC := REAL#20.0, ConductivityUs := REAL#1000.0));
Tower.Update(SeasonRequest := INT#2, WaterTempC := REAL#20.0, OutdoorTempC := REAL#0.0, ConductivityUs := REAL#2000.0);
ASSERT_TRUE(IN := Tower.BasinHeater);
END_TEST_PROGRAM
