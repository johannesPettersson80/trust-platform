PROGRAM Main
VAR
    WorkStack : STACK_16;
    CurrentWorkOrder : DWORD;
END_VAR

WorkStack(DIN := DWORD#16#2001, E := TRUE, RD := FALSE, WD := TRUE, RST := FALSE);
WorkStack(DIN := DWORD#16#2002, E := TRUE, RD := FALSE, WD := TRUE, RST := FALSE);
WorkStack(DIN := DWORD#0, E := TRUE, RD := TRUE, WD := FALSE, RST := FALSE);
CurrentWorkOrder := WorkStack.DOUT;
END_PROGRAM
