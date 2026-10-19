(* Bounded-C corpus, overlays: STRUCT OVERLAP members that share their bits (specification 02
   section 3.6; slice C1-15). Every scan writes the shared bits through one member and reads them
   through another: a double integer and its bytes, a bit of it, an enumeration and its integer, and
   a nested structure whose gap bits only the word over it sets. A copy of the nested structure
   carries those gap bits into another overlay. Texts and an array of texts lie beside the shared
   bits. A retained overlay, an array of overlays, a structure holding one and a function's local
   overlay take the copies, restarts and frames of the flat store. *)
TYPE Mode : (Idle, Running, Stopped)
END_TYPE

TYPE Flags :
STRUCT
    enabled : BOOL;
    level : BYTE;
END_STRUCT
END_TYPE

TYPE Packet : STRUCT OVERLAP
    value32 AT %B0 : UDINT;
    bytes AT %B0 : ARRAY[0..3] OF BYTE;
    flag AT %X0.0 : BOOL;
    status AT %B4 : Flags;
    raw AT %B4 : UINT;
    mode AT %B6 : Mode;
    mode_raw AT %B6 : INT;
    label AT %B8 : STRING[8];
    names AT %B16 : ARRAY[1..2] OF STRING[4];
END_STRUCT; END_TYPE

TYPE Small : STRUCT OVERLAP
    whole AT %B0 : UINT;
    high AT %B1 : BYTE;
END_STRUCT; END_TYPE

TYPE Frame :
STRUCT
    head : Small;
    count : INT;
END_STRUCT
END_TYPE

VAR_GLOBAL
    ticks : DINT := DINT#0;
END_VAR

VAR_GLOBAL RETAIN
    kept : Packet;
END_VAR

FUNCTION HighByte : BYTE
VAR_INPUT
    value : UINT;
END_VAR
VAR
    view : Small;
END_VAR
view.whole := value;
HighByte := view.high;
END_FUNCTION

PROGRAM Overlays
VAR
    packet : Packet;
    mirror : Packet;
    saved : Flags;
    pair : ARRAY[1..2] OF Small;
    frame : Frame;
    low : BYTE;
    level : BYTE;
    high : BYTE;
    state : INT;
    gaps : UINT;
    text : STRING[8];
END_VAR
ticks := ticks + DINT#1;
packet.value32 := packet.value32 + UDINT#16#00010001;
low := packet.bytes[0];
packet.flag := NOT packet.flag;
packet.raw := packet.raw + UINT#16#0103;
saved := packet.status;
mirror.status := saved;
gaps := mirror.raw;
level := mirror.status.level;
CASE packet.mode OF
    Mode#Idle: packet.mode := Mode#Running;
    Mode#Running: packet.mode := Mode#Stopped;
ELSE
    packet.mode := Mode#Idle;
END_CASE;
state := packet.mode_raw;
IF (ticks MOD DINT#2) = DINT#0 THEN
    packet.label := 'even';
    packet.names[1] := 'ab';
ELSE
    packet.label := 'odd';
    packet.names[2] := 'cd';
END_IF;
text := packet.label;
IF (ticks MOD DINT#5) = DINT#0 THEN
    mirror := packet;
END_IF;
kept.value32 := kept.value32 + UDINT#16#00000100;
kept.label := packet.label;
pair[1].whole := pair[1].whole + UINT#16#0101;
pair[2].high := pair[1].high;
frame.count := frame.count + INT#1;
frame.head.whole := UINT#16#1200 + INT_TO_UINT(frame.count);
high := HighByte(frame.head.whole);
END_PROGRAM

CONFIGURATION BoundedC
RESOURCE Controller ON PLC
TASK Scan (INTERVAL := T#10ms, PRIORITY := 0);
PROGRAM Plant WITH Scan : Overlays;
END_RESOURCE
END_CONFIGURATION
