(* Managers by composition (specification trust-plant-library §9.3, §9.4): the program names the
   blocks a manager covers in its call. Nothing registers. The HMI alarm list needs no manager:
   the runtime lists every alarm by itself. *)
NAMESPACE Plant

(* A summary of the alarms of up to sixteen blocks, for a horn or an area acknowledgement. *)
FUNCTION_BLOCK FB_AlarmManager IMPLEMENTS IAlarmManager
VAR_INPUT
    Source1 : IAlarmSource;
    Source2 : IAlarmSource;
    Source3 : IAlarmSource;
    Source4 : IAlarmSource;
    Source5 : IAlarmSource;
    Source6 : IAlarmSource;
    Source7 : IAlarmSource;
    Source8 : IAlarmSource;
    Source9 : IAlarmSource;
    Source10 : IAlarmSource;
    Source11 : IAlarmSource;
    Source12 : IAlarmSource;
    Source13 : IAlarmSource;
    Source14 : IAlarmSource;
    Source15 : IAlarmSource;
    Source16 : IAlarmSource;
END_VAR
VAR_OUTPUT NON_RETAIN
    ActiveCount : INT;
    UnacknowledgedCount : INT;
    (* TRUE from a new unacknowledged alarm until Silence or AcknowledgeAll. *)
    Horn : BOOL;
END_VAR
VAR NON_RETAIN
    Sources : ARRAY[1..16] OF IAlarmSource;
    Raised : ULINT;
    Primed : BOOL;
END_VAR
VAR_TEMP
    Index : INT;
    Item : INT;
    Total : ULINT;
    Entry : REF_TO Alarm.Alarm;
END_VAR
    Collect();
    ActiveCount := 0;
    UnacknowledgedCount := 0;
    Total := 0;
    FOR Index := 1 TO 16 DO
        IF Sources[Index] <> NULL THEN
            FOR Item := 1 TO Sources[Index].GetAlarmCount() DO
                Entry := Sources[Index].GetAlarm(Index := Item);
                IF Entry <> NULL THEN
                    (* A suppressed, shelved or out-of-service alarm is not counted (ISA-18.2). *)
                    IF Entry^.Active AND NOT (Entry^.SuppressedByDesign OR Entry^.Shelved OR Entry^.OutOfService) THEN
                        ActiveCount := ActiveCount + 1;
                    END_IF;
                    IF Entry^.Unacknowledged THEN
                        UnacknowledgedCount := UnacknowledgedCount + 1;
                    END_IF;
                    Total := Total + Entry^.AnnunciationCount;
                END_IF;
            END_FOR;
        END_IF;
    END_FOR;
    IF Primed AND Total > Raised THEN
        Horn := TRUE;
    END_IF;
    Raised := Total;
    Primed := TRUE;

METHOD PROTECTED Collect
    Sources[1] := Source1;
    Sources[2] := Source2;
    Sources[3] := Source3;
    Sources[4] := Source4;
    Sources[5] := Source5;
    Sources[6] := Source6;
    Sources[7] := Source7;
    Sources[8] := Source8;
    Sources[9] := Source9;
    Sources[10] := Source10;
    Sources[11] := Source11;
    Sources[12] := Source12;
    Sources[13] := Source13;
    Sources[14] := Source14;
    Sources[15] := Source15;
    Sources[16] := Source16;
END_METHOD

(* Acknowledges the alarms present now; an alarm raised later stays unacknowledged. *)
METHOD PUBLIC AcknowledgeAll : BOOL
VAR
    Index : INT;
    Item : INT;
    Entry : REF_TO Alarm.Alarm;
END_VAR
    Collect();
    FOR Index := 1 TO 16 DO
        IF Sources[Index] <> NULL THEN
            FOR Item := 1 TO Sources[Index].GetAlarmCount() DO
                Entry := Sources[Index].GetAlarm(Index := Item);
                IF Entry <> NULL THEN
                    Entry^.Acknowledge();
                END_IF;
            END_FOR;
        END_IF;
    END_FOR;
    Horn := FALSE;
    AcknowledgeAll := TRUE;
END_METHOD

METHOD PUBLIC Silence : BOOL
    Horn := FALSE;
    Silence := TRUE;
END_METHOD
END_FUNCTION_BLOCK

(* An overview of up to eight sequences; the program still executes each one. *)
FUNCTION_BLOCK FB_SequenceManager IMPLEMENTS ISequenceManager
VAR_INPUT
    Sequence1 : ISequence;
    Sequence2 : ISequence;
    Sequence3 : ISequence;
    Sequence4 : ISequence;
    Sequence5 : ISequence;
    Sequence6 : ISequence;
    Sequence7 : ISequence;
    Sequence8 : ISequence;
END_VAR
VAR_OUTPUT NON_RETAIN
    RunningCount : INT;
    HeldCount : INT;
    FailedCount : INT;
END_VAR
VAR NON_RETAIN
    Sequences : ARRAY[1..8] OF ISequence;
END_VAR
VAR_TEMP
    Index : INT;
    State : E_SequenceState;
END_VAR
    Collect();
    RunningCount := 0;
    HeldCount := 0;
    FailedCount := 0;
    FOR Index := 1 TO 8 DO
        IF Sequences[Index] <> NULL THEN
            State := Sequences[Index].GetState();
            IF State = E_SequenceState#Running OR State = E_SequenceState#Stopping THEN
                RunningCount := RunningCount + 1;
            ELSIF State = E_SequenceState#Held THEN
                HeldCount := HeldCount + 1;
            ELSIF State = E_SequenceState#Failed THEN
                FailedCount := FailedCount + 1;
            END_IF;
        END_IF;
    END_FOR;

METHOD PROTECTED Collect
    Sequences[1] := Sequence1;
    Sequences[2] := Sequence2;
    Sequences[3] := Sequence3;
    Sequences[4] := Sequence4;
    Sequences[5] := Sequence5;
    Sequences[6] := Sequence6;
    Sequences[7] := Sequence7;
    Sequences[8] := Sequence8;
END_METHOD

METHOD PUBLIC StopAll : BOOL
VAR
    Index : INT;
    Accepted : BOOL;
END_VAR
    Collect();
    StopAll := FALSE;
    FOR Index := 1 TO 8 DO
        IF Sequences[Index] <> NULL THEN
            Accepted := Sequences[Index].Stop();
            StopAll := StopAll OR Accepted;
        END_IF;
    END_FOR;
END_METHOD

METHOD PUBLIC AbortAll : BOOL
VAR
    Index : INT;
    Accepted : BOOL;
END_VAR
    Collect();
    AbortAll := FALSE;
    FOR Index := 1 TO 8 DO
        IF Sequences[Index] <> NULL THEN
            Accepted := Sequences[Index].Abort();
            AbortAll := AbortAll OR Accepted;
        END_IF;
    END_FOR;
END_METHOD
END_FUNCTION_BLOCK

END_NAMESPACE
