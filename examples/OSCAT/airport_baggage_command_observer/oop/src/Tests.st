TEST_PROGRAM BaggageCommandExecutesScannerDecision
VAR
    Diverter : BaggageDiverter;
END_VAR
Diverter.Initialize();
ASSERT_EQUAL(EXPECTED := INT#2, ACTUAL := Diverter.ProcessBag(BagId := INT#42, ScannerResult := INT#2));
ASSERT_EQUAL(EXPECTED := INT#42, ACTUAL := Diverter.LastBagId);
END_TEST_PROGRAM

TEST_PROGRAM BaggageObserverPublishesAuditEvent
VAR
    Diverter : BaggageDiverter;
END_VAR
Diverter.Initialize();
Diverter.ProcessBag(BagId := INT#43, ScannerResult := INT#9);
ASSERT_EQUAL(EXPECTED := INT#1, ACTUAL := Diverter.EventCount);
ASSERT_TRUE(IN := Diverter.MqttPublishReady);
ASSERT_EQUAL(EXPECTED := INT#3, ACTUAL := Diverter.LastCommandId);
END_TEST_PROGRAM

(* Multi-event scan-loop test: drives the diverter through several
   different scanner results and verifies the historian counts every
   event and LastBagId/LastCommandId reflect the most recent decision.
   Catches bugs where command selection or event publication leaks
   state between scans. *)
TEST_PROGRAM BaggageDiverterAccumulatesEventsAcrossScans
VAR
    Diverter : BaggageDiverter;
    I : INT;
END_VAR

Diverter.Initialize();

Diverter.ProcessBag(BagId := INT#1, ScannerResult := INT#1);
ASSERT_EQUAL(EXPECTED := INT#1, ACTUAL := Diverter.LastCommandId);
ASSERT_EQUAL(EXPECTED := INT#1, ACTUAL := Diverter.LastBagId);

Diverter.ProcessBag(BagId := INT#2, ScannerResult := INT#2);
ASSERT_EQUAL(EXPECTED := INT#2, ACTUAL := Diverter.LastCommandId);
ASSERT_EQUAL(EXPECTED := INT#2, ACTUAL := Diverter.LastBagId);

Diverter.ProcessBag(BagId := INT#3, ScannerResult := INT#9);
ASSERT_EQUAL(EXPECTED := INT#3, ACTUAL := Diverter.LastCommandId);
ASSERT_EQUAL(EXPECTED := INT#3, ACTUAL := Diverter.LastBagId);

(* All three bags should have published to the historian *)
ASSERT_EQUAL(EXPECTED := INT#3, ACTUAL := Diverter.EventCount);
END_TEST_PROGRAM
