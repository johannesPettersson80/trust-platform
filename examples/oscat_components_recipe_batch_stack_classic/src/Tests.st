TEST_PROGRAM RecipeBatchStackClassicReturnsLatestRecipe
VAR
    RecipeStack : STACK_16;
END_VAR

RecipeStack(DIN := DWORD#0, E := TRUE, RD := FALSE, WD := FALSE, RST := TRUE);
RecipeStack(DIN := DWORD#16#A001, E := TRUE, RD := FALSE, WD := TRUE, RST := FALSE);
RecipeStack(DIN := DWORD#16#A002, E := TRUE, RD := FALSE, WD := TRUE, RST := FALSE);
RecipeStack(DIN := DWORD#0, E := TRUE, RD := TRUE, WD := FALSE, RST := FALSE);

ASSERT_EQUAL(EXPECTED := DWORD#16#A002, ACTUAL := RecipeStack.DOUT);
END_TEST_PROGRAM
