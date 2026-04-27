PROGRAM Main
VAR
    OrderQueue : FIFO_16;
    NextOrderId : DWORD;
    QueueEmpty : BOOL;
END_VAR

OrderQueue(DIN := DWORD#16#1001, E := TRUE, RD := FALSE, WD := TRUE, RST := FALSE);
OrderQueue(DIN := DWORD#0, E := TRUE, RD := TRUE, WD := FALSE, RST := FALSE);
NextOrderId := OrderQueue.DOUT;
QueueEmpty := OrderQueue.EMPTY;
END_PROGRAM
