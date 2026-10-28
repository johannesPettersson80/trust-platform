(* Bounded-C corpus, io: the process-image bindings of census rows S1 to S7. Flat %I, %Q and %M
   bits, bytes, words and double words; an INT array and a structure bound element by element;
   STRING[n] byte windows, the input one longer than 23 bytes; and hierarchical %IX and %QX
   addresses. The program makes no call and holds no text beyond the bound windows. *)
TYPE Pair :
STRUCT
    a : INT;
    b : BYTE;
END_STRUCT
END_TYPE

VAR_GLOBAL
    ticks : DINT := DINT#0;
END_VAR

PROGRAM Io
VAR
    start AT %IX0.0 : BOOL;
    level AT %IB1 : BYTE;
    speed AT %IW2 : WORD;
    count AT %ID4 : DWORD;
    samples AT %IW8 : ARRAY[0..1] OF INT;
    label AT %IB12 : STRING[24];
    remote AT %IX1.2.3 : BOOL;
    lamp AT %QX0.0 : BOOL;
    shown AT %QB1 : BYTE;
    target AT %QW2 : WORD;
    pair AT %QB4 : Pair;
    echo AT %QB8 : STRING[8];
    beacon AT %QX1.2.4 : BOOL;
    held AT %MW0 : WORD;
    seen : DWORD;
END_VAR
ticks := ticks + DINT#1;
lamp := start XOR remote;
shown := level;
target := speed;
seen := count;
pair.a := samples[1];
pair.b := level;
echo := label;
beacon := start;
held := held XOR speed;
END_PROGRAM

CONFIGURATION BoundedC
RESOURCE Controller ON PLC
TASK Scan (INTERVAL := T#10ms, PRIORITY := 0);
PROGRAM Plant WITH Scan : Io;
END_RESOURCE
END_CONFIGURATION
