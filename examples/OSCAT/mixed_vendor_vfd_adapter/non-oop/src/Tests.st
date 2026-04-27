TEST_PROGRAM ClassicMotorCellDecodesVendorBitsInline
VAR
    Cell : ClassicMotorCell;
END_VAR

ASSERT_EQUAL(EXPECTED := INT#1, ACTUAL := Cell.Update(AbbStatus := WORD#16#0004, DanfossStatus := WORD#16#0002, SiemensStatus := WORD#16#0400, RunCommand := TRUE, PullCordOk := TRUE));
ASSERT_EQUAL(EXPECTED := DWORD#16#20000022, ACTUAL := Cell.LastFaultCode);
END_TEST_PROGRAM
