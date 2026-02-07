PROGRAM HelloCounter
VAR
    enable : BOOL := TRUE;
    reset_cmd : BOOL := FALSE;
    increment : DINT := 1;
    count : DINT := 0;
END_VAR

IF reset_cmd THEN
    count := 0;
ELSIF enable THEN
    count := count + increment;
END_IF;
END_PROGRAM
