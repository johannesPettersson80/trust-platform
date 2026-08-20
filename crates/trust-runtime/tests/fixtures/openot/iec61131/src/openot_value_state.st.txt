FUNCTION_BLOCK OPENOT_EncodeValueChangedReal
VAR_INPUT
    SourceId : UDINT;
    RunId : ULINT;
    Seq : ULINT;
    SourceTime : ULINT;
    ValueId : UDINT;
    NewValue : REAL;
    HasPrevious : BOOL;
    PreviousValue : REAL;
    HasQuality : BOOL;
    Quality : UINT;
END_VAR
VAR_OUTPUT
    Buffer : OPENOT_BYTE_BUFFER;
    Length : UINT;
END_VAR
VAR
    Pos : UINT;
    TotalLength : UINT;
    Crc : DWORD;
    ValueBits : DWORD;
END_VAR

TotalLength := UINT#60;
IF HasPrevious THEN
    TotalLength := TotalLength + UINT#8;
END_IF;
IF HasQuality THEN
    TotalLength := TotalLength + UINT#8;
END_IF;

Pos := OPENOT_WriteHeader(
    TotalLength := TotalLength,
    Flags := UINT#16#0008,
    SourceTime := SourceTime,
    RunId := RunId,
    Seq := Seq,
    SourceId := SourceId,
    EventTypeId := UDINT#16#0002,
    Buffer := Buffer);

Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#000D, TypeTag := BYTE#16#05, PayloadLength := BYTE#4, Buffer := Buffer);
Pos := OPENOT_WriteU32LE(Offset := Pos, Value := ValueId, Buffer := Buffer);

ValueBits := REAL_TO_DWORD(NewValue);
Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#0010, TypeTag := BYTE#16#09, PayloadLength := BYTE#4, Buffer := Buffer);
Pos := OPENOT_WriteDWordLE(Offset := Pos, Value := ValueBits, Buffer := Buffer);

IF HasPrevious THEN
    ValueBits := REAL_TO_DWORD(PreviousValue);
    Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#000F, TypeTag := BYTE#16#09, PayloadLength := BYTE#4, Buffer := Buffer);
    Pos := OPENOT_WriteDWordLE(Offset := Pos, Value := ValueBits, Buffer := Buffer);
END_IF;

