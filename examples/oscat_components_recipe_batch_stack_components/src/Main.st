PROGRAM Main
VAR
    RecipeStack : DwordStack16;
    StackObject : IDwordStack;
    ActiveRecipeId : DWORD;
END_VAR

StackObject := RecipeStack;
StackObject.Initialize();
StackObject.Reset();
IF StackObject.Push(Value := DWORD#16#A001) THEN
END_IF;
IF StackObject.Push(Value := DWORD#16#A002) THEN
END_IF;
IF StackObject.TryPop() THEN
    ActiveRecipeId := StackObject.Value;
END_IF;
END_PROGRAM
