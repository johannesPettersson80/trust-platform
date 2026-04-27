TEST_PROGRAM ProductionQueueComponentPushesAndPopsOrderId
VAR
    OrderQueue : DwordFifo16;
    QueueStatus : ComponentStatus;
END_VAR

OrderQueue.Initialize();
OrderQueue.Reset();

ASSERT_TRUE(IN := OrderQueue.Push(Value := DWORD#16#1001));
ASSERT_TRUE(IN := OrderQueue.TryPop());
ASSERT_EQUAL(EXPECTED := DWORD#16#1001, ACTUAL := OrderQueue.Value);
ASSERT_TRUE(IN := OrderQueue.Empty);
QueueStatus := OrderQueue.Snapshot();
ASSERT_TRUE(IN := QueueStatus.Ready);
ASSERT_FALSE(IN := QueueStatus.Error);
END_TEST_PROGRAM

(* Multi-order FIFO test: pushes three orders then pops three.
   Verifies FIFO ordering and Empty/non-Empty transitions across
   sequential operations. *)
TEST_PROGRAM ProductionQueuePreservesFifoOrderAcrossOperations
VAR
    OrderQueue : DwordFifo16;
END_VAR

OrderQueue.Initialize();
OrderQueue.Reset();

OrderQueue.Push(Value := DWORD#16#1001);
OrderQueue.Push(Value := DWORD#16#1002);
OrderQueue.Push(Value := DWORD#16#1003);
ASSERT_FALSE(IN := OrderQueue.Empty);

(* FIFO order: oldest first *)
ASSERT_TRUE(IN := OrderQueue.TryPop());
ASSERT_EQUAL(EXPECTED := DWORD#16#1001, ACTUAL := OrderQueue.Value);

ASSERT_TRUE(IN := OrderQueue.TryPop());
ASSERT_EQUAL(EXPECTED := DWORD#16#1002, ACTUAL := OrderQueue.Value);

ASSERT_TRUE(IN := OrderQueue.TryPop());
ASSERT_EQUAL(EXPECTED := DWORD#16#1003, ACTUAL := OrderQueue.Value);

(* Empty after three pops *)
ASSERT_TRUE(IN := OrderQueue.Empty);
END_TEST_PROGRAM

(* TryPop on empty returns FALSE without changing Value. The classic
   FIFO_16 had no such signal; the wrapper makes it explicit. *)
TEST_PROGRAM ProductionQueueTryPopOnEmptyReturnsFalse
VAR
    OrderQueue : DwordFifo16;
    PopOk : BOOL;
END_VAR

OrderQueue.Initialize();
OrderQueue.Reset();

(* Empty after Reset *)
ASSERT_TRUE(IN := OrderQueue.Empty);
ASSERT_FALSE(IN := OrderQueue.Full);

(* TryPop on empty must return FALSE *)
PopOk := OrderQueue.TryPop();
ASSERT_FALSE(IN := PopOk);

(* Push one then pop empties; another TryPop returns FALSE *)
OrderQueue.Push(Value := DWORD#16#1001);
OrderQueue.TryPop();
ASSERT_TRUE(IN := OrderQueue.Empty);
PopOk := OrderQueue.TryPop();
ASSERT_FALSE(IN := PopOk);
END_TEST_PROGRAM

(* Boundary: fill the queue to capacity (16) then attempt to push one
   more. Push must return FALSE and the snapshot must report
   QueueFull. *)
TEST_PROGRAM ProductionQueuePushReturnsFalseWhenFull
VAR
    OrderQueue : DwordFifo16;
    Status : ComponentStatus;
    I : INT;
    LastOk : BOOL;
END_VAR

OrderQueue.Initialize();
OrderQueue.Reset();

(* Capacity is 16; push 16 items - all should succeed *)
FOR I := 1 TO 16 DO
    LastOk := OrderQueue.Push(Value := INT_TO_DWORD(I));
    ASSERT_TRUE(IN := LastOk);
END_FOR;
ASSERT_TRUE(IN := OrderQueue.Full);

(* 17th push must fail *)
LastOk := OrderQueue.Push(Value := DWORD#99);
ASSERT_FALSE(IN := LastOk);
Status := OrderQueue.Snapshot();
ASSERT_TRUE(IN := Status.Error);
ASSERT_EQUAL(EXPECTED := ComponentErrorQueueFull, ACTUAL := Status.ErrorId);

(* Pop one; Push must succeed again and clear the error *)
OrderQueue.TryPop();
ASSERT_FALSE(IN := OrderQueue.Full);
LastOk := OrderQueue.Push(Value := DWORD#100);
ASSERT_TRUE(IN := LastOk);
Status := OrderQueue.Snapshot();
ASSERT_FALSE(IN := Status.Error);
END_TEST_PROGRAM

(* Two queues are independent: Push on A does not affect B's Empty,
   Full, or Value. *)
TEST_PROGRAM ProductionQueueTwoQueuesAreIndependent
VAR
    QueueA : DwordFifo16;
    QueueB : DwordFifo16;
END_VAR

QueueA.Initialize();
QueueB.Initialize();
QueueA.Reset();
QueueB.Reset();

QueueA.Push(Value := DWORD#16#A001);
QueueA.Push(Value := DWORD#16#A002);

(* B is still empty *)
ASSERT_TRUE(IN := QueueB.Empty);

QueueB.Push(Value := DWORD#16#B001);
ASSERT_FALSE(IN := QueueA.Empty);
ASSERT_FALSE(IN := QueueB.Empty);

(* Pop from A: B's value is unchanged *)
QueueA.TryPop();
ASSERT_EQUAL(EXPECTED := DWORD#16#A001, ACTUAL := QueueA.Value);
QueueB.TryPop();
ASSERT_EQUAL(EXPECTED := DWORD#16#B001, ACTUAL := QueueB.Value);

(* A still has one element *)
ASSERT_FALSE(IN := QueueA.Empty);
ASSERT_TRUE(IN := QueueB.Empty);
END_TEST_PROGRAM

(* Reset clears the queue mid-stream. After Reset, Empty is TRUE and
   Full is FALSE regardless of previous fill level. *)
TEST_PROGRAM ProductionQueueResetClearsState
VAR
    OrderQueue : DwordFifo16;
    I : INT;
END_VAR

OrderQueue.Initialize();
OrderQueue.Reset();

(* Push some items *)
FOR I := 1 TO 5 DO
    OrderQueue.Push(Value := INT_TO_DWORD(I));
END_FOR;
ASSERT_FALSE(IN := OrderQueue.Empty);

(* Reset wipes the queue *)
OrderQueue.Reset();
ASSERT_TRUE(IN := OrderQueue.Empty);
ASSERT_FALSE(IN := OrderQueue.Full);
ASSERT_FALSE(IN := OrderQueue.TryPop());

(* Queue is reusable after Reset *)
OrderQueue.Push(Value := DWORD#16#2001);
OrderQueue.TryPop();
ASSERT_EQUAL(EXPECTED := DWORD#16#2001, ACTUAL := OrderQueue.Value);
END_TEST_PROGRAM
