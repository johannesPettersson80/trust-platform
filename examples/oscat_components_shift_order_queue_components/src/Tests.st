TEST_PROGRAM ShiftOrderQueueComponentReturnsFirstOrderFirst
VAR
    ShiftQueue : DwordFifo16;
    QueueObject : IDwordQueue;
END_VAR

QueueObject := ShiftQueue;
QueueObject.Initialize();
QueueObject.Reset();

ASSERT_TRUE(IN := QueueObject.Push(Value := DWORD#16#3001));
ASSERT_TRUE(IN := QueueObject.Push(Value := DWORD#16#3002));
ASSERT_TRUE(IN := QueueObject.TryPop());
ASSERT_EQUAL(EXPECTED := DWORD#16#3001, ACTUAL := QueueObject.Value);
END_TEST_PROGRAM
