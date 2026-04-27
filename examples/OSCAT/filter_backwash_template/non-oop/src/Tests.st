TEST_PROGRAM ClassicBackwashBranchesOnMediaType
VAR
    Backwash : ClassicBackwashController;
END_VAR

ASSERT_EQUAL(EXPECTED := INT#100, ACTUAL := Backwash.Update(FilterType := INT#1, ElapsedSeconds := INT#360, TurbidityNtu := REAL#1.5, FlowOk := TRUE));
ASSERT_EQUAL(EXPECTED := INT#20, ACTUAL := Backwash.Update(FilterType := INT#2, ElapsedSeconds := INT#360, TurbidityNtu := REAL#1.5, FlowOk := TRUE));
END_TEST_PROGRAM
