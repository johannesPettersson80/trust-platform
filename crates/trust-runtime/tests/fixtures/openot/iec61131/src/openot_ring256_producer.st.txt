TYPE
    OPENOT_RING256_BUFFER : ARRAY[0..255] OF BYTE;

    OPENOT_RING256_SPAN : STRUCT
        StartAbs : ULINT;
        EndAbs : ULINT;
        RunId : ULINT;
        SourceId : UDINT;
        Seq : ULINT;
    END_STRUCT;

    OPENOT_PRODUCER_LOSS_RANGE : STRUCT
        RunId : ULINT;
        SourceId : UDINT;
        FirstSeq : ULINT;
        LastSeq : ULINT;
    END_STRUCT;

    OPENOT_PRODUCER_LOSS_RANGES : ARRAY[0..15] OF OPENOT_PRODUCER_LOSS_RANGE;
END_TYPE

FUNCTION_BLOCK OPENOT_Ring256Producer
VAR_INPUT
    Execute : BOOL;
    Record : OPENOT_BYTE_BUFFER;
    Length : UINT;
    RunId : ULINT;
    SourceId : UDINT;
    Seq : ULINT;
END_VAR
VAR_OUTPUT
    Ring : OPENOT_RING256_BUFFER;
    HeadAbs : ULINT;
    OldestAbs : ULINT;
    LostCount : ULINT;
    LastStartAbs : ULINT;
    LastEndAbs : ULINT;
    LossRanges : OPENOT_PRODUCER_LOSS_RANGES;
    LossRangeCount : UINT;
    LossRangeOverflow : BOOL;
    Done : BOOL;
    Error : BOOL;
END_VAR
VAR
    Spans : ARRAY[0..5] OF OPENOT_RING256_SPAN;
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
    LastLossIndex : UINT;
    ContinueEvict : BOOL;
    CanExtendLoss : BOOL;
END_VAR

Done := FALSE;
Error := LossRangeOverflow;

IF Execute THEN
    IF (Length = UINT#0) OR (Length > UINT#256) THEN
        Error := TRUE;
    ELSE
        StartPhysAbs := HeadAbs MOD ULINT#256;
        StartPhys := ULINT_TO_UINT(StartPhysAbs);

        IF (StartPhys = UINT#0) OR ((StartPhys + Length) <= UINT#256) THEN
            RecordStartAbs := HeadAbs;
        ELSE
            RecordStartAbs := HeadAbs + (ULINT#256 - StartPhysAbs);
        END_IF;

        FinalHead := RecordStartAbs + UINT_TO_ULINT(Length);

        IF FinalHead > ULINT#256 THEN
            RetentionFloor := FinalHead - ULINT#256;
        ELSE
            RetentionFloor := ULINT#0;
        END_IF;

        ContinueEvict := SpanCount > UINT#0;
        WHILE ContinueEvict DO
            IF Spans[SpanHead].StartAbs < RetentionFloor THEN
                LostCount := LostCount + ULINT#1;

                CanExtendLoss := FALSE;
                IF LossRangeCount > UINT#0 THEN
                    LastLossIndex := LossRangeCount - UINT#1;
                    CanExtendLoss :=
                        (LossRanges[LastLossIndex].RunId = Spans[SpanHead].RunId) AND
                        (LossRanges[LastLossIndex].SourceId = Spans[SpanHead].SourceId) AND
                        ((LossRanges[LastLossIndex].LastSeq + ULINT#1) = Spans[SpanHead].Seq);
                END_IF;

                IF CanExtendLoss THEN
                    LossRanges[LastLossIndex].LastSeq := Spans[SpanHead].Seq;
                ELSE
                    IF LossRangeCount >= UINT#16 THEN
                        LossRangeOverflow := TRUE;
                        Error := TRUE;
                    ELSE
                        LossRanges[LossRangeCount].RunId := Spans[SpanHead].RunId;
                        LossRanges[LossRangeCount].SourceId := Spans[SpanHead].SourceId;
                        LossRanges[LossRangeCount].FirstSeq := Spans[SpanHead].Seq;
                        LossRanges[LossRangeCount].LastSeq := Spans[SpanHead].Seq;
                        LossRangeCount := LossRangeCount + UINT#1;
                    END_IF;
                END_IF;

                IF SpanHead = UINT#5 THEN
                    SpanHead := UINT#0;
                ELSE
                    SpanHead := SpanHead + UINT#1;
                END_IF;
                SpanCount := SpanCount - UINT#1;
                ContinueEvict := (SpanCount > UINT#0) AND (NOT LossRangeOverflow);
            ELSE
                ContinueEvict := FALSE;
            END_IF;
        END_WHILE;

        IF SpanCount > UINT#0 THEN
            OldestAbs := Spans[SpanHead].StartAbs;
        ELSE
            OldestAbs := RecordStartAbs;
        END_IF;

        IF SpanCount >= UINT#6 THEN
            Error := TRUE;
        END_IF;

        IF NOT Error THEN
            IF RecordStartAbs <> HeadAbs THEN
                Ring[StartPhys] := BYTE#16#00;
            END_IF;

            DestPhys := ULINT_TO_UINT(RecordStartAbs MOD ULINT#256);
            FOR CopyIndex := UINT#0 TO Length - UINT#1 DO
                RingPos := DestPhys + CopyIndex;
                Ring[RingPos] := Record[CopyIndex];
            END_FOR;

            Spans[SpanTail].StartAbs := RecordStartAbs;
            Spans[SpanTail].EndAbs := FinalHead;
            Spans[SpanTail].RunId := RunId;
            Spans[SpanTail].SourceId := SourceId;
            Spans[SpanTail].Seq := Seq;

            IF SpanTail = UINT#5 THEN
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
