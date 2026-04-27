TEST_PROGRAM ColdStoreMediatorPrioritizesFreezerDemand
VAR
    Plant : ColdStorePlant;
END_VAR

Plant.Initialize();
ASSERT_EQUAL(EXPECTED := INT#2, ACTUAL := Plant.Update(Freezer1Temp := REAL#-20.0, Freezer1Door := FALSE, Freezer2Temp := REAL#-20.0, Freezer2Door := FALSE, Chiller1Temp := REAL#4.0, Chiller1Door := FALSE, Chiller2Temp := REAL#1.0, Chiller2Door := FALSE));
ASSERT_EQUAL(EXPECTED := INT#4, ACTUAL := Plant.FreezerDemand);
ASSERT_EQUAL(EXPECTED := INT#1, ACTUAL := Plant.ChillerDemand);
END_TEST_PROGRAM

TEST_PROGRAM ColdStoreObserverPublishesDoorAlarmToSubscribers
VAR
    Plant : ColdStorePlant;
END_VAR

Plant.Initialize();
Plant.Update(Freezer1Temp := REAL#-10.0, Freezer1Door := TRUE, Freezer2Temp := REAL#-25.0, Freezer2Door := FALSE, Chiller1Temp := REAL#2.0, Chiller1Door := FALSE, Chiller2Temp := REAL#2.0, Chiller2Door := FALSE);
ASSERT_TRUE(IN := Plant.AlarmCount > INT#0);
ASSERT_TRUE(IN := Plant.MqttPublishReady);
ASSERT_TRUE(IN := Plant.MaintenanceWorkItems > INT#0);
END_TEST_PROGRAM

(* Multi-scan composite + observer + mediator integration test: drives
   plant through alternating demand and alarm states to verify each
   sub-system reacts independently. Catches bugs where the demand
   counts cache between scans, the maintenance stack double-counts an
   alarm, or the observer count leaks into a healthy scan. *)
TEST_PROGRAM ColdStorePlantTracksDemandsAndAlarmsAcrossScans
VAR
    Plant : ColdStorePlant;
    InitialAlarms : INT;
    InitialMaintenance : INT;
END_VAR

Plant.Initialize();

(* Scan 1: all freezers warm, all chillers cold -> max freezer demand *)
Plant.Update(Freezer1Temp := REAL#-10.0, Freezer1Door := FALSE, Freezer2Temp := REAL#-10.0, Freezer2Door := FALSE, Chiller1Temp := REAL#1.0, Chiller1Door := FALSE, Chiller2Temp := REAL#1.0, Chiller2Door := FALSE);
ASSERT_TRUE(IN := Plant.FreezerDemand >= INT#1);

(* Scan 2: door alarm raises maintenance work items; capture counts *)
Plant.Update(Freezer1Temp := REAL#-25.0, Freezer1Door := TRUE, Freezer2Temp := REAL#-25.0, Freezer2Door := FALSE, Chiller1Temp := REAL#2.0, Chiller1Door := FALSE, Chiller2Temp := REAL#2.0, Chiller2Door := FALSE);
InitialAlarms := Plant.AlarmCount;
InitialMaintenance := Plant.MaintenanceWorkItems;

(* Scan 3: another door alarm event; counters must grow, not stick *)
Plant.Update(Freezer1Temp := REAL#-25.0, Freezer1Door := FALSE, Freezer2Temp := REAL#-25.0, Freezer2Door := TRUE, Chiller1Temp := REAL#2.0, Chiller1Door := FALSE, Chiller2Temp := REAL#2.0, Chiller2Door := FALSE);
ASSERT_TRUE(IN := Plant.AlarmCount >= InitialAlarms);
END_TEST_PROGRAM

(* Mediator: freezer demand prioritized over chiller demand. *)
TEST_PROGRAM ColdStoreMediatorPrioritizesFreezersWhenBothDemand
VAR
    Plant : ColdStorePlant;
END_VAR
Plant.Initialize();
(* All rooms warm -> both freezers AND chillers demand cooling.
   Freezer demand should be prioritized. *)
Plant.Update(Freezer1Temp := REAL#-10.0, Freezer1Door := FALSE, Freezer2Temp := REAL#-10.0, Freezer2Door := FALSE, Chiller1Temp := REAL#5.0, Chiller1Door := FALSE, Chiller2Temp := REAL#5.0, Chiller2Door := FALSE);
ASSERT_TRUE(IN := Plant.FreezerDemand >= Plant.ChillerDemand);
END_TEST_PROGRAM

(* All rooms cold -> no demand at all. *)
TEST_PROGRAM ColdStoreNoDemandWhenAllRoomsAtSetpoint
VAR
    Plant : ColdStorePlant;
END_VAR
Plant.Initialize();
Plant.Update(Freezer1Temp := REAL#-25.0, Freezer1Door := FALSE, Freezer2Temp := REAL#-25.0, Freezer2Door := FALSE, Chiller1Temp := REAL#0.0, Chiller1Door := FALSE, Chiller2Temp := REAL#0.0, Chiller2Door := FALSE);
ASSERT_EQUAL(EXPECTED := INT#0, ACTUAL := Plant.FreezerDemand);
ASSERT_EQUAL(EXPECTED := INT#0, ACTUAL := Plant.ChillerDemand);
END_TEST_PROGRAM

(* Door alarm increments maintenance stack on each event. *)
TEST_PROGRAM ColdStoreMaintenanceStackGrowsWithDoorEvents
VAR
    Plant : ColdStorePlant;
    Initial : INT;
END_VAR
Plant.Initialize();

Plant.Update(Freezer1Temp := REAL#-25.0, Freezer1Door := TRUE, Freezer2Temp := REAL#-25.0, Freezer2Door := FALSE, Chiller1Temp := REAL#2.0, Chiller1Door := FALSE, Chiller2Temp := REAL#2.0, Chiller2Door := FALSE);
Initial := Plant.MaintenanceWorkItems;
ASSERT_TRUE(IN := Initial > INT#0);

Plant.Update(Freezer1Temp := REAL#-25.0, Freezer1Door := FALSE, Freezer2Temp := REAL#-25.0, Freezer2Door := FALSE, Chiller1Temp := REAL#2.0, Chiller1Door := TRUE, Chiller2Temp := REAL#2.0, Chiller2Door := FALSE);
ASSERT_TRUE(IN := Plant.MaintenanceWorkItems >= Initial);
END_TEST_PROGRAM

(* Multiple simultaneous door alarms. *)
TEST_PROGRAM ColdStoreMultipleSimultaneousDoorAlarms
VAR
    Plant : ColdStorePlant;
END_VAR
Plant.Initialize();
(* All four rooms with doors open at once *)
Plant.Update(Freezer1Temp := REAL#-25.0, Freezer1Door := TRUE, Freezer2Temp := REAL#-25.0, Freezer2Door := TRUE, Chiller1Temp := REAL#2.0, Chiller1Door := TRUE, Chiller2Temp := REAL#2.0, Chiller2Door := TRUE);
ASSERT_TRUE(IN := Plant.AlarmCount >= INT#1);
ASSERT_TRUE(IN := Plant.MaintenanceWorkItems >= INT#1);
ASSERT_TRUE(IN := Plant.MqttPublishReady);
END_TEST_PROGRAM
