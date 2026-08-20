FUNCTION OPENOT_AuthoringHeaderOk : BOOL
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

OPENOT_AuthoringHeaderOk := HeaderOk;
END_FUNCTION

FUNCTION_BLOCK OPENOT_TestAuthoringApi
VAR_OUTPUT
    Passed : BOOL;
    FirstBatchCount : UINT;
    FirstBatchDelta : ULINT;
    NoChangeCount : UINT;
    NoChangeDelta : ULINT;
    HeadersOk : BOOL;
    EmissionsOk : BOOL;
    ChangeSuppressionOk : BOOL;
END_VAR
VAR
    Producer : OPENOT_Producer;
    BeforeCount : ULINT;
    Offset0 : UINT;
    Offset1 : UINT;
    Offset2 : UINT;
    Offset3 : UINT;
    Length0 : UINT;
    Length1 : UINT;
    Length2 : UINT;
    Length3 : UINT;
END_VAR

Producer(Execute := FALSE, ResetScanRecords := TRUE);
Producer(Execute := FALSE, ResetScanRecords := FALSE);
BeforeCount := Producer.PublishedRecordCount;
Producer(
    Execute := TRUE,
    Op := UINT#6,
    ValueId := UDINT#2001,
    ValueReal := REAL#12.5,
    DeadbandReal := REAL#0.5,
    HasQuality := FALSE,
    Quality := UINT#0);
Producer(Execute := FALSE, Op := UINT#6);
Producer(
    Execute := TRUE,
    Op := UINT#7,
    ValueId := UDINT#2002,
    ValueInt := DINT#42,
    HasQuality := FALSE,
    Quality := UINT#0);
Producer(Execute := FALSE, Op := UINT#7);
Producer(
    Execute := TRUE,
    Op := UINT#10,
    ValueId := UDINT#2003,
    ValueTypeTag := BYTE#16#00,
    ValuePayloadLength := UINT#1,
    ValueBits := ULINT#1,
    HasQuality := FALSE,
    Quality := UINT#0);
Producer(Execute := FALSE, Op := UINT#10);
Producer(
    Execute := TRUE,
    Op := UINT#8,
    StateMachineId := UDINT#7,
    Category := UINT#2,
    PreviousState := UINT#0,
    NewState := UINT#1);
Producer(Execute := FALSE, Op := UINT#8);

FirstBatchCount := Producer.ScanRecordCount;
FirstBatchDelta := Producer.PublishedRecordCount - BeforeCount;
Offset0 := Producer.ScanRecordOffsets[0];
Offset1 := Producer.ScanRecordOffsets[1];
Offset2 := Producer.ScanRecordOffsets[2];
Offset3 := Producer.ScanRecordOffsets[3];
Length0 := Producer.ScanRecordLengths[0];
Length1 := Producer.ScanRecordLengths[1];
Length2 := Producer.ScanRecordLengths[2];
Length3 := Producer.ScanRecordLengths[3];
HeadersOk :=
    OPENOT_AuthoringHeaderOk(
        Records := Producer.ScanRecords,
        Offset := Offset0,
        Length := Length0,
        EventTypeId := UDINT#16#0002,
        SourceId := UDINT#1) AND
    OPENOT_AuthoringHeaderOk(
        Records := Producer.ScanRecords,
        Offset := Offset1,
        Length := Length1,
        EventTypeId := UDINT#16#0002,
        SourceId := UDINT#1) AND
    OPENOT_AuthoringHeaderOk(
        Records := Producer.ScanRecords,
        Offset := Offset2,
        Length := Length2,
        EventTypeId := UDINT#16#0002,
        SourceId := UDINT#1) AND
    OPENOT_AuthoringHeaderOk(
        Records := Producer.ScanRecords,
        Offset := Offset3,
        Length := Length3,
        EventTypeId := UDINT#16#0001,
        SourceId := UDINT#1);

EmissionsOk :=
    (Producer.EmissionCount = UINT#4) AND
    (Producer.Emissions[0].EventTypeId = UDINT#16#0002) AND
    (Producer.Emissions[0].Seq = ULINT#0) AND
    (Producer.Emissions[1].EventTypeId = UDINT#16#0002) AND
    (Producer.Emissions[1].Seq = ULINT#1) AND
    (Producer.Emissions[2].EventTypeId = UDINT#16#0002) AND
    (Producer.Emissions[2].Seq = ULINT#2) AND
    (Producer.Emissions[3].EventTypeId = UDINT#16#0001) AND
    (Producer.Emissions[3].Seq = ULINT#3);

Producer(Execute := FALSE, ResetScanRecords := TRUE);
Producer(Execute := FALSE, ResetScanRecords := FALSE);
BeforeCount := Producer.PublishedRecordCount;
Producer(
    Execute := TRUE,
    Op := UINT#6,
    ValueId := UDINT#2001,
    ValueReal := REAL#12.5,
    DeadbandReal := REAL#0.5,
    HasQuality := FALSE,
    Quality := UINT#0);
Producer(Execute := FALSE, Op := UINT#6);
Producer(
    Execute := TRUE,
    Op := UINT#7,
    ValueId := UDINT#2002,
    ValueInt := DINT#42,
    HasQuality := FALSE,
    Quality := UINT#0);
Producer(Execute := FALSE, Op := UINT#7);
Producer(
    Execute := TRUE,
    Op := UINT#10,
    ValueId := UDINT#2003,
    ValueTypeTag := BYTE#16#00,
    ValuePayloadLength := UINT#1,
    ValueBits := ULINT#1,
    HasQuality := FALSE,
    Quality := UINT#0);
Producer(Execute := FALSE, Op := UINT#10);
Producer(
    Execute := TRUE,
    Op := UINT#8,
    StateMachineId := UDINT#7,
    Category := UINT#2,
    PreviousState := UINT#0,
    NewState := UINT#1);
Producer(Execute := FALSE, Op := UINT#8);

NoChangeCount := Producer.ScanRecordCount;
NoChangeDelta := Producer.PublishedRecordCount - BeforeCount;
ChangeSuppressionOk := (NoChangeCount = UINT#0) AND (NoChangeDelta = ULINT#0);

Passed :=
    (FirstBatchCount = UINT#4) AND
    (FirstBatchDelta = ULINT#4) AND
    HeadersOk AND
    EmissionsOk AND
    ChangeSuppressionOk;
END_FUNCTION_BLOCK
