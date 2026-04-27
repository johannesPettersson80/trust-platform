TEST_PROGRAM BessFacadeExposesCabinetSnapshot
VAR
    Cabinet : EnergyStorageCabinet;
    Snapshot : BessSnapshot;
END_VAR
Cabinet.Initialize();
Cabinet.Enable();
Snapshot := Cabinet.Update(StateOfChargePct := REAL#70.0, GridPowerCommandKw := REAL#120.0, FireInputHealthy := TRUE);
ASSERT_TRUE(IN := Snapshot.Ready);
ASSERT_NEAR(EXPECTED := REAL#120.0, ACTUAL := Snapshot.PowerCommandKw, DELTA := REAL#0.001);
END_TEST_PROGRAM

TEST_PROGRAM BessObserverPublishesFireAlarm
VAR
    Cabinet : EnergyStorageCabinet;
    Snapshot : BessSnapshot;
END_VAR
Cabinet.Initialize();
Cabinet.Enable();
Snapshot := Cabinet.Update(StateOfChargePct := REAL#70.0, GridPowerCommandKw := REAL#120.0, FireInputHealthy := FALSE);
ASSERT_TRUE(IN := Snapshot.FireIsolated);
ASSERT_EQUAL(EXPECTED := INT#1, ACTUAL := Snapshot.AlarmCount);
ASSERT_TRUE(IN := Cabinet.MqttPublishReady);
END_TEST_PROGRAM

(* Multi-scan facade lifecycle test: drives the cabinet through enable,
   alarm trip, and disable on one instance. Catches bugs where Enable/
   Disable state leaks across scans, where the snapshot freezes after
   the first call, or where Disable doesn't actually zero the power
   command. *)
TEST_PROGRAM BessFacadeLifecycleTracksEnableDisableAcrossScans
VAR
    Cabinet : EnergyStorageCabinet;
    SnapshotEnabled : BessSnapshot;
    SnapshotAlarmed : BessSnapshot;
    SnapshotDisabled : BessSnapshot;
END_VAR

Cabinet.Initialize();
Cabinet.Enable();

(* Scan 1: healthy, command honored *)
SnapshotEnabled := Cabinet.Update(StateOfChargePct := REAL#70.0, GridPowerCommandKw := REAL#100.0, FireInputHealthy := TRUE);
ASSERT_TRUE(IN := SnapshotEnabled.Ready);

(* Scan 2: fire input fails, cabinet must isolate *)
SnapshotAlarmed := Cabinet.Update(StateOfChargePct := REAL#70.0, GridPowerCommandKw := REAL#100.0, FireInputHealthy := FALSE);
ASSERT_TRUE(IN := SnapshotAlarmed.FireIsolated);

(* Scan 3: explicitly disabled - cabinet stops regardless of command *)
Cabinet.Disable();
SnapshotDisabled := Cabinet.Update(StateOfChargePct := REAL#70.0, GridPowerCommandKw := REAL#100.0, FireInputHealthy := TRUE);
ASSERT_FALSE(IN := SnapshotDisabled.Ready);
ASSERT_NEAR(EXPECTED := REAL#0.0, ACTUAL := SnapshotDisabled.PowerCommandKw, DELTA := REAL#0.001);
END_TEST_PROGRAM

(* Inverter power-command boundary regression: the inverter clamps to
   [-250, 250] kW. Test off-by-one at each rail and beyond. *)
TEST_PROGRAM BessInverterClampsPowerCommandAtRails
VAR
    Cabinet : EnergyStorageCabinet;
    Snapshot : BessSnapshot;
END_VAR
Cabinet.Initialize();
Cabinet.Enable();

(* Just below positive rail: pass-through *)
Snapshot := Cabinet.Update(StateOfChargePct := REAL#50.0, GridPowerCommandKw := REAL#249.9, FireInputHealthy := TRUE);
ASSERT_NEAR(EXPECTED := REAL#249.9, ACTUAL := Snapshot.PowerCommandKw, DELTA := REAL#0.001);

(* At positive rail: pass-through *)
Snapshot := Cabinet.Update(StateOfChargePct := REAL#50.0, GridPowerCommandKw := REAL#250.0, FireInputHealthy := TRUE);
ASSERT_NEAR(EXPECTED := REAL#250.0, ACTUAL := Snapshot.PowerCommandKw, DELTA := REAL#0.001);

(* Above positive rail: clamps to 250 *)
Snapshot := Cabinet.Update(StateOfChargePct := REAL#50.0, GridPowerCommandKw := REAL#500.0, FireInputHealthy := TRUE);
ASSERT_NEAR(EXPECTED := REAL#250.0, ACTUAL := Snapshot.PowerCommandKw, DELTA := REAL#0.001);

(* At negative rail: pass-through *)
Snapshot := Cabinet.Update(StateOfChargePct := REAL#50.0, GridPowerCommandKw := REAL#-250.0, FireInputHealthy := TRUE);
ASSERT_NEAR(EXPECTED := REAL#-250.0, ACTUAL := Snapshot.PowerCommandKw, DELTA := REAL#0.001);

(* Below negative rail: clamps to -250 *)
Snapshot := Cabinet.Update(StateOfChargePct := REAL#50.0, GridPowerCommandKw := REAL#-1000.0, FireInputHealthy := TRUE);
ASSERT_NEAR(EXPECTED := REAL#-250.0, ACTUAL := Snapshot.PowerCommandKw, DELTA := REAL#0.001);
END_TEST_PROGRAM

(* SoC clamp regression: BatteryRack.Update LIMITs SoC to [0, 100]. *)
TEST_PROGRAM BessRackClampsStateOfChargeToValidRange
VAR
    Rack : BatteryRack;
END_VAR
Rack.Initialize();

(* Negative input: clamped to 0 *)
Rack.Update(StateOfChargePct := REAL#-25.0);
ASSERT_NEAR(EXPECTED := REAL#0.0, ACTUAL := Rack.StateOfChargePct, DELTA := REAL#0.001);

(* Above-100 input: clamped to 100 *)
Rack.Update(StateOfChargePct := REAL#150.0);
ASSERT_NEAR(EXPECTED := REAL#100.0, ACTUAL := Rack.StateOfChargePct, DELTA := REAL#0.001);

(* Boundary at 100: pass-through *)
Rack.Update(StateOfChargePct := REAL#100.0);
ASSERT_NEAR(EXPECTED := REAL#100.0, ACTUAL := Rack.StateOfChargePct, DELTA := REAL#0.001);

(* Boundary at 0: pass-through *)
Rack.Update(StateOfChargePct := REAL#0.0);
ASSERT_NEAR(EXPECTED := REAL#0.0, ACTUAL := Rack.StateOfChargePct, DELTA := REAL#0.001);
END_TEST_PROGRAM

(* Per-subscriber isolation: drive the alarm bus directly. Both
   subscribers should fire on every Publish; counts and flags
   accumulate independently. *)
TEST_PROGRAM BessAlarmBusFansOutToAllSubscribers
VAR
    Bus : BessAlarmBus;
    Historian : BessHistorianSubscriber;
    Mqtt : BessMqttSubscriber;
END_VAR
Bus.Subscribe(HistorianSub := Historian, MqttSub := Mqtt);

(* No publishes yet *)
ASSERT_EQUAL(EXPECTED := INT#0, ACTUAL := Historian.AlarmCount);
ASSERT_FALSE(IN := Mqtt.PublishReady);

(* One publish: each subscriber sees it *)
Bus.Publish(AlarmClass := BYTE#1, AlarmCode := WORD#16#0001);
ASSERT_EQUAL(EXPECTED := INT#1, ACTUAL := Historian.AlarmCount);
ASSERT_TRUE(IN := Mqtt.PublishReady);

(* Many publishes: historian counts each, mqtt flag latches *)
Bus.Publish(AlarmClass := BYTE#2, AlarmCode := WORD#16#0002);
Bus.Publish(AlarmClass := BYTE#3, AlarmCode := WORD#16#0003);
ASSERT_EQUAL(EXPECTED := INT#3, ACTUAL := Historian.AlarmCount);
END_TEST_PROGRAM

(* Repeated fire trips on one cabinet: AlarmCount must increment per
   trip, FireIsolated must follow the input. *)
TEST_PROGRAM BessFacadeAlarmCountAccumulatesAcrossFireTrips
VAR
    Cabinet : EnergyStorageCabinet;
    Snapshot : BessSnapshot;
END_VAR
Cabinet.Initialize();
Cabinet.Enable();

(* Healthy scan *)
Snapshot := Cabinet.Update(StateOfChargePct := REAL#50.0, GridPowerCommandKw := REAL#0.0, FireInputHealthy := TRUE);
ASSERT_EQUAL(EXPECTED := INT#0, ACTUAL := Snapshot.AlarmCount);

(* First fire trip: count goes to 1 *)
Snapshot := Cabinet.Update(StateOfChargePct := REAL#50.0, GridPowerCommandKw := REAL#0.0, FireInputHealthy := FALSE);
ASSERT_EQUAL(EXPECTED := INT#1, ACTUAL := Snapshot.AlarmCount);

(* Re-enable, healthy, then a second fire trip: count goes to 2 *)
Cabinet.Enable();
Snapshot := Cabinet.Update(StateOfChargePct := REAL#50.0, GridPowerCommandKw := REAL#0.0, FireInputHealthy := TRUE);
Snapshot := Cabinet.Update(StateOfChargePct := REAL#50.0, GridPowerCommandKw := REAL#0.0, FireInputHealthy := FALSE);
ASSERT_EQUAL(EXPECTED := INT#2, ACTUAL := Snapshot.AlarmCount);

(* Third trip without re-enable: still triggers a publish (Disable was already true but fire still latches) *)
Snapshot := Cabinet.Update(StateOfChargePct := REAL#50.0, GridPowerCommandKw := REAL#0.0, FireInputHealthy := FALSE);
ASSERT_EQUAL(EXPECTED := INT#3, ACTUAL := Snapshot.AlarmCount);
ASSERT_TRUE(IN := Snapshot.FireIsolated);
END_TEST_PROGRAM

(* Inverter without enable: any command rounds to zero. *)
TEST_PROGRAM BessInverterRequiresEnableForPowerCommand
VAR
    Inverter : InverterUnit;
END_VAR
(* Disabled: PowerCommandKw is zero regardless of input *)
Inverter.Command(Enabled := FALSE, PowerKw := REAL#100.0);
ASSERT_NEAR(EXPECTED := REAL#0.0, ACTUAL := Inverter.PowerCommandKw, DELTA := REAL#0.001);
ASSERT_FALSE(IN := Inverter.Enabled);

(* Enabled: command honored *)
Inverter.Command(Enabled := TRUE, PowerKw := REAL#100.0);
ASSERT_NEAR(EXPECTED := REAL#100.0, ACTUAL := Inverter.PowerCommandKw, DELTA := REAL#0.001);
ASSERT_TRUE(IN := Inverter.Enabled);

(* Disable again: drops to zero on next call *)
Inverter.Command(Enabled := FALSE, PowerKw := REAL#100.0);
ASSERT_NEAR(EXPECTED := REAL#0.0, ACTUAL := Inverter.PowerCommandKw, DELTA := REAL#0.001);
END_TEST_PROGRAM
