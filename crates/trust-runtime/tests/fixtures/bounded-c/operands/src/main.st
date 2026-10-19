(* Bounded-C corpus, operands: comparisons whose operands are globals, read through their static
   references, and are not scalars that copy for free: two enumerations, two WSTRINGs and two
   references, each compared as the whole of an assignment to a global, and an enumeration compared
   with a constant as the whole of an IF condition, so that the register tier reads both operands
   within one operation (C1-18). The values change from scan to scan, so every comparison is true in
   some scans and false in others. *)
TYPE Mode : (Idle, Run, Stop);
END_TYPE

VAR_GLOBAL
    ticks : DINT := DINT#0;
    current : Mode := Mode#Idle;
    wanted : Mode := Mode#Run;
    same_mode : BOOL;
    text_a : WSTRING[8] := "ab";
    text_b : WSTRING[8] := "ab";
    same_text : BOOL;
    target : DINT;
    ref_a : REF_TO DINT;
    ref_b : REF_TO DINT;
    same_ref : BOOL;
END_VAR

PROGRAM Operands
ticks := ticks + DINT#1;
same_mode := current = wanted;
same_text := text_a = text_b;
same_ref := ref_a = ref_b;
IF current = Mode#Idle THEN
    current := Mode#Run;
ELSE
    current := Mode#Idle;
END_IF;
IF same_text THEN
    text_b := "cd";
ELSE
    text_b := "ab";
END_IF;
IF same_ref THEN
    ref_b := NULL;
ELSE
    ref_b := ref_a;
END_IF;
ref_a := REF(target);
END_PROGRAM

CONFIGURATION BoundedC
RESOURCE Controller ON PLC
TASK Scan (INTERVAL := T#10ms, PRIORITY := 0);
PROGRAM Plant WITH Scan : Operands;
END_RESOURCE
END_CONFIGURATION
