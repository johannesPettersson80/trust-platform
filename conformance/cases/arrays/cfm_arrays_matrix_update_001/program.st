PROGRAM Main
VAR
    grid : ARRAY[0..1, 0..1] OF DINT;
    total : DINT := DINT#0;
END_VAR

grid[0,0] := DINT#1;
grid[0,1] := DINT#2;
grid[1,0] := DINT#3;
grid[1,1] := DINT#4;
total := grid[0,0] + grid[0,1] + grid[1,0] + grid[1,1];
END_PROGRAM
