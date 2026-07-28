FUNCTION_BLOCK OPENOT_EncodeMinimalMessage
VAR_INPUT
    SourceId : UDINT;
    RunId : ULINT;
    Seq : ULINT;
    SourceTime : ULINT;
    MessageTemplateId : UDINT;
    HasSeverity : BOOL;
    Severity : UINT;
    ArgCount : UINT;
    Arg1TypeTag : BYTE;
    Arg1PayloadLength : UINT;
    Arg1Bits : ULINT;
    Arg1String : STRING[96];
    Arg2TypeTag : BYTE;
    Arg2PayloadLength : UINT;
    Arg2Bits : ULINT;
    Arg2String : STRING[96];
    Arg3TypeTag : BYTE;
    Arg3PayloadLength : UINT;
    Arg3Bits : ULINT;
    Arg3String : STRING[96];
    Arg4TypeTag : BYTE;
    Arg4PayloadLength : UINT;
    Arg4Bits : ULINT;
    Arg4String : STRING[96];
END_VAR
VAR_OUTPUT
    Buffer : OPENOT_BYTE_BUFFER;
    Length : UINT;
END_VAR
VAR
    Pos : UINT;
    TotalLength : UINT;
    ArgLength : UINT;
    Crc : DWORD;
END_VAR

TotalLength := UINT#40 + UINT#8 + UINT#4;
IF ArgCount >= UINT#1 THEN
    IF Arg1TypeTag = BYTE#16#0C THEN
        ArgLength := INT_TO_UINT(LEN(Arg1String));
    ELSE
        ArgLength := Arg1PayloadLength;
    END_IF;
    TotalLength := TotalLength + OPENOT_ValueSlotEncodedLength(PayloadLength := ArgLength);
END_IF;
IF ArgCount >= UINT#2 THEN
    IF Arg2TypeTag = BYTE#16#0C THEN
        ArgLength := INT_TO_UINT(LEN(Arg2String));
    ELSE
        ArgLength := Arg2PayloadLength;
    END_IF;
    TotalLength := TotalLength + OPENOT_ValueSlotEncodedLength(PayloadLength := ArgLength);
END_IF;
IF ArgCount >= UINT#3 THEN
    IF Arg3TypeTag = BYTE#16#0C THEN
        ArgLength := INT_TO_UINT(LEN(Arg3String));
    ELSE
        ArgLength := Arg3PayloadLength;
    END_IF;
    TotalLength := TotalLength + OPENOT_ValueSlotEncodedLength(PayloadLength := ArgLength);
END_IF;
IF ArgCount >= UINT#4 THEN
    IF Arg4TypeTag = BYTE#16#0C THEN
        ArgLength := INT_TO_UINT(LEN(Arg4String));
    ELSE
        ArgLength := Arg4PayloadLength;
    END_IF;
    TotalLength := TotalLength + OPENOT_ValueSlotEncodedLength(PayloadLength := ArgLength);
END_IF;
IF HasSeverity THEN
    TotalLength := TotalLength + UINT#8;
END_IF;

Pos := OPENOT_WriteHeader(
    TotalLength := TotalLength,
    Flags := UINT#16#0008,
    SourceTime := SourceTime,
    RunId := RunId,
    Seq := Seq,
    SourceId := SourceId,
    EventTypeId := UDINT#16#0003,
    Buffer := Buffer);

Pos := OPENOT_WriteSlotHeader(
    Offset := Pos,
    Key := UINT#16#0014,
    TypeTag := BYTE#16#05,
    PayloadLength := BYTE#4,
    Buffer := Buffer);
Pos := OPENOT_WriteU32LE(Offset := Pos, Value := MessageTemplateId, Buffer := Buffer);

