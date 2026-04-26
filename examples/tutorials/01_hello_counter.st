PROGRAM HelloCounter
VAR
    Enable : BOOL := TRUE;
    ResetCmd : BOOL := FALSE;
    Increment : DINT := 1;
    Count : DINT := 0;
END_VAR

IF ResetCmd THEN
    Count := 0;
ELSIF Enable THEN
    Count := Count + Increment;
END_IF;
END_PROGRAM
