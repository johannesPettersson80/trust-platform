PROGRAM ConveyorSystem
VAR
    start_cmd : BOOL := FALSE;
    stop_cmd : BOOL := FALSE;
    jam_sensor : BOOL := FALSE;
    part_at_entry : BOOL := FALSE;
    part_at_exit : BOOL := FALSE;
    conveyor_run : BOOL := FALSE;
    reject_gate : BOOL := FALSE;
    part_counter : DINT := 0;
END_VAR

IF stop_cmd OR jam_sensor THEN
    conveyor_run := FALSE;
ELSIF start_cmd THEN
    conveyor_run := TRUE;
END_IF;

IF conveyor_run AND part_at_entry THEN
    reject_gate := FALSE;
END_IF;

IF conveyor_run AND part_at_exit THEN
    part_counter := part_counter + 1;
END_IF;

IF jam_sensor THEN
    reject_gate := TRUE;
END_IF;
END_PROGRAM
