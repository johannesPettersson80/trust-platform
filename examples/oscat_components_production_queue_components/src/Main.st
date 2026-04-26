PROGRAM Main
VAR
    OrderQueue : DwordFifo16;
    QueueObject : IDwordQueue;
    NextOrderId : DWORD;
    QueueEmpty : BOOL;
END_VAR

QueueObject := OrderQueue;
QueueObject.Initialize();
QueueObject.Reset();
IF QueueObject.Push(Value := DWORD#16#1001) THEN
    IF QueueObject.TryPop() THEN
        NextOrderId := QueueObject.Value;
    END_IF;
END_IF;
QueueEmpty := QueueObject.Empty;
END_PROGRAM
