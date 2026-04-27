TEST_PROGRAM MaintenanceStackComponentReturnsNewestWorkOrderFirst
VAR
    WorkStack : DwordStack16;
END_VAR

WorkStack.Initialize();
WorkStack.Reset();

ASSERT_TRUE(IN := WorkStack.Push(Value := DWORD#16#2001));
ASSERT_TRUE(IN := WorkStack.Push(Value := DWORD#16#2002));
ASSERT_TRUE(IN := WorkStack.TryPop());
ASSERT_EQUAL(EXPECTED := DWORD#16#2002, ACTUAL := WorkStack.Value);
END_TEST_PROGRAM

(* Multi-push/pop LIFO test: pushes three work orders then pops three.
   Verifies LIFO ordering and Empty/non-Empty transitions across
   sequential operations. *)
TEST_PROGRAM MaintenanceStackPreservesLifoOrderAcrossOperations
VAR
    WorkStack : DwordStack16;
END_VAR

WorkStack.Initialize();
WorkStack.Reset();

WorkStack.Push(Value := DWORD#16#3001);
WorkStack.Push(Value := DWORD#16#3002);
WorkStack.Push(Value := DWORD#16#3003);
ASSERT_FALSE(IN := WorkStack.Empty);

(* LIFO order: most recent first *)
ASSERT_TRUE(IN := WorkStack.TryPop());
ASSERT_EQUAL(EXPECTED := DWORD#16#3003, ACTUAL := WorkStack.Value);

ASSERT_TRUE(IN := WorkStack.TryPop());
ASSERT_EQUAL(EXPECTED := DWORD#16#3002, ACTUAL := WorkStack.Value);

ASSERT_TRUE(IN := WorkStack.TryPop());
ASSERT_EQUAL(EXPECTED := DWORD#16#3001, ACTUAL := WorkStack.Value);

(* Empty after three pops *)
ASSERT_TRUE(IN := WorkStack.Empty);
END_TEST_PROGRAM

(* TryPop on empty stack returns FALSE (not garbage). Catches bugs
   where the caller could read stale Value after a failed pop. *)
TEST_PROGRAM MaintenanceStackTryPopOnEmptyReturnsFalse
VAR
    WorkStack : DwordStack16;
END_VAR

WorkStack.Initialize();
WorkStack.Reset();

ASSERT_TRUE(IN := WorkStack.Empty);
(* TryPop on empty: returns FALSE, no transition *)
ASSERT_FALSE(IN := WorkStack.TryPop());
ASSERT_TRUE(IN := WorkStack.Empty);

(* Push, pop to empty, then pop again - second pop fails *)
WorkStack.Push(Value := DWORD#16#4001);
ASSERT_TRUE(IN := WorkStack.TryPop());
ASSERT_TRUE(IN := WorkStack.Empty);
ASSERT_FALSE(IN := WorkStack.TryPop());
END_TEST_PROGRAM

(* Stack capacity boundary: STACK_16 holds 16 items. Push 16, then
   verify Full, then verify push #17 returns FALSE without losing
   any prior values. *)
TEST_PROGRAM MaintenanceStackRespectsCapacityBoundary
VAR
    WorkStack : DwordStack16;
    I : INT;
    PushOk : BOOL;
END_VAR

WorkStack.Initialize();
WorkStack.Reset();

(* Fill the stack: 16 pushes must all succeed *)
FOR I := INT#1 TO INT#16 DO
    PushOk := WorkStack.Push(Value := DWORD_TO_DWORD(INT_TO_DWORD(I)));
    ASSERT_TRUE(IN := PushOk);
END_FOR;

ASSERT_TRUE(IN := WorkStack.Full);

(* 17th push: must fail without disturbing the stack *)
ASSERT_FALSE(IN := WorkStack.Push(Value := DWORD#16#FFFF));
ASSERT_TRUE(IN := WorkStack.Full);

(* Pop the top: should return 16, not 0xFFFF *)
ASSERT_TRUE(IN := WorkStack.TryPop());
ASSERT_EQUAL(EXPECTED := DWORD#16, ACTUAL := WorkStack.Value);
(* Now Full is FALSE, but stack has 15 items *)
ASSERT_FALSE(IN := WorkStack.Full);
ASSERT_FALSE(IN := WorkStack.Empty);
END_TEST_PROGRAM

(* Reset clears the stack: items pushed before Reset are gone. *)
TEST_PROGRAM MaintenanceStackResetClearsAllPriorItems
VAR
    WorkStack : DwordStack16;
END_VAR

WorkStack.Initialize();
WorkStack.Reset();

WorkStack.Push(Value := DWORD#16#5001);
WorkStack.Push(Value := DWORD#16#5002);
WorkStack.Push(Value := DWORD#16#5003);
ASSERT_FALSE(IN := WorkStack.Empty);

(* Reset wipes all pushed items *)
WorkStack.Reset();
ASSERT_TRUE(IN := WorkStack.Empty);
ASSERT_FALSE(IN := WorkStack.Full);

(* Subsequent TryPop on the reset stack returns FALSE *)
ASSERT_FALSE(IN := WorkStack.TryPop());

(* New pushes start fresh *)
WorkStack.Push(Value := DWORD#16#6001);
ASSERT_TRUE(IN := WorkStack.TryPop());
ASSERT_EQUAL(EXPECTED := DWORD#16#6001, ACTUAL := WorkStack.Value);
END_TEST_PROGRAM

(* Multi-cycle simulation: simulates a maintenance scan loop running
   over many iterations, pushing and popping interleaved. Asserts the
   stack state does not leak between cycles - a cycle's pop returns
   that cycle's most recently pushed value. *)
TEST_PROGRAM MaintenanceStackInterleavedPushPopCyclesAreClean
VAR
    WorkStack : DwordStack16;
    I : INT;
    PoppedValues : ARRAY[1..3] OF DWORD;
END_VAR

WorkStack.Initialize();
WorkStack.Reset();

(* Cycle 1: push 7001, push 7002, pop -> expect 7002 *)
WorkStack.Push(Value := DWORD#16#7001);
WorkStack.Push(Value := DWORD#16#7002);
WorkStack.TryPop();
PoppedValues[1] := WorkStack.Value;

(* Cycle 2: push 7003, pop -> expect 7003 *)
WorkStack.Push(Value := DWORD#16#7003);
WorkStack.TryPop();
PoppedValues[2] := WorkStack.Value;

(* Cycle 3: pop the remaining 7001 *)
WorkStack.TryPop();
PoppedValues[3] := WorkStack.Value;

ASSERT_EQUAL(EXPECTED := DWORD#16#7002, ACTUAL := PoppedValues[1]);
ASSERT_EQUAL(EXPECTED := DWORD#16#7003, ACTUAL := PoppedValues[2]);
ASSERT_EQUAL(EXPECTED := DWORD#16#7001, ACTUAL := PoppedValues[3]);
ASSERT_TRUE(IN := WorkStack.Empty);
END_TEST_PROGRAM

(* Push of zero value (DWORD#0) is a legal work-order id and must round-trip
   without being conflated with an empty/error sentinel. *)
TEST_PROGRAM MaintenanceStackZeroValuePushRoundTrips
VAR
    WorkStack : DwordStack16;
END_VAR

WorkStack.Initialize();
WorkStack.Reset();

ASSERT_TRUE(IN := WorkStack.Push(Value := DWORD#0));
ASSERT_FALSE(IN := WorkStack.Empty);
ASSERT_TRUE(IN := WorkStack.TryPop());
ASSERT_EQUAL(EXPECTED := DWORD#0, ACTUAL := WorkStack.Value);
ASSERT_TRUE(IN := WorkStack.Empty);
END_TEST_PROGRAM
