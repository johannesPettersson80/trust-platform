(* C1-22 (specification 34 MEM-12; checklist C1, "values read and written in place, no heap
   allocation for a value"): a global and a program variable of 5,000 DINTs each, 20,000 bytes apiece
   in the persistent block. A chip builds the default of each in its own bytes; as a value, each would
   take 5,000 values on the heap at once. *)
CONFIGURATION Config
RESOURCE Res ON PLC
    TASK Main (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM P1 WITH Main : Fill;
END_RESOURCE
END_CONFIGURATION

VAR_GLOBAL
    Table : ARRAY[0..4999] OF DINT;
END_VAR

PROGRAM Fill
VAR
    Samples : ARRAY[0..4999] OF DINT;
    I : INT;
END_VAR
Samples[I] := Table[I] + 1;
I := (I + 1) MOD 5000;
END_PROGRAM
