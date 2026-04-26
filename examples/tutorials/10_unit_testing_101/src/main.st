FUNCTION LimitAdd : INT
VAR_INPUT
    AddendA : INT;
    AddendB : INT;
    MaxValue : INT;
END_VAR
VAR
    Sum : INT;
END_VAR

Sum := AddendA + AddendB;
IF Sum > MaxValue THEN
    LimitAdd := MaxValue;
ELSE
    LimitAdd := Sum;
END_IF;
END_FUNCTION

FUNCTION ScaleRawToPercent : REAL
VAR_INPUT
    Raw : INT;
END_VAR

ScaleRawToPercent := INT_TO_REAL(IN := Raw) * REAL#0.1;
END_FUNCTION

FUNCTION_BLOCK StartStopLatch
VAR_INPUT
    StartCmd : BOOL;
    StopCmd : BOOL;
END_VAR
VAR_OUTPUT
    Running : BOOL;
END_VAR

IF StopCmd THEN
    Running := FALSE;
ELSIF StartCmd THEN
    Running := TRUE;
END_IF;
END_FUNCTION_BLOCK
