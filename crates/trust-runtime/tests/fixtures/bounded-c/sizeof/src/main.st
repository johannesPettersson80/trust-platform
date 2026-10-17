(* Bounded-C corpus, sizeof: SIZEOF of a structured type, of a variable of that type, of a
   bounded string variable and of an elementary type on every scan (census row I11). *)
TYPE Block :
STRUCT
    id : DINT;
    data : ARRAY[0..3] OF INT;
    name : STRING[12];
END_STRUCT
END_TYPE

VAR_GLOBAL
    ticks : DINT := DINT#0;
    sample : Block;
END_VAR

PROGRAM Sizes
VAR
    label : STRING[12] := 'gauge';
    of_type : DINT;
    of_value : DINT;
    of_text : DINT;
    of_scalar : DINT;
    same : BOOL;
END_VAR
ticks := ticks + DINT#1;
of_type := SIZEOF(Block);
of_value := SIZEOF(sample);
of_text := SIZEOF(label);
of_scalar := SIZEOF(LREAL);
same := of_type = of_value;
END_PROGRAM

CONFIGURATION BoundedC
RESOURCE Controller ON PLC
TASK Scan (INTERVAL := T#10ms, PRIORITY := 0);
PROGRAM Plant WITH Scan : Sizes;
END_RESOURCE
END_CONFIGURATION
