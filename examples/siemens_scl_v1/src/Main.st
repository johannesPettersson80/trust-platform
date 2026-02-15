FUNCTION_BLOCK FB_EdgeCounter
VAR_INPUT
    Enable : BOOL;
END_VAR
VAR_OUTPUT
    Count : DINT;
END_VAR
VAR
    LastEnable : BOOL;
END_VAR

IF #Enable AND NOT #LastEnable THEN
    #Count := #Count + 1;
END_IF;
#LastEnable := #Enable;
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    Counter : FB_EdgeCounter;
    Total : DINT;
    Pulse : BOOL;
    CountNonZero : BOOL;
END_VAR

#Counter(Enable := #Pulse);
#Total := #Counter.Count;
#CountNonZero := #Total > 0;
END_PROGRAM
