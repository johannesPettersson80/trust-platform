TEST_PROGRAM RecipeBatchStackComponentReturnsLatestRecipe
VAR
    RecipeStack : DwordStack16;
    StackObject : IDwordStack;
END_VAR

StackObject := RecipeStack;
StackObject.Initialize();
StackObject.Reset();

ASSERT_TRUE(IN := StackObject.Push(Value := DWORD#16#A001));
ASSERT_TRUE(IN := StackObject.Push(Value := DWORD#16#A002));
ASSERT_TRUE(IN := StackObject.TryPop());
ASSERT_EQUAL(EXPECTED := DWORD#16#A002, ACTUAL := StackObject.Value);
END_TEST_PROGRAM