IF ArgCount >= UINT#1 THEN
    IF Arg1TypeTag = BYTE#16#0C THEN
        ArgLength := INT_TO_UINT(LEN(Arg1String));
        Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#0015, TypeTag := Arg1TypeTag, PayloadLength := UINT_TO_BYTE(ArgLength), Buffer := Buffer);
        Pos := OPENOT_WriteStringPayload(Offset := Pos, Value := Arg1String, PayloadLength := ArgLength, Buffer := Buffer);
    ELSE
        Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#0015, TypeTag := Arg1TypeTag, PayloadLength := UINT_TO_BYTE(Arg1PayloadLength), Buffer := Buffer);
        Pos := OPENOT_WriteValueBitsLE(Offset := Pos, Value := Arg1Bits, PayloadLength := Arg1PayloadLength, Buffer := Buffer);
    END_IF;
END_IF;
IF ArgCount >= UINT#2 THEN
    IF Arg2TypeTag = BYTE#16#0C THEN
        ArgLength := INT_TO_UINT(LEN(Arg2String));
        Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#0015, TypeTag := Arg2TypeTag, PayloadLength := UINT_TO_BYTE(ArgLength), Buffer := Buffer);
        Pos := OPENOT_WriteStringPayload(Offset := Pos, Value := Arg2String, PayloadLength := ArgLength, Buffer := Buffer);
    ELSE
        Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#0015, TypeTag := Arg2TypeTag, PayloadLength := UINT_TO_BYTE(Arg2PayloadLength), Buffer := Buffer);
        Pos := OPENOT_WriteValueBitsLE(Offset := Pos, Value := Arg2Bits, PayloadLength := Arg2PayloadLength, Buffer := Buffer);
    END_IF;
END_IF;
IF ArgCount >= UINT#3 THEN
    IF Arg3TypeTag = BYTE#16#0C THEN
        ArgLength := INT_TO_UINT(LEN(Arg3String));
        Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#0015, TypeTag := Arg3TypeTag, PayloadLength := UINT_TO_BYTE(ArgLength), Buffer := Buffer);
        Pos := OPENOT_WriteStringPayload(Offset := Pos, Value := Arg3String, PayloadLength := ArgLength, Buffer := Buffer);
    ELSE
        Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#0015, TypeTag := Arg3TypeTag, PayloadLength := UINT_TO_BYTE(Arg3PayloadLength), Buffer := Buffer);
        Pos := OPENOT_WriteValueBitsLE(Offset := Pos, Value := Arg3Bits, PayloadLength := Arg3PayloadLength, Buffer := Buffer);
    END_IF;
END_IF;
IF ArgCount >= UINT#4 THEN
    IF Arg4TypeTag = BYTE#16#0C THEN
        ArgLength := INT_TO_UINT(LEN(Arg4String));
        Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#0015, TypeTag := Arg4TypeTag, PayloadLength := UINT_TO_BYTE(ArgLength), Buffer := Buffer);
        Pos := OPENOT_WriteStringPayload(Offset := Pos, Value := Arg4String, PayloadLength := ArgLength, Buffer := Buffer);
    ELSE
        Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#0015, TypeTag := Arg4TypeTag, PayloadLength := UINT_TO_BYTE(Arg4PayloadLength), Buffer := Buffer);
        Pos := OPENOT_WriteValueBitsLE(Offset := Pos, Value := Arg4Bits, PayloadLength := Arg4PayloadLength, Buffer := Buffer);
    END_IF;
END_IF;

IF HasSeverity THEN
    Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#0008, TypeTag := BYTE#16#03, PayloadLength := BYTE#2, Buffer := Buffer);
    Pos := OPENOT_WriteU16LE(Offset := Pos, Value := Severity, Buffer := Buffer);
    Pos := OPENOT_WriteByte(Offset := Pos, Value := BYTE#16#00, Buffer := Buffer);
    Pos := OPENOT_WriteByte(Offset := Pos, Value := BYTE#16#00, Buffer := Buffer);
END_IF;

Crc := OPENOT_Crc32c(Data := Buffer, Length := Pos);
Pos := OPENOT_WriteDWordLE(Offset := Pos, Value := Crc, Buffer := Buffer);
Length := Pos;
END_FUNCTION_BLOCK
