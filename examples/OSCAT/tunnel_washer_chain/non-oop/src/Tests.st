TEST_PROGRAM ClassicWasherFallsThroughToReject
VAR
    Washer : ClassicWasherQuality;
END_VAR

Washer.Initialize();
ASSERT_EQUAL(EXPECTED := INT#4, ACTUAL := Washer.Evaluate(SoilLevel := REAL#40.0, Conductivity := REAL#4.0, RinseConductivity := REAL#0.5, OperatorAvailable := FALSE));
ASSERT_EQUAL(EXPECTED := DWORD#1, ACTUAL := Washer.RejectCount);
END_TEST_PROGRAM
