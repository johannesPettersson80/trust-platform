TEST_PROGRAM BoosterMediatorSelectsLowestRuntimeHealthyPump
VAR
    Station : BoosterStation;
END_VAR

Station.Initialize();
ASSERT_EQUAL(EXPECTED := INT#2, ACTUAL := Station.Update(StartCommand := TRUE, SuctionOk := TRUE, HeaderPressure := REAL#4.0, PressureSetpoint := REAL#4.5));
ASSERT_EQUAL(EXPECTED := INT#2, ACTUAL := Station.LeadPumpId);
ASSERT_EQUAL(EXPECTED := INT#1, ACTUAL := Station.RunningPumpCount);
END_TEST_PROGRAM

TEST_PROGRAM BoosterMediatorFailsOverAndObserverPublishesAlarms
VAR
    Station : BoosterStation;
END_VAR

Station.Initialize();
Station.SetPumpFaults(Fault1 := FALSE, Fault2 := TRUE, Fault3 := FALSE);
ASSERT_EQUAL(EXPECTED := INT#3, ACTUAL := Station.Update(StartCommand := TRUE, SuctionOk := TRUE, HeaderPressure := REAL#4.0, PressureSetpoint := REAL#4.5));

Station.SetPumpFaults(Fault1 := TRUE, Fault2 := TRUE, Fault3 := TRUE);
ASSERT_EQUAL(EXPECTED := INT#0, ACTUAL := Station.Update(StartCommand := TRUE, SuctionOk := TRUE, HeaderPressure := REAL#4.0, PressureSetpoint := REAL#4.5));
ASSERT_TRUE(IN := Station.AlarmActive);
ASSERT_EQUAL(EXPECTED := INT#1, ACTUAL := Station.HistorianAlarmCount);
ASSERT_TRUE(IN := Station.MqttPublishReady);
END_TEST_PROGRAM

(* Multi-cycle observer-fan-out test: drives several alarm-triggering
   scans through one Station and verifies HistorianAlarmCount grows
   monotonically. Catches bugs where the alarm bus re-publishes the
   same event repeatedly, or where the historian counter is reset
   between scans. *)
TEST_PROGRAM BoosterAlarmBusAccumulatesAcrossMultipleEvents
VAR
    Station : BoosterStation;
    InitialHistorian : INT;
END_VAR

Station.Initialize();

(* Scan 1: dry-run condition forces alarm *)
Station.Update(StartCommand := TRUE, SuctionOk := FALSE, HeaderPressure := REAL#4.0, PressureSetpoint := REAL#4.5);
ASSERT_TRUE(IN := Station.AlarmActive);
InitialHistorian := Station.HistorianAlarmCount;
ASSERT_TRUE(IN := InitialHistorian >= INT#1);

(* Scan 2: all pumps faulted forces a different alarm *)
Station.SetPumpFaults(Fault1 := TRUE, Fault2 := TRUE, Fault3 := TRUE);
Station.Update(StartCommand := TRUE, SuctionOk := TRUE, HeaderPressure := REAL#4.0, PressureSetpoint := REAL#4.5);
ASSERT_TRUE(IN := Station.HistorianAlarmCount > InitialHistorian);
ASSERT_TRUE(IN := Station.MqttPublishReady);
END_TEST_PROGRAM

(* Mediator failover regression: pump 2 leads first; when pump 2
   faults, pump 3 takes over within a single same-instance sequence
   (no Initialize between calls). Catches bugs where the mediator
   caches lead selection or fails to re-evaluate fault state. *)
TEST_PROGRAM BoosterMediatorReevaluatesLeadAfterFault
VAR
    Station : BoosterStation;
END_VAR

Station.Initialize();

(* All pumps healthy -> pump 2 leads (lowest runtime) *)
Station.Update(StartCommand := TRUE, SuctionOk := TRUE, HeaderPressure := REAL#4.0, PressureSetpoint := REAL#4.5);
ASSERT_EQUAL(EXPECTED := INT#2, ACTUAL := Station.LeadPumpId);

(* Pump 2 faults -> pump 3 takes lead in the next scan *)
Station.SetPumpFaults(Fault1 := FALSE, Fault2 := TRUE, Fault3 := FALSE);
Station.Update(StartCommand := TRUE, SuctionOk := TRUE, HeaderPressure := REAL#4.0, PressureSetpoint := REAL#4.5);
ASSERT_EQUAL(EXPECTED := INT#3, ACTUAL := Station.LeadPumpId);

(* Pump 2 recovers -> mediator must select a healthy pump (any of 1, 2,
   or 3 is acceptable depending on runtime balance). The point is that
   the mediator re-evaluates rather than caching the previous lead. *)
Station.SetPumpFaults(Fault1 := FALSE, Fault2 := FALSE, Fault3 := FALSE);
Station.Update(StartCommand := TRUE, SuctionOk := TRUE, HeaderPressure := REAL#4.0, PressureSetpoint := REAL#4.5);
ASSERT_TRUE(IN := Station.LeadPumpId >= INT#1);
ASSERT_TRUE(IN := Station.LeadPumpId <= INT#3);
END_TEST_PROGRAM

(* Dry-run permissive: SuctionOk = FALSE blocks all pumps. *)
TEST_PROGRAM BoosterDryRunPermissiveBlocksAllPumps
VAR
    Station : BoosterStation;
END_VAR
Station.Initialize();

(* Suction healthy -> at least one pump runs *)
Station.Update(StartCommand := TRUE, SuctionOk := TRUE, HeaderPressure := REAL#4.0, PressureSetpoint := REAL#4.5);
ASSERT_TRUE(IN := Station.RunningPumpCount >= INT#1);

(* Suction lost -> all pumps must stop, no lead pump *)
Station.Update(StartCommand := TRUE, SuctionOk := FALSE, HeaderPressure := REAL#4.0, PressureSetpoint := REAL#4.5);
ASSERT_EQUAL(EXPECTED := INT#0, ACTUAL := Station.RunningPumpCount);
ASSERT_TRUE(IN := Station.AlarmActive);

(* Suction recovers -> at least one pump runs again (low pressure
   triggers demand). *)
Station.Update(StartCommand := TRUE, SuctionOk := TRUE, HeaderPressure := REAL#4.0, PressureSetpoint := REAL#4.5);
ASSERT_TRUE(IN := Station.RunningPumpCount >= INT#1);
ASSERT_TRUE(IN := Station.LeadPumpId >= INT#1);
END_TEST_PROGRAM

(* StartCommand = FALSE stops all pumps regardless of pressure error. *)
TEST_PROGRAM BoosterStartCommandFalseStopsAllPumps
VAR
    Station : BoosterStation;
END_VAR
Station.Initialize();
Station.Update(StartCommand := TRUE, SuctionOk := TRUE, HeaderPressure := REAL#4.0, PressureSetpoint := REAL#4.5);
ASSERT_TRUE(IN := Station.RunningPumpCount >= INT#1);

(* Stop command -> all pumps stop *)
Station.Update(StartCommand := FALSE, SuctionOk := TRUE, HeaderPressure := REAL#4.0, PressureSetpoint := REAL#4.5);
ASSERT_EQUAL(EXPECTED := INT#0, ACTUAL := Station.RunningPumpCount);
END_TEST_PROGRAM

(* Pressure-on-setpoint case: when pressure == setpoint, no demand. *)
TEST_PROGRAM BoosterAtSetpointHasMinimalDemand
VAR
    Station : BoosterStation;
END_VAR
Station.Initialize();
(* Pressure exactly at setpoint - no error -> minimal pump activity *)
Station.Update(StartCommand := TRUE, SuctionOk := TRUE, HeaderPressure := REAL#4.5, PressureSetpoint := REAL#4.5);
(* Either no pumps or one pump running depending on idle policy *)
ASSERT_TRUE(IN := Station.RunningPumpCount <= INT#1);
END_TEST_PROGRAM

(* Each subscriber observes events independently. *)
TEST_PROGRAM BoosterAlarmBusFansOutToBothSubscribers
VAR
    Station : BoosterStation;
END_VAR
Station.Initialize();

(* Trigger alarm: both Historian count + MQTT flag get set *)
Station.SetPumpFaults(Fault1 := TRUE, Fault2 := TRUE, Fault3 := TRUE);
Station.Update(StartCommand := TRUE, SuctionOk := TRUE, HeaderPressure := REAL#4.0, PressureSetpoint := REAL#4.5);
ASSERT_TRUE(IN := Station.HistorianAlarmCount > INT#0);
ASSERT_TRUE(IN := Station.MqttPublishReady);
END_TEST_PROGRAM

(* Single-pump-fault graceful degradation. *)
TEST_PROGRAM BoosterContinuesWithOnePumpFaulted
VAR
    Station : BoosterStation;
END_VAR
Station.Initialize();
Station.SetPumpFaults(Fault1 := TRUE, Fault2 := FALSE, Fault3 := FALSE);
Station.Update(StartCommand := TRUE, SuctionOk := TRUE, HeaderPressure := REAL#4.0, PressureSetpoint := REAL#4.5);
(* Lead pump is one of the healthy ones (2 or 3) *)
ASSERT_TRUE(IN := (Station.LeadPumpId = INT#2) OR (Station.LeadPumpId = INT#3));
ASSERT_TRUE(IN := Station.LeadPumpId <> INT#1);
END_TEST_PROGRAM

(* Multi-call regression: 20 same-input scans should keep the same lead
   pump selected and not duplicate alarms or recurring publish events
   on healthy operation. *)
TEST_PROGRAM BoosterStableLeadOnHealthyOperation
VAR
    Station : BoosterStation;
    I : INT;
END_VAR
Station.Initialize();
FOR I := 1 TO 20 DO
    Station.Update(StartCommand := TRUE, SuctionOk := TRUE, HeaderPressure := REAL#4.0, PressureSetpoint := REAL#4.5);
END_FOR;
ASSERT_EQUAL(EXPECTED := INT#2, ACTUAL := Station.LeadPumpId);
ASSERT_FALSE(IN := Station.AlarmActive);
ASSERT_EQUAL(EXPECTED := INT#0, ACTUAL := Station.HistorianAlarmCount);
END_TEST_PROGRAM

(* Boundary test: high demand (pressure error >= 1.0) drives the
   mediator to start a lag pump. Total running pumps should be 2. *)
TEST_PROGRAM BoosterHighDemandRunsTwoPumps
VAR
    Station : BoosterStation;
END_VAR
Station.Initialize();
(* Big pressure deficit triggers DemandLevel = 2 (lag pump) *)
Station.Update(StartCommand := TRUE, SuctionOk := TRUE, HeaderPressure := REAL#1.0, PressureSetpoint := REAL#5.0);
ASSERT_EQUAL(EXPECTED := INT#2, ACTUAL := Station.RunningPumpCount);
ASSERT_TRUE(IN := Station.LeadPumpId >= INT#1);
END_TEST_PROGRAM

(* Per-FB isolation: drive a bare PumpDrive object directly and verify
   Run/Stop transitions update IsRunning + SpeedReference. *)
TEST_PROGRAM BoosterPumpDriveStartsAndStopsIndependently
VAR
    Pump : PumpDrive;
END_VAR
Pump.Initialize();
Pump.Configure(PumpId := INT#5, RuntimeHoursInitial := REAL#10.0, Faulted := FALSE);
ASSERT_FALSE(IN := Pump.IsRunning);

Pump.Run(SpeedReference := REAL#75.0);
ASSERT_TRUE(IN := Pump.IsRunning);
ASSERT_NEAR(EXPECTED := REAL#75.0, ACTUAL := Pump.SpeedReference, DELTA := REAL#0.001);

Pump.Stop();
ASSERT_FALSE(IN := Pump.IsRunning);
ASSERT_NEAR(EXPECTED := REAL#0.0, ACTUAL := Pump.SpeedReference, DELTA := REAL#0.001);
END_TEST_PROGRAM

(* Edge input: Run on a faulted pump should not start it; the pump's
   IsHealthy property must gate Run internally. *)
TEST_PROGRAM BoosterPumpDriveRefusesToRunWhenFaulted
VAR
    Pump : PumpDrive;
END_VAR
Pump.Initialize();
Pump.Configure(PumpId := INT#9, RuntimeHoursInitial := REAL#5.0, Faulted := TRUE);
ASSERT_FALSE(IN := Pump.IsHealthy);

Pump.Run(SpeedReference := REAL#80.0);
ASSERT_FALSE(IN := Pump.IsRunning);
ASSERT_NEAR(EXPECTED := REAL#0.0, ACTUAL := Pump.SpeedReference, DELTA := REAL#0.001);
END_TEST_PROGRAM
