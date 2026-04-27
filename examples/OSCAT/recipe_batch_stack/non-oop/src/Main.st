PROGRAM Main
VAR
    RecipeStack : STACK_16;
    ActiveRecipeId : DWORD;
END_VAR

RecipeStack(DIN := DWORD#16#A001, E := TRUE, RD := FALSE, WD := TRUE, RST := FALSE);
RecipeStack(DIN := DWORD#16#A002, E := TRUE, RD := FALSE, WD := TRUE, RST := FALSE);
RecipeStack(DIN := DWORD#0, E := TRUE, RD := TRUE, WD := FALSE, RST := FALSE);
ActiveRecipeId := RecipeStack.DOUT;
END_PROGRAM
