FUNCTION OPENOT_WriteByte : UINT
VAR_INPUT
    Offset : UINT;
    Value : BYTE;
END_VAR
VAR_IN_OUT
    Buffer : OPENOT_BYTE_BUFFER;
END_VAR

Buffer[Offset] := Value;
OPENOT_WriteByte := Offset + UINT#1;
END_FUNCTION

FUNCTION OPENOT_WriteU16LE : UINT
VAR_INPUT
    Offset : UINT;
    Value : UINT;
END_VAR
VAR_IN_OUT
    Buffer : OPENOT_BYTE_BUFFER;
END_VAR
VAR
    Remaining : UINT;
    Pos : UINT;
END_VAR

Remaining := Value;
Pos := Offset;
Pos := OPENOT_WriteByte(Offset := Pos, Value := UINT_TO_BYTE(Remaining MOD UINT#256), Buffer := Buffer);
Remaining := Remaining / UINT#256;
Pos := OPENOT_WriteByte(Offset := Pos, Value := UINT_TO_BYTE(Remaining MOD UINT#256), Buffer := Buffer);
OPENOT_WriteU16LE := Pos;
END_FUNCTION

FUNCTION OPENOT_WriteU32LE : UINT
VAR_INPUT
    Offset : UINT;
    Value : UDINT;
END_VAR
VAR_IN_OUT
    Buffer : OPENOT_BYTE_BUFFER;
END_VAR
VAR
    Remaining : UDINT;
    Index : UINT;
    Pos : UINT;
END_VAR

Remaining := Value;
Pos := Offset;
FOR Index := UINT#0 TO UINT#3 DO
    Pos := OPENOT_WriteByte(Offset := Pos, Value := UDINT_TO_BYTE(Remaining MOD UDINT#256), Buffer := Buffer);
    Remaining := Remaining / UDINT#256;
END_FOR;
OPENOT_WriteU32LE := Pos;
END_FUNCTION

FUNCTION OPENOT_WriteDWordLE : UINT
VAR_INPUT
    Offset : UINT;
    Value : DWORD;
END_VAR
VAR_IN_OUT
    Buffer : OPENOT_BYTE_BUFFER;
END_VAR
VAR
    Remaining : DWORD;
    Index : UINT;
    Pos : UINT;
END_VAR

Remaining := Value;
Pos := Offset;
FOR Index := UINT#0 TO UINT#3 DO
    Pos := OPENOT_WriteByte(
        Offset := Pos,
        Value := DWORD_TO_BYTE(Remaining AND DWORD#16#000000FF),
        Buffer := Buffer);
    Remaining := SHR(Remaining, UINT#8);
END_FOR;
OPENOT_WriteDWordLE := Pos;
END_FUNCTION

FUNCTION OPENOT_WriteU64LE : UINT
VAR_INPUT
    Offset : UINT;
    Value : ULINT;
END_VAR
VAR_IN_OUT
    Buffer : OPENOT_BYTE_BUFFER;
END_VAR
VAR
    Remaining : ULINT;
    Index : UINT;
    Pos : UINT;
END_VAR

Remaining := Value;
Pos := Offset;
FOR Index := UINT#0 TO UINT#7 DO
    Pos := OPENOT_WriteByte(Offset := Pos, Value := ULINT_TO_BYTE(Remaining MOD ULINT#256), Buffer := Buffer);
    Remaining := Remaining / ULINT#256;
END_FOR;
OPENOT_WriteU64LE := Pos;
END_FUNCTION

FUNCTION OPENOT_WriteHeader : UINT
VAR_INPUT
    TotalLength : UINT;
    Flags : UINT;
    SourceTime : ULINT;
    RunId : ULINT;
    Seq : ULINT;
    SourceId : UDINT;
    EventTypeId : UDINT;
END_VAR
VAR_IN_OUT
    Buffer : OPENOT_BYTE_BUFFER;
END_VAR
VAR
    Pos : UINT;
END_VAR

Pos := UINT#0;
Pos := OPENOT_WriteByte(Offset := Pos, Value := BYTE#16#4F, Buffer := Buffer);
Pos := OPENOT_WriteByte(Offset := Pos, Value := BYTE#16#4F, Buffer := Buffer);
Pos := OPENOT_WriteByte(Offset := Pos, Value := BYTE#16#54, Buffer := Buffer);
Pos := OPENOT_WriteByte(Offset := Pos, Value := BYTE#16#32, Buffer := Buffer);
Pos := OPENOT_WriteU16LE(Offset := Pos, Value := TotalLength, Buffer := Buffer);
Pos := OPENOT_WriteU16LE(Offset := Pos, Value := Flags, Buffer := Buffer);
Pos := OPENOT_WriteU64LE(Offset := Pos, Value := SourceTime, Buffer := Buffer);
Pos := OPENOT_WriteU64LE(Offset := Pos, Value := RunId, Buffer := Buffer);
Pos := OPENOT_WriteU64LE(Offset := Pos, Value := Seq, Buffer := Buffer);
Pos := OPENOT_WriteU32LE(Offset := Pos, Value := SourceId, Buffer := Buffer);
Pos := OPENOT_WriteU32LE(Offset := Pos, Value := EventTypeId, Buffer := Buffer);
OPENOT_WriteHeader := Pos;
END_FUNCTION

FUNCTION OPENOT_WriteSlotHeader : UINT
VAR_INPUT
    Offset : UINT;
    Key : UINT;
    TypeTag : BYTE;
    PayloadLength : BYTE;
END_VAR
VAR_IN_OUT
    Buffer : OPENOT_BYTE_BUFFER;
END_VAR
VAR
    Pos : UINT;
END_VAR

Pos := OPENOT_WriteU16LE(Offset := Offset, Value := Key, Buffer := Buffer);
Pos := OPENOT_WriteByte(Offset := Pos, Value := TypeTag, Buffer := Buffer);
Pos := OPENOT_WriteByte(Offset := Pos, Value := PayloadLength, Buffer := Buffer);
OPENOT_WriteSlotHeader := Pos;
END_FUNCTION

FUNCTION_BLOCK OPENOT_EncodeConformantStateTransition
VAR_OUTPUT
    Buffer : OPENOT_BYTE_BUFFER;
    Length : UINT;
END_VAR
VAR
    Pos : UINT;
    Crc : DWORD;
END_VAR

Pos := OPENOT_WriteHeader(
    TotalLength := UINT#76,
    Flags := UINT#16#0008,
    SourceTime := ULINT#1000000000,
    RunId := ULINT#1,
    Seq := ULINT#1,
    SourceId := UDINT#66,
    EventTypeId := UDINT#16#0001,
    Buffer := Buffer);

Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#0001, TypeTag := BYTE#16#05, PayloadLength := BYTE#4, Buffer := Buffer);
Pos := OPENOT_WriteU32LE(Offset := Pos, Value := UDINT#7, Buffer := Buffer);
Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#0002, TypeTag := BYTE#16#03, PayloadLength := BYTE#2, Buffer := Buffer);
Pos := OPENOT_WriteU16LE(Offset := Pos, Value := UINT#2, Buffer := Buffer);
Pos := OPENOT_WriteByte(Offset := Pos, Value := BYTE#16#00, Buffer := Buffer);
Pos := OPENOT_WriteByte(Offset := Pos, Value := BYTE#16#00, Buffer := Buffer);
Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#0003, TypeTag := BYTE#16#03, PayloadLength := BYTE#2, Buffer := Buffer);
Pos := OPENOT_WriteU16LE(Offset := Pos, Value := UINT#3, Buffer := Buffer);
Pos := OPENOT_WriteByte(Offset := Pos, Value := BYTE#16#00, Buffer := Buffer);
Pos := OPENOT_WriteByte(Offset := Pos, Value := BYTE#16#00, Buffer := Buffer);
Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#0004, TypeTag := BYTE#16#03, PayloadLength := BYTE#2, Buffer := Buffer);
Pos := OPENOT_WriteU16LE(Offset := Pos, Value := UINT#4, Buffer := Buffer);
Pos := OPENOT_WriteByte(Offset := Pos, Value := BYTE#16#00, Buffer := Buffer);
Pos := OPENOT_WriteByte(Offset := Pos, Value := BYTE#16#00, Buffer := Buffer);
Crc := OPENOT_Crc32c(Data := Buffer, Length := Pos);
Pos := OPENOT_WriteDWordLE(Offset := Pos, Value := Crc, Buffer := Buffer);
Length := Pos;
END_FUNCTION_BLOCK

FUNCTION_BLOCK OPENOT_EncodeConformantMessage
VAR_OUTPUT
    Buffer : OPENOT_BYTE_BUFFER;
    Length : UINT;
END_VAR
VAR
    Pos : UINT;
    Crc : DWORD;
END_VAR

Pos := OPENOT_WriteHeader(
    TotalLength := UINT#76,
    Flags := UINT#16#0008,
    SourceTime := ULINT#1000000010,
    RunId := ULINT#1,
    Seq := ULINT#2,
    SourceId := UDINT#66,
    EventTypeId := UDINT#16#0003,
    Buffer := Buffer);

Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#0014, TypeTag := BYTE#16#05, PayloadLength := BYTE#4, Buffer := Buffer);
Pos := OPENOT_WriteU32LE(Offset := Pos, Value := UDINT#1001, Buffer := Buffer);
Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#0015, TypeTag := BYTE#16#0C, PayloadLength := BYTE#11, Buffer := Buffer);
Pos := OPENOT_WriteByte(Offset := Pos, Value := BYTE#16#70, Buffer := Buffer);
Pos := OPENOT_WriteByte(Offset := Pos, Value := BYTE#16#68, Buffer := Buffer);
Pos := OPENOT_WriteByte(Offset := Pos, Value := BYTE#16#61, Buffer := Buffer);
Pos := OPENOT_WriteByte(Offset := Pos, Value := BYTE#16#73, Buffer := Buffer);
Pos := OPENOT_WriteByte(Offset := Pos, Value := BYTE#16#65, Buffer := Buffer);
Pos := OPENOT_WriteByte(Offset := Pos, Value := BYTE#16#20, Buffer := Buffer);
Pos := OPENOT_WriteByte(Offset := Pos, Value := BYTE#16#72, Buffer := Buffer);
Pos := OPENOT_WriteByte(Offset := Pos, Value := BYTE#16#65, Buffer := Buffer);
Pos := OPENOT_WriteByte(Offset := Pos, Value := BYTE#16#61, Buffer := Buffer);
Pos := OPENOT_WriteByte(Offset := Pos, Value := BYTE#16#64, Buffer := Buffer);
Pos := OPENOT_WriteByte(Offset := Pos, Value := BYTE#16#79, Buffer := Buffer);
Pos := OPENOT_WriteByte(Offset := Pos, Value := BYTE#16#00, Buffer := Buffer);
Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#0008, TypeTag := BYTE#16#03, PayloadLength := BYTE#2, Buffer := Buffer);
Pos := OPENOT_WriteU16LE(Offset := Pos, Value := UINT#500, Buffer := Buffer);
Pos := OPENOT_WriteByte(Offset := Pos, Value := BYTE#16#00, Buffer := Buffer);
Pos := OPENOT_WriteByte(Offset := Pos, Value := BYTE#16#00, Buffer := Buffer);
Crc := OPENOT_Crc32c(Data := Buffer, Length := Pos);
Pos := OPENOT_WriteDWordLE(Offset := Pos, Value := Crc, Buffer := Buffer);
Length := Pos;
END_FUNCTION_BLOCK

FUNCTION_BLOCK OPENOT_EncodeConformantRecordsDropped
VAR_OUTPUT
    Buffer : OPENOT_BYTE_BUFFER;
    Length : UINT;
END_VAR
VAR
    Pos : UINT;
    Crc : DWORD;
END_VAR

Pos := OPENOT_WriteHeader(
    TotalLength := UINT#100,
    Flags := UINT#16#0008,
    SourceTime := ULINT#0,
    RunId := ULINT#9,
    Seq := ULINT#100,
    SourceId := UDINT#66,
    EventTypeId := UDINT#16#0104,
    Buffer := Buffer);

Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#0016, TypeTag := BYTE#16#05, PayloadLength := BYTE#4, Buffer := Buffer);
Pos := OPENOT_WriteU32LE(Offset := Pos, Value := UDINT#18, Buffer := Buffer);
Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#0017, TypeTag := BYTE#16#07, PayloadLength := BYTE#8, Buffer := Buffer);
Pos := OPENOT_WriteU64LE(Offset := Pos, Value := ULINT#40, Buffer := Buffer);
Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#0018, TypeTag := BYTE#16#07, PayloadLength := BYTE#8, Buffer := Buffer);
Pos := OPENOT_WriteU64LE(Offset := Pos, Value := ULINT#57, Buffer := Buffer);
Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#0019, TypeTag := BYTE#16#0B, PayloadLength := BYTE#8, Buffer := Buffer);
Pos := OPENOT_WriteU64LE(Offset := Pos, Value := ULINT#1000000000, Buffer := Buffer);
Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#001A, TypeTag := BYTE#16#0B, PayloadLength := BYTE#8, Buffer := Buffer);
Pos := OPENOT_WriteU64LE(Offset := Pos, Value := ULINT#1000010000, Buffer := Buffer);
Crc := OPENOT_Crc32c(Data := Buffer, Length := Pos);
Pos := OPENOT_WriteDWordLE(Offset := Pos, Value := Crc, Buffer := Buffer);
Length := Pos;
END_FUNCTION_BLOCK

FUNCTION_BLOCK OPENOT_EncodeConformantSourceHighWater
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
    SourceTime := ULINT#0,
    RunId := ULINT#1,
    Seq := ULINT#5,
    SourceId := UDINT#88,
        EventTypeId := UDINT#16#0108,
    Buffer := Buffer);

Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#0038, TypeTag := BYTE#16#07, PayloadLength := BYTE#8, Buffer := Buffer);
Pos := OPENOT_WriteU64LE(Offset := Pos, Value := ULINT#5, Buffer := Buffer);
Crc := OPENOT_Crc32c(Data := Buffer, Length := Pos);
Pos := OPENOT_WriteDWordLE(Offset := Pos, Value := Crc, Buffer := Buffer);
Length := Pos;
END_FUNCTION_BLOCK
