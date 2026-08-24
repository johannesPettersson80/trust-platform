FUNCTION OPENOT_TestScanRecordCrcValid : BOOL
VAR_INPUT
    Records : OPENOT_SCAN_RECORD_BUFFER;
    Offset : UINT;
    Length : UINT;
END_VAR
VAR
    Work : OPENOT_BYTE_BUFFER;
    Index : UINT;
    CrcLength : UINT;
    ActualCrc : DWORD;
    ExpectedCrc : DWORD;
END_VAR

IF (Length < UINT#44) OR (Length > UINT#256) THEN
    OPENOT_TestScanRecordCrcValid := FALSE;
ELSE
    FOR Index := UINT#0 TO Length - UINT#1 DO
        Work[Index] := Records[Offset + Index];
    END_FOR;

    CrcLength := Length - UINT#4;
    ActualCrc := OPENOT_Crc32c(Data := Work, Length := CrcLength);
    ExpectedCrc :=
        BYTE_TO_DWORD(Work[Length - UINT#4]) OR
        SHL(BYTE_TO_DWORD(Work[Length - UINT#3]), UINT#8) OR
        SHL(BYTE_TO_DWORD(Work[Length - UINT#2]), UINT#16) OR
        SHL(BYTE_TO_DWORD(Work[Length - UINT#1]), UINT#24);
    OPENOT_TestScanRecordCrcValid := ActualCrc = ExpectedCrc;
END_IF;
END_FUNCTION

FUNCTION OPENOT_TestScanRecordHeaderOk : BOOL
VAR_INPUT
    Records : OPENOT_SCAN_RECORD_BUFFER;
    Offset : UINT;
    Length : UINT;
    EventTypeId : UDINT;
    SourceId : UDINT;
END_VAR
VAR
    HeaderOk : BOOL;
    SourceRemaining : UDINT;
    EventRemaining : UDINT;
    Index : UINT;
    Pos : UINT;
END_VAR

HeaderOk :=
    (Length >= UINT#44) AND
    (Records[Offset] = BYTE#16#4F) AND
    (Records[Offset + UINT#1] = BYTE#16#4F) AND
    (Records[Offset + UINT#2] = BYTE#16#54) AND
    (Records[Offset + UINT#3] = BYTE#16#32) AND
    (Records[Offset + UINT#4] = UINT_TO_BYTE(Length MOD UINT#256)) AND
    (Records[Offset + UINT#5] = UINT_TO_BYTE((Length / UINT#256) MOD UINT#256)) AND
    (Records[Offset + UINT#6] = BYTE#16#08) AND
    (Records[Offset + UINT#7] = BYTE#16#00);

SourceRemaining := SourceId;
FOR Index := UINT#0 TO UINT#3 DO
    Pos := Offset + UINT#32 + Index;
    HeaderOk := HeaderOk AND (Records[Pos] = UDINT_TO_BYTE(SourceRemaining MOD UDINT#256));
    SourceRemaining := SourceRemaining / UDINT#256;
END_FOR;

EventRemaining := EventTypeId;
FOR Index := UINT#0 TO UINT#3 DO
    Pos := Offset + UINT#36 + Index;
    HeaderOk := HeaderOk AND (Records[Pos] = UDINT_TO_BYTE(EventRemaining MOD UDINT#256));
    EventRemaining := EventRemaining / UDINT#256;
END_FOR;

OPENOT_TestScanRecordHeaderOk := HeaderOk;
END_FUNCTION

FUNCTION_BLOCK OPENOT_TestScanRecordsBurst
VAR_OUTPUT
    Passed : BOOL;
    SingleRecordOpsOk : BOOL;
    BurstCrcOk : BOOL;
    BurstOrderOk : BOOL;
    BurstContiguousOk : BOOL;
    BurstScanRecordCount : UINT;
    BurstPublishedDelta : ULINT;
    BurstBytes : UINT;
    DataScanRecordCount : UINT;
    DefinitionChangedScanRecordCount : UINT;
    LoggerStartedScanRecordCount : UINT;
    CheckpointScanRecordCount : UINT;
    BurstNoEdgeScanRecordCount : UINT;
    SingleNoEdgeScanRecordCount : UINT;
END_VAR
VAR
    Producer : OPENOT_Producer;
    DefHashOld : OPENOT_HASH8 := [
        BYTE#16#6F, BYTE#16#6C, BYTE#16#64, BYTE#16#68,
        BYTE#16#61, BYTE#16#73, BYTE#16#68, BYTE#16#31
    ];
    DefHashNew : OPENOT_HASH8 := [
        BYTE#16#6E, BYTE#16#65, BYTE#16#77, BYTE#16#68,
        BYTE#16#61, BYTE#16#73, BYTE#16#68, BYTE#16#32
    ];
    Ran : BOOL;
    OpsOk : BOOL;
    CountBefore : ULINT;
    Delta : ULINT;
    Offset0 : UINT;
    Offset1 : UINT;
    Offset2 : UINT;
    Length0 : UINT;
    Length1 : UINT;
    Length2 : UINT;
END_VAR

IF NOT Ran THEN
    OpsOk := TRUE;
    SingleRecordOpsOk := TRUE;

    Producer(Execute := TRUE, Op := UINT#0, SourceId := UDINT#30);
    CountBefore := ULINT#0;
    DataScanRecordCount := Producer.ScanRecordCount;
    SingleRecordOpsOk :=
        SingleRecordOpsOk AND
        (Producer.ScanRecordCount = UINT#1) AND
        (Producer.PublishedRecordCount - CountBefore = ULINT#1);
    OpsOk := OpsOk AND Producer.Done AND (NOT Producer.Error) AND (Producer.AssignedSeq = ULINT#0);
    Producer(Execute := FALSE, Op := UINT#0, SourceId := UDINT#30);

    CountBefore := Producer.PublishedRecordCount;
    Producer(Execute := TRUE, Op := UINT#0, SourceId := UDINT#10);
    SingleRecordOpsOk :=
        SingleRecordOpsOk AND
        (Producer.ScanRecordCount = UINT#1) AND
        (Producer.PublishedRecordCount - CountBefore = ULINT#1);
    OpsOk := OpsOk AND Producer.Done AND (NOT Producer.Error) AND (Producer.AssignedSeq = ULINT#0);
    Producer(Execute := FALSE, Op := UINT#0, SourceId := UDINT#10);

    CountBefore := Producer.PublishedRecordCount;
    Producer(
        Execute := TRUE,
        Op := UINT#1,
        DefHashOld := DefHashOld,
        DefHashNew := DefHashNew,
        TransitionEpochId := ULINT#2);
    BurstScanRecordCount := Producer.ScanRecordCount;
    BurstPublishedDelta := Producer.PublishedRecordCount - CountBefore;
    Offset0 := Producer.ScanRecordOffsets[0];
    Offset1 := Producer.ScanRecordOffsets[1];
    Offset2 := Producer.ScanRecordOffsets[2];
    Length0 := Producer.ScanRecordLengths[0];
    Length1 := Producer.ScanRecordLengths[1];
    Length2 := Producer.ScanRecordLengths[2];
    BurstBytes := Length0 + Length1 + Length2;
    BurstContiguousOk :=
        (BurstScanRecordCount = UINT#3) AND
        (BurstPublishedDelta = ULINT#3) AND
        (Offset0 = UINT#0) AND
        (Length0 = UINT#44) AND
        (Offset1 = Offset0 + Length0) AND
        (Length1 = UINT#56) AND
        (Offset2 = Offset1 + Length1) AND
        (Length2 = UINT#56) AND
        (BurstBytes = UINT#156);
    BurstCrcOk :=
        OPENOT_TestScanRecordCrcValid(
            Records := Producer.ScanRecords,
            Offset := Offset0,
            Length := Length0) AND
        OPENOT_TestScanRecordCrcValid(
            Records := Producer.ScanRecords,
            Offset := Offset1,
            Length := Length1) AND
        OPENOT_TestScanRecordCrcValid(
            Records := Producer.ScanRecords,
            Offset := Offset2,
            Length := Length2);
    BurstOrderOk :=
        OPENOT_TestScanRecordHeaderOk(
            Records := Producer.ScanRecords,
            Offset := Offset0,
            Length := Length0,
            EventTypeId := UDINT#16#0102,
            SourceId := UDINT#0) AND
        OPENOT_TestScanRecordHeaderOk(
            Records := Producer.ScanRecords,
            Offset := Offset1,
            Length := Length1,
            EventTypeId := UDINT#16#0108,
            SourceId := UDINT#10) AND
        OPENOT_TestScanRecordHeaderOk(
            Records := Producer.ScanRecords,
            Offset := Offset2,
            Length := Length2,
            EventTypeId := UDINT#16#0108,
            SourceId := UDINT#30);
    OpsOk := OpsOk AND Producer.Done AND (NOT Producer.Error) AND Producer.TransitionOpen;
    Producer(Execute := FALSE, Op := UINT#1);
    BurstNoEdgeScanRecordCount := Producer.ScanRecordCount;

    CountBefore := Producer.PublishedRecordCount;
    Producer(Execute := TRUE, Op := UINT#2);
    Delta := Producer.PublishedRecordCount - CountBefore;
    DefinitionChangedScanRecordCount := Producer.ScanRecordCount;
    SingleRecordOpsOk :=
        SingleRecordOpsOk AND
        (Producer.ScanRecordCount = UINT#1) AND
        (Delta = ULINT#1);
    OpsOk := OpsOk AND Producer.Done AND (NOT Producer.Error) AND Producer.DefinitionChangedEmitted;
    Producer(Execute := FALSE, Op := UINT#2);

    CountBefore := Producer.PublishedRecordCount;
    Producer(Execute := TRUE, Op := UINT#3);
    Delta := Producer.PublishedRecordCount - CountBefore;
    LoggerStartedScanRecordCount := Producer.ScanRecordCount;
    SingleRecordOpsOk :=
        SingleRecordOpsOk AND
        (Producer.ScanRecordCount = UINT#1) AND
        (Delta = ULINT#1);
    OpsOk :=
        OpsOk AND
        Producer.Done AND
        (NOT Producer.Error) AND
        (NOT Producer.TransitionOpen);
    Producer(Execute := FALSE, Op := UINT#3);

    CountBefore := Producer.PublishedRecordCount;
    Producer(Execute := TRUE, Checkpoint := TRUE, SourceId := UDINT#30);
    Delta := Producer.PublishedRecordCount - CountBefore;
    CheckpointScanRecordCount := Producer.ScanRecordCount;
    SingleRecordOpsOk :=
        SingleRecordOpsOk AND
        (Producer.ScanRecordCount = UINT#1) AND
        (Delta = ULINT#1);
    OpsOk := OpsOk AND Producer.Done AND Producer.CheckpointDone AND (NOT Producer.Error);
    Producer(Execute := FALSE, Checkpoint := TRUE, SourceId := UDINT#30);
    SingleNoEdgeScanRecordCount := Producer.ScanRecordCount;

    Ran := TRUE;
END_IF;

Passed :=
    OpsOk AND
    SingleRecordOpsOk AND
    BurstCrcOk AND
    BurstOrderOk AND
    BurstContiguousOk AND
    (BurstNoEdgeScanRecordCount = UINT#3) AND
    (SingleNoEdgeScanRecordCount = UINT#1) AND
    (Producer.PublishedRecordCount = ULINT#8) AND
    (Producer.EmissionCount = UINT#8);
END_FUNCTION_BLOCK
