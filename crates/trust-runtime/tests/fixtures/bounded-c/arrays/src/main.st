(* Bounded-C corpus, arrays: indexed reads and writes with dynamic indices, whole-array
   assignment, a two-dimensional array, and an array of structures with element and field
   writes (census rows I1, I2, I6 and I7). *)
TYPE Point :
STRUCT
    x : INT;
    y : INT;
END_STRUCT
END_TYPE

VAR_GLOBAL
    ticks : DINT := DINT#0;
END_VAR

PROGRAM Arrays
VAR
    data : ARRAY[1..8] OF DINT;
    copy : ARRAY[1..8] OF DINT;
    grid : ARRAY[0..2, 0..2] OF INT;
    points : ARRAY[0..3] OF Point;
    index : DINT := DINT#1;
    row : INT := INT#0;
    total : DINT := DINT#0;
END_VAR
ticks := ticks + DINT#1;
FOR index := DINT#1 TO DINT#8 DO
    data[index] := data[index] + index;
END_FOR;
copy := data;
total := copy[3] + data[(ticks MOD DINT#8) + DINT#1];
grid[row, row] := grid[row, row] + INT#1;
grid[2, 0] := grid[0, 2];
points[1].x := points[1].x + INT#2;
points[2] := points[1];
points[3].y := points[2].x;
row := row + INT#1;
IF row > INT#2 THEN
    row := INT#0;
END_IF;
END_PROGRAM

CONFIGURATION BoundedC
RESOURCE Controller ON PLC
TASK Scan (INTERVAL := T#10ms, PRIORITY := 0);
PROGRAM Plant WITH Scan : Arrays;
END_RESOURCE
END_CONFIGURATION
