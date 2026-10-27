(* Bounded-C corpus, references: REF of a global, of an array element with a changing index and
   of a structure member, loads and stores through them, a NULL reference, and a reference
   assignment attempt (census rows I1, I2 and I4 to I6). *)
TYPE Sample :
STRUCT
    amount : DINT;
    valid : BOOL;
END_STRUCT
END_TYPE

VAR_GLOBAL
    ticks : DINT := DINT#0;
    store : ARRAY[1..4] OF DINT;
    reading : Sample;
    target : DINT := DINT#5;
END_VAR

PROGRAM References
VAR
    whole : REF_TO DINT;
    element : REF_TO DINT;
    member : REF_TO DINT;
    nothing : REF_TO DINT;
    checked : REF_TO DINT;
    slot : DINT := DINT#1;
    result : DINT;
    present : BOOL;
END_VAR
ticks := ticks + DINT#1;
whole := REF(target);
element := REF(store[slot]);
member := REF(reading.amount);
whole^ := whole^ + DINT#1;
element^ := element^ + ticks;
member^ := member^ + element^;
checked ?= element;
present := (checked <> NULL) AND (nothing = NULL);
result := checked^ + member^;
slot := slot + DINT#1;
IF slot > DINT#4 THEN
    slot := DINT#1;
END_IF;
END_PROGRAM

CONFIGURATION BoundedC
RESOURCE Controller ON PLC
TASK Scan (INTERVAL := T#10ms, PRIORITY := 0);
PROGRAM Plant WITH Scan : References;
END_RESOURCE
END_CONFIGURATION
