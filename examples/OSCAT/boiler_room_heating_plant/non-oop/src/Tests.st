TEST_PROGRAM ClassicBoilerRoomSelectsLeadAndLagInline
VAR
    Room : ClassicBoilerRoom;
    Snapshot : ClassicBoilerRoomStatus;
END_VAR

Room.Initialize();
Room.Start();
Snapshot := Room.Update(OutdoorTemp := REAL#0.0, SupplyTemp := REAL#55.0, GasPressureOk := TRUE, LowWaterOk := TRUE, EStopHealthy := TRUE, HeatDemand := REAL#80.0);
ASSERT_EQUAL(EXPECTED := INT#2, ACTUAL := Snapshot.LeadBoilerId);
ASSERT_TRUE(IN := Snapshot.LagBoilerActive);
END_TEST_PROGRAM
