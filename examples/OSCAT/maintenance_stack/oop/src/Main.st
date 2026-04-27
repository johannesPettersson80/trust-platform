(* Compact showcase: Main keeps the component call sequence visible for the ST tests. *)
PROGRAM Main
VAR
    WorkStack : DwordStack16;
    CurrentWorkOrder : DWORD;
END_VAR

WorkStack.Initialize();
WorkStack.Reset();
IF WorkStack.Push(Value := DWORD#16#2001) THEN
END_IF;
IF WorkStack.Push(Value := DWORD#16#2002) THEN
END_IF;
IF WorkStack.TryPop() THEN
    CurrentWorkOrder := WorkStack.Value;
END_IF;
END_PROGRAM
