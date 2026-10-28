(* Bounded-C corpus, structures: field writes, whole and nested structure assignment, a write
   into a copy, an enumeration, and a STRUCT OVERLAP pair of views (census rows I1 to I3, I5,
   I7 and I9). *)
TYPE Mode : (Idle, Running, Stopped)
END_TYPE

TYPE Band :
STRUCT
    low : REAL;
    high : REAL;
END_STRUCT
END_TYPE

TYPE Axis :
STRUCT
    position : REAL;
    limits : Band;
    mode : Mode;
END_STRUCT
END_TYPE

TYPE Packed : STRUCT OVERLAP
    as_word AT %B0 : DWORD;
    as_int AT %B0 : DINT;
END_STRUCT; END_TYPE

VAR_GLOBAL
    ticks : DINT := DINT#0;
END_VAR

PROGRAM Structures
VAR
    axis : Axis;
    spare : Axis;
    bounds : Band;
    state : Mode := Mode#Idle;
    packed : Packed;
    raw : DINT;
END_VAR
ticks := ticks + DINT#1;
axis.position := axis.position + REAL#0.5;
axis.limits.high := axis.position * REAL#2.0;
bounds := axis.limits;
spare := axis;
spare.limits.low := bounds.high;
CASE state OF
    Idle: state := Mode#Running;
    Running: state := Mode#Stopped;
ELSE
    state := Mode#Idle;
END_CASE;
axis.mode := state;
packed.as_word := packed.as_word XOR DWORD#1;
raw := packed.as_int;
END_PROGRAM

CONFIGURATION BoundedC
RESOURCE Controller ON PLC
TASK Scan (INTERVAL := T#10ms, PRIORITY := 0);
PROGRAM Plant WITH Scan : Structures;
END_RESOURCE
END_CONFIGURATION
