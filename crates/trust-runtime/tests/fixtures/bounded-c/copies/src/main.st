(* Bounded-C corpus, copies: assignments of whole arrays, structures and texts, which the flat store
   copies as bytes (specification 34 MEM-12; C1-10c, C1-12). An array to an array, a structure
   through dynamic indices on both sides, an array of structures, a global structure from an
   element, a structure that holds an array and an enumeration, a structure that holds a text, two
   copies whose source a call changes between the load and the store, which store the value the
   load read (a structure of numbers and one that holds a text, C1-19b), and texts: one of the same
   type, one cut into a shorter STRING, one cut into a shorter WSTRING and one cut into a
   structure's member through a dynamic reference. *)
TYPE Mode : (Idle, Run, Stop);
END_TYPE

TYPE Pair :
STRUCT
    a : DINT;
    b : REAL;
END_STRUCT
END_TYPE

TYPE Row :
STRUCT
    mode : Mode;
    cells : ARRAY[1..3] OF INT;
END_STRUCT
END_TYPE

TYPE Tagged :
STRUCT
    id : INT;
    name : STRING[8];
END_STRUCT
END_TYPE

VAR_GLOBAL
    ticks : DINT := DINT#0;
    latest : Pair;
END_VAR

FUNCTION Bump : INT
VAR_IN_OUT
    target : DINT;
END_VAR
target := target + DINT#100;
Bump := INT#2;
END_FUNCTION

FUNCTION Flip : INT
VAR_IN_OUT
    target : INT;
END_VAR
target := -target;
Flip := INT#2;
END_FUNCTION

PROGRAM Copies
VAR
    source : ARRAY[1..6] OF DINT;
    target : ARRAY[1..6] OF DINT;
    pairs : ARRAY[1..3] OF Pair;
    pairs_copy : ARRAY[1..3] OF Pair;
    rows : ARRAY[1..2] OF Row;
    rows_copy : ARRAY[1..2] OF Row;
    tags : ARRAY[1..2] OF Tagged;
    label : STRING[16] := 'station-north';
    label_copy : STRING[16];
    short : STRING[4];
    wide : WSTRING[8] := "Ventil-7";
    wide_short : WSTRING[3];
    i : INT := INT#1;
    j : INT := INT#2;
    k : INT;
END_VAR
ticks := ticks + DINT#1;
FOR k := INT#1 TO INT#6 DO
    source[k] := source[k] + ticks;
END_FOR;
target := source;
pairs[i].a := pairs[i].a + ticks;
pairs[i].b := pairs[i].b + REAL#0.5;
pairs[j] := pairs[i];
pairs_copy := pairs;
latest := pairs[j];
rows[i].mode := Mode#Run;
rows[i].cells[j] := rows[i].cells[j] + INT#1;
rows[j] := rows[i];
rows_copy := rows;
pairs[Bump(target := pairs[1].a)] := pairs[1];
tags[1].id := tags[1].id + INT#1;
tags[1].name := 'pump';
tags[2] := tags[1];
tags[Flip(target := tags[1].id)] := tags[1];
label_copy := label;
short := label;
wide_short := wide;
tags[i].name := label;
i := INT#3 - i;
j := INT#3 - j;
END_PROGRAM

CONFIGURATION BoundedC
RESOURCE Controller ON PLC
TASK Scan (INTERVAL := T#10ms, PRIORITY := 0);
PROGRAM Plant WITH Scan : Copies;
END_RESOURCE
END_CONFIGURATION
