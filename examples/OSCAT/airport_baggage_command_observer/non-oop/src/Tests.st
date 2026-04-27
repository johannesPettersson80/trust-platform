TEST_PROGRAM ClassicBaggageBranchesOnScannerResult
VAR
    Diverter : ClassicBaggageDiverter;
END_VAR
ASSERT_EQUAL(EXPECTED := INT#2, ACTUAL := Diverter.ProcessBag(BagId := INT#42, ScannerResult := INT#2));
ASSERT_EQUAL(EXPECTED := INT#1, ACTUAL := Diverter.EventCount);
END_TEST_PROGRAM
