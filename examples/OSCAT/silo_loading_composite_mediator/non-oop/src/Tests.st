TEST_PROGRAM ClassicSiloLoadingBranchesOverSilos
VAR
    System : ClassicSiloLoadingSystem;
END_VAR

ASSERT_EQUAL(EXPECTED := INT#2, ACTUAL := System.Update(Request1 := TRUE, Level1 := REAL#95.0, Request2 := TRUE, Level2 := REAL#40.0, Request3 := FALSE, Level3 := REAL#20.0));
ASSERT_EQUAL(EXPECTED := INT#1, ACTUAL := System.BlockedCount);
END_TEST_PROGRAM
