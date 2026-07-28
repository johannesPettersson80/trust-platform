TYPE
    OPENOT_RING128_BUFFER : ARRAY[0..127] OF BYTE;

    OPENOT_RING_SPAN : STRUCT
        StartAbs : ULINT;
        EndAbs : ULINT;
        RunId : ULINT;
        SourceId : UDINT;
        Seq : ULINT;
    END_STRUCT;
END_TYPE

FUNCTION_BLOCK OPENOT_Ring128
VAR_INPUT
    Execute : BOOL;
    Record : OPENOT_BYTE_BUFFER;
    Length : UINT;
    RunId : ULINT;
    SourceId : UDINT;
    Seq : ULINT;
END_VAR
VAR_OUTPUT
    Ring : OPENOT_RING128_BUFFER;
    HeadAbs : ULINT;
    OldestAbs : ULINT;
    LostCount : ULINT;
    LastStartAbs : ULINT;
    LastEndAbs : ULINT;
    Done : BOOL;
    Error : BOOL;
END_VAR
VAR
    Spans : ARRAY[0..3] OF OPENOT_RING_SPAN;
    SpanHead : UINT;
    SpanTail : UINT;
    SpanCount : UINT;
    StartPhysAbs : ULINT;
    StartPhys : UINT;
    RecordStartAbs : ULINT;
    FinalHead : ULINT;
    RetentionFloor : ULINT;
    DestPhys : UINT;
    RingPos : UINT;
    CopyIndex : UINT;
    ContinueEvict : BOOL;
END_VAR

Done := FALSE;
Error := FALSE;

IF Execute THEN
    IF (Length = UINT#0) OR (Length > UINT#128) THEN
        Error := TRUE;
    ELSE
        StartPhysAbs := HeadAbs MOD ULINT#128;
        StartPhys := ULINT_TO_UINT(StartPhysAbs);

        IF (StartPhys = UINT#0) OR ((StartPhys + Length) <= UINT#128) THEN
            RecordStartAbs := HeadAbs;
        ELSE
            RecordStartAbs := HeadAbs + (ULINT#128 - StartPhysAbs);
        END_IF;

        FinalHead := RecordStartAbs + UINT_TO_ULINT(Length);

        IF FinalHead > ULINT#128 THEN
            RetentionFloor := FinalHead - ULINT#128;
        ELSE
            RetentionFloor := ULINT#0;
        END_IF;

        ContinueEvict := SpanCount > UINT#0;
        WHILE ContinueEvict DO
            IF Spans[SpanHead].StartAbs < RetentionFloor THEN
                LostCount := LostCount + ULINT#1;
                IF SpanHead = UINT#3 THEN
                    SpanHead := UINT#0;
                ELSE
                    SpanHead := SpanHead + UINT#1;
                END_IF;
                SpanCount := SpanCount - UINT#1;
                ContinueEvict := SpanCount > UINT#0;
            ELSE
                ContinueEvict := FALSE;
            END_IF;
        END_WHILE;

        IF SpanCount > UINT#0 THEN
            OldestAbs := Spans[SpanHead].StartAbs;
        ELSE
            OldestAbs := RecordStartAbs;
        END_IF;

        IF SpanCount >= UINT#4 THEN
            Error := TRUE;
        ELSE
            IF RecordStartAbs <> HeadAbs THEN
                Ring[StartPhys] := BYTE#16#00;
            END_IF;

            DestPhys := ULINT_TO_UINT(RecordStartAbs MOD ULINT#128);
            FOR CopyIndex := UINT#0 TO Length - UINT#1 DO
                RingPos := DestPhys + CopyIndex;
                Ring[RingPos] := Record[CopyIndex];
            END_FOR;

            Spans[SpanTail].StartAbs := RecordStartAbs;
            Spans[SpanTail].EndAbs := FinalHead;
            Spans[SpanTail].RunId := RunId;
            Spans[SpanTail].SourceId := SourceId;
            Spans[SpanTail].Seq := Seq;

            IF SpanTail = UINT#3 THEN
                SpanTail := UINT#0;
            ELSE
                SpanTail := SpanTail + UINT#1;
            END_IF;
            SpanCount := SpanCount + UINT#1;

            HeadAbs := FinalHead;
            LastStartAbs := RecordStartAbs;
            LastEndAbs := FinalHead;
            Done := TRUE;
        END_IF;
    END_IF;
END_IF;
END_FUNCTION_BLOCK
