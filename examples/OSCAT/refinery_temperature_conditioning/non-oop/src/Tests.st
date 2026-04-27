TEST_PROGRAM ClassicConditioningRejectsSingleScanSpike
VAR
    Chain : ClassicTemperatureConditioning;
    Value : REAL;
END_VAR

Chain.Initialize();
Value := Chain.Update(Input1 := REAL#100.0, Quality1 := BYTE#2, Input2 := REAL#102.0, Quality2 := BYTE#2, Input3 := REAL#99.0, Quality3 := BYTE#2);
ASSERT_EQUAL(EXPECTED := BYTE#2, ACTUAL := Chain.Quality);
Value := Chain.Update(Input1 := REAL#180.0, Quality1 := BYTE#2, Input2 := REAL#181.0, Quality2 := BYTE#2, Input3 := REAL#182.0, Quality3 := BYTE#2);
ASSERT_EQUAL(EXPECTED := BYTE#1, ACTUAL := Chain.Quality);
ASSERT_EQUAL(EXPECTED := INT#1, ACTUAL := Chain.RejectedCount);
END_TEST_PROGRAM
