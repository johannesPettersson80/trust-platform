TEST_PROGRAM ProductionQueueComponentPushesAndPopsOrderId
VAR
    OrderQueue : DwordFifo16;
    QueueObject : IDwordQueue;
END_VAR

QueueObject := OrderQueue;
QueueObject.Initialize();
QueueObject.Reset();

ASSERT_TRUE(IN := QueueObject.Push(Value := DWORD#16#1001));
ASSERT_TRUE(IN := QueueObject.TryPop());
ASSERT_EQUAL(EXPECTED := DWORD#16#1001, ACTUAL := QueueObject.Value);
ASSERT_TRUE(IN := QueueObject.Empty);
END_TEST_PROGRAM
