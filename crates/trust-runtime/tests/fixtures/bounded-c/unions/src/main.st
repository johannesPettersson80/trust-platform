(* Bounded-C corpus, unions: a truST UNION keeps every variant as a member of its own
   (specification 02 section 3.7; slice C1-15). Every scan writes one variant and reads the others
   back unchanged, copies whole unions, and keeps a union in a structure, in an array, beside the
   shared bits of an overlay and as a retained global. *)
TYPE Choice : UNION
    count : INT := 1;
    ready : BOOL;
    label : STRING[6];
END_UNION
END_TYPE

TYPE Holder :
STRUCT
    choice : Choice;
    weight : REAL;
END_STRUCT
END_TYPE

TYPE Framed : STRUCT OVERLAP
    whole AT %B0 : UDINT;
    low AT %B0 : UINT;
    pick AT %B4 : Choice;
END_STRUCT; END_TYPE

VAR_GLOBAL
    ticks : DINT := DINT#0;
END_VAR

VAR_GLOBAL RETAIN
    kept : Choice;
END_VAR

PROGRAM Unions
VAR
    choice : Choice;
    copy : Choice;
    holder : Holder;
    many : ARRAY[1..2] OF Choice;
    framed : Framed;
    seen : INT;
    flag : BOOL;
    text : STRING[6];
    low : UINT;
END_VAR
ticks := ticks + DINT#1;
choice.count := choice.count + INT#1;
choice.ready := NOT choice.ready;
IF (ticks MOD DINT#3) = DINT#0 THEN
    choice.label := 'three';
END_IF;
seen := choice.count;
flag := choice.ready;
text := choice.label;
copy := choice;
holder.choice := copy;
holder.weight := holder.weight + REAL#0.25;
many[1].count := seen;
many[2] := many[1];
framed.whole := framed.whole + UDINT#16#00010001;
framed.pick.count := framed.pick.count + INT#2;
low := framed.low;
kept.count := kept.count + INT#1;
kept.label := text;
END_PROGRAM

CONFIGURATION BoundedC
RESOURCE Controller ON PLC
TASK Scan (INTERVAL := T#10ms, PRIORITY := 0);
PROGRAM Plant WITH Scan : Unions;
END_RESOURCE
END_CONFIGURATION
