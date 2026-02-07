PROGRAM Blinker
VAR
    enable : BOOL := TRUE;
    lamp : BOOL := FALSE;
    pulse_pt : TIME := T#250ms;
    timer_enable : BOOL := TRUE;
    blink_timer : TON;
END_VAR

blink_timer(IN := timer_enable, PT := pulse_pt);

IF NOT enable THEN
    lamp := FALSE;
    timer_enable := FALSE;
ELSIF blink_timer.Q THEN
    lamp := NOT lamp;
    timer_enable := FALSE;
ELSE
    timer_enable := TRUE;
END_IF;
END_PROGRAM
