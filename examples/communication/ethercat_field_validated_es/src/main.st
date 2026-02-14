PROGRAM Main
VAR_EXTERNAL
    DO0 : BOOL;
    DO1 : BOOL;
    DO2 : BOOL;
    DO3 : BOOL;
    DO4 : BOOL;
    DO5 : BOOL;
    DO6 : BOOL;
    DO7 : BOOL;
END_VAR

VAR
    step_timer : TON;
    step_time : TIME := T#200MS;
    position : INT := 0;
    direction : INT := 1;
END_VAR

step_timer(IN := NOT step_timer.Q, PT := step_time);

IF step_timer.Q THEN
    position := position + direction;
    IF position >= 15 THEN
        direction := -1;
    END_IF;
    IF position <= 0 THEN
        direction := 1;
    END_IF;
END_IF;

IF position <= 7 THEN
    DO0 := (position >= 0);
    DO1 := (position >= 1);
    DO2 := (position >= 2);
    DO3 := (position >= 3);
    DO4 := (position >= 4);
    DO5 := (position >= 5);
    DO6 := (position >= 6);
    DO7 := (position >= 7);
ELSE
    DO0 := (position < 15);
    DO1 := (position < 14);
    DO2 := (position < 13);
    DO3 := (position < 12);
    DO4 := (position < 11);
    DO5 := (position < 10);
    DO6 := (position < 9);
    DO7 := (position < 8);
END_IF;
END_PROGRAM
