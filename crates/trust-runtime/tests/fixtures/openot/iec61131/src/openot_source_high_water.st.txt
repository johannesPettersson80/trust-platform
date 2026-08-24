FUNCTION_BLOCK OPENOT_EncodeSourceHighWater
VAR_INPUT
    SourceId : UDINT;
    RunId : ULINT;
    ProducedCount : ULINT;
    SourceTime : ULINT;
END_VAR
VAR_OUTPUT
    Buffer : OPENOT_BYTE_BUFFER;
    Length : UINT;
END_VAR
VAR
    Pos : UINT;
    Crc : DWORD;
END_VAR

Pos := OPENOT_WriteHeader(
    TotalLength := UINT#56,
    Flags := UINT#16#0008,
    SourceTime := SourceTime,
    RunId := RunId,
    Seq := ProducedCount,
    SourceId := SourceId,
        EventTypeId := UDINT#16#0108,
    Buffer := Buffer);

Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#0038, TypeTag := BYTE#16#07, PayloadLength := BYTE#8, Buffer := Buffer);
Pos := OPENOT_WriteU64LE(Offset := Pos, Value := ProducedCount, Buffer := Buffer);
Crc := OPENOT_Crc32c(Data := Buffer, Length := Pos);
Pos := OPENOT_WriteDWordLE(Offset := Pos, Value := Crc, Buffer := Buffer);
Length := Pos;
END_FUNCTION_BLOCK
