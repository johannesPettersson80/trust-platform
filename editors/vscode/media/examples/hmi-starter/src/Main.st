PROGRAM Main
VAR
    level   : INT;
    running : BOOL;
END_VAR
    running := TRUE;
    level := (level + 1) MOD 100;
END_PROGRAM
