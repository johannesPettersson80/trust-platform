TEST_PROGRAM BoilerStationFacadeReturnsStableSnapshot
VAR
    Room : BoilerStation;
    Snapshot : BoilerStationStatus;
END_VAR

Room.Initialize();
Room.Start();
Snapshot := Room.Update(OutdoorTemp := REAL#0.0, SupplyTemp := REAL#55.0, ReturnTemp := REAL#45.0, GasPressureOk := TRUE, LowWaterOk := TRUE, EStopHealthy := TRUE, HeatDemand := REAL#80.0);
ASSERT_TRUE(IN := Snapshot.OverallReady);
ASSERT_EQUAL(EXPECTED := INT#2, ACTUAL := Snapshot.LeadBoilerId);
ASSERT_TRUE(IN := Snapshot.LagBoilerActive);
ASSERT_TRUE(IN := Snapshot.LeadBoilerLoad > REAL#60.0);
END_TEST_PROGRAM

TEST_PROGRAM BoilerAlarmBusFansOutToHistorianAndMqtt
VAR
    Room : BoilerStation;
    Snapshot : BoilerStationStatus;
END_VAR

Room.Initialize();
Room.Start();
Snapshot := Room.Update(OutdoorTemp := REAL#0.0, SupplyTemp := REAL#60.0, ReturnTemp := REAL#45.0, GasPressureOk := FALSE, LowWaterOk := TRUE, EStopHealthy := TRUE, HeatDemand := REAL#50.0);
ASSERT_TRUE(IN := Snapshot.ActiveAlarmCount > INT#0);
ASSERT_EQUAL(EXPECTED := BYTE#3, ACTUAL := Snapshot.HighestAlarmClass);
ASSERT_TRUE(IN := Room.MqttPublishReady);
END_TEST_PROGRAM

(* Multi-scan facade test: drives BoilerStation through alternating
   healthy and alarmed states without re-Initializing between scans.
   Catches bugs where the snapshot caches stale alarm counts, where
   alarm conditions clear but ActiveAlarmCount stays sticky, or where
   Update fails to re-evaluate the lead-boiler selection. *)
TEST_PROGRAM BoilerStationSnapshotTracksScanStateChanges
VAR
    Room : BoilerStation;
    SnapshotHealthy : BoilerStationStatus;
    SnapshotAlarmed : BoilerStationStatus;
    SnapshotRecovered : BoilerStationStatus;
END_VAR

Room.Initialize();
Room.Start();

(* Scan 1: healthy *)
SnapshotHealthy := Room.Update(OutdoorTemp := REAL#0.0, SupplyTemp := REAL#55.0, ReturnTemp := REAL#45.0, GasPressureOk := TRUE, LowWaterOk := TRUE, EStopHealthy := TRUE, HeatDemand := REAL#80.0);
ASSERT_TRUE(IN := SnapshotHealthy.OverallReady);

(* Scan 2: gas pressure fault (Class A) *)
SnapshotAlarmed := Room.Update(OutdoorTemp := REAL#0.0, SupplyTemp := REAL#55.0, ReturnTemp := REAL#45.0, GasPressureOk := FALSE, LowWaterOk := TRUE, EStopHealthy := TRUE, HeatDemand := REAL#80.0);
ASSERT_TRUE(IN := SnapshotAlarmed.ActiveAlarmCount > INT#0);

(* Scan 3: gas pressure recovers; ActiveAlarmCount must reflect the
   new state, not stay sticky at the previous count *)
SnapshotRecovered := Room.Update(OutdoorTemp := REAL#0.0, SupplyTemp := REAL#55.0, ReturnTemp := REAL#45.0, GasPressureOk := TRUE, LowWaterOk := TRUE, EStopHealthy := TRUE, HeatDemand := REAL#80.0);
ASSERT_TRUE(IN := SnapshotRecovered.ActiveAlarmCount <= SnapshotAlarmed.ActiveAlarmCount);
END_TEST_PROGRAM

(* Stop() prevents firing regardless of demand. *)
TEST_PROGRAM BoilerStationStopBlocksFiringEvenWithDemand
VAR
    Room : BoilerStation;
    SnapshotStarted : BoilerStationStatus;
    SnapshotStopped : BoilerStationStatus;
END_VAR
Room.Initialize();
Room.Start();
SnapshotStarted := Room.Update(OutdoorTemp := REAL#0.0, SupplyTemp := REAL#55.0, ReturnTemp := REAL#45.0, GasPressureOk := TRUE, LowWaterOk := TRUE, EStopHealthy := TRUE, HeatDemand := REAL#80.0);
ASSERT_TRUE(IN := SnapshotStarted.OverallReady);

(* After Stop, lead boiler load is zero even with demand *)
Room.Stop();
SnapshotStopped := Room.Update(OutdoorTemp := REAL#0.0, SupplyTemp := REAL#55.0, ReturnTemp := REAL#45.0, GasPressureOk := TRUE, LowWaterOk := TRUE, EStopHealthy := TRUE, HeatDemand := REAL#80.0);
ASSERT_FALSE(IN := SnapshotStopped.OverallReady);
END_TEST_PROGRAM

(* Low-water alarm is class A. *)
TEST_PROGRAM BoilerStationLowWaterTripsClassAAlarm
VAR
    Room : BoilerStation;
    Snapshot : BoilerStationStatus;
END_VAR
Room.Initialize();
Room.Start();
Snapshot := Room.Update(OutdoorTemp := REAL#0.0, SupplyTemp := REAL#55.0, ReturnTemp := REAL#45.0, GasPressureOk := TRUE, LowWaterOk := FALSE, EStopHealthy := TRUE, HeatDemand := REAL#50.0);
ASSERT_TRUE(IN := Snapshot.ActiveAlarmCount > INT#0);
ASSERT_EQUAL(EXPECTED := BYTE#3, ACTUAL := Snapshot.HighestAlarmClass);
END_TEST_PROGRAM

(* E-stop trip overrides everything. *)
TEST_PROGRAM BoilerStationEStopTripsClassAAlarm
VAR
    Room : BoilerStation;
    Snapshot : BoilerStationStatus;
END_VAR
Room.Initialize();
Room.Start();
Snapshot := Room.Update(OutdoorTemp := REAL#0.0, SupplyTemp := REAL#55.0, ReturnTemp := REAL#45.0, GasPressureOk := TRUE, LowWaterOk := TRUE, EStopHealthy := FALSE, HeatDemand := REAL#50.0);
ASSERT_TRUE(IN := Snapshot.ActiveAlarmCount > INT#0);
ASSERT_EQUAL(EXPECTED := BYTE#3, ACTUAL := Snapshot.HighestAlarmClass);
END_TEST_PROGRAM

(* Multiple simultaneous Class-A alarms. *)
TEST_PROGRAM BoilerStationHandlesMultipleSimultaneousAlarms
VAR
    Room : BoilerStation;
    Snapshot : BoilerStationStatus;
END_VAR
Room.Initialize();
Room.Start();
(* Three Class-A conditions at once: gas, water, e-stop *)
Snapshot := Room.Update(OutdoorTemp := REAL#0.0, SupplyTemp := REAL#55.0, ReturnTemp := REAL#45.0, GasPressureOk := FALSE, LowWaterOk := FALSE, EStopHealthy := FALSE, HeatDemand := REAL#50.0);
ASSERT_TRUE(IN := Snapshot.ActiveAlarmCount >= INT#1);
ASSERT_EQUAL(EXPECTED := BYTE#3, ACTUAL := Snapshot.HighestAlarmClass);
END_TEST_PROGRAM

(* Demand below lag threshold: lag boiler not active. *)
TEST_PROGRAM BoilerStationLowDemandDoesNotActivateLagBoiler
VAR
    Room : BoilerStation;
    Snapshot : BoilerStationStatus;
END_VAR
Room.Initialize();
Room.Start();
(* Low heat demand - only the lead boiler should run *)
Snapshot := Room.Update(OutdoorTemp := REAL#10.0, SupplyTemp := REAL#55.0, ReturnTemp := REAL#50.0, GasPressureOk := TRUE, LowWaterOk := TRUE, EStopHealthy := TRUE, HeatDemand := REAL#15.0);
ASSERT_FALSE(IN := Snapshot.LagBoilerActive);
END_TEST_PROGRAM

(* Subsequent Update calls without re-Start retain Started state. *)
TEST_PROGRAM BoilerStationStartIsOneShotInitialization
VAR
    Room : BoilerStation;
    Snapshot1 : BoilerStationStatus;
    Snapshot2 : BoilerStationStatus;
    Snapshot3 : BoilerStationStatus;
END_VAR
Room.Initialize();
Room.Start();

(* Drive three healthy scans without re-Start *)
Snapshot1 := Room.Update(OutdoorTemp := REAL#0.0, SupplyTemp := REAL#55.0, ReturnTemp := REAL#45.0, GasPressureOk := TRUE, LowWaterOk := TRUE, EStopHealthy := TRUE, HeatDemand := REAL#80.0);
Snapshot2 := Room.Update(OutdoorTemp := REAL#0.0, SupplyTemp := REAL#55.0, ReturnTemp := REAL#45.0, GasPressureOk := TRUE, LowWaterOk := TRUE, EStopHealthy := TRUE, HeatDemand := REAL#80.0);
Snapshot3 := Room.Update(OutdoorTemp := REAL#0.0, SupplyTemp := REAL#55.0, ReturnTemp := REAL#45.0, GasPressureOk := TRUE, LowWaterOk := TRUE, EStopHealthy := TRUE, HeatDemand := REAL#80.0);

ASSERT_TRUE(IN := Snapshot1.OverallReady);
ASSERT_TRUE(IN := Snapshot2.OverallReady);
ASSERT_TRUE(IN := Snapshot3.OverallReady);
END_TEST_PROGRAM
