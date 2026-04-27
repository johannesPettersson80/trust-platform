TEST_PROGRAM ClassicBessHandlesFireInline
VAR
    Cabinet : ClassicBessCabinet;
END_VAR
Cabinet.Enable();
ASSERT_NEAR(EXPECTED := REAL#0.0, ACTUAL := Cabinet.Update(StateOfChargePct := REAL#70.0, GridPowerCommandKw := REAL#120.0, FireInputHealthy := FALSE), DELTA := REAL#0.001);
ASSERT_TRUE(IN := Cabinet.FireIsolated);
ASSERT_EQUAL(EXPECTED := INT#1, ACTUAL := Cabinet.AlarmCount);
END_TEST_PROGRAM
