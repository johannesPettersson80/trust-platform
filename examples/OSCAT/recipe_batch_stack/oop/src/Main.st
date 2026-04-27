(* Compact showcase: Main keeps the component call sequence visible for the ST tests. *)
PROGRAM Main
VAR
    RecipeStack : DwordStack16;
    ActiveRecipeId : DWORD;
END_VAR

RecipeStack.Initialize();
RecipeStack.Reset();
IF RecipeStack.Push(Value := DWORD#16#A001) THEN
END_IF;
IF RecipeStack.Push(Value := DWORD#16#A002) THEN
END_IF;
IF RecipeStack.TryPop() THEN
    ActiveRecipeId := RecipeStack.Value;
END_IF;
END_PROGRAM
