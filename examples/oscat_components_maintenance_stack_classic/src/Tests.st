TEST_PROGRAM MaintenanceStackClassicReturnsNewestWorkOrderFirst
VAR
    WorkStack : STACK_16;
END_VAR

WorkStack(DIN := DWORD#0, E := TRUE, RD := FALSE, WD := FALSE, RST := TRUE);
WorkStack(DIN := DWORD#16#2001, E := TRUE, RD := FALSE, WD := TRUE, RST := FALSE);
WorkStack(DIN := DWORD#16#2002, E := TRUE, RD := FALSE, WD := TRUE, RST := FALSE);
WorkStack(DIN := DWORD#0, E := TRUE, RD := TRUE, WD := FALSE, RST := FALSE);

ASSERT_EQUAL(EXPECTED := DWORD#16#2002, ACTUAL := WorkStack.DOUT);
END_TEST_PROGRAM
