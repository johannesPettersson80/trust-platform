PROGRAM Main
VAR
    ShiftQueue : FIFO_16;
    NextShiftOrder : DWORD;
END_VAR

ShiftQueue(DIN := DWORD#16#3001, E := TRUE, RD := FALSE, WD := TRUE, RST := FALSE);
ShiftQueue(DIN := DWORD#16#3002, E := TRUE, RD := FALSE, WD := TRUE, RST := FALSE);
ShiftQueue(DIN := DWORD#0, E := TRUE, RD := TRUE, WD := FALSE, RST := FALSE);
NextShiftOrder := ShiftQueue.DOUT;
END_PROGRAM
