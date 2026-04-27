TEST_PROGRAM ShiftOrderQueueClassicReturnsFirstOrderFirst
VAR
    ShiftQueue : FIFO_16;
END_VAR

ShiftQueue(DIN := DWORD#0, E := TRUE, RD := FALSE, WD := FALSE, RST := TRUE);
ShiftQueue(DIN := DWORD#16#3001, E := TRUE, RD := FALSE, WD := TRUE, RST := FALSE);
ShiftQueue(DIN := DWORD#16#3002, E := TRUE, RD := FALSE, WD := TRUE, RST := FALSE);
ShiftQueue(DIN := DWORD#0, E := TRUE, RD := TRUE, WD := FALSE, RST := FALSE);

ASSERT_EQUAL(EXPECTED := DWORD#16#3001, ACTUAL := ShiftQueue.DOUT);
END_TEST_PROGRAM
