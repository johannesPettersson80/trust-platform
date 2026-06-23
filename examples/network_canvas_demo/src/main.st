PROGRAM Main
VAR_EXTERNAL
    conveyor_speed : REAL;
    running : BOOL;
    line_state : INT;
END_VAR

running := TRUE;
IF conveyor_speed < 2.0 THEN
    conveyor_speed := conveyor_speed + 0.1;
END_IF;
line_state := 1;
END_PROGRAM
