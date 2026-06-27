PROGRAM MainProgram
VAR
    e_stop         : BOOL;   (* %IX0.0 — write TRUE to stop the line *)
    running        : BOOL;   (* %QX0.0 *)
    conveyor_speed : INT;    (* %QW0   *)
    part_count     : INT;    (* %MW0   *)
END_VAR

running := NOT e_stop;
IF running THEN
    IF conveyor_speed < 100 THEN
        conveyor_speed := conveyor_speed + 5;
    ELSE
        part_count := part_count + 1;
        conveyor_speed := 0;
    END_IF;
ELSE
    conveyor_speed := 0;
END_IF;
END_PROGRAM
