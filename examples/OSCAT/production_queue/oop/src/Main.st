(* Compact showcase: Main keeps the component call sequence visible for the ST tests. *)
PROGRAM Main
VAR
    OrderQueue : DwordFifo16;
    QueueStatus : ComponentStatus;
    NextOrderId : DWORD;
    QueueEmpty : BOOL;
    QueueHealthy : BOOL;
END_VAR

OrderQueue.Initialize();
OrderQueue.Reset();
IF OrderQueue.Push(Value := DWORD#16#1001) THEN
    IF OrderQueue.TryPop() THEN
        NextOrderId := OrderQueue.Value;
    END_IF;
END_IF;
QueueEmpty := OrderQueue.Empty;
QueueStatus := OrderQueue.Snapshot();
QueueHealthy := QueueStatus.Ready AND NOT QueueStatus.Error;
END_PROGRAM
