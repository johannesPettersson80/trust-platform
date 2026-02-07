PROGRAM TrafficLight
VAR
    phase : INT := 0;
    phase_pt : TIME := T#500ms;
    phase_timer_enable : BOOL := TRUE;
    phase_timer : TON;
    red : BOOL := TRUE;
    yellow : BOOL := FALSE;
    green : BOOL := FALSE;
END_VAR

phase_timer(IN := phase_timer_enable, PT := phase_pt);

CASE phase OF
    0:
        red := TRUE;
        yellow := FALSE;
        green := FALSE;
    1:
        red := TRUE;
        yellow := TRUE;
        green := FALSE;
    2:
        red := FALSE;
        yellow := FALSE;
        green := TRUE;
    3:
        red := FALSE;
        yellow := TRUE;
        green := FALSE;
END_CASE;

IF phase_timer.Q THEN
    CASE phase OF
        0:
            phase := 1;
        1:
            phase := 2;
        2:
            phase := 3;
        3:
            phase := 0;
    END_CASE;
    phase_timer_enable := FALSE;
ELSE
    phase_timer_enable := TRUE;
END_IF;
END_PROGRAM
