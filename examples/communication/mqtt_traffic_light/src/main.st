PROGRAM TrafficLight
VAR
    Phase : INT := 0;
    PhasePeriod : TIME := T#500ms;
    PhaseTimerEnable : BOOL := TRUE;
    PhaseTimer : TON;
    Red : BOOL := TRUE;
    Yellow : BOOL := FALSE;
    Green : BOOL := FALSE;
END_VAR

PhaseTimer(IN := PhaseTimerEnable, PT := PhasePeriod);

CASE Phase OF
    0:
        Red := TRUE;
        Yellow := FALSE;
        Green := FALSE;
    1:
        Red := TRUE;
        Yellow := TRUE;
        Green := FALSE;
    2:
        Red := FALSE;
        Yellow := FALSE;
        Green := TRUE;
    3:
        Red := FALSE;
        Yellow := TRUE;
        Green := FALSE;
END_CASE;

IF PhaseTimer.Q THEN
    CASE Phase OF
        0:
            Phase := 1;
        1:
            Phase := 2;
        2:
            Phase := 3;
        3:
            Phase := 0;
    END_CASE;
    PhaseTimerEnable := FALSE;
ELSE
    PhaseTimerEnable := TRUE;
END_IF;
END_PROGRAM
