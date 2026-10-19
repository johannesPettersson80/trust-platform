(* Bounded-C corpus, retained_blocks: a function-block instance and a reference in retained
   globals, beside a retained global and a retained program variable. An online change keeps the
   instance and the reference live when its candidate declares the same retained storage
   (specification 34 section 11.3; specification 11 section 6.7.1). *)
FUNCTION_BLOCK Tally
VAR_INPUT
    delta : DINT := DINT#1;
END_VAR
VAR_OUTPUT
    count : DINT;
END_VAR
count := count + delta;
END_FUNCTION_BLOCK

VAR_GLOBAL RETAIN
    kept : Tally;
    follow : REF_TO DINT;
    total : DINT := DINT#0;
END_VAR

PROGRAM RetainedBlocks
VAR RETAIN
    runs : DINT;
END_VAR
VAR
    seen : DINT;
END_VAR
kept(delta := DINT#1);
follow := REF(kept.count);
seen := follow^;
total := total + DINT#1;
runs := runs + DINT#1;
END_PROGRAM

CONFIGURATION BoundedC
RESOURCE Controller ON PLC
TASK Scan (INTERVAL := T#10ms, PRIORITY := 0);
PROGRAM Plant WITH Scan : RetainedBlocks;
END_RESOURCE
END_CONFIGURATION
