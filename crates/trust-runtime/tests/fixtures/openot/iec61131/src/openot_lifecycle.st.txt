TYPE
    OPENOT_HASH8 : ARRAY[0..7] OF BYTE;
END_TYPE

FUNCTION_BLOCK OPENOT_EncodeLoggerStarted
VAR_INPUT
    RunId : ULINT;
    Seq : ULINT;
    ColdStart : BOOL;
    SourceTime : ULINT;
END_VAR
VAR_OUTPUT
    Buffer : OPENOT_BYTE_BUFFER;
    Length : UINT;
END_VAR
VAR
    Pos : UINT;
    Crc : DWORD;
    ColdStartByte : BYTE;
END_VAR

IF ColdStart THEN
    ColdStartByte := BYTE#16#01;
ELSE
    ColdStartByte := BYTE#16#00;
END_IF;

Pos := OPENOT_WriteHeader(
    TotalLength := UINT#52,
    Flags := UINT#16#0008,
    SourceTime := SourceTime,
    RunId := RunId,
    Seq := Seq,
    SourceId := UDINT#0,
    EventTypeId := UDINT#16#0101,
    Buffer := Buffer);

Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#003B, TypeTag := BYTE#16#00, PayloadLength := BYTE#1, Buffer := Buffer);
Pos := OPENOT_WriteByte(Offset := Pos, Value := ColdStartByte, Buffer := Buffer);
Pos := OPENOT_WriteByte(Offset := Pos, Value := BYTE#16#00, Buffer := Buffer);
Pos := OPENOT_WriteByte(Offset := Pos, Value := BYTE#16#00, Buffer := Buffer);
Pos := OPENOT_WriteByte(Offset := Pos, Value := BYTE#16#00, Buffer := Buffer);
Crc := OPENOT_Crc32c(Data := Buffer, Length := Pos);
Pos := OPENOT_WriteDWordLE(Offset := Pos, Value := Crc, Buffer := Buffer);
Length := Pos;
END_FUNCTION_BLOCK

FUNCTION_BLOCK OPENOT_EncodeLoggerStopped
VAR_INPUT
    RunId : ULINT;
    Seq : ULINT;
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
    TotalLength := UINT#44,
    Flags := UINT#16#0008,
    SourceTime := SourceTime,
    RunId := RunId,
    Seq := Seq,
    SourceId := UDINT#0,
    EventTypeId := UDINT#16#0102,
    Buffer := Buffer);

Crc := OPENOT_Crc32c(Data := Buffer, Length := Pos);
Pos := OPENOT_WriteDWordLE(Offset := Pos, Value := Crc, Buffer := Buffer);
Length := Pos;
END_FUNCTION_BLOCK

FUNCTION_BLOCK OPENOT_EncodeDefinitionChanged
VAR_INPUT
    RunId : ULINT;
    Seq : ULINT;
    DefHashOld : OPENOT_HASH8;
    DefHashNew : OPENOT_HASH8;
    EpochId : ULINT;
    SourceTime : ULINT;
END_VAR
VAR_OUTPUT
    Buffer : OPENOT_BYTE_BUFFER;
    Length : UINT;
END_VAR
VAR
    Pos : UINT;
    Crc : DWORD;
    Index : UINT;
END_VAR

Pos := OPENOT_WriteHeader(
    TotalLength := UINT#80,
    Flags := UINT#16#0008,
    SourceTime := SourceTime,
    RunId := RunId,
    Seq := Seq,
    SourceId := UDINT#0,
    EventTypeId := UDINT#16#0106,
    Buffer := Buffer);

Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#0039, TypeTag := BYTE#16#0D, PayloadLength := BYTE#8, Buffer := Buffer);
FOR Index := UINT#0 TO UINT#7 DO
    Pos := OPENOT_WriteByte(Offset := Pos, Value := DefHashOld[Index], Buffer := Buffer);
END_FOR;
Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#001C, TypeTag := BYTE#16#0D, PayloadLength := BYTE#8, Buffer := Buffer);
FOR Index := UINT#0 TO UINT#7 DO
    Pos := OPENOT_WriteByte(Offset := Pos, Value := DefHashNew[Index], Buffer := Buffer);
END_FOR;
Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#003A, TypeTag := BYTE#16#07, PayloadLength := BYTE#8, Buffer := Buffer);
Pos := OPENOT_WriteU64LE(Offset := Pos, Value := EpochId, Buffer := Buffer);
Crc := OPENOT_Crc32c(Data := Buffer, Length := Pos);
Pos := OPENOT_WriteDWordLE(Offset := Pos, Value := Crc, Buffer := Buffer);
Length := Pos;
END_FUNCTION_BLOCK
