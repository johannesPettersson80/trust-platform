PROGRAM Main
VAR
    WorkStack : DwordStack16;
    StackObject : IDwordStack;
    CurrentWorkOrder : DWORD;
END_VAR

StackObject := WorkStack;
StackObject.Initialize();
StackObject.Reset();
IF StackObject.Push(Value := DWORD#16#2001) THEN
END_IF;
IF StackObject.Push(Value := DWORD#16#2002) THEN
END_IF;
IF StackObject.TryPop() THEN
    CurrentWorkOrder := StackObject.Value;
END_IF;
END_PROGRAM
