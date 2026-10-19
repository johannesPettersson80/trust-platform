(* Bounded-C corpus, retained_values: retained arrays, a structure and texts, in globals and in a
   program. A warm restart carries them into the next generation, on the flat store as their bytes
   (specification 34 section 11.3; slice C1-14b). Every scan changes each of them, an element at a
   time, so a restart that lost or misplaced a byte would show. *)
TYPE Reading :
STRUCT
    level : REAL;
    count : DINT;
    label : STRING[12];
END_STRUCT
END_TYPE

VAR_GLOBAL
    ticks : DINT := DINT#0;
END_VAR

VAR_GLOBAL RETAIN
    window : ARRAY[1..4] OF INT := [INT#1, INT#2, INT#3, INT#4];
    last : Reading := (level := REAL#1.5, count := DINT#0, label := 'start');
    note : STRING[20] := 'idle';
    readings : ARRAY[1..2] OF Reading;
END_VAR

PROGRAM RetainedValues
VAR RETAIN
    history : ARRAY[1..3] OF DINT;
    tag : WSTRING[8] := "w";
END_VAR
VAR
    slot : INT := INT#1;
END_VAR
ticks := ticks + DINT#1;
window[slot] := window[slot] + INT#10;
last.count := last.count + DINT#1;
last.level := last.level + REAL#0.5;
history[slot] := ticks;
readings[2].count := ticks;
IF slot = INT#2 THEN
    note := 'busy';
    last.label := 'running';
    readings[1].label := 'second';
    tag := "ww";
END_IF;
slot := slot + INT#1;
IF slot > INT#3 THEN
    slot := INT#1;
END_IF;
END_PROGRAM

CONFIGURATION BoundedC
RESOURCE Controller ON PLC
TASK Scan (INTERVAL := T#10ms, PRIORITY := 0);
PROGRAM Plant WITH Scan : RetainedValues;
END_RESOURCE
END_CONFIGURATION
