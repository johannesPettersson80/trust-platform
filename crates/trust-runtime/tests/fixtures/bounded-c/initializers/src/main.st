(* Bounded-C corpus, initializers: call-time initializers (census rows N1 to N4 and I13). A
   function and a method each initialize a reference to an element selected by their input
   on every call, the function also builds an array from its initializer on every call, and
   it keeps a static that is initialized once. *)
VAR_GLOBAL
    ticks : DINT := DINT#0;
    table : ARRAY[1..3] OF INT := [INT#4, INT#5, INT#6];
END_VAR

FUNCTION Pick : INT
VAR_INPUT
    slot : INT;
END_VAR
VAR
    chosen : REF_TO INT := REF(table[slot]);
    window : ARRAY[1..2] OF INT := [INT#7, INT#8];
END_VAR
VAR_STAT
    calls : INT := INT#0;
END_VAR
calls := calls + INT#1;
Pick := chosen^ + window[1] + calls;
END_FUNCTION

FUNCTION_BLOCK Tracker
VAR
    history : ARRAY[1..3] OF INT := [INT#1, INT#2, INT#3];
END_VAR
METHOD PUBLIC Look : INT
VAR_INPUT
    slot : INT;
END_VAR
VAR
    previous : REF_TO INT := REF(history[slot]);
END_VAR
Look := previous^;
END_METHOD
END_FUNCTION_BLOCK

PROGRAM Initializers
VAR
    tracker : Tracker;
    slot : INT := INT#1;
    result : INT;
END_VAR
ticks := ticks + DINT#1;
result := Pick(slot) + tracker.Look(slot);
slot := slot + INT#1;
IF slot > INT#3 THEN
    slot := INT#1;
END_IF;
END_PROGRAM

CONFIGURATION BoundedC
RESOURCE Controller ON PLC
TASK Scan (INTERVAL := T#10ms, PRIORITY := 0);
PROGRAM Plant WITH Scan : Initializers;
END_RESOURCE
END_CONFIGURATION
