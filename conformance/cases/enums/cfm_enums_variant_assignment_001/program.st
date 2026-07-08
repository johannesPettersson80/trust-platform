TYPE Phase :
(
    IDLE := 0,
    RUNNING := 1,
    DONE := 2
);
END_TYPE

PROGRAM Main
VAR
    state : Phase := Phase#IDLE;
    is_running : BOOL := FALSE;
END_VAR

state := RUNNING;
is_running := state = RUNNING;
END_PROGRAM
