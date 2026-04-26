TEST_PROGRAM MaintenanceStackComponentReturnsNewestWorkOrderFirst
VAR
    WorkStack : DwordStack16;
    StackObject : IDwordStack;
END_VAR

StackObject := WorkStack;
StackObject.Initialize();
StackObject.Reset();

ASSERT_TRUE(IN := StackObject.Push(Value := DWORD#16#2001));
ASSERT_TRUE(IN := StackObject.Push(Value := DWORD#16#2002));
ASSERT_TRUE(IN := StackObject.TryPop());
ASSERT_EQUAL(EXPECTED := DWORD#16#2002, ACTUAL := StackObject.Value);
END_TEST_PROGRAM
