PROGRAM Main
VAR
    Count : INT := 0;
END_VAR

VAR_EXTERNAL
    InSignal : BOOL;
    OutSignal : BOOL;
END_VAR

IF InSignal THEN
    Count := Count + 1;
END_IF;

OutSignal := (Count MOD 2) = 1;
END_PROGRAM
