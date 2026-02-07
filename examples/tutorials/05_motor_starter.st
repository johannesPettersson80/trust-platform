PROGRAM MotorStarter
VAR
    start_pb : BOOL := FALSE;
    stop_pb : BOOL := FALSE;
    overload_trip : BOOL := FALSE;
    motor_run : BOOL := FALSE;
    seal_in_contact : BOOL := FALSE;
END_VAR

IF stop_pb OR overload_trip THEN
    motor_run := FALSE;
ELSIF start_pb OR seal_in_contact THEN
    motor_run := TRUE;
END_IF;

seal_in_contact := motor_run;
END_PROGRAM
