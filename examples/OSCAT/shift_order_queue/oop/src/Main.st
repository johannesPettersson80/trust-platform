(* Compact showcase: Main keeps the component call sequence visible for the ST tests. *)
PROGRAM Main
VAR
    ShiftQueue : DwordFifo16;
    NextShiftOrder : DWORD;
END_VAR

ShiftQueue.Initialize();
ShiftQueue.Reset();
IF ShiftQueue.Push(Value := DWORD#16#3001) THEN
END_IF;
IF ShiftQueue.Push(Value := DWORD#16#3002) THEN
END_IF;
IF ShiftQueue.TryPop() THEN
    NextShiftOrder := ShiftQueue.Value;
END_IF;
END_PROGRAM
