(* Groups of interchangeable run devices (specification trust-plant-library §9.2). Members are
   chosen by composition: Member1..Member4 in the call. A group never registers anything; it
   commands its members through IDutyMember.Demand, which they follow only in Auto, and every
   member keeps its own interlocks, faults and supervision. *)
NAMESPACE Plant
USING Alarm;

FUNCTION_BLOCK ABSTRACT FB_Group IMPLEMENTS IAlarmSource
VAR_INPUT
    Member1 : IDutyMember;
    Member2 : IDutyMember;
    Member3 : IDutyMember;
    Member4 : IDutyMember;
END_VAR
VAR_OUTPUT NON_RETAIN
    (* How many members run, are available, and the group wants running. *)
    RunningCount : INT;
    AvailableCount : INT;
    Target : INT;
    (* Fewer members run than wanted, for longer than ShortfallDelay. *)
    Shortfall : BOOL;
    Selected : ARRAY[1..4] OF BOOL;
END_VAR
VAR PUBLIC RETAIN
    (* The member to start first when available; 0 chooses by fewest run hours. *)
    Preferred : INT := 0;
    StartDelay : TIME := T#2s;
    StopDelay : TIME := T#2s;
    ShortfallDelay : TIME := T#10s;
END_VAR
VAR NON_RETAIN
    Members : ARRAY[1..4] OF IDutyMember;
    Changed : BOOL;
    SinceChange : TON;
    ShortfallTimer : TON;
    ShortfallAlarm : Alarm.DigitalAlarm;
END_VAR

(* One scan: Wanted members should run. *)
METHOD PROTECTED Coordinate
VAR_INPUT
    Wanted : INT;
END_VAR
VAR
    Index : INT;
    Count : INT;
    Active : INT;
    Running : INT;
    Available : INT;
    Pick : INT;
    Hours : LREAL;
    Best : LREAL;
