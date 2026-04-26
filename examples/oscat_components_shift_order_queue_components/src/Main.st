PROGRAM Main
VAR
    ShiftQueue : DwordFifo16;
    QueueObject : IDwordQueue;
    NextShiftOrder : DWORD;
END_VAR

QueueObject := ShiftQueue;
QueueObject.Initialize();
QueueObject.Reset();
IF QueueObject.Push(Value := DWORD#16#3001) THEN
END_IF;
IF QueueObject.Push(Value := DWORD#16#3002) THEN
END_IF;
IF QueueObject.TryPop() THEN
    NextShiftOrder := QueueObject.Value;
END_IF;
END_PROGRAM
