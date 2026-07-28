FUNCTION_BLOCK OPENOT_EncodeProducerRecordsDropped
VAR_INPUT
    RunId : ULINT;
    SourceId : UDINT;
    FirstSeq : ULINT;
    LastSeq : ULINT;
    Seq : ULINT;
    SourceTime : ULINT;
END_VAR
VAR_OUTPUT
    Buffer : OPENOT_BYTE_BUFFER;
    Length : UINT;
    Done : BOOL;
    Error : BOOL;
END_VAR
VAR
    Pos : UINT;
    Crc : DWORD;
    CountWide : ULINT;
    CountValue : UDINT;
END_VAR

Done := FALSE;
Error := FALSE;
Length := UINT#0;

IF LastSeq < FirstSeq THEN
    Error := TRUE;
ELSE
    CountWide := (LastSeq - FirstSeq) + ULINT#1;
    IF CountWide > ULINT#4294967295 THEN
        Error := TRUE;
    ELSE
        CountValue := ULINT_TO_UDINT(CountWide);
        Pos := OPENOT_WriteHeader(
            TotalLength := UINT#76,
            Flags := UINT#16#0008,
            SourceTime := SourceTime,
            RunId := RunId,
            Seq := Seq,
            SourceId := SourceId,
            EventTypeId := UDINT#16#0104,
            Buffer := Buffer);

        Pos := OPENOT_WriteSlotHeader(
            Offset := Pos,
            Key := UINT#16#0016,
            TypeTag := BYTE#16#05,
            PayloadLength := BYTE#4,
            Buffer := Buffer);
        Pos := OPENOT_WriteU32LE(Offset := Pos, Value := CountValue, Buffer := Buffer);
        Pos := OPENOT_WriteSlotHeader(
            Offset := Pos,
            Key := UINT#16#0017,
            TypeTag := BYTE#16#07,
            PayloadLength := BYTE#8,
            Buffer := Buffer);
        Pos := OPENOT_WriteU64LE(Offset := Pos, Value := FirstSeq, Buffer := Buffer);
        Pos := OPENOT_WriteSlotHeader(
            Offset := Pos,
            Key := UINT#16#0018,
            TypeTag := BYTE#16#07,
            PayloadLength := BYTE#8,
            Buffer := Buffer);
        Pos := OPENOT_WriteU64LE(Offset := Pos, Value := LastSeq, Buffer := Buffer);

        Crc := OPENOT_Crc32c(Data := Buffer, Length := Pos);
        Pos := OPENOT_WriteDWordLE(Offset := Pos, Value := Crc, Buffer := Buffer);
        Length := Pos;
        Done := TRUE;
    END_IF;
END_IF;
END_FUNCTION_BLOCK