IF HasQuality THEN
    Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#0011, TypeTag := BYTE#16#03, PayloadLength := BYTE#2, Buffer := Buffer);
    Pos := OPENOT_WriteU16LE(Offset := Pos, Value := Quality, Buffer := Buffer);
    Pos := OPENOT_WriteByte(Offset := Pos, Value := BYTE#16#00, Buffer := Buffer);
    Pos := OPENOT_WriteByte(Offset := Pos, Value := BYTE#16#00, Buffer := Buffer);
END_IF;

Crc := OPENOT_Crc32c(Data := Buffer, Length := Pos);
Pos := OPENOT_WriteDWordLE(Offset := Pos, Value := Crc, Buffer := Buffer);
Length := Pos;
END_FUNCTION_BLOCK

FUNCTION_BLOCK OPENOT_EncodeConditionLifecycle
VAR_INPUT
    SourceId : UDINT;
    RunId : ULINT;
    Seq : ULINT;
    SourceTime : ULINT;
    EventTypeId : UDINT;
    ConditionId : UDINT;
    CorrelationId : UDINT;
    HasAckBy : BOOL;
    AckBy : STRING[96];
    HasShelveSecs : BOOL;
    ShelveSecs : UDINT;
    HasReason : BOOL;
    Reason : STRING[96];
    HasComment : BOOL;
    Comment : STRING[96];
    HasPreviousPriority : BOOL;
    PreviousPriority : UINT;
    HasNewPriority : BOOL;
    NewPriority : UINT;
    HasRefreshId : BOOL;
    RefreshId : UDINT;
END_VAR
VAR_OUTPUT
    Buffer : OPENOT_BYTE_BUFFER;
    Length : UINT;
END_VAR
VAR
    Pos : UINT;
    TotalLength : UINT;
    Crc : DWORD;
    AckByLength : UINT;
    ReasonLength : UINT;
    CommentLength : UINT;
    IsActivationScoped : BOOL;
END_VAR

AckByLength := INT_TO_UINT(LEN(AckBy));
ReasonLength := INT_TO_UINT(LEN(Reason));
CommentLength := INT_TO_UINT(LEN(Comment));
IF AckByLength > UINT#96 THEN
    AckByLength := UINT#96;
END_IF;
IF ReasonLength > UINT#96 THEN
    ReasonLength := UINT#96;
END_IF;
IF CommentLength > UINT#96 THEN
    CommentLength := UINT#96;
END_IF;

IsActivationScoped := (EventTypeId = UDINT#16#0202) OR (EventTypeId = UDINT#16#0203) OR (EventTypeId = UDINT#16#0204) OR (EventTypeId = UDINT#16#0205) OR (EventTypeId = UDINT#16#020A) OR (EventTypeId = UDINT#16#020B);
TotalLength := UINT#52;
IF IsActivationScoped THEN
    TotalLength := TotalLength + UINT#8;
END_IF;
IF HasComment THEN
    TotalLength := TotalLength + OPENOT_ValueSlotEncodedLength(PayloadLength := CommentLength);
END_IF;
IF HasAckBy THEN
    TotalLength := TotalLength + OPENOT_ValueSlotEncodedLength(PayloadLength := AckByLength);
END_IF;
IF HasShelveSecs THEN
    TotalLength := TotalLength + UINT#8;
END_IF;
IF HasReason THEN
    TotalLength := TotalLength + OPENOT_ValueSlotEncodedLength(PayloadLength := ReasonLength);
END_IF;
IF HasPreviousPriority THEN
    TotalLength := TotalLength + UINT#8;
END_IF;
IF HasNewPriority THEN
    TotalLength := TotalLength + UINT#8;
END_IF;
IF HasRefreshId THEN
    TotalLength := TotalLength + UINT#8;
END_IF;

Pos := OPENOT_WriteHeader(
    TotalLength := TotalLength,
    Flags := UINT#16#0008,
    SourceTime := SourceTime,
    RunId := RunId,
    Seq := Seq,
    SourceId := SourceId,
    EventTypeId := EventTypeId,
    Buffer := Buffer);

Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#0005, TypeTag := BYTE#16#05, PayloadLength := BYTE#4, Buffer := Buffer);
Pos := OPENOT_WriteU32LE(Offset := Pos, Value := ConditionId, Buffer := Buffer);

IF IsActivationScoped THEN
    Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#0007, TypeTag := BYTE#16#05, PayloadLength := BYTE#4, Buffer := Buffer);
    Pos := OPENOT_WriteU32LE(Offset := Pos, Value := CorrelationId, Buffer := Buffer);
END_IF;

IF EventTypeId = UDINT#16#020A THEN
    IF HasComment THEN
        Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#0037, TypeTag := BYTE#16#0C, PayloadLength := UINT_TO_BYTE(CommentLength), Buffer := Buffer);
        Pos := OPENOT_WriteStringPayload(Offset := Pos, Value := Comment, PayloadLength := CommentLength, Buffer := Buffer);
    END_IF;

    IF HasAckBy THEN
        Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#001D, TypeTag := BYTE#16#0C, PayloadLength := UINT_TO_BYTE(AckByLength), Buffer := Buffer);
        Pos := OPENOT_WriteStringPayload(Offset := Pos, Value := AckBy, PayloadLength := AckByLength, Buffer := Buffer);
    END_IF;
ELSIF EventTypeId = UDINT#16#020C THEN
    IF HasPreviousPriority THEN
        Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#002E, TypeTag := BYTE#16#03, PayloadLength := BYTE#2, Buffer := Buffer);
        Pos := OPENOT_WriteU16LE(Offset := Pos, Value := PreviousPriority, Buffer := Buffer);
        Pos := OPENOT_WriteByte(Offset := Pos, Value := BYTE#16#00, Buffer := Buffer);
        Pos := OPENOT_WriteByte(Offset := Pos, Value := BYTE#16#00, Buffer := Buffer);
    END_IF;

    IF HasNewPriority THEN
        Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#002C, TypeTag := BYTE#16#03, PayloadLength := BYTE#2, Buffer := Buffer);
        Pos := OPENOT_WriteU16LE(Offset := Pos, Value := NewPriority, Buffer := Buffer);
        Pos := OPENOT_WriteByte(Offset := Pos, Value := BYTE#16#00, Buffer := Buffer);
        Pos := OPENOT_WriteByte(Offset := Pos, Value := BYTE#16#00, Buffer := Buffer);
    END_IF;

    IF HasAckBy THEN
        Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#001D, TypeTag := BYTE#16#0C, PayloadLength := UINT_TO_BYTE(AckByLength), Buffer := Buffer);
        Pos := OPENOT_WriteStringPayload(Offset := Pos, Value := AckBy, PayloadLength := AckByLength, Buffer := Buffer);
    END_IF;
ELSE
    IF HasAckBy THEN
        Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#001D, TypeTag := BYTE#16#0C, PayloadLength := UINT_TO_BYTE(AckByLength), Buffer := Buffer);
        Pos := OPENOT_WriteStringPayload(Offset := Pos, Value := AckBy, PayloadLength := AckByLength, Buffer := Buffer);
    END_IF;

    IF HasShelveSecs THEN
        Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#001E, TypeTag := BYTE#16#05, PayloadLength := BYTE#4, Buffer := Buffer);
        Pos := OPENOT_WriteU32LE(Offset := Pos, Value := ShelveSecs, Buffer := Buffer);
    END_IF;

    IF HasReason THEN
        Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#001F, TypeTag := BYTE#16#0C, PayloadLength := UINT_TO_BYTE(ReasonLength), Buffer := Buffer);
        Pos := OPENOT_WriteStringPayload(Offset := Pos, Value := Reason, PayloadLength := ReasonLength, Buffer := Buffer);
    END_IF;
END_IF;

IF HasRefreshId THEN
    Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#0034, TypeTag := BYTE#16#05, PayloadLength := BYTE#4, Buffer := Buffer);
    Pos := OPENOT_WriteU32LE(Offset := Pos, Value := RefreshId, Buffer := Buffer);
END_IF;

Crc := OPENOT_Crc32c(Data := Buffer, Length := Pos);
Pos := OPENOT_WriteDWordLE(Offset := Pos, Value := Crc, Buffer := Buffer);
Length := Pos;
END_FUNCTION_BLOCK

FUNCTION_BLOCK OPENOT_EncodeBatchRecipe
VAR_INPUT
    SourceId : UDINT;
    RunId : ULINT;
    Seq : ULINT;
    SourceTime : ULINT;
    EventTypeId : UDINT;
    RecipeId : UDINT;
    HasRecipeId : BOOL;
    RecipeVersion : STRING[96];
    HasBatchId : BOOL;
    BatchId : UDINT;
    HasAuthResult : BOOL;
    AuthResult : UINT;
    HasAckBy : BOOL;
    AckBy : STRING[96];
    NewState : UINT;
    MaterialId : UDINT;
    Quantity : LREAL;
    HasUnit : BOOL;
    UnitId : UINT;
END_VAR
VAR_OUTPUT
    Buffer : OPENOT_BYTE_BUFFER;
    Length : UINT;
END_VAR
VAR
    Pos : UINT;
    TotalLength : UINT;
    Crc : DWORD;
    RecipeVersionLength : UINT;
    AckByLength : UINT;
    QuantityBits : ULINT;
END_VAR

RecipeVersionLength := INT_TO_UINT(LEN(RecipeVersion));
AckByLength := INT_TO_UINT(LEN(AckBy));
IF RecipeVersionLength > UINT#96 THEN
    RecipeVersionLength := UINT#96;
END_IF;
IF AckByLength > UINT#96 THEN
    AckByLength := UINT#96;
END_IF;

IF EventTypeId = UDINT#16#0301 THEN
    TotalLength := UINT#52 + OPENOT_ValueSlotEncodedLength(PayloadLength := RecipeVersionLength);
    IF HasBatchId THEN
        TotalLength := TotalLength + UINT#8;
    END_IF;
ELSIF EventTypeId = UDINT#16#0302 THEN
    TotalLength := UINT#52 + OPENOT_ValueSlotEncodedLength(PayloadLength := RecipeVersionLength);
    IF HasAuthResult THEN
        TotalLength := TotalLength + UINT#8;
    END_IF;
    IF HasAckBy THEN
        TotalLength := TotalLength + OPENOT_ValueSlotEncodedLength(PayloadLength := AckByLength);
    END_IF;
ELSIF EventTypeId = UDINT#16#0303 THEN
    TotalLength := UINT#60;
    IF HasRecipeId THEN
        TotalLength := TotalLength + UINT#8;
    END_IF;
ELSIF EventTypeId = UDINT#16#0304 THEN
    TotalLength := UINT#72;
    IF HasUnit THEN
        TotalLength := TotalLength + UINT#8;
    END_IF;
ELSE
    TotalLength := UINT#0;
END_IF;

IF TotalLength > UINT#0 THEN
    Pos := OPENOT_WriteHeader(
        TotalLength := TotalLength,
        Flags := UINT#16#0008,
        SourceTime := SourceTime,
        RunId := RunId,
        Seq := Seq,
        SourceId := SourceId,
        EventTypeId := EventTypeId,
        Buffer := Buffer);

    IF EventTypeId = UDINT#16#0301 THEN
        Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#0023, TypeTag := BYTE#16#05, PayloadLength := BYTE#4, Buffer := Buffer);
        Pos := OPENOT_WriteU32LE(Offset := Pos, Value := RecipeId, Buffer := Buffer);
        Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#0024, TypeTag := BYTE#16#0C, PayloadLength := UINT_TO_BYTE(RecipeVersionLength), Buffer := Buffer);
        Pos := OPENOT_WriteStringPayload(Offset := Pos, Value := RecipeVersion, PayloadLength := RecipeVersionLength, Buffer := Buffer);
        IF HasBatchId THEN
            Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#0025, TypeTag := BYTE#16#05, PayloadLength := BYTE#4, Buffer := Buffer);
            Pos := OPENOT_WriteU32LE(Offset := Pos, Value := BatchId, Buffer := Buffer);
        END_IF;
    ELSIF EventTypeId = UDINT#16#0302 THEN
        Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#0023, TypeTag := BYTE#16#05, PayloadLength := BYTE#4, Buffer := Buffer);
        Pos := OPENOT_WriteU32LE(Offset := Pos, Value := RecipeId, Buffer := Buffer);
        Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#0024, TypeTag := BYTE#16#0C, PayloadLength := UINT_TO_BYTE(RecipeVersionLength), Buffer := Buffer);
        Pos := OPENOT_WriteStringPayload(Offset := Pos, Value := RecipeVersion, PayloadLength := RecipeVersionLength, Buffer := Buffer);
        IF HasAuthResult THEN
            Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#0020, TypeTag := BYTE#16#03, PayloadLength := BYTE#2, Buffer := Buffer);
            Pos := OPENOT_WriteU16LE(Offset := Pos, Value := AuthResult, Buffer := Buffer);
            Pos := OPENOT_WriteByte(Offset := Pos, Value := BYTE#16#00, Buffer := Buffer);
            Pos := OPENOT_WriteByte(Offset := Pos, Value := BYTE#16#00, Buffer := Buffer);
        END_IF;
        IF HasAckBy THEN
            Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#001D, TypeTag := BYTE#16#0C, PayloadLength := UINT_TO_BYTE(AckByLength), Buffer := Buffer);
            Pos := OPENOT_WriteStringPayload(Offset := Pos, Value := AckBy, PayloadLength := AckByLength, Buffer := Buffer);
        END_IF;
    ELSIF EventTypeId = UDINT#16#0303 THEN
        Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#0025, TypeTag := BYTE#16#05, PayloadLength := BYTE#4, Buffer := Buffer);
        Pos := OPENOT_WriteU32LE(Offset := Pos, Value := BatchId, Buffer := Buffer);
        Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#0004, TypeTag := BYTE#16#03, PayloadLength := BYTE#2, Buffer := Buffer);
        Pos := OPENOT_WriteU16LE(Offset := Pos, Value := NewState, Buffer := Buffer);
        Pos := OPENOT_WriteByte(Offset := Pos, Value := BYTE#16#00, Buffer := Buffer);
        Pos := OPENOT_WriteByte(Offset := Pos, Value := BYTE#16#00, Buffer := Buffer);
        IF HasRecipeId THEN
            Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#0023, TypeTag := BYTE#16#05, PayloadLength := BYTE#4, Buffer := Buffer);
            Pos := OPENOT_WriteU32LE(Offset := Pos, Value := RecipeId, Buffer := Buffer);
        END_IF;
    ELSIF EventTypeId = UDINT#16#0304 THEN
        Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#0025, TypeTag := BYTE#16#05, PayloadLength := BYTE#4, Buffer := Buffer);
        Pos := OPENOT_WriteU32LE(Offset := Pos, Value := BatchId, Buffer := Buffer);
        Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#0026, TypeTag := BYTE#16#05, PayloadLength := BYTE#4, Buffer := Buffer);
        Pos := OPENOT_WriteU32LE(Offset := Pos, Value := MaterialId, Buffer := Buffer);
        QuantityBits := LWORD_TO_ULINT(LREAL_TO_LWORD(Quantity));
        Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#0027, TypeTag := BYTE#16#0A, PayloadLength := BYTE#8, Buffer := Buffer);
        Pos := OPENOT_WriteU64LE(Offset := Pos, Value := QuantityBits, Buffer := Buffer);
        IF HasUnit THEN
            Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#0013, TypeTag := BYTE#16#03, PayloadLength := BYTE#2, Buffer := Buffer);
            Pos := OPENOT_WriteU16LE(Offset := Pos, Value := UnitId, Buffer := Buffer);
            Pos := OPENOT_WriteByte(Offset := Pos, Value := BYTE#16#00, Buffer := Buffer);
            Pos := OPENOT_WriteByte(Offset := Pos, Value := BYTE#16#00, Buffer := Buffer);
        END_IF;
    END_IF;

    Crc := OPENOT_Crc32c(Data := Buffer, Length := Pos);
    Pos := OPENOT_WriteDWordLE(Offset := Pos, Value := Crc, Buffer := Buffer);
    Length := Pos;
ELSE
    Length := UINT#0;
END_IF;
END_FUNCTION_BLOCK

FUNCTION_BLOCK OPENOT_EncodeRegulated
VAR_INPUT
    SourceId : UDINT;
    RunId : ULINT;
    Seq : ULINT;
    SourceTime : ULINT;
    EventTypeId : UDINT;
    ActionId : UDINT;
    Actor : STRING[96];
    HasContext1 : BOOL;
    Context1 : UDINT;
    HasContext2 : BOOL;
    Context2 : UDINT;
    HasContext3 : BOOL;
    Context3 : UDINT;
    HasContext4 : BOOL;
    Context4 : UDINT;
    HasAuthResult : BOOL;
    AuthResult : UINT;
    HasWorkstation : BOOL;
    Workstation : STRING[96];
    HasRole : BOOL;
    Role : UINT;
    HasReason : BOOL;
    Reason : STRING[96];
END_VAR
VAR_OUTPUT
    Buffer : OPENOT_BYTE_BUFFER;
    Length : UINT;
END_VAR
VAR
    Pos : UINT;
    TotalLength : UINT;
    Crc : DWORD;
    ActorLength : UINT;
    WorkstationLength : UINT;
    ReasonLength : UINT;
END_VAR

ActorLength := INT_TO_UINT(LEN(Actor));
WorkstationLength := INT_TO_UINT(LEN(Workstation));
ReasonLength := INT_TO_UINT(LEN(Reason));
IF ActorLength > UINT#96 THEN
    ActorLength := UINT#96;
END_IF;
IF WorkstationLength > UINT#96 THEN
    WorkstationLength := UINT#96;
END_IF;
IF ReasonLength > UINT#96 THEN
    ReasonLength := UINT#96;
END_IF;

IF EventTypeId = UDINT#16#0400 THEN
    TotalLength := UINT#52 + OPENOT_ValueSlotEncodedLength(PayloadLength := ActorLength);
    IF HasContext1 THEN
        TotalLength := TotalLength + UINT#8;
    END_IF;
    IF HasContext2 THEN
        TotalLength := TotalLength + UINT#8;
    END_IF;
    IF HasContext3 THEN
        TotalLength := TotalLength + UINT#8;
    END_IF;
    IF HasContext4 THEN
        TotalLength := TotalLength + UINT#8;
    END_IF;
    IF HasAuthResult THEN
        TotalLength := TotalLength + UINT#8;
    END_IF;
    IF HasWorkstation THEN
        TotalLength := TotalLength + OPENOT_ValueSlotEncodedLength(PayloadLength := WorkstationLength);
    END_IF;
ELSIF EventTypeId = UDINT#16#0401 THEN
    TotalLength := UINT#44 + OPENOT_ValueSlotEncodedLength(PayloadLength := ActorLength) + UINT#8;
    IF HasWorkstation THEN
        TotalLength := TotalLength + OPENOT_ValueSlotEncodedLength(PayloadLength := WorkstationLength);
    END_IF;
    IF HasRole THEN
        TotalLength := TotalLength + UINT#8;
    END_IF;
ELSIF EventTypeId = UDINT#16#0402 THEN
    TotalLength := UINT#44 + OPENOT_ValueSlotEncodedLength(PayloadLength := ActorLength);
    IF HasWorkstation THEN
        TotalLength := TotalLength + OPENOT_ValueSlotEncodedLength(PayloadLength := WorkstationLength);
    END_IF;
ELSIF EventTypeId = UDINT#16#0405 THEN
    TotalLength := UINT#44 + OPENOT_ValueSlotEncodedLength(PayloadLength := ActorLength);
    IF HasWorkstation THEN
        TotalLength := TotalLength + OPENOT_ValueSlotEncodedLength(PayloadLength := WorkstationLength);
    END_IF;
    IF HasReason THEN
        TotalLength := TotalLength + OPENOT_ValueSlotEncodedLength(PayloadLength := ReasonLength);
    END_IF;
ELSE
    TotalLength := UINT#0;
END_IF;

IF TotalLength > UINT#0 THEN
    Pos := OPENOT_WriteHeader(
        TotalLength := TotalLength,
        Flags := UINT#16#0008,
        SourceTime := SourceTime,
        RunId := RunId,
        Seq := Seq,
        SourceId := SourceId,
        EventTypeId := EventTypeId,
        Buffer := Buffer);

    IF EventTypeId = UDINT#16#0400 THEN
        Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#000A, TypeTag := BYTE#16#05, PayloadLength := BYTE#4, Buffer := Buffer);
        Pos := OPENOT_WriteU32LE(Offset := Pos, Value := ActionId, Buffer := Buffer);
        Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#000B, TypeTag := BYTE#16#0C, PayloadLength := UINT_TO_BYTE(ActorLength), Buffer := Buffer);
        Pos := OPENOT_WriteStringPayload(Offset := Pos, Value := Actor, PayloadLength := ActorLength, Buffer := Buffer);
        IF HasContext1 THEN
            Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#000C, TypeTag := BYTE#16#05, PayloadLength := BYTE#4, Buffer := Buffer);
            Pos := OPENOT_WriteU32LE(Offset := Pos, Value := Context1, Buffer := Buffer);
        END_IF;
        IF HasContext2 THEN
            Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#000C, TypeTag := BYTE#16#05, PayloadLength := BYTE#4, Buffer := Buffer);
            Pos := OPENOT_WriteU32LE(Offset := Pos, Value := Context2, Buffer := Buffer);
        END_IF;
        IF HasContext3 THEN
            Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#000C, TypeTag := BYTE#16#05, PayloadLength := BYTE#4, Buffer := Buffer);
            Pos := OPENOT_WriteU32LE(Offset := Pos, Value := Context3, Buffer := Buffer);
        END_IF;
        IF HasContext4 THEN
            Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#000C, TypeTag := BYTE#16#05, PayloadLength := BYTE#4, Buffer := Buffer);
            Pos := OPENOT_WriteU32LE(Offset := Pos, Value := Context4, Buffer := Buffer);
        END_IF;
        IF HasAuthResult THEN
            Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#0020, TypeTag := BYTE#16#03, PayloadLength := BYTE#2, Buffer := Buffer);
            Pos := OPENOT_WriteU16LE(Offset := Pos, Value := AuthResult, Buffer := Buffer);
            Pos := OPENOT_WriteByte(Offset := Pos, Value := BYTE#16#00, Buffer := Buffer);
            Pos := OPENOT_WriteByte(Offset := Pos, Value := BYTE#16#00, Buffer := Buffer);
        END_IF;
        IF HasWorkstation THEN
            Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#0021, TypeTag := BYTE#16#0C, PayloadLength := UINT_TO_BYTE(WorkstationLength), Buffer := Buffer);
            Pos := OPENOT_WriteStringPayload(Offset := Pos, Value := Workstation, PayloadLength := WorkstationLength, Buffer := Buffer);
        END_IF;
    ELSIF EventTypeId = UDINT#16#0401 THEN
        Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#000B, TypeTag := BYTE#16#0C, PayloadLength := UINT_TO_BYTE(ActorLength), Buffer := Buffer);
        Pos := OPENOT_WriteStringPayload(Offset := Pos, Value := Actor, PayloadLength := ActorLength, Buffer := Buffer);
        Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#0020, TypeTag := BYTE#16#03, PayloadLength := BYTE#2, Buffer := Buffer);
        Pos := OPENOT_WriteU16LE(Offset := Pos, Value := AuthResult, Buffer := Buffer);
        Pos := OPENOT_WriteByte(Offset := Pos, Value := BYTE#16#00, Buffer := Buffer);
        Pos := OPENOT_WriteByte(Offset := Pos, Value := BYTE#16#00, Buffer := Buffer);
        IF HasWorkstation THEN
            Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#0021, TypeTag := BYTE#16#0C, PayloadLength := UINT_TO_BYTE(WorkstationLength), Buffer := Buffer);
            Pos := OPENOT_WriteStringPayload(Offset := Pos, Value := Workstation, PayloadLength := WorkstationLength, Buffer := Buffer);
        END_IF;
        IF HasRole THEN
            Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#0033, TypeTag := BYTE#16#03, PayloadLength := BYTE#2, Buffer := Buffer);
            Pos := OPENOT_WriteU16LE(Offset := Pos, Value := Role, Buffer := Buffer);
            Pos := OPENOT_WriteByte(Offset := Pos, Value := BYTE#16#00, Buffer := Buffer);
            Pos := OPENOT_WriteByte(Offset := Pos, Value := BYTE#16#00, Buffer := Buffer);
        END_IF;
    ELSIF EventTypeId = UDINT#16#0402 THEN
        Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#000B, TypeTag := BYTE#16#0C, PayloadLength := UINT_TO_BYTE(ActorLength), Buffer := Buffer);
        Pos := OPENOT_WriteStringPayload(Offset := Pos, Value := Actor, PayloadLength := ActorLength, Buffer := Buffer);
        IF HasWorkstation THEN
            Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#0021, TypeTag := BYTE#16#0C, PayloadLength := UINT_TO_BYTE(WorkstationLength), Buffer := Buffer);
            Pos := OPENOT_WriteStringPayload(Offset := Pos, Value := Workstation, PayloadLength := WorkstationLength, Buffer := Buffer);
        END_IF;
    ELSIF EventTypeId = UDINT#16#0405 THEN
        Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#000B, TypeTag := BYTE#16#0C, PayloadLength := UINT_TO_BYTE(ActorLength), Buffer := Buffer);
        Pos := OPENOT_WriteStringPayload(Offset := Pos, Value := Actor, PayloadLength := ActorLength, Buffer := Buffer);
        IF HasWorkstation THEN
            Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#0021, TypeTag := BYTE#16#0C, PayloadLength := UINT_TO_BYTE(WorkstationLength), Buffer := Buffer);
            Pos := OPENOT_WriteStringPayload(Offset := Pos, Value := Workstation, PayloadLength := WorkstationLength, Buffer := Buffer);
        END_IF;
        IF HasReason THEN
            Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#001F, TypeTag := BYTE#16#0C, PayloadLength := UINT_TO_BYTE(ReasonLength), Buffer := Buffer);
            Pos := OPENOT_WriteStringPayload(Offset := Pos, Value := Reason, PayloadLength := ReasonLength, Buffer := Buffer);
        END_IF;
    END_IF;

    Crc := OPENOT_Crc32c(Data := Buffer, Length := Pos);
    Pos := OPENOT_WriteDWordLE(Offset := Pos, Value := Crc, Buffer := Buffer);
    Length := Pos;
ELSE
    Length := UINT#0;
END_IF;
END_FUNCTION_BLOCK

FUNCTION_BLOCK OPENOT_EncodeESignature
VAR_INPUT
    SourceId : UDINT;
    RunId : ULINT;
    Seq : ULINT;
    SourceTime : ULINT;
    ActionId : UDINT;
    Actor : STRING[96];
    SignatureMeaning : UINT;
    SignedEventSeq : ULINT;
    HasAuthResult : BOOL;
    AuthResult : UINT;
END_VAR
VAR_OUTPUT
    Buffer : OPENOT_BYTE_BUFFER;
    Length : UINT;
END_VAR
VAR
    Pos : UINT;
    TotalLength : UINT;
    Crc : DWORD;
    ActorLength : UINT;
END_VAR

ActorLength := INT_TO_UINT(LEN(Actor));
IF ActorLength > UINT#96 THEN
    ActorLength := UINT#96;
END_IF;

TotalLength := UINT#72 + OPENOT_ValueSlotEncodedLength(PayloadLength := ActorLength);
IF HasAuthResult THEN
    TotalLength := TotalLength + UINT#8;
END_IF;

Pos := OPENOT_WriteHeader(
    TotalLength := TotalLength,
    Flags := UINT#16#0008,
    SourceTime := SourceTime,
    RunId := RunId,
    Seq := Seq,
    SourceId := SourceId,
    EventTypeId := UDINT#16#0404,
    Buffer := Buffer);

Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#000A, TypeTag := BYTE#16#05, PayloadLength := BYTE#4, Buffer := Buffer);
Pos := OPENOT_WriteU32LE(Offset := Pos, Value := ActionId, Buffer := Buffer);

Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#000B, TypeTag := BYTE#16#0C, PayloadLength := UINT_TO_BYTE(ActorLength), Buffer := Buffer);
Pos := OPENOT_WriteStringPayload(Offset := Pos, Value := Actor, PayloadLength := ActorLength, Buffer := Buffer);

Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#0022, TypeTag := BYTE#16#03, PayloadLength := BYTE#2, Buffer := Buffer);
Pos := OPENOT_WriteU16LE(Offset := Pos, Value := SignatureMeaning, Buffer := Buffer);
Pos := OPENOT_WriteByte(Offset := Pos, Value := BYTE#16#00, Buffer := Buffer);
Pos := OPENOT_WriteByte(Offset := Pos, Value := BYTE#16#00, Buffer := Buffer);

Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#002F, TypeTag := BYTE#16#07, PayloadLength := BYTE#8, Buffer := Buffer);
Pos := OPENOT_WriteU64LE(Offset := Pos, Value := SignedEventSeq, Buffer := Buffer);

IF HasAuthResult THEN
    Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#0020, TypeTag := BYTE#16#03, PayloadLength := BYTE#2, Buffer := Buffer);
    Pos := OPENOT_WriteU16LE(Offset := Pos, Value := AuthResult, Buffer := Buffer);
    Pos := OPENOT_WriteByte(Offset := Pos, Value := BYTE#16#00, Buffer := Buffer);
    Pos := OPENOT_WriteByte(Offset := Pos, Value := BYTE#16#00, Buffer := Buffer);
END_IF;

Crc := OPENOT_Crc32c(Data := Buffer, Length := Pos);
Pos := OPENOT_WriteDWordLE(Offset := Pos, Value := Crc, Buffer := Buffer);
Length := Pos;
END_FUNCTION_BLOCK

FUNCTION_BLOCK OPENOT_EncodeParameterChangeReal
VAR_INPUT
    SourceId : UDINT;
    RunId : ULINT;
    Seq : ULINT;
    SourceTime : ULINT;
    ValueId : UDINT;
    PreviousValue : REAL;
    NewValue : REAL;
    Actor : STRING[96];
    Reason : STRING[96];
    HasAuthResult : BOOL;
    AuthResult : UINT;
END_VAR
VAR_OUTPUT
    Buffer : OPENOT_BYTE_BUFFER;
    Length : UINT;
END_VAR
VAR
    Pos : UINT;
    TotalLength : UINT;
    Crc : DWORD;
    ValueBits : DWORD;
    ActorLength : UINT;
    ReasonLength : UINT;
END_VAR

ActorLength := INT_TO_UINT(LEN(Actor));
ReasonLength := INT_TO_UINT(LEN(Reason));
IF ActorLength > UINT#96 THEN
    ActorLength := UINT#96;
END_IF;
IF ReasonLength > UINT#96 THEN
    ReasonLength := UINT#96;
END_IF;

TotalLength := UINT#52 + UINT#8 + UINT#8 + OPENOT_ValueSlotEncodedLength(PayloadLength := ActorLength) + OPENOT_ValueSlotEncodedLength(PayloadLength := ReasonLength);
IF HasAuthResult THEN
    TotalLength := TotalLength + UINT#8;
END_IF;

Pos := OPENOT_WriteHeader(
    TotalLength := TotalLength,
    Flags := UINT#16#0008,
    SourceTime := SourceTime,
    RunId := RunId,
    Seq := Seq,
    SourceId := SourceId,
    EventTypeId := UDINT#16#0403,
    Buffer := Buffer);

Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#000D, TypeTag := BYTE#16#05, PayloadLength := BYTE#4, Buffer := Buffer);
Pos := OPENOT_WriteU32LE(Offset := Pos, Value := ValueId, Buffer := Buffer);

ValueBits := REAL_TO_DWORD(PreviousValue);
Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#000F, TypeTag := BYTE#16#09, PayloadLength := BYTE#4, Buffer := Buffer);
Pos := OPENOT_WriteDWordLE(Offset := Pos, Value := ValueBits, Buffer := Buffer);

ValueBits := REAL_TO_DWORD(NewValue);
Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#0010, TypeTag := BYTE#16#09, PayloadLength := BYTE#4, Buffer := Buffer);
Pos := OPENOT_WriteDWordLE(Offset := Pos, Value := ValueBits, Buffer := Buffer);

Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#000B, TypeTag := BYTE#16#0C, PayloadLength := UINT_TO_BYTE(ActorLength), Buffer := Buffer);
Pos := OPENOT_WriteStringPayload(Offset := Pos, Value := Actor, PayloadLength := ActorLength, Buffer := Buffer);

Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#001F, TypeTag := BYTE#16#0C, PayloadLength := UINT_TO_BYTE(ReasonLength), Buffer := Buffer);
Pos := OPENOT_WriteStringPayload(Offset := Pos, Value := Reason, PayloadLength := ReasonLength, Buffer := Buffer);

IF HasAuthResult THEN
    Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#0020, TypeTag := BYTE#16#03, PayloadLength := BYTE#2, Buffer := Buffer);
    Pos := OPENOT_WriteU16LE(Offset := Pos, Value := AuthResult, Buffer := Buffer);
    Pos := OPENOT_WriteByte(Offset := Pos, Value := BYTE#16#00, Buffer := Buffer);
    Pos := OPENOT_WriteByte(Offset := Pos, Value := BYTE#16#00, Buffer := Buffer);
END_IF;

Crc := OPENOT_Crc32c(Data := Buffer, Length := Pos);
Pos := OPENOT_WriteDWordLE(Offset := Pos, Value := Crc, Buffer := Buffer);
Length := Pos;
END_FUNCTION_BLOCK

FUNCTION_BLOCK OPENOT_EncodeParameterChangeInt
VAR_INPUT
    SourceId : UDINT;
    RunId : ULINT;
    Seq : ULINT;
    SourceTime : ULINT;
    ValueId : UDINT;
    PreviousValue : DINT;
    NewValue : DINT;
    Actor : STRING[96];
    Reason : STRING[96];
    HasAuthResult : BOOL;
    AuthResult : UINT;
END_VAR
VAR_OUTPUT
    Buffer : OPENOT_BYTE_BUFFER;
    Length : UINT;
END_VAR
VAR
    Pos : UINT;
    TotalLength : UINT;
    Crc : DWORD;
    ValueBits : DWORD;
    ActorLength : UINT;
    ReasonLength : UINT;
END_VAR

ActorLength := INT_TO_UINT(LEN(Actor));
ReasonLength := INT_TO_UINT(LEN(Reason));
IF ActorLength > UINT#96 THEN
    ActorLength := UINT#96;
END_IF;
IF ReasonLength > UINT#96 THEN
    ReasonLength := UINT#96;
END_IF;

TotalLength := UINT#52 + UINT#8 + UINT#8 + OPENOT_ValueSlotEncodedLength(PayloadLength := ActorLength) + OPENOT_ValueSlotEncodedLength(PayloadLength := ReasonLength);
IF HasAuthResult THEN
    TotalLength := TotalLength + UINT#8;
END_IF;

Pos := OPENOT_WriteHeader(
    TotalLength := TotalLength,
    Flags := UINT#16#0008,
    SourceTime := SourceTime,
    RunId := RunId,
    Seq := Seq,
    SourceId := SourceId,
    EventTypeId := UDINT#16#0403,
    Buffer := Buffer);

Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#000D, TypeTag := BYTE#16#05, PayloadLength := BYTE#4, Buffer := Buffer);
Pos := OPENOT_WriteU32LE(Offset := Pos, Value := ValueId, Buffer := Buffer);

ValueBits := DINT_TO_DWORD(PreviousValue);
Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#000F, TypeTag := BYTE#16#06, PayloadLength := BYTE#4, Buffer := Buffer);
Pos := OPENOT_WriteDWordLE(Offset := Pos, Value := ValueBits, Buffer := Buffer);

ValueBits := DINT_TO_DWORD(NewValue);
Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#0010, TypeTag := BYTE#16#06, PayloadLength := BYTE#4, Buffer := Buffer);
Pos := OPENOT_WriteDWordLE(Offset := Pos, Value := ValueBits, Buffer := Buffer);

Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#000B, TypeTag := BYTE#16#0C, PayloadLength := UINT_TO_BYTE(ActorLength), Buffer := Buffer);
Pos := OPENOT_WriteStringPayload(Offset := Pos, Value := Actor, PayloadLength := ActorLength, Buffer := Buffer);

Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#001F, TypeTag := BYTE#16#0C, PayloadLength := UINT_TO_BYTE(ReasonLength), Buffer := Buffer);
Pos := OPENOT_WriteStringPayload(Offset := Pos, Value := Reason, PayloadLength := ReasonLength, Buffer := Buffer);

IF HasAuthResult THEN
    Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#0020, TypeTag := BYTE#16#03, PayloadLength := BYTE#2, Buffer := Buffer);
    Pos := OPENOT_WriteU16LE(Offset := Pos, Value := AuthResult, Buffer := Buffer);
    Pos := OPENOT_WriteByte(Offset := Pos, Value := BYTE#16#00, Buffer := Buffer);
    Pos := OPENOT_WriteByte(Offset := Pos, Value := BYTE#16#00, Buffer := Buffer);
END_IF;

Crc := OPENOT_Crc32c(Data := Buffer, Length := Pos);
Pos := OPENOT_WriteDWordLE(Offset := Pos, Value := Crc, Buffer := Buffer);
Length := Pos;
END_FUNCTION_BLOCK

FUNCTION_BLOCK OPENOT_EncodeParameterChangeBits
VAR_INPUT
    SourceId : UDINT;
    RunId : ULINT;
    Seq : ULINT;
    SourceTime : ULINT;
    ValueId : UDINT;
    TypeTag : BYTE;
    PayloadLength : UINT;
    PreviousBits : ULINT;
    NewBits : ULINT;
    Actor : STRING[96];
    Reason : STRING[96];
    HasAuthResult : BOOL;
    AuthResult : UINT;
END_VAR
VAR_OUTPUT
    Buffer : OPENOT_BYTE_BUFFER;
    Length : UINT;
END_VAR
VAR
    Pos : UINT;
    TotalLength : UINT;
    Crc : DWORD;
    ActorLength : UINT;
    ReasonLength : UINT;
END_VAR

ActorLength := INT_TO_UINT(LEN(Actor));
ReasonLength := INT_TO_UINT(LEN(Reason));
IF ActorLength > UINT#96 THEN
    ActorLength := UINT#96;
END_IF;
IF ReasonLength > UINT#96 THEN
    ReasonLength := UINT#96;
END_IF;

TotalLength := UINT#52 + OPENOT_ValueSlotEncodedLength(PayloadLength := PayloadLength) + OPENOT_ValueSlotEncodedLength(PayloadLength := PayloadLength) + OPENOT_ValueSlotEncodedLength(PayloadLength := ActorLength) + OPENOT_ValueSlotEncodedLength(PayloadLength := ReasonLength);
IF HasAuthResult THEN
    TotalLength := TotalLength + UINT#8;
END_IF;

Pos := OPENOT_WriteHeader(
    TotalLength := TotalLength,
    Flags := UINT#16#0008,
    SourceTime := SourceTime,
    RunId := RunId,
    Seq := Seq,
    SourceId := SourceId,
    EventTypeId := UDINT#16#0403,
    Buffer := Buffer);

Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#000D, TypeTag := BYTE#16#05, PayloadLength := BYTE#4, Buffer := Buffer);
Pos := OPENOT_WriteU32LE(Offset := Pos, Value := ValueId, Buffer := Buffer);

Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#000F, TypeTag := TypeTag, PayloadLength := UINT_TO_BYTE(PayloadLength), Buffer := Buffer);
Pos := OPENOT_WriteValueBitsLE(Offset := Pos, Value := PreviousBits, PayloadLength := PayloadLength, Buffer := Buffer);

Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#0010, TypeTag := TypeTag, PayloadLength := UINT_TO_BYTE(PayloadLength), Buffer := Buffer);
Pos := OPENOT_WriteValueBitsLE(Offset := Pos, Value := NewBits, PayloadLength := PayloadLength, Buffer := Buffer);

Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#000B, TypeTag := BYTE#16#0C, PayloadLength := UINT_TO_BYTE(ActorLength), Buffer := Buffer);
Pos := OPENOT_WriteStringPayload(Offset := Pos, Value := Actor, PayloadLength := ActorLength, Buffer := Buffer);

Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#001F, TypeTag := BYTE#16#0C, PayloadLength := UINT_TO_BYTE(ReasonLength), Buffer := Buffer);
Pos := OPENOT_WriteStringPayload(Offset := Pos, Value := Reason, PayloadLength := ReasonLength, Buffer := Buffer);

IF HasAuthResult THEN
    Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#0020, TypeTag := BYTE#16#03, PayloadLength := BYTE#2, Buffer := Buffer);
    Pos := OPENOT_WriteU16LE(Offset := Pos, Value := AuthResult, Buffer := Buffer);
    Pos := OPENOT_WriteByte(Offset := Pos, Value := BYTE#16#00, Buffer := Buffer);
    Pos := OPENOT_WriteByte(Offset := Pos, Value := BYTE#16#00, Buffer := Buffer);
END_IF;

Crc := OPENOT_Crc32c(Data := Buffer, Length := Pos);
Pos := OPENOT_WriteDWordLE(Offset := Pos, Value := Crc, Buffer := Buffer);
Length := Pos;
END_FUNCTION_BLOCK

FUNCTION_BLOCK OPENOT_EncodeParameterChangeString
VAR_INPUT
    SourceId : UDINT;
    RunId : ULINT;
    Seq : ULINT;
    SourceTime : ULINT;
    ValueId : UDINT;
    PreviousValue : STRING[96];
    NewValue : STRING[96];
    Actor : STRING[96];
    Reason : STRING[96];
    HasAuthResult : BOOL;
    AuthResult : UINT;
END_VAR
VAR_OUTPUT
    Buffer : OPENOT_BYTE_BUFFER;
    Length : UINT;
END_VAR
VAR
    Pos : UINT;
    TotalLength : UINT;
    PreviousLength : UINT;
    NewLength : UINT;
    ActorLength : UINT;
    ReasonLength : UINT;
    Crc : DWORD;
END_VAR

PreviousLength := INT_TO_UINT(LEN(PreviousValue));
NewLength := INT_TO_UINT(LEN(NewValue));
ActorLength := INT_TO_UINT(LEN(Actor));
ReasonLength := INT_TO_UINT(LEN(Reason));
IF PreviousLength > UINT#96 THEN
    PreviousLength := UINT#96;
END_IF;
IF NewLength > UINT#96 THEN
    NewLength := UINT#96;
END_IF;
IF ActorLength > UINT#96 THEN
    ActorLength := UINT#96;
END_IF;
IF ReasonLength > UINT#96 THEN
    ReasonLength := UINT#96;
END_IF;

TotalLength := UINT#52 + OPENOT_ValueSlotEncodedLength(PayloadLength := PreviousLength) + OPENOT_ValueSlotEncodedLength(PayloadLength := NewLength) + OPENOT_ValueSlotEncodedLength(PayloadLength := ActorLength) + OPENOT_ValueSlotEncodedLength(PayloadLength := ReasonLength);
IF HasAuthResult THEN
    TotalLength := TotalLength + UINT#8;
END_IF;

Pos := OPENOT_WriteHeader(
    TotalLength := TotalLength,
    Flags := UINT#16#0008,
    SourceTime := SourceTime,
    RunId := RunId,
    Seq := Seq,
    SourceId := SourceId,
    EventTypeId := UDINT#16#0403,
    Buffer := Buffer);

Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#000D, TypeTag := BYTE#16#05, PayloadLength := BYTE#4, Buffer := Buffer);
Pos := OPENOT_WriteU32LE(Offset := Pos, Value := ValueId, Buffer := Buffer);

Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#000F, TypeTag := BYTE#16#0C, PayloadLength := UINT_TO_BYTE(PreviousLength), Buffer := Buffer);
Pos := OPENOT_WriteStringPayload(Offset := Pos, Value := PreviousValue, PayloadLength := PreviousLength, Buffer := Buffer);

Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#0010, TypeTag := BYTE#16#0C, PayloadLength := UINT_TO_BYTE(NewLength), Buffer := Buffer);
Pos := OPENOT_WriteStringPayload(Offset := Pos, Value := NewValue, PayloadLength := NewLength, Buffer := Buffer);

Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#000B, TypeTag := BYTE#16#0C, PayloadLength := UINT_TO_BYTE(ActorLength), Buffer := Buffer);
Pos := OPENOT_WriteStringPayload(Offset := Pos, Value := Actor, PayloadLength := ActorLength, Buffer := Buffer);

Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#001F, TypeTag := BYTE#16#0C, PayloadLength := UINT_TO_BYTE(ReasonLength), Buffer := Buffer);
Pos := OPENOT_WriteStringPayload(Offset := Pos, Value := Reason, PayloadLength := ReasonLength, Buffer := Buffer);

IF HasAuthResult THEN
    Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#0020, TypeTag := BYTE#16#03, PayloadLength := BYTE#2, Buffer := Buffer);
    Pos := OPENOT_WriteU16LE(Offset := Pos, Value := AuthResult, Buffer := Buffer);
    Pos := OPENOT_WriteByte(Offset := Pos, Value := BYTE#16#00, Buffer := Buffer);
    Pos := OPENOT_WriteByte(Offset := Pos, Value := BYTE#16#00, Buffer := Buffer);
END_IF;

Crc := OPENOT_Crc32c(Data := Buffer, Length := Pos);
Pos := OPENOT_WriteDWordLE(Offset := Pos, Value := Crc, Buffer := Buffer);
Length := Pos;
END_FUNCTION_BLOCK

FUNCTION_BLOCK OPENOT_EncodeConformantConditionAcknowledged
VAR_OUTPUT
    Buffer : OPENOT_BYTE_BUFFER;
    Length : UINT;
END_VAR
VAR
    Encoder : OPENOT_EncodeConditionLifecycle;
END_VAR

Encoder(
    SourceId := UDINT#66,
    RunId := ULINT#1,
    Seq := ULINT#17,
    SourceTime := ULINT#1000000160,
    EventTypeId := UDINT#16#0202,
    ConditionId := UDINT#9001,
    CorrelationId := UDINT#77,
    HasAckBy := TRUE,
    AckBy := 'operator-a',
    HasShelveSecs := FALSE,
    ShelveSecs := UDINT#0,
    HasReason := FALSE,
    Reason := '',
    HasComment := FALSE,
    Comment := '',
    HasPreviousPriority := FALSE,
    PreviousPriority := UINT#0,
    HasNewPriority := FALSE,
    NewPriority := UINT#0,
    HasRefreshId := FALSE,
    RefreshId := UDINT#0);
Buffer := Encoder.Buffer;
Length := Encoder.Length;
END_FUNCTION_BLOCK

FUNCTION_BLOCK OPENOT_EncodeConformantConditionShelved
VAR_OUTPUT
    Buffer : OPENOT_BYTE_BUFFER;
    Length : UINT;
END_VAR
VAR
    Encoder : OPENOT_EncodeConditionLifecycle;
END_VAR

Encoder(
    SourceId := UDINT#66,
    RunId := ULINT#1,
    Seq := ULINT#18,
    SourceTime := ULINT#1000000170,
    EventTypeId := UDINT#16#0204,
    ConditionId := UDINT#9001,
    CorrelationId := UDINT#77,
    HasAckBy := TRUE,
    AckBy := 'operator-a',
    HasShelveSecs := TRUE,
    ShelveSecs := UDINT#300,
    HasReason := FALSE,
    Reason := '',
    HasComment := FALSE,
    Comment := '',
    HasPreviousPriority := FALSE,
    PreviousPriority := UINT#0,
    HasNewPriority := FALSE,
    NewPriority := UINT#0,
    HasRefreshId := FALSE,
    RefreshId := UDINT#0);
Buffer := Encoder.Buffer;
Length := Encoder.Length;
END_FUNCTION_BLOCK

FUNCTION_BLOCK OPENOT_EncodeConformantConditionSuppressed
VAR_OUTPUT
    Buffer : OPENOT_BYTE_BUFFER;
    Length : UINT;
END_VAR
VAR
    Encoder : OPENOT_EncodeConditionLifecycle;
END_VAR

Encoder(
    SourceId := UDINT#66,
    RunId := ULINT#1,
    Seq := ULINT#19,
    SourceTime := ULINT#1000000180,
    EventTypeId := UDINT#16#0206,
    ConditionId := UDINT#9001,
    CorrelationId := UDINT#0,
    HasAckBy := FALSE,
    AckBy := '',
    HasShelveSecs := FALSE,
    ShelveSecs := UDINT#0,
    HasReason := TRUE,
    Reason := 'maintenance',
    HasComment := FALSE,
    Comment := '',
    HasPreviousPriority := FALSE,
    PreviousPriority := UINT#0,
    HasNewPriority := FALSE,
    NewPriority := UINT#0,
    HasRefreshId := FALSE,
    RefreshId := UDINT#0);
Buffer := Encoder.Buffer;
Length := Encoder.Length;
END_FUNCTION_BLOCK

FUNCTION_BLOCK OPENOT_EncodeConformantConditionOutOfService
VAR_OUTPUT
    Buffer : OPENOT_BYTE_BUFFER;
    Length : UINT;
END_VAR
VAR
    Encoder : OPENOT_EncodeConditionLifecycle;
END_VAR

Encoder(
    SourceId := UDINT#66,
    RunId := ULINT#1,
    Seq := ULINT#20,
    SourceTime := ULINT#1000000190,
    EventTypeId := UDINT#16#0208,
    ConditionId := UDINT#9001,
    CorrelationId := UDINT#0,
    HasAckBy := TRUE,
    AckBy := 'operator-a',
    HasShelveSecs := FALSE,
    ShelveSecs := UDINT#0,
    HasReason := FALSE,
    Reason := '',
    HasComment := FALSE,
    Comment := '',
    HasPreviousPriority := FALSE,
    PreviousPriority := UINT#0,
    HasNewPriority := FALSE,
    NewPriority := UINT#0,
    HasRefreshId := FALSE,
    RefreshId := UDINT#0);
Buffer := Encoder.Buffer;
Length := Encoder.Length;
END_FUNCTION_BLOCK

FUNCTION_BLOCK OPENOT_EncodeConformantConditionConfirmed
VAR_OUTPUT
    Buffer : OPENOT_BYTE_BUFFER;
    Length : UINT;
END_VAR
VAR
    Encoder : OPENOT_EncodeConditionLifecycle;
END_VAR

Encoder(
    SourceId := UDINT#66,
    RunId := ULINT#1,
    Seq := ULINT#21,
    SourceTime := ULINT#1000000200,
    EventTypeId := UDINT#16#0203,
    ConditionId := UDINT#9001,
    CorrelationId := UDINT#77,
    HasAckBy := TRUE,
    AckBy := 'operator-a',
    HasShelveSecs := FALSE,
    ShelveSecs := UDINT#0,
    HasReason := FALSE,
    Reason := '',
    HasComment := FALSE,
    Comment := '',
    HasPreviousPriority := FALSE,
    PreviousPriority := UINT#0,
    HasNewPriority := FALSE,
    NewPriority := UINT#0,
    HasRefreshId := FALSE,
    RefreshId := UDINT#0);
Buffer := Encoder.Buffer;
Length := Encoder.Length;
END_FUNCTION_BLOCK

FUNCTION_BLOCK OPENOT_EncodeConformantConditionUnshelved
VAR_OUTPUT
    Buffer : OPENOT_BYTE_BUFFER;
    Length : UINT;
END_VAR
VAR
    Encoder : OPENOT_EncodeConditionLifecycle;
END_VAR

Encoder(
    SourceId := UDINT#66,
    RunId := ULINT#1,
    Seq := ULINT#22,
    SourceTime := ULINT#1000000210,
    EventTypeId := UDINT#16#0205,
    ConditionId := UDINT#9001,
    CorrelationId := UDINT#77,
    HasAckBy := FALSE,
    AckBy := '',
    HasShelveSecs := FALSE,
    ShelveSecs := UDINT#0,
    HasReason := FALSE,
    Reason := '',
    HasComment := FALSE,
    Comment := '',
    HasPreviousPriority := FALSE,
    PreviousPriority := UINT#0,
    HasNewPriority := FALSE,
    NewPriority := UINT#0,
    HasRefreshId := FALSE,
    RefreshId := UDINT#0);
Buffer := Encoder.Buffer;
Length := Encoder.Length;
END_FUNCTION_BLOCK

FUNCTION_BLOCK OPENOT_EncodeConformantConditionUnsuppressed
VAR_OUTPUT
    Buffer : OPENOT_BYTE_BUFFER;
    Length : UINT;
END_VAR
VAR
    Encoder : OPENOT_EncodeConditionLifecycle;
END_VAR

Encoder(
    SourceId := UDINT#66,
    RunId := ULINT#1,
    Seq := ULINT#23,
    SourceTime := ULINT#1000000220,
    EventTypeId := UDINT#16#0207,
    ConditionId := UDINT#9001,
    CorrelationId := UDINT#0,
    HasAckBy := FALSE,
    AckBy := '',
    HasShelveSecs := FALSE,
    ShelveSecs := UDINT#0,
    HasReason := FALSE,
    Reason := '',
    HasComment := FALSE,
    Comment := '',
    HasPreviousPriority := FALSE,
    PreviousPriority := UINT#0,
    HasNewPriority := FALSE,
    NewPriority := UINT#0,
    HasRefreshId := FALSE,
    RefreshId := UDINT#0);
Buffer := Encoder.Buffer;
Length := Encoder.Length;
END_FUNCTION_BLOCK

FUNCTION_BLOCK OPENOT_EncodeConformantConditionInService
VAR_OUTPUT
    Buffer : OPENOT_BYTE_BUFFER;
    Length : UINT;
END_VAR
VAR
    Encoder : OPENOT_EncodeConditionLifecycle;
END_VAR

Encoder(
    SourceId := UDINT#66,
    RunId := ULINT#1,
    Seq := ULINT#24,
    SourceTime := ULINT#1000000230,
    EventTypeId := UDINT#16#0209,
    ConditionId := UDINT#9001,
    CorrelationId := UDINT#0,
    HasAckBy := FALSE,
    AckBy := '',
    HasShelveSecs := FALSE,
    ShelveSecs := UDINT#0,
    HasReason := FALSE,
    Reason := '',
    HasComment := FALSE,
    Comment := '',
    HasPreviousPriority := FALSE,
    PreviousPriority := UINT#0,
    HasNewPriority := FALSE,
    NewPriority := UINT#0,
    HasRefreshId := FALSE,
    RefreshId := UDINT#0);
Buffer := Encoder.Buffer;
Length := Encoder.Length;
END_FUNCTION_BLOCK

FUNCTION_BLOCK OPENOT_EncodeConformantConditionReset
VAR_OUTPUT
    Buffer : OPENOT_BYTE_BUFFER;
    Length : UINT;
END_VAR
VAR
    Encoder : OPENOT_EncodeConditionLifecycle;
END_VAR

Encoder(
    SourceId := UDINT#66,
    RunId := ULINT#1,
    Seq := ULINT#25,
    SourceTime := ULINT#1000000240,
    EventTypeId := UDINT#16#020B,
    ConditionId := UDINT#9001,
    CorrelationId := UDINT#77,
    HasAckBy := TRUE,
    AckBy := 'operator-a',
    HasShelveSecs := FALSE,
    ShelveSecs := UDINT#0,
    HasReason := FALSE,
    Reason := '',
    HasComment := FALSE,
    Comment := '',
    HasPreviousPriority := FALSE,
    PreviousPriority := UINT#0,
    HasNewPriority := FALSE,
    NewPriority := UINT#0,
    HasRefreshId := FALSE,
    RefreshId := UDINT#0);
Buffer := Encoder.Buffer;
Length := Encoder.Length;
END_FUNCTION_BLOCK

FUNCTION_BLOCK OPENOT_EncodeConformantConditionCommented
VAR_OUTPUT
    Buffer : OPENOT_BYTE_BUFFER;
    Length : UINT;
END_VAR
VAR
    Encoder : OPENOT_EncodeConditionLifecycle;
END_VAR

Encoder(
    SourceId := UDINT#66,
    RunId := ULINT#1,
    Seq := ULINT#26,
    SourceTime := ULINT#1000000250,
    EventTypeId := UDINT#16#020A,
    ConditionId := UDINT#9001,
    CorrelationId := UDINT#77,
    HasAckBy := TRUE,
    AckBy := 'operator-a',
    HasShelveSecs := FALSE,
    ShelveSecs := UDINT#0,
    HasReason := FALSE,
    Reason := '',
    HasComment := TRUE,
    Comment := 'operator comment',
    HasPreviousPriority := FALSE,
    PreviousPriority := UINT#0,
    HasNewPriority := FALSE,
    NewPriority := UINT#0,
    HasRefreshId := FALSE,
    RefreshId := UDINT#0);
Buffer := Encoder.Buffer;
Length := Encoder.Length;
END_FUNCTION_BLOCK

FUNCTION_BLOCK OPENOT_EncodeConformantConditionPriorityChanged
VAR_OUTPUT
    Buffer : OPENOT_BYTE_BUFFER;
    Length : UINT;
END_VAR
VAR
    Encoder : OPENOT_EncodeConditionLifecycle;
END_VAR

Encoder(
    SourceId := UDINT#66,
    RunId := ULINT#1,
    Seq := ULINT#27,
    SourceTime := ULINT#1000000260,
    EventTypeId := UDINT#16#020C,
    ConditionId := UDINT#9001,
    CorrelationId := UDINT#0,
    HasAckBy := TRUE,
    AckBy := 'operator-a',
    HasShelveSecs := FALSE,
    ShelveSecs := UDINT#0,
    HasReason := FALSE,
    Reason := '',
    HasComment := FALSE,
    Comment := '',
    HasPreviousPriority := TRUE,
    PreviousPriority := UINT#600,
    HasNewPriority := TRUE,
    NewPriority := UINT#900,
    HasRefreshId := FALSE,
    RefreshId := UDINT#0);
Buffer := Encoder.Buffer;
Length := Encoder.Length;
END_FUNCTION_BLOCK

FUNCTION_BLOCK OPENOT_EncodeConformantRecipeLoaded
VAR_OUTPUT
    Buffer : OPENOT_BYTE_BUFFER;
    Length : UINT;
END_VAR
VAR
    Encoder : OPENOT_EncodeBatchRecipe;
END_VAR

Encoder(
    SourceId := UDINT#66,
    RunId := ULINT#1,
    Seq := ULINT#28,
    SourceTime := ULINT#1000000270,
    EventTypeId := UDINT#16#0301,
    RecipeId := UDINT#3001,
    HasRecipeId := TRUE,
    RecipeVersion := 'v1.2.3',
    HasBatchId := TRUE,
    BatchId := UDINT#4001,
    HasAuthResult := FALSE,
    AuthResult := UINT#0,
    HasAckBy := FALSE,
    AckBy := '',
    NewState := UINT#0,
    MaterialId := UDINT#0,
    Quantity := LREAL#0.0,
    HasUnit := FALSE,
    UnitId := UINT#0);
Buffer := Encoder.Buffer;
Length := Encoder.Length;
END_FUNCTION_BLOCK

FUNCTION_BLOCK OPENOT_EncodeConformantRecipeApproved
VAR_OUTPUT
    Buffer : OPENOT_BYTE_BUFFER;
    Length : UINT;
END_VAR
VAR
    Encoder : OPENOT_EncodeBatchRecipe;
END_VAR

Encoder(
    SourceId := UDINT#66,
    RunId := ULINT#1,
    Seq := ULINT#29,
    SourceTime := ULINT#1000000280,
    EventTypeId := UDINT#16#0302,
    RecipeId := UDINT#3001,
    HasRecipeId := TRUE,
    RecipeVersion := 'v1.2.3',
    HasBatchId := FALSE,
    BatchId := UDINT#0,
    HasAuthResult := TRUE,
    AuthResult := UINT#1,
    HasAckBy := TRUE,
    AckBy := 'approver-a',
    NewState := UINT#0,
    MaterialId := UDINT#0,
    Quantity := LREAL#0.0,
    HasUnit := FALSE,
    UnitId := UINT#0);
Buffer := Encoder.Buffer;
Length := Encoder.Length;
END_FUNCTION_BLOCK

FUNCTION_BLOCK OPENOT_EncodeConformantBatchEvent
VAR_OUTPUT
    Buffer : OPENOT_BYTE_BUFFER;
    Length : UINT;
END_VAR
VAR
    Encoder : OPENOT_EncodeBatchRecipe;
END_VAR

Encoder(
    SourceId := UDINT#66,
    RunId := ULINT#1,
    Seq := ULINT#30,
    SourceTime := ULINT#1000000290,
    EventTypeId := UDINT#16#0303,
    RecipeId := UDINT#3001,
    HasRecipeId := TRUE,
    RecipeVersion := '',
    HasBatchId := TRUE,
    BatchId := UDINT#4001,
    HasAuthResult := FALSE,
    AuthResult := UINT#0,
    HasAckBy := FALSE,
    AckBy := '',
    NewState := UINT#2,
    MaterialId := UDINT#0,
    Quantity := LREAL#0.0,
    HasUnit := FALSE,
    UnitId := UINT#0);
Buffer := Encoder.Buffer;
Length := Encoder.Length;
END_FUNCTION_BLOCK

FUNCTION_BLOCK OPENOT_EncodeConformantMaterialAddition
VAR_OUTPUT
    Buffer : OPENOT_BYTE_BUFFER;
    Length : UINT;
END_VAR
VAR
    Encoder : OPENOT_EncodeBatchRecipe;
END_VAR

Encoder(
    SourceId := UDINT#66,
    RunId := ULINT#1,
    Seq := ULINT#31,
    SourceTime := ULINT#1000000300,
    EventTypeId := UDINT#16#0304,
    RecipeId := UDINT#0,
    HasRecipeId := FALSE,
    RecipeVersion := '',
    HasBatchId := TRUE,
    BatchId := UDINT#4001,
    HasAuthResult := FALSE,
    AuthResult := UINT#0,
    HasAckBy := FALSE,
    AckBy := '',
    NewState := UINT#0,
    MaterialId := UDINT#5001,
    Quantity := LREAL#12.25,
    HasUnit := TRUE,
    UnitId := UINT#8);
Buffer := Encoder.Buffer;
Length := Encoder.Length;
END_FUNCTION_BLOCK

FUNCTION_BLOCK OPENOT_EncodeConformantOperatorAction
VAR_OUTPUT
    Buffer : OPENOT_BYTE_BUFFER;
    Length : UINT;
END_VAR
VAR
    Encoder : OPENOT_EncodeRegulated;
END_VAR

Encoder(
    SourceId := UDINT#66,
    RunId := ULINT#1,
    Seq := ULINT#32,
    SourceTime := ULINT#1000000310,
    EventTypeId := UDINT#16#0400,
    ActionId := UDINT#6001,
    Actor := 'operator-a',
    HasContext1 := TRUE,
    Context1 := UDINT#7001,
    HasContext2 := TRUE,
    Context2 := UDINT#7002,
    HasContext3 := FALSE,
    Context3 := UDINT#0,
    HasContext4 := FALSE,
    Context4 := UDINT#0,
    HasAuthResult := TRUE,
    AuthResult := UINT#0,
    HasWorkstation := TRUE,
    Workstation := 'station-1',
    HasRole := FALSE,
    Role := UINT#0,
    HasReason := FALSE,
    Reason := '');
Buffer := Encoder.Buffer;
Length := Encoder.Length;
END_FUNCTION_BLOCK

FUNCTION_BLOCK OPENOT_EncodeConformantOperatorLogin
VAR_OUTPUT
    Buffer : OPENOT_BYTE_BUFFER;
    Length : UINT;
END_VAR
VAR
    Encoder : OPENOT_EncodeRegulated;
END_VAR

Encoder(
    SourceId := UDINT#66,
    RunId := ULINT#1,
    Seq := ULINT#33,
    SourceTime := ULINT#1000000320,
    EventTypeId := UDINT#16#0401,
    ActionId := UDINT#0,
    Actor := 'operator-a',
    HasContext1 := FALSE,
    Context1 := UDINT#0,
    HasContext2 := FALSE,
    Context2 := UDINT#0,
    HasContext3 := FALSE,
    Context3 := UDINT#0,
    HasContext4 := FALSE,
    Context4 := UDINT#0,
    HasAuthResult := TRUE,
    AuthResult := UINT#0,
    HasWorkstation := TRUE,
    Workstation := 'station-1',
    HasRole := TRUE,
    Role := UINT#3,
    HasReason := FALSE,
    Reason := '');
Buffer := Encoder.Buffer;
Length := Encoder.Length;
END_FUNCTION_BLOCK

FUNCTION_BLOCK OPENOT_EncodeConformantOperatorLogout
VAR_OUTPUT
    Buffer : OPENOT_BYTE_BUFFER;
    Length : UINT;
END_VAR
VAR
    Encoder : OPENOT_EncodeRegulated;
END_VAR

Encoder(
    SourceId := UDINT#66,
    RunId := ULINT#1,
    Seq := ULINT#34,
    SourceTime := ULINT#1000000330,
    EventTypeId := UDINT#16#0402,
    ActionId := UDINT#0,
    Actor := 'operator-a',
    HasContext1 := FALSE,
    Context1 := UDINT#0,
    HasContext2 := FALSE,
    Context2 := UDINT#0,
    HasContext3 := FALSE,
    Context3 := UDINT#0,
    HasContext4 := FALSE,
    Context4 := UDINT#0,
    HasAuthResult := FALSE,
    AuthResult := UINT#0,
    HasWorkstation := TRUE,
    Workstation := 'station-1',
    HasRole := FALSE,
    Role := UINT#0,
    HasReason := FALSE,
    Reason := '');
Buffer := Encoder.Buffer;
Length := Encoder.Length;
END_FUNCTION_BLOCK

FUNCTION_BLOCK OPENOT_EncodeConformantSecurityAccessFailure
VAR_OUTPUT
    Buffer : OPENOT_BYTE_BUFFER;
    Length : UINT;
END_VAR
VAR
    Encoder : OPENOT_EncodeRegulated;
END_VAR

Encoder(
    SourceId := UDINT#66,
    RunId := ULINT#1,
    Seq := ULINT#35,
    SourceTime := ULINT#1000000340,
    EventTypeId := UDINT#16#0405,
    ActionId := UDINT#0,
    Actor := 'operator-x',
    HasContext1 := FALSE,
    Context1 := UDINT#0,
    HasContext2 := FALSE,
    Context2 := UDINT#0,
    HasContext3 := FALSE,
    Context3 := UDINT#0,
    HasContext4 := FALSE,
    Context4 := UDINT#0,
    HasAuthResult := FALSE,
    AuthResult := UINT#0,
    HasWorkstation := TRUE,
    Workstation := 'station-2',
    HasRole := FALSE,
    Role := UINT#0,
    HasReason := TRUE,
    Reason := 'denied');
Buffer := Encoder.Buffer;
Length := Encoder.Length;
END_FUNCTION_BLOCK

FUNCTION_BLOCK OPENOT_EncodeConformantParameterChangeReal
VAR_OUTPUT
    Buffer : OPENOT_BYTE_BUFFER;
    Length : UINT;
END_VAR
VAR
    Encoder : OPENOT_EncodeParameterChangeReal;
END_VAR

Encoder(
    SourceId := UDINT#66,
    RunId := ULINT#1,
    Seq := ULINT#36,
    SourceTime := ULINT#1000000350,
    ValueId := UDINT#2001,
    PreviousValue := REAL#12.5,
    NewValue := REAL#13.75,
    Actor := 'operator-a',
    Reason := 'setpoint change',
    HasAuthResult := TRUE,
    AuthResult := UINT#0);
Buffer := Encoder.Buffer;
Length := Encoder.Length;
END_FUNCTION_BLOCK

FUNCTION_BLOCK OPENOT_EncodeConformantParameterChangeInt
VAR_OUTPUT
    Buffer : OPENOT_BYTE_BUFFER;
    Length : UINT;
END_VAR
VAR
    Encoder : OPENOT_EncodeParameterChangeInt;
END_VAR

Encoder(
    SourceId := UDINT#66,
    RunId := ULINT#1,
    Seq := ULINT#37,
    SourceTime := ULINT#1000000360,
    ValueId := UDINT#2002,
    PreviousValue := DINT#40,
    NewValue := DINT#42,
    Actor := 'operator-a',
    Reason := 'count corrected',
    HasAuthResult := FALSE,
    AuthResult := UINT#0);
Buffer := Encoder.Buffer;
Length := Encoder.Length;
END_FUNCTION_BLOCK

FUNCTION_BLOCK OPENOT_EncodeConformantParameterChangeBits
VAR_OUTPUT
    Buffer : OPENOT_BYTE_BUFFER;
    Length : UINT;
END_VAR
VAR
    Encoder : OPENOT_EncodeParameterChangeBits;
END_VAR

Encoder(
    SourceId := UDINT#66,
    RunId := ULINT#1,
    Seq := ULINT#38,
    SourceTime := ULINT#1000000370,
    ValueId := UDINT#2003,
    TypeTag := BYTE#16#00,
    PayloadLength := UINT#1,
    PreviousBits := ULINT#0,
    NewBits := ULINT#1,
    Actor := 'operator-b',
    Reason := 'enable audit',
    HasAuthResult := FALSE,
    AuthResult := UINT#0);
Buffer := Encoder.Buffer;
Length := Encoder.Length;
END_FUNCTION_BLOCK

FUNCTION_BLOCK OPENOT_EncodeConformantParameterChangeString
VAR_OUTPUT
    Buffer : OPENOT_BYTE_BUFFER;
    Length : UINT;
END_VAR
VAR
    Encoder : OPENOT_EncodeParameterChangeString;
END_VAR

Encoder(
    SourceId := UDINT#66,
    RunId := ULINT#1,
    Seq := ULINT#39,
    SourceTime := ULINT#1000000380,
    ValueId := UDINT#2012,
    PreviousValue := 'manual',
    NewValue := 'auto',
    Actor := 'operator-c',
    Reason := 'mode note',
    HasAuthResult := TRUE,
    AuthResult := UINT#1);
Buffer := Encoder.Buffer;
Length := Encoder.Length;
END_FUNCTION_BLOCK

FUNCTION_BLOCK OPENOT_EncodeConformantESignature
VAR_OUTPUT
    Buffer : OPENOT_BYTE_BUFFER;
    Length : UINT;
END_VAR
VAR
    Encoder : OPENOT_EncodeESignature;
END_VAR

Encoder(
    SourceId := UDINT#66,
    RunId := ULINT#1,
    Seq := ULINT#40,
    SourceTime := ULINT#1000000390,
    ActionId := UDINT#6002,
    Actor := 'signer-a',
    SignatureMeaning := UINT#2,
    SignedEventSeq := ULINT#37,
    HasAuthResult := TRUE,
    AuthResult := UINT#0);
Buffer := Encoder.Buffer;
Length := Encoder.Length;
END_FUNCTION_BLOCK

FUNCTION_BLOCK OPENOT_EncodeValueChangedInt
VAR_INPUT
    SourceId : UDINT;
    RunId : ULINT;
    Seq : ULINT;
    SourceTime : ULINT;
    ValueId : UDINT;
    NewValue : DINT;
    HasPrevious : BOOL;
    PreviousValue : DINT;
    HasQuality : BOOL;
    Quality : UINT;
END_VAR
VAR_OUTPUT
    Buffer : OPENOT_BYTE_BUFFER;
    Length : UINT;
END_VAR
VAR
    Pos : UINT;
    TotalLength : UINT;
    Crc : DWORD;
    ValueBits : DWORD;
END_VAR

TotalLength := UINT#60;
IF HasPrevious THEN
    TotalLength := TotalLength + UINT#8;
END_IF;
IF HasQuality THEN
    TotalLength := TotalLength + UINT#8;
END_IF;

Pos := OPENOT_WriteHeader(
    TotalLength := TotalLength,
    Flags := UINT#16#0008,
    SourceTime := SourceTime,
    RunId := RunId,
    Seq := Seq,
    SourceId := SourceId,
    EventTypeId := UDINT#16#0002,
    Buffer := Buffer);

Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#000D, TypeTag := BYTE#16#05, PayloadLength := BYTE#4, Buffer := Buffer);
Pos := OPENOT_WriteU32LE(Offset := Pos, Value := ValueId, Buffer := Buffer);

ValueBits := DINT_TO_DWORD(NewValue);
Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#0010, TypeTag := BYTE#16#06, PayloadLength := BYTE#4, Buffer := Buffer);
Pos := OPENOT_WriteDWordLE(Offset := Pos, Value := ValueBits, Buffer := Buffer);

IF HasPrevious THEN
    ValueBits := DINT_TO_DWORD(PreviousValue);
    Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#000F, TypeTag := BYTE#16#06, PayloadLength := BYTE#4, Buffer := Buffer);
    Pos := OPENOT_WriteDWordLE(Offset := Pos, Value := ValueBits, Buffer := Buffer);
END_IF;

IF HasQuality THEN
    Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#0011, TypeTag := BYTE#16#03, PayloadLength := BYTE#2, Buffer := Buffer);
    Pos := OPENOT_WriteU16LE(Offset := Pos, Value := Quality, Buffer := Buffer);
    Pos := OPENOT_WriteByte(Offset := Pos, Value := BYTE#16#00, Buffer := Buffer);
    Pos := OPENOT_WriteByte(Offset := Pos, Value := BYTE#16#00, Buffer := Buffer);
END_IF;

Crc := OPENOT_Crc32c(Data := Buffer, Length := Pos);
Pos := OPENOT_WriteDWordLE(Offset := Pos, Value := Crc, Buffer := Buffer);
Length := Pos;
END_FUNCTION_BLOCK

FUNCTION OPENOT_ValueSlotEncodedLength : UINT
VAR_INPUT
    PayloadLength : UINT;
END_VAR
VAR
    Padding : UINT;
END_VAR

Padding := UINT#0;
IF (PayloadLength MOD UINT#4) <> UINT#0 THEN
    Padding := UINT#4 - (PayloadLength MOD UINT#4);
END_IF;

OPENOT_ValueSlotEncodedLength := UINT#4 + PayloadLength + Padding;
END_FUNCTION

FUNCTION OPENOT_WriteValueBitsLE : UINT
VAR_INPUT
    Offset : UINT;
    Value : ULINT;
    PayloadLength : UINT;
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
FOR Index := UINT#0 TO PayloadLength - UINT#1 DO
    Pos := OPENOT_WriteByte(Offset := Pos, Value := ULINT_TO_BYTE(Remaining MOD ULINT#256), Buffer := Buffer);
    Remaining := Remaining / ULINT#256;
END_FOR;
WHILE (Pos MOD UINT#4) <> UINT#0 DO
    Pos := OPENOT_WriteByte(Offset := Pos, Value := BYTE#16#00, Buffer := Buffer);
END_WHILE;
OPENOT_WriteValueBitsLE := Pos;
END_FUNCTION

FUNCTION_BLOCK OPENOT_EncodeValueChangedBits
VAR_INPUT
    SourceId : UDINT;
    RunId : ULINT;
    Seq : ULINT;
    SourceTime : ULINT;
    ValueId : UDINT;
    TypeTag : BYTE;
    PayloadLength : UINT;
    NewBits : ULINT;
    HasPrevious : BOOL;
    PreviousBits : ULINT;
    HasQuality : BOOL;
    Quality : UINT;
END_VAR
VAR_OUTPUT
    Buffer : OPENOT_BYTE_BUFFER;
    Length : UINT;
END_VAR
VAR
    Pos : UINT;
    TotalLength : UINT;
    Crc : DWORD;
END_VAR

TotalLength := UINT#40 + UINT#8 + OPENOT_ValueSlotEncodedLength(PayloadLength := PayloadLength) + UINT#4;
IF HasPrevious THEN
    TotalLength := TotalLength + OPENOT_ValueSlotEncodedLength(PayloadLength := PayloadLength);
END_IF;
IF HasQuality THEN
    TotalLength := TotalLength + UINT#8;
END_IF;

Pos := OPENOT_WriteHeader(
    TotalLength := TotalLength,
    Flags := UINT#16#0008,
    SourceTime := SourceTime,
    RunId := RunId,
    Seq := Seq,
    SourceId := SourceId,
    EventTypeId := UDINT#16#0002,
    Buffer := Buffer);

Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#000D, TypeTag := BYTE#16#05, PayloadLength := BYTE#4, Buffer := Buffer);
Pos := OPENOT_WriteU32LE(Offset := Pos, Value := ValueId, Buffer := Buffer);

Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#0010, TypeTag := TypeTag, PayloadLength := UINT_TO_BYTE(PayloadLength), Buffer := Buffer);
Pos := OPENOT_WriteValueBitsLE(Offset := Pos, Value := NewBits, PayloadLength := PayloadLength, Buffer := Buffer);

IF HasPrevious THEN
    Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#000F, TypeTag := TypeTag, PayloadLength := UINT_TO_BYTE(PayloadLength), Buffer := Buffer);
    Pos := OPENOT_WriteValueBitsLE(Offset := Pos, Value := PreviousBits, PayloadLength := PayloadLength, Buffer := Buffer);
END_IF;

IF HasQuality THEN
    Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#0011, TypeTag := BYTE#16#03, PayloadLength := BYTE#2, Buffer := Buffer);
    Pos := OPENOT_WriteU16LE(Offset := Pos, Value := Quality, Buffer := Buffer);
    Pos := OPENOT_WriteByte(Offset := Pos, Value := BYTE#16#00, Buffer := Buffer);
    Pos := OPENOT_WriteByte(Offset := Pos, Value := BYTE#16#00, Buffer := Buffer);
END_IF;

Crc := OPENOT_Crc32c(Data := Buffer, Length := Pos);
Pos := OPENOT_WriteDWordLE(Offset := Pos, Value := Crc, Buffer := Buffer);
Length := Pos;
END_FUNCTION_BLOCK

FUNCTION OPENOT_WriteStringPayload : UINT
VAR_INPUT
    Offset : UINT;
    Value : STRING[96];
    PayloadLength : UINT;
END_VAR
VAR_IN_OUT
    Buffer : OPENOT_BYTE_BUFFER;
END_VAR
VAR
    Index : UINT;
    Pos : UINT;
    Slice : STRING[1];
END_VAR

Pos := Offset;
IF PayloadLength > UINT#0 THEN
    FOR Index := UINT#0 TO PayloadLength - UINT#1 DO
        Slice := MID(IN := Value, L := INT#1, P := UINT_TO_INT(Index + UINT#1));
        Pos := OPENOT_WriteByte(Offset := Pos, Value := CHAR_TO_BYTE(STRING_TO_CHAR(Slice)), Buffer := Buffer);
    END_FOR;
END_IF;
WHILE (Pos MOD UINT#4) <> UINT#0 DO
    Pos := OPENOT_WriteByte(Offset := Pos, Value := BYTE#16#00, Buffer := Buffer);
END_WHILE;
OPENOT_WriteStringPayload := Pos;
END_FUNCTION

FUNCTION_BLOCK OPENOT_EncodeValueChangedString
VAR_INPUT
    SourceId : UDINT;
    RunId : ULINT;
    Seq : ULINT;
    SourceTime : ULINT;
    ValueId : UDINT;
    NewValue : STRING[96];
    HasPrevious : BOOL;
    PreviousValue : STRING[96];
    HasQuality : BOOL;
    Quality : UINT;
END_VAR
VAR_OUTPUT
    Buffer : OPENOT_BYTE_BUFFER;
    Length : UINT;
END_VAR
VAR
    Pos : UINT;
    TotalLength : UINT;
    NewLength : UINT;
    PreviousLength : UINT;
    Crc : DWORD;
END_VAR

NewLength := INT_TO_UINT(LEN(NewValue));
PreviousLength := INT_TO_UINT(LEN(PreviousValue));
IF NewLength > UINT#96 THEN
    NewLength := UINT#96;
END_IF;
IF PreviousLength > UINT#96 THEN
    PreviousLength := UINT#96;
END_IF;

TotalLength := UINT#40 + UINT#8 + OPENOT_ValueSlotEncodedLength(PayloadLength := NewLength) + UINT#4;
IF HasPrevious THEN
    TotalLength := TotalLength + OPENOT_ValueSlotEncodedLength(PayloadLength := PreviousLength);
END_IF;
IF HasQuality THEN
    TotalLength := TotalLength + UINT#8;
END_IF;

Pos := OPENOT_WriteHeader(
    TotalLength := TotalLength,
    Flags := UINT#16#0008,
    SourceTime := SourceTime,
    RunId := RunId,
    Seq := Seq,
    SourceId := SourceId,
    EventTypeId := UDINT#16#0002,
    Buffer := Buffer);

Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#000D, TypeTag := BYTE#16#05, PayloadLength := BYTE#4, Buffer := Buffer);
Pos := OPENOT_WriteU32LE(Offset := Pos, Value := ValueId, Buffer := Buffer);

Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#0010, TypeTag := BYTE#16#0C, PayloadLength := UINT_TO_BYTE(NewLength), Buffer := Buffer);
Pos := OPENOT_WriteStringPayload(Offset := Pos, Value := NewValue, PayloadLength := NewLength, Buffer := Buffer);

IF HasPrevious THEN
    Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#000F, TypeTag := BYTE#16#0C, PayloadLength := UINT_TO_BYTE(PreviousLength), Buffer := Buffer);
    Pos := OPENOT_WriteStringPayload(Offset := Pos, Value := PreviousValue, PayloadLength := PreviousLength, Buffer := Buffer);
END_IF;

IF HasQuality THEN
    Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#0011, TypeTag := BYTE#16#03, PayloadLength := BYTE#2, Buffer := Buffer);
    Pos := OPENOT_WriteU16LE(Offset := Pos, Value := Quality, Buffer := Buffer);
    Pos := OPENOT_WriteByte(Offset := Pos, Value := BYTE#16#00, Buffer := Buffer);
    Pos := OPENOT_WriteByte(Offset := Pos, Value := BYTE#16#00, Buffer := Buffer);
END_IF;

Crc := OPENOT_Crc32c(Data := Buffer, Length := Pos);
Pos := OPENOT_WriteDWordLE(Offset := Pos, Value := Crc, Buffer := Buffer);
Length := Pos;
END_FUNCTION_BLOCK

FUNCTION_BLOCK OPENOT_EncodeStateTransition
VAR_INPUT
    SourceId : UDINT;
    RunId : ULINT;
    Seq : ULINT;
    SourceTime : ULINT;
    StateMachineId : UDINT;
    Category : UINT;
    PreviousState : UINT;
    NewState : UINT;
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
    TotalLength := UINT#76,
    Flags := UINT#16#0008,
    SourceTime := SourceTime,
    RunId := RunId,
    Seq := Seq,
    SourceId := SourceId,
    EventTypeId := UDINT#16#0001,
    Buffer := Buffer);

Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#0001, TypeTag := BYTE#16#05, PayloadLength := BYTE#4, Buffer := Buffer);
Pos := OPENOT_WriteU32LE(Offset := Pos, Value := StateMachineId, Buffer := Buffer);
Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#0002, TypeTag := BYTE#16#03, PayloadLength := BYTE#2, Buffer := Buffer);
Pos := OPENOT_WriteU16LE(Offset := Pos, Value := Category, Buffer := Buffer);
Pos := OPENOT_WriteByte(Offset := Pos, Value := BYTE#16#00, Buffer := Buffer);
Pos := OPENOT_WriteByte(Offset := Pos, Value := BYTE#16#00, Buffer := Buffer);
Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#0003, TypeTag := BYTE#16#03, PayloadLength := BYTE#2, Buffer := Buffer);
Pos := OPENOT_WriteU16LE(Offset := Pos, Value := PreviousState, Buffer := Buffer);
Pos := OPENOT_WriteByte(Offset := Pos, Value := BYTE#16#00, Buffer := Buffer);
Pos := OPENOT_WriteByte(Offset := Pos, Value := BYTE#16#00, Buffer := Buffer);
Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#0004, TypeTag := BYTE#16#03, PayloadLength := BYTE#2, Buffer := Buffer);
Pos := OPENOT_WriteU16LE(Offset := Pos, Value := NewState, Buffer := Buffer);
Pos := OPENOT_WriteByte(Offset := Pos, Value := BYTE#16#00, Buffer := Buffer);
Pos := OPENOT_WriteByte(Offset := Pos, Value := BYTE#16#00, Buffer := Buffer);
Crc := OPENOT_Crc32c(Data := Buffer, Length := Pos);
Pos := OPENOT_WriteDWordLE(Offset := Pos, Value := Crc, Buffer := Buffer);
Length := Pos;
END_FUNCTION_BLOCK

FUNCTION_BLOCK OPENOT_EncodeCondition
VAR_INPUT
    SourceId : UDINT;
    RunId : ULINT;
    Seq : ULINT;
    SourceTime : ULINT;
    EventTypeId : UDINT;
    ConditionId : UDINT;
    ConditionClass : UINT;
    Severity : UINT;
    HasCorrelationId : BOOL;
    CorrelationId : UDINT;
    HasCauseOperand : BOOL;
    CauseOperand : UDINT;
END_VAR
VAR_OUTPUT
    Buffer : OPENOT_BYTE_BUFFER;
    Length : UINT;
END_VAR
VAR
    Pos : UINT;
    Crc : DWORD;
    TotalLength : UINT;
END_VAR

TotalLength := UINT#60;
IF HasCorrelationId THEN
    TotalLength := TotalLength + UINT#8;
END_IF;
IF EventTypeId = UDINT#16#0200 THEN
    TotalLength := TotalLength + UINT#8;
END_IF;
IF (EventTypeId = UDINT#16#0200) AND HasCauseOperand THEN
    TotalLength := TotalLength + UINT#8;
END_IF;

Pos := OPENOT_WriteHeader(
    TotalLength := TotalLength,
    Flags := UINT#16#0008,
    SourceTime := SourceTime,
    RunId := RunId,
    Seq := Seq,
    SourceId := SourceId,
    EventTypeId := EventTypeId,
    Buffer := Buffer);

Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#0005, TypeTag := BYTE#16#05, PayloadLength := BYTE#4, Buffer := Buffer);
Pos := OPENOT_WriteU32LE(Offset := Pos, Value := ConditionId, Buffer := Buffer);
Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#0006, TypeTag := BYTE#16#03, PayloadLength := BYTE#2, Buffer := Buffer);
Pos := OPENOT_WriteU16LE(Offset := Pos, Value := ConditionClass, Buffer := Buffer);
Pos := OPENOT_WriteByte(Offset := Pos, Value := BYTE#16#00, Buffer := Buffer);
Pos := OPENOT_WriteByte(Offset := Pos, Value := BYTE#16#00, Buffer := Buffer);
IF HasCorrelationId THEN
    Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#0007, TypeTag := BYTE#16#05, PayloadLength := BYTE#4, Buffer := Buffer);
    Pos := OPENOT_WriteU32LE(Offset := Pos, Value := CorrelationId, Buffer := Buffer);
END_IF;
IF EventTypeId = UDINT#16#0200 THEN
    Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#0008, TypeTag := BYTE#16#03, PayloadLength := BYTE#2, Buffer := Buffer);
    Pos := OPENOT_WriteU16LE(Offset := Pos, Value := Severity, Buffer := Buffer);
    Pos := OPENOT_WriteByte(Offset := Pos, Value := BYTE#16#00, Buffer := Buffer);
    Pos := OPENOT_WriteByte(Offset := Pos, Value := BYTE#16#00, Buffer := Buffer);
END_IF;
IF (EventTypeId = UDINT#16#0200) AND HasCauseOperand THEN
    Pos := OPENOT_WriteSlotHeader(Offset := Pos, Key := UINT#16#0009, TypeTag := BYTE#16#05, PayloadLength := BYTE#4, Buffer := Buffer);
    Pos := OPENOT_WriteU32LE(Offset := Pos, Value := CauseOperand, Buffer := Buffer);
END_IF;
Crc := OPENOT_Crc32c(Data := Buffer, Length := Pos);
Pos := OPENOT_WriteDWordLE(Offset := Pos, Value := Crc, Buffer := Buffer);
Length := Pos;
END_FUNCTION_BLOCK
