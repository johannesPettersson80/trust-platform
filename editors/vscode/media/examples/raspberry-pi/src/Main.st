PROGRAM Main
VAR
    output_on : BOOL;
    ticks     : DINT;
END_VAR
    ticks := ticks + 1;
    output_on := (ticks MOD 100) < 50;
END_PROGRAM
