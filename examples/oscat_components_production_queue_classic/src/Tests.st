TEST_PROGRAM ProductionQueueClassicPushesAndPopsOrderId
VAR
    OrderQueue : FIFO_16;
END_VAR

OrderQueue(DIN := DWORD#0, E := TRUE, RD := FALSE, WD := FALSE, RST := TRUE);
OrderQueue(DIN := DWORD#16#1001, E := TRUE, RD := FALSE, WD := TRUE, RST := FALSE);
OrderQueue(DIN := DWORD#0, E := TRUE, RD := TRUE, WD := FALSE, RST := FALSE);

ASSERT_EQUAL(EXPECTED := DWORD#16#1001, ACTUAL := OrderQueue.DOUT);
ASSERT_TRUE(IN := OrderQueue.EMPTY);
END_TEST_PROGRAM