END_VAR
    Members[1] := Member1;
    Members[2] := Member2;
    Members[3] := Member3;
    Members[4] := Member4;
    Count := 0;
    Active := 0;
    Running := 0;
    Available := 0;
    FOR Index := 1 TO 4 DO
        IF Members[Index] = NULL THEN
            Selected[Index] := FALSE;
        ELSE
            Count := Count + 1;
            (* A member that becomes unavailable is replaced. *)
            IF NOT Members[Index].IsAvailable() THEN
                Selected[Index] := FALSE;
            ELSE
                Available := Available + 1;
            END_IF;
            IF Members[Index].IsRunning() THEN
                Running := Running + 1;
            END_IF;
            IF Selected[Index] THEN
                Active := Active + 1;
            END_IF;
        END_IF;
    END_FOR;
    Target := LIMIT(0, Wanted, Count);

    (* One change at a time, StartDelay or StopDelay apart. *)
    SinceChange(IN := Changed, PT := T#24h);
    IF Active < Target AND (NOT Changed OR SinceChange.ET >= StartDelay) THEN
        Pick := 0;
        IF Preferred >= 1 AND Preferred <= 4 THEN
            IF Members[Preferred] <> NULL AND NOT Selected[Preferred] THEN
                IF Members[Preferred].IsAvailable() THEN
                    Pick := Preferred;
                END_IF;
            END_IF;
        END_IF;
        IF Pick = 0 THEN
            FOR Index := 1 TO 4 DO
                IF Members[Index] <> NULL AND NOT Selected[Index] THEN
                    IF Members[Index].IsAvailable() THEN
                        Hours := Members[Index].GetRunHours();
                        IF Pick = 0 OR Hours < Best THEN
                            Pick := Index;
                            Best := Hours;
                        END_IF;
                    END_IF;
                END_IF;
            END_FOR;
        END_IF;
        IF Pick > 0 THEN
            Selected[Pick] := TRUE;
            Changed := TRUE;
            SinceChange(IN := FALSE, PT := T#24h);
        END_IF;
    ELSIF Active > Target AND (NOT Changed OR SinceChange.ET >= StopDelay) THEN
        (* Stop the member that has run longest. *)
        Pick := 0;
        FOR Index := 1 TO 4 DO
            IF Selected[Index] THEN
                Hours := Members[Index].GetRunHours();
                IF Pick = 0 OR Hours > Best THEN
                    Pick := Index;
                    Best := Hours;
                END_IF;
            END_IF;
        END_FOR;
        IF Pick > 0 THEN
            Selected[Pick] := FALSE;
            Changed := TRUE;
            SinceChange(IN := FALSE, PT := T#24h);
        END_IF;
    END_IF;
    FOR Index := 1 TO 4 DO
        IF Members[Index] <> NULL THEN
            Members[Index].Demand(Run := Selected[Index]);
        END_IF;
    END_FOR;
    RunningCount := Running;
    AvailableCount := Available;
    ShortfallTimer(IN := Running < Target, PT := ShortfallDelay);
    Shortfall := ShortfallTimer.Q;
    ShortfallAlarm(Condition := Shortfall, Message := 'Not enough equipment available',
        Priority := AlarmPriority#High);
END_METHOD

METHOD PUBLIC GetAlarmCount : INT
    GetAlarmCount := 1;
END_METHOD

METHOD PUBLIC GetAlarm : REF_TO Alarm.Alarm
VAR_INPUT
    Index : INT;
END_VAR
    IF Index = 1 THEN
        GetAlarm := REF(ShortfallAlarm);
    ELSE
        GetAlarm := NULL;
    END_IF;
END_METHOD
END_FUNCTION_BLOCK

(* One member runs while Run is TRUE; a failed member is replaced by an available one. *)
FUNCTION_BLOCK FB_DutyStandbyGroup EXTENDS FB_Group
VAR_INPUT
    Run : BOOL;
END_VAR
    IF Run THEN
        Coordinate(Wanted := 1);
    ELSE
        Coordinate(Wanted := 0);
    END_IF;
END_FUNCTION_BLOCK

(* Required members run; the program decides how many. *)
FUNCTION_BLOCK FB_LeadLagGroup EXTENDS FB_Group
VAR_INPUT
    Required : INT;
END_VAR
    Coordinate(Wanted := Required);
END_FUNCTION_BLOCK

(* Demand in % of the group's equal stages, with hysteresis and minimum run and stop times. *)
FUNCTION_BLOCK FB_StagedGroup EXTENDS FB_Group
VAR_INPUT
    Demand : LREAL;
END_VAR
VAR PUBLIC RETAIN
    Hysteresis : LREAL := 5.0;
    MinRunTime : TIME := T#0s;
    MinStopTime : TIME := T#0s;
END_VAR
VAR NON_RETAIN
    Stages : INT;
    SinceStage : TON;
    Staged : BOOL;
END_VAR
VAR_TEMP
    Size : INT;
    StageSize : LREAL;
    Next : INT;
END_VAR
    Size := 0;
    IF Member1 <> NULL THEN Size := Size + 1; END_IF;
    IF Member2 <> NULL THEN Size := Size + 1; END_IF;
    IF Member3 <> NULL THEN Size := Size + 1; END_IF;
    IF Member4 <> NULL THEN Size := Size + 1; END_IF;
    Next := Stages;
    IF Size = 0 OR NOT F_Finite(Demand) OR Demand <= 0.0 THEN
        Next := 0;
    ELSE
        StageSize := 100.0 / INT_TO_LREAL(Size);
        WHILE Next < Size AND Demand > INT_TO_LREAL(Next) * StageSize DO
            Next := Next + 1;
        END_WHILE;
        WHILE Next > 1 AND Demand < INT_TO_LREAL(Next - 1) * StageSize - Hysteresis DO
            Next := Next - 1;
        END_WHILE;
    END_IF;
    SinceStage(IN := Staged, PT := T#24h);
    IF Next > Stages AND (NOT Staged OR SinceStage.ET >= MinStopTime) THEN
        Stages := Stages + 1;
        Staged := TRUE;
        SinceStage(IN := FALSE, PT := T#24h);
    ELSIF Next < Stages AND (NOT Staged OR SinceStage.ET >= MinRunTime) THEN
        Stages := Stages - 1;
        Staged := TRUE;
        SinceStage(IN := FALSE, PT := T#24h);
    END_IF;
    Coordinate(Wanted := Stages);
END_FUNCTION_BLOCK

END_NAMESPACE
