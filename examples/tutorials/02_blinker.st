PROGRAM Blinker
VAR
    Enable : BOOL := TRUE;
    Lamp : BOOL := FALSE;
    PulsePeriod : TIME := T#250ms;
    TimerEnable : BOOL := TRUE;
    BlinkTimer : TON;
END_VAR

BlinkTimer(IN := TimerEnable, PT := PulsePeriod);

IF NOT Enable THEN
    Lamp := FALSE;
    TimerEnable := FALSE;
ELSIF BlinkTimer.Q THEN
    Lamp := NOT Lamp;
    TimerEnable := FALSE;
ELSE
    TimerEnable := TRUE;
END_IF;
END_PROGRAM
