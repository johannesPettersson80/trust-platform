TEST_PROGRAM RecipeBatchStackComponentReturnsLatestRecipe
VAR
    RecipeStack : DwordStack16;
END_VAR

RecipeStack.Initialize();
RecipeStack.Reset();

ASSERT_TRUE(IN := RecipeStack.Push(Value := DWORD#16#A001));
ASSERT_TRUE(IN := RecipeStack.Push(Value := DWORD#16#A002));
ASSERT_TRUE(IN := RecipeStack.TryPop());
ASSERT_EQUAL(EXPECTED := DWORD#16#A002, ACTUAL := RecipeStack.Value);
END_TEST_PROGRAM

(* Recipe rollback test: pushes a base + variant + revision, then
   pops them. Verifies the LIFO pattern rolls revisions back to the
   most recently committed variant in the correct order. *)
TEST_PROGRAM RecipeBatchStackRollsBackInLifoOrder
VAR
    RecipeStack : DwordStack16;
END_VAR

RecipeStack.Initialize();
RecipeStack.Reset();

(* Stack three recipe revisions *)
RecipeStack.Push(Value := DWORD#16#A001);
RecipeStack.Push(Value := DWORD#16#A002);
RecipeStack.Push(Value := DWORD#16#A003);

(* Roll back to most recent variant (A003 is on top) *)
ASSERT_TRUE(IN := RecipeStack.TryPop());
ASSERT_EQUAL(EXPECTED := DWORD#16#A003, ACTUAL := RecipeStack.Value);

(* Roll back further to the previous variant *)
ASSERT_TRUE(IN := RecipeStack.TryPop());
ASSERT_EQUAL(EXPECTED := DWORD#16#A002, ACTUAL := RecipeStack.Value);

(* And finally the base *)
ASSERT_TRUE(IN := RecipeStack.TryPop());
ASSERT_EQUAL(EXPECTED := DWORD#16#A001, ACTUAL := RecipeStack.Value);

ASSERT_TRUE(IN := RecipeStack.Empty);
END_TEST_PROGRAM

(* Empty contract: after Initialize+Reset the stack is Empty and
   TryPop returns FALSE without faulting the component. *)
TEST_PROGRAM RecipeBatchStackEmptyAfterReset
VAR
    RecipeStack : DwordStack16;
END_VAR

RecipeStack.Initialize();
RecipeStack.Reset();
ASSERT_TRUE(IN := RecipeStack.Empty);
ASSERT_FALSE(IN := RecipeStack.Full);
(* TryPop on empty: returns FALSE, no rollback to do *)
ASSERT_FALSE(IN := RecipeStack.TryPop());
ASSERT_TRUE(IN := RecipeStack.Empty);
END_TEST_PROGRAM

(* Capacity boundary: 16 successful pushes fills the stack; the
   17th push must return FALSE without corrupting the contents. *)
TEST_PROGRAM RecipeBatchStackRefusesOverflow
VAR
    RecipeStack : DwordStack16;
END_VAR

RecipeStack.Initialize();
RecipeStack.Reset();
(* Push 16 distinct values: each must succeed *)
ASSERT_TRUE(IN := RecipeStack.Push(Value := DWORD#16#B001));
ASSERT_TRUE(IN := RecipeStack.Push(Value := DWORD#16#B002));
ASSERT_TRUE(IN := RecipeStack.Push(Value := DWORD#16#B003));
ASSERT_TRUE(IN := RecipeStack.Push(Value := DWORD#16#B004));
ASSERT_TRUE(IN := RecipeStack.Push(Value := DWORD#16#B005));
ASSERT_TRUE(IN := RecipeStack.Push(Value := DWORD#16#B006));
ASSERT_TRUE(IN := RecipeStack.Push(Value := DWORD#16#B007));
ASSERT_TRUE(IN := RecipeStack.Push(Value := DWORD#16#B008));
ASSERT_TRUE(IN := RecipeStack.Push(Value := DWORD#16#B009));
ASSERT_TRUE(IN := RecipeStack.Push(Value := DWORD#16#B00A));
ASSERT_TRUE(IN := RecipeStack.Push(Value := DWORD#16#B00B));
ASSERT_TRUE(IN := RecipeStack.Push(Value := DWORD#16#B00C));
ASSERT_TRUE(IN := RecipeStack.Push(Value := DWORD#16#B00D));
ASSERT_TRUE(IN := RecipeStack.Push(Value := DWORD#16#B00E));
ASSERT_TRUE(IN := RecipeStack.Push(Value := DWORD#16#B00F));
ASSERT_TRUE(IN := RecipeStack.Push(Value := DWORD#16#B010));
ASSERT_TRUE(IN := RecipeStack.Full);
(* 17th push: refused, no overflow *)
ASSERT_FALSE(IN := RecipeStack.Push(Value := DWORD#16#BFFF));
(* Stack still full and intact: top is the 16th value B010 *)
ASSERT_TRUE(IN := RecipeStack.TryPop());
ASSERT_EQUAL(EXPECTED := DWORD#16#B010, ACTUAL := RecipeStack.Value);
END_TEST_PROGRAM

(* Push-pop interleaving: pushing after a pop must keep LIFO ordering
   intact. Catches a class of bug where TryPop side-effects corrupt
   subsequent push positions. *)
TEST_PROGRAM RecipeBatchStackPushPopInterleavingKeepsOrder
VAR
    RecipeStack : DwordStack16;
END_VAR

RecipeStack.Initialize();
RecipeStack.Reset();
RecipeStack.Push(Value := DWORD#16#C001);
RecipeStack.Push(Value := DWORD#16#C002);
ASSERT_TRUE(IN := RecipeStack.TryPop());
ASSERT_EQUAL(EXPECTED := DWORD#16#C002, ACTUAL := RecipeStack.Value);
(* Push a third recipe on top of what is left (C001 underneath) *)
RecipeStack.Push(Value := DWORD#16#C003);
ASSERT_TRUE(IN := RecipeStack.TryPop());
ASSERT_EQUAL(EXPECTED := DWORD#16#C003, ACTUAL := RecipeStack.Value);
ASSERT_TRUE(IN := RecipeStack.TryPop());
ASSERT_EQUAL(EXPECTED := DWORD#16#C001, ACTUAL := RecipeStack.Value);
ASSERT_TRUE(IN := RecipeStack.Empty);
END_TEST_PROGRAM

(* Reset clears the stack mid-run: after pushing values, Reset must
   restore Empty and clear residual telemetry. *)
TEST_PROGRAM RecipeBatchStackResetClearsResidualState
VAR
    RecipeStack : DwordStack16;
END_VAR

RecipeStack.Initialize();
RecipeStack.Reset();
RecipeStack.Push(Value := DWORD#16#D001);
RecipeStack.Push(Value := DWORD#16#D002);
ASSERT_FALSE(IN := RecipeStack.Empty);
(* Reset mid-run *)
RecipeStack.Reset();
ASSERT_TRUE(IN := RecipeStack.Empty);
ASSERT_FALSE(IN := RecipeStack.Full);
ASSERT_FALSE(IN := RecipeStack.TryPop());
(* Stack is reusable after reset *)
ASSERT_TRUE(IN := RecipeStack.Push(Value := DWORD#16#E001));
ASSERT_TRUE(IN := RecipeStack.TryPop());
ASSERT_EQUAL(EXPECTED := DWORD#16#E001, ACTUAL := RecipeStack.Value);
END_TEST_PROGRAM
