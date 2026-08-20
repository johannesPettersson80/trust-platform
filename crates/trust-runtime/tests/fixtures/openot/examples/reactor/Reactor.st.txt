TYPE E_ReactorStep : (Idle := 0, Filling := 1, Mixing := 2, Draining := 3, Done := 4) END_TYPE

PROGRAM Main
VAR
    StartCmd : BOOL := TRUE;
    BatchStarted : BOOL {attribute 'oot' := 'message', 'template' := 'batch started count', 'severity' := '100', 'arg1' := 'BatchCount'};
    Step : E_ReactorStep {attribute 'oot' := 'state', 'category' := 'process'} := Idle;
    Level : REAL {attribute 'oot' := 'value', 'unit' := 'L', 'deadband' := '0.5'};
    BatchCount : DINT {attribute 'oot' := 'value'};
    HighPhAlarm : BOOL {attribute 'oot' := 'alarm', 'class' := 'alarm', 'severity' := '900', 'cause' := 'Level'};
END_VAR

CASE Step OF
    Idle:
        IF StartCmd THEN
            BatchStarted := TRUE;
            BatchCount := BatchCount + DINT#1;
            Step := Filling;
        END_IF;
    Filling:
        Level := Level + REAL#6.0;
        IF Level >= REAL#12.0 THEN
            Step := Mixing;
        END_IF;
    Mixing:
        Level := Level + REAL#1.5;
        IF Level >= REAL#15.0 THEN
            HighPhAlarm := TRUE;
            Step := Draining;
        END_IF;
    Draining:
        Level := Level - REAL#7.5;
        IF Level <= REAL#0.0 THEN
            HighPhAlarm := FALSE;
            Step := Done;
        END_IF;
    Done:
        ;
ELSE
END_CASE;
END_PROGRAM
