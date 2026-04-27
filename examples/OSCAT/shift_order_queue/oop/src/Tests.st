TEST_PROGRAM ShiftOrderQueueComponentReturnsFirstOrderFirst
VAR
    ShiftQueue : DwordFifo16;
END_VAR

ShiftQueue.Initialize();
ShiftQueue.Reset();

ASSERT_TRUE(IN := ShiftQueue.Push(Value := DWORD#16#3001));
ASSERT_TRUE(IN := ShiftQueue.Push(Value := DWORD#16#3002));
ASSERT_TRUE(IN := ShiftQueue.TryPop());
ASSERT_EQUAL(EXPECTED := DWORD#16#3001, ACTUAL := ShiftQueue.Value);
END_TEST_PROGRAM

(* Multi-shift FIFO test: pushes four shift orders then pops them.
   Verifies FIFO ordering across more than two operations and that
   Empty/non-Empty transitions reflect the queue contents. *)
TEST_PROGRAM ShiftOrderQueuePreservesFifoOrderForMultipleShifts
VAR
    ShiftQueue : DwordFifo16;
END_VAR

ShiftQueue.Initialize();
ShiftQueue.Reset();

ShiftQueue.Push(Value := DWORD#16#4001);
ShiftQueue.Push(Value := DWORD#16#4002);
ShiftQueue.Push(Value := DWORD#16#4003);
ShiftQueue.Push(Value := DWORD#16#4004);
ASSERT_FALSE(IN := ShiftQueue.Empty);

ShiftQueue.TryPop();
ASSERT_EQUAL(EXPECTED := DWORD#16#4001, ACTUAL := ShiftQueue.Value);
ShiftQueue.TryPop();
ASSERT_EQUAL(EXPECTED := DWORD#16#4002, ACTUAL := ShiftQueue.Value);
ShiftQueue.TryPop();
ASSERT_EQUAL(EXPECTED := DWORD#16#4003, ACTUAL := ShiftQueue.Value);
ShiftQueue.TryPop();
ASSERT_EQUAL(EXPECTED := DWORD#16#4004, ACTUAL := ShiftQueue.Value);
ASSERT_TRUE(IN := ShiftQueue.Empty);
END_TEST_PROGRAM

(* Empty contract: after Initialize+Reset the queue is Empty and
   TryPop returns FALSE without faulting. Validates the safe-default
   state for a freshly composed component. *)
TEST_PROGRAM ShiftOrderQueueEmptyAfterReset
VAR
    ShiftQueue : DwordFifo16;
END_VAR

ShiftQueue.Initialize();
ShiftQueue.Reset();
ASSERT_TRUE(IN := ShiftQueue.Empty);
ASSERT_FALSE(IN := ShiftQueue.Full);
ASSERT_FALSE(IN := ShiftQueue.TryPop());
ASSERT_TRUE(IN := ShiftQueue.Empty);
END_TEST_PROGRAM

(* Capacity boundary: 16 successful pushes fills the queue; the 17th
   push must return FALSE without corrupting contents. *)
TEST_PROGRAM ShiftOrderQueueRefusesOverflow
VAR
    ShiftQueue : DwordFifo16;
END_VAR

ShiftQueue.Initialize();
ShiftQueue.Reset();
ASSERT_TRUE(IN := ShiftQueue.Push(Value := DWORD#16#5001));
ASSERT_TRUE(IN := ShiftQueue.Push(Value := DWORD#16#5002));
ASSERT_TRUE(IN := ShiftQueue.Push(Value := DWORD#16#5003));
ASSERT_TRUE(IN := ShiftQueue.Push(Value := DWORD#16#5004));
ASSERT_TRUE(IN := ShiftQueue.Push(Value := DWORD#16#5005));
ASSERT_TRUE(IN := ShiftQueue.Push(Value := DWORD#16#5006));
ASSERT_TRUE(IN := ShiftQueue.Push(Value := DWORD#16#5007));
ASSERT_TRUE(IN := ShiftQueue.Push(Value := DWORD#16#5008));
ASSERT_TRUE(IN := ShiftQueue.Push(Value := DWORD#16#5009));
ASSERT_TRUE(IN := ShiftQueue.Push(Value := DWORD#16#500A));
ASSERT_TRUE(IN := ShiftQueue.Push(Value := DWORD#16#500B));
ASSERT_TRUE(IN := ShiftQueue.Push(Value := DWORD#16#500C));
ASSERT_TRUE(IN := ShiftQueue.Push(Value := DWORD#16#500D));
ASSERT_TRUE(IN := ShiftQueue.Push(Value := DWORD#16#500E));
ASSERT_TRUE(IN := ShiftQueue.Push(Value := DWORD#16#500F));
ASSERT_TRUE(IN := ShiftQueue.Push(Value := DWORD#16#5010));
ASSERT_TRUE(IN := ShiftQueue.Full);
(* 17th push: refused, no overflow *)
ASSERT_FALSE(IN := ShiftQueue.Push(Value := DWORD#16#5FFF));
(* Front of queue is still the oldest (5001) - FIFO order preserved *)
ASSERT_TRUE(IN := ShiftQueue.TryPop());
ASSERT_EQUAL(EXPECTED := DWORD#16#5001, ACTUAL := ShiftQueue.Value);
END_TEST_PROGRAM

(* Push-pop interleaving: pushing after a pop must keep FIFO ordering
   intact. Catches a class of bug where TryPop side-effects corrupt
   subsequent push positions. *)
TEST_PROGRAM ShiftOrderQueuePushPopInterleavingKeepsOrder
VAR
    ShiftQueue : DwordFifo16;
END_VAR

ShiftQueue.Initialize();
ShiftQueue.Reset();
ShiftQueue.Push(Value := DWORD#16#6001);
ShiftQueue.Push(Value := DWORD#16#6002);
ASSERT_TRUE(IN := ShiftQueue.TryPop());
ASSERT_EQUAL(EXPECTED := DWORD#16#6001, ACTUAL := ShiftQueue.Value);
(* Push a third order onto the back (6002 still queued) *)
ShiftQueue.Push(Value := DWORD#16#6003);
(* Pop must give 6002 next (FIFO order, not LIFO) *)
ASSERT_TRUE(IN := ShiftQueue.TryPop());
ASSERT_EQUAL(EXPECTED := DWORD#16#6002, ACTUAL := ShiftQueue.Value);
ASSERT_TRUE(IN := ShiftQueue.TryPop());
ASSERT_EQUAL(EXPECTED := DWORD#16#6003, ACTUAL := ShiftQueue.Value);
ASSERT_TRUE(IN := ShiftQueue.Empty);
END_TEST_PROGRAM

(* Reset clears the queue mid-run: after pushing values, Reset must
   restore Empty so subsequent operators do not re-process stale orders. *)
TEST_PROGRAM ShiftOrderQueueResetClearsResidualState
VAR
    ShiftQueue : DwordFifo16;
END_VAR

ShiftQueue.Initialize();
ShiftQueue.Reset();
ShiftQueue.Push(Value := DWORD#16#7001);
ShiftQueue.Push(Value := DWORD#16#7002);
ASSERT_FALSE(IN := ShiftQueue.Empty);
(* Reset mid-run *)
ShiftQueue.Reset();
ASSERT_TRUE(IN := ShiftQueue.Empty);
ASSERT_FALSE(IN := ShiftQueue.Full);
ASSERT_FALSE(IN := ShiftQueue.TryPop());
(* Queue is reusable after reset *)
ASSERT_TRUE(IN := ShiftQueue.Push(Value := DWORD#16#8001));
ASSERT_TRUE(IN := ShiftQueue.TryPop());
ASSERT_EQUAL(EXPECTED := DWORD#16#8001, ACTUAL := ShiftQueue.Value);
END_TEST_PROGRAM
