(* Bounded-C corpus, bindings: values a call binds and copies back that hold references or lie in
   a structure (design row 28): an output copied back into a structure's member; a function whose
   frame holds a per-call instance with a REF_TO input and an array of references; one that binds
   an element of its own array to a VAR_IN_OUT; a member with a long name reached through a
   changing index; a structure that holds a reference assigned whole. *)
TYPE Reading :
STRUCT
    level : DINT;
    label : STRING[12];
END_STRUCT
END_TYPE

TYPE Linked :
STRUCT
    count : DINT;
    target : REF_TO DINT;
END_STRUCT
END_TYPE

TYPE Measured :
STRUCT
    a_member_whose_name_is_rather_long : DINT;
END_STRUCT
END_TYPE

VAR_GLOBAL
    ticks : DINT := DINT#0;
    shared : DINT := DINT#7;
END_VAR

FUNCTION Split : BOOL
VAR_INPUT
    value : DINT;
END_VAR
VAR_OUTPUT
    high : DINT;
END_VAR
high := value / DINT#10;
Split := TRUE;
END_FUNCTION

FUNCTION_BLOCK Follower
VAR_INPUT
    source : REF_TO DINT;
END_VAR
VAR_OUTPUT
    seen : DINT;
END_VAR
IF source <> NULL THEN
    seen := source^;
END_IF;
END_FUNCTION_BLOCK

FUNCTION Follow : DINT
VAR_INPUT
    origin : REF_TO DINT;
END_VAR
VAR
    follower : Follower;
    pointers : ARRAY[1..2] OF REF_TO DINT;
END_VAR
pointers[1] := origin;
pointers[2] := origin;
follower(source := pointers[2]);
Follow := follower.seen;
END_FUNCTION

FUNCTION Accumulate : DINT
VAR_IN_OUT
    slot : DINT;
END_VAR
slot := slot + DINT#1;
Accumulate := slot;
END_FUNCTION

FUNCTION Tally : DINT
VAR_INPUT
    increment : DINT;
END_VAR
VAR
    cells : ARRAY[1..2] OF DINT;
    k : INT := INT#2;
END_VAR
cells[k] := increment;
Tally := Accumulate(slot := cells[k]);
END_FUNCTION

PROGRAM Bindings
VAR
    measurement : Reading;
    link : Linked;
    copied : Linked;
    samples : ARRAY[1..2] OF Measured;
    seen : DINT;
    counted : DINT;
    done : BOOL;
    i : INT := INT#1;
END_VAR
ticks := ticks + DINT#1;
done := Split(value := ticks, high => measurement.level);
link.count := ticks;
link.target := REF(shared);
copied := link;
seen := Follow(origin := copied.target);
counted := Tally(increment := ticks);
samples[i].a_member_whose_name_is_rather_long := ticks;
i := INT#3 - i;
END_PROGRAM

CONFIGURATION BoundedC
RESOURCE Controller ON PLC
TASK Scan (INTERVAL := T#10ms, PRIORITY := 0);
PROGRAM Plant WITH Scan : Bindings;
END_RESOURCE
END_CONFIGURATION
