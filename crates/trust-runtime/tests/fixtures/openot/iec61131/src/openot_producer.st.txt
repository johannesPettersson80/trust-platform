TYPE
    OPENOT_SOURCE_ENTRY : STRUCT
        Used : BOOL;
        SourceId : UDINT;
        NextSeq : ULINT;
    END_STRUCT;

    OPENOT_SOURCE_USED_TABLE : ARRAY[0..15] OF BOOL;
    OPENOT_SOURCE_ID_TABLE : ARRAY[0..15] OF UDINT;
    OPENOT_SOURCE_NEXT_SEQ_TABLE : ARRAY[0..15] OF ULINT;
    OPENOT_SCAN_RECORD_BUFFER : ARRAY[0..1023] OF BYTE;
    OPENOT_SCAN_RECORD_UINT_TABLE : ARRAY[0..16] OF UINT;

    OPENOT_REAL_VALUE_STATE : STRUCT
        Used : BOOL;
        ValueId : UDINT;
        HasLast : BOOL;
        LastValue : REAL;
        HasLastEmitSourceTime : BOOL;
        LastEmitSourceTime : ULINT;
        HasBandCenter : BOOL;
        BandCenter : REAL;
    END_STRUCT;

    OPENOT_INT_VALUE_STATE : STRUCT
        Used : BOOL;
        ValueId : UDINT;
        HasLast : BOOL;
        LastValue : DINT;
        HasLastEmitSourceTime : BOOL;
        LastEmitSourceTime : ULINT;
    END_STRUCT;

    OPENOT_BITS_VALUE_STATE : STRUCT
        Used : BOOL;
        ValueId : UDINT;
        HasLast : BOOL;
        LastBits : ULINT;
        HasLastEmitSourceTime : BOOL;
        LastEmitSourceTime : ULINT;
    END_STRUCT;

    OPENOT_STRING_VALUE_STATE : STRUCT
        Used : BOOL;
        ValueId : UDINT;
        HasLast : BOOL;
        LastValue : STRING[96];
        HasLastEmitSourceTime : BOOL;
        LastEmitSourceTime : ULINT;
    END_STRUCT;

    OPENOT_STATE_MACHINE_STATE : STRUCT
        Used : BOOL;
        StateMachineId : UDINT;
        HasLast : BOOL;
        LastState : UINT;
    END_STRUCT;

    OPENOT_CONDITION_STATE : STRUCT
        Used : BOOL;
        SourceId : UDINT;
        ConditionId : UDINT;
        HasLast : BOOL;
        LastActive : BOOL;
        CorrelationId : UDINT;
    END_STRUCT;

    OPENOT_EMISSION_ENTRY : STRUCT
        EventTypeId : UDINT;
        RunId : ULINT;
        SourceId : UDINT;
        Seq : ULINT;
    END_STRUCT;

    OPENOT_EMISSION_LOG : ARRAY[0..31] OF OPENOT_EMISSION_ENTRY;

    OPENOT_ATTESTABLE_EVENT_STATE : STRUCT
        Used : BOOL;
        AttestableId : UDINT;
        SourceId : UDINT;
        HasSeq : BOOL;
        LastSeq : ULINT;
    END_STRUCT;

    OPENOT_ATTESTABLE_EVENT_LOG : ARRAY[0..31] OF OPENOT_ATTESTABLE_EVENT_STATE;
END_TYPE

FUNCTION OPENOT_AppendScanRecord : BOOL
VAR_INPUT
    Record : OPENOT_BYTE_BUFFER;
    Length : UINT;
END_VAR
VAR_IN_OUT
    ScanRecords : OPENOT_SCAN_RECORD_BUFFER;
    ScanRecordOffsets : OPENOT_SCAN_RECORD_UINT_TABLE;
    ScanRecordLengths : OPENOT_SCAN_RECORD_UINT_TABLE;
    ScanWritePos : UINT;
    ScanRecordCount : UINT;
END_VAR
VAR
    CopyIndex : UINT;
    DestIndex : UINT;
END_VAR

IF (Length = UINT#0) OR (Length > UINT#256) OR (ScanRecordCount > UINT#16) OR ((ScanWritePos + Length) > UINT#1024) THEN
    OPENOT_AppendScanRecord := FALSE;
ELSE
    ScanRecordOffsets[ScanRecordCount] := ScanWritePos;
    ScanRecordLengths[ScanRecordCount] := Length;

    FOR CopyIndex := UINT#0 TO Length - UINT#1 DO
        DestIndex := ScanWritePos + CopyIndex;
        ScanRecords[DestIndex] := Record[CopyIndex];
    END_FOR;

    ScanWritePos := ScanWritePos + Length;
    ScanRecordCount := ScanRecordCount + UINT#1;
    OPENOT_AppendScanRecord := TRUE;
END_IF;
END_FUNCTION

FUNCTION OPENOT_ResetScanRecordList : BOOL
VAR_IN_OUT
    ScanRecordOffsets : OPENOT_SCAN_RECORD_UINT_TABLE;
    ScanRecordLengths : OPENOT_SCAN_RECORD_UINT_TABLE;
    ScanWritePos : UINT;
    ScanRecordCount : UINT;
END_VAR
VAR
    Index : UINT;
END_VAR

ScanRecordCount := UINT#0;
ScanWritePos := UINT#0;
FOR Index := UINT#0 TO UINT#16 DO
    ScanRecordOffsets[Index] := UINT#0;
    ScanRecordLengths[Index] := UINT#0;
END_FOR;

OPENOT_ResetScanRecordList := TRUE;
END_FUNCTION

FUNCTION OPENOT_RecordAttestableSeq : BOOL
VAR_INPUT
    AttestableId : UDINT;
    SourceId : UDINT;
    Seq : ULINT;
END_VAR
VAR_IN_OUT
    AttestableEvents : OPENOT_ATTESTABLE_EVENT_LOG;
END_VAR
VAR
    Index : UINT;
    EntryIndex : UINT;
    Found : BOOL;
    FreeFound : BOOL;
END_VAR

OPENOT_RecordAttestableSeq := FALSE;
IF (AttestableId > UDINT#0) AND (AttestableId <= UDINT#32) THEN
    Found := FALSE;
    FreeFound := FALSE;
    EntryIndex := UINT#0;

    FOR Index := UINT#0 TO UINT#31 DO
        IF AttestableEvents[Index].Used AND (AttestableEvents[Index].AttestableId = AttestableId) THEN
            IF NOT Found THEN
                EntryIndex := Index;
                Found := TRUE;
            END_IF;
        ELSIF (NOT Found) AND (NOT AttestableEvents[Index].Used) AND (NOT FreeFound) THEN
            EntryIndex := Index;
            FreeFound := TRUE;
        END_IF;
    END_FOR;

    IF Found OR FreeFound THEN
        AttestableEvents[EntryIndex].Used := TRUE;
        AttestableEvents[EntryIndex].AttestableId := AttestableId;
        AttestableEvents[EntryIndex].SourceId := SourceId;
        AttestableEvents[EntryIndex].HasSeq := TRUE;
        AttestableEvents[EntryIndex].LastSeq := Seq;
        OPENOT_RecordAttestableSeq := TRUE;
    END_IF;
END_IF;
END_FUNCTION

FUNCTION OPENOT_NextSourceTime : ULINT
VAR_INPUT
    UseSourceTimeInput : BOOL;
    SourceTimeInput : ULINT;
END_VAR
VAR_IN_OUT
    StateSourceTime : ULINT;
END_VAR

IF UseSourceTimeInput THEN
    OPENOT_NextSourceTime := SourceTimeInput;
ELSE
    OPENOT_NextSourceTime := StateSourceTime;
    StateSourceTime := StateSourceTime + ULINT#1;
END_IF;
END_FUNCTION

FUNCTION_BLOCK OPENOT_Producer
VAR_INPUT
    Execute : BOOL;
    Op : UINT;
    SourceId : UDINT;
    Checkpoint : BOOL;
    DefHashOld : OPENOT_HASH8;
    DefHashNew : OPENOT_HASH8;
    TransitionEpochId : ULINT;
    UseSourceTimeInput : BOOL;
    SourceTimeInput : ULINT;
    StageRecord : OPENOT_BYTE_BUFFER;
    StageLength : UINT;
    StageSeq : ULINT;
    StageEventTypeId : UDINT;
    ResetScanRecords : BOOL;
    AccumulateScanRecords : BOOL;
    ValueId : UDINT;
    ValueReal : REAL;
    ValueInt : DINT;
    ValueBits : ULINT;
    ValueTypeTag : BYTE;
    ValuePayloadLength : UINT;
    ValueString : STRING[96];
    DeadbandReal : REAL;
    SamplingMode : UINT;
    SamplingIntervalMs : ULINT;
    SuppressPrevious : BOOL;
    HasQuality : BOOL;
    Quality : UINT;
    StateMachineId : UDINT;
    Category : UINT;
    PreviousState : UINT;
    NewState : UINT;
    ConditionId : UDINT;
    ConditionClass : UINT;
    Severity : UINT;
    ConditionActive : BOOL;
    HasCauseOperand : BOOL;
    CauseOperand : UDINT;
    ConditionLifecycleEventTypeId : UDINT;
    LifecycleHasAckBy : BOOL;
    LifecycleAckBy : STRING[96];
    LifecycleHasShelveSecs : BOOL;
    LifecycleShelveSecs : UDINT;
    LifecycleHasReason : BOOL;
    LifecycleReason : STRING[96];
    LifecycleHasComment : BOOL;
    LifecycleComment : STRING[96];
    LifecycleHasPreviousPriority : BOOL;
    LifecyclePreviousPriority : UINT;
    LifecycleHasNewPriority : BOOL;
    LifecycleNewPriority : UINT;
    ProcedureEventTypeId : UDINT;
    ProcedureRecipeId : UDINT;
    ProcedureHasRecipeId : BOOL;
    ProcedureRecipeVersion : STRING[96];
    ProcedureHasBatchId : BOOL;
    ProcedureBatchId : UDINT;
    ProcedureHasAuthResult : BOOL;
    ProcedureAuthResult : UINT;
    ProcedureHasAckBy : BOOL;
    ProcedureAckBy : STRING[96];
    ProcedureNewState : UINT;
    ProcedureMaterialId : UDINT;
    ProcedureQuantity : LREAL;
    ProcedureHasUnit : BOOL;
    ProcedureUnitId : UINT;
    RegulatedEventTypeId : UDINT;
    RegulatedActionId : UDINT;
    RegulatedActor : STRING[96];
    RegulatedHasContext1 : BOOL;
    RegulatedContext1 : UDINT;
    RegulatedHasContext2 : BOOL;
    RegulatedContext2 : UDINT;
    RegulatedHasContext3 : BOOL;
    RegulatedContext3 : UDINT;
    RegulatedHasContext4 : BOOL;
    RegulatedContext4 : UDINT;
    RegulatedHasAuthResult : BOOL;
    RegulatedAuthResult : UINT;
    RegulatedHasWorkstation : BOOL;
    RegulatedWorkstation : STRING[96];
    RegulatedHasRole : BOOL;
    RegulatedRole : UINT;
    RegulatedHasReason : BOOL;
    RegulatedReason : STRING[96];
    AuditActor : STRING[96];
    AuditReason : STRING[96];
    AuditHasAuthResult : BOOL;
    AuditAuthResult : UINT;
    MessageTemplateId : UDINT;
    MessageHasSeverity : BOOL;
    MessageSeverity : UINT;
    MessageArgCount : UINT;
    MessageArg1TypeTag : BYTE;
    MessageArg1PayloadLength : UINT;
    MessageArg1Bits : ULINT;
    MessageArg1String : STRING[96];
    MessageArg2TypeTag : BYTE;
    MessageArg2PayloadLength : UINT;
    MessageArg2Bits : ULINT;
    MessageArg2String : STRING[96];
    MessageArg3TypeTag : BYTE;
    MessageArg3PayloadLength : UINT;
    MessageArg3Bits : ULINT;
    MessageArg3String : STRING[96];
    MessageArg4TypeTag : BYTE;
    MessageArg4PayloadLength : UINT;
    MessageArg4Bits : ULINT;
    MessageArg4String : STRING[96];
    AttestableId : UDINT;
    SignatureActionId : UDINT;
    SignatureActor : STRING[96];
    SignatureMeaning : UINT;
    SignatureAttestsId : UDINT;
    SignatureHasAuthResult : BOOL;
    SignatureAuthResult : UINT;
END_VAR
VAR_OUTPUT
    AssignedSeq : ULINT;
    ProducedCount : ULINT;
    Error : BOOL;
    SourceTableFull : BOOL;
    Done : BOOL;
    CheckpointDone : BOOL;
    SourceCount : UINT;
    SourceUsed : OPENOT_SOURCE_USED_TABLE;
    SourceIds : OPENOT_SOURCE_ID_TABLE;
    SourceNextSeqs : OPENOT_SOURCE_NEXT_SEQ_TABLE;
    Ring : OPENOT_RING256_BUFFER;
    HeadAbs : ULINT;
    OldestAbs : ULINT;
    LostCount : ULINT;
    LastStartAbs : ULINT;
    LastEndAbs : ULINT;
    LossRanges : OPENOT_PRODUCER_LOSS_RANGES;
    LossRangeCount : UINT;
    LossRangeOverflow : BOOL;
    RingError : BOOL;
    RunId : ULINT;
    SourceTime : ULINT;
    SystemSeq : ULINT;
    EpochId : ULINT;
    EpochFirstAbs : ULINT;
    DefinitionHash : OPENOT_HASH8;
    PrevDefinitionHash : OPENOT_HASH8;
    TransitionOpen : BOOL;
    DefinitionChangedEmitted : BOOL;
    Emissions : OPENOT_EMISSION_LOG;
    EmissionCount : UINT;
    PublishedRecordCount : ULINT;
    DroppedLifecycleCount : ULINT;
    LastLifecycleError : UINT;
    DroppedCommandCount : ULINT;
    LastCommandError : UINT;
    ScanRecords : OPENOT_SCAN_RECORD_BUFFER;
    ScanRecordOffsets : OPENOT_SCAN_RECORD_UINT_TABLE;
    ScanRecordLengths : OPENOT_SCAN_RECORD_UINT_TABLE;
    ScanRecordCount : UINT;
END_VAR
VAR
    Sources : ARRAY[0..15] OF OPENOT_SOURCE_ENTRY;
    RealValueStates : ARRAY[0..15] OF OPENOT_REAL_VALUE_STATE;
    IntValueStates : ARRAY[0..15] OF OPENOT_INT_VALUE_STATE;
    BitsValueStates : ARRAY[0..15] OF OPENOT_BITS_VALUE_STATE;
    StringValueStates : ARRAY[0..15] OF OPENOT_STRING_VALUE_STATE;
    StateMachineStates : ARRAY[0..15] OF OPENOT_STATE_MACHINE_STATE;
    ConditionStates : ARRAY[0..15] OF OPENOT_CONDITION_STATE;
    AttestableEvents : OPENOT_ATTESTABLE_EVENT_LOG;
    RingProducer : OPENOT_Ring256Producer;
    Encoder : OPENOT_EncodeMinimalMessage;
    ValueRealEncoder : OPENOT_EncodeValueChangedReal;
    ValueIntEncoder : OPENOT_EncodeValueChangedInt;
    ValueBitsEncoder : OPENOT_EncodeValueChangedBits;
    ValueStringEncoder : OPENOT_EncodeValueChangedString;
    StateTransitionEncoder : OPENOT_EncodeStateTransition;
    ConditionEncoder : OPENOT_EncodeCondition;
    ConditionLifecycleEncoder : OPENOT_EncodeConditionLifecycle;
    BatchRecipeEncoder : OPENOT_EncodeBatchRecipe;
    RegulatedEncoder : OPENOT_EncodeRegulated;
    ParameterRealEncoder : OPENOT_EncodeParameterChangeReal;
    ParameterIntEncoder : OPENOT_EncodeParameterChangeInt;
    ParameterBitsEncoder : OPENOT_EncodeParameterChangeBits;
    ParameterStringEncoder : OPENOT_EncodeParameterChangeString;
    ESignatureEncoder : OPENOT_EncodeESignature;
    HighWaterEncoder : OPENOT_EncodeSourceHighWater;
    StoppedEncoder : OPENOT_EncodeLoggerStopped;
    StartedEncoder : OPENOT_EncodeLoggerStarted;
    DefinitionEncoder : OPENOT_EncodeDefinitionChanged;
    Index : UINT;
    ByteIndex : UINT;
    LossIndex : UINT;
    EntryIndex : UINT;
    Found : BOOL;
    FreeFound : BOOL;
    Initialized : BOOL;
    PreviousExecute : BOOL;
    ExecuteEdge : BOOL;
    StateRunId : ULINT;
    StateSourceTime : ULINT;
    StateSystemSeq : ULINT;
    StateEpochId : ULINT;
    StateEpochFirstAbs : ULINT;
    StateNextConditionCorrelationId : UDINT;
    StateDroppedLifecycleCount : ULINT;
    StateLastLifecycleError : UINT;
    StateDroppedCommandCount : ULINT;
    StateLastCommandError : UINT;
    StateDefinitionHash : OPENOT_HASH8;
    StatePrevDefinitionHash : OPENOT_HASH8;
    StatePublishedRecordCount : ULINT;
    PendingDefHashOld : OPENOT_HASH8;
    PendingDefHashNew : OPENOT_HASH8;
    PendingEpochId : ULINT;
    PendingOpen : BOOL;
    PendingDefChangedEmitted : BOOL;
    EmitSeq : ULINT;
    EmitSourceTime : ULINT;
    SourceLoopActive : BOOL;
    HasLastSourceId : BOOL;
    LastSourceId : UDINT;
    NextSourceId : UDINT;
    SelectedIndex : UINT;
    StateIndex : UINT;
    StateFound : BOOL;
    StateFreeFound : BOOL;
    ActiveSourceId : UDINT;
    ShouldEmit : BOOL;
    DeltaReal : REAL;
    PolicySourceTime : ULINT;
    SamplingIntervalNs : ULINT;
    PeriodicDue : BOOL;
    CandidateFound : BOOL;
    ScanWritePos : UINT;
    AppendOk : BOOL;
    ResetOk : BOOL;
    ConditionEventTypeId : UDINT;
    ConditionCorrelationId : UDINT;
    LifecycleShouldEmit : BOOL;
    LifecycleActivationScoped : BOOL;
    LifecycleHasAllowedAckBy : BOOL;
    LifecycleHasAllowedShelveSecs : BOOL;
    LifecycleHasAllowedReason : BOOL;
    LifecycleHasAllowedComment : BOOL;
    LifecycleHasAllowedPreviousPriority : BOOL;
    LifecycleHasAllowedNewPriority : BOOL;
    LifecycleAckByLength : UINT;
    LifecycleReasonLength : UINT;
    LifecycleCommentLength : UINT;
    LifecycleRecordLength : UINT;
    ProcedureRecipeVersionLength : UINT;
    ProcedureAckByLength : UINT;
    ProcedureRecordLength : UINT;
    ProcedureShouldEmit : BOOL;
    RegulatedActorLength : UINT;
    RegulatedWorkstationLength : UINT;
    RegulatedReasonLength : UINT;
    RegulatedRecordLength : UINT;
    RegulatedShouldEmit : BOOL;
    AuditActorLength : UINT;
    AuditReasonLength : UINT;
    AuditPreviousStringLength : UINT;
    AuditNewStringLength : UINT;
    AuditRecordLength : UINT;
    AuditShouldEmit : BOOL;
    SignatureActorLength : UINT;
    SignatureRecordLength : UINT;
    SignatureShouldEmit : BOOL;
    SignatureSeq : ULINT;
    AttestableRecordOk : BOOL;
END_VAR

IF NOT Initialized THEN
    StateRunId := ULINT#1;
    StateSourceTime := ULINT#1780000000000000000;
    StateSystemSeq := ULINT#0;
    StateEpochId := ULINT#1;
    StateEpochFirstAbs := ULINT#0;
    StateNextConditionCorrelationId := UDINT#1;
    StateDroppedLifecycleCount := ULINT#0;
    StateLastLifecycleError := UINT#0;
    StateDroppedCommandCount := ULINT#0;
    StateLastCommandError := UINT#0;
    StatePublishedRecordCount := ULINT#0;
    FOR Index := UINT#0 TO UINT#31 DO
        AttestableEvents[Index].Used := FALSE;
        AttestableEvents[Index].AttestableId := UDINT#0;
        AttestableEvents[Index].SourceId := UDINT#0;
        AttestableEvents[Index].HasSeq := FALSE;
        AttestableEvents[Index].LastSeq := ULINT#0;
    END_FOR;
    Initialized := TRUE;
END_IF;

Done := FALSE;
Error := FALSE;
SourceTableFull := FALSE;
AssignedSeq := ULINT#0;
ProducedCount := ULINT#0;
CheckpointDone := FALSE;
ExecuteEdge := Execute AND (NOT PreviousExecute);

IF ResetScanRecords THEN
    ResetOk := OPENOT_ResetScanRecordList(
        ScanRecordOffsets := ScanRecordOffsets,
        ScanRecordLengths := ScanRecordLengths,
        ScanWritePos := ScanWritePos,
        ScanRecordCount := ScanRecordCount);
END_IF;

IF ExecuteEdge THEN
    IF (NOT AccumulateScanRecords) AND (Op <> UINT#6) AND (Op <> UINT#7) AND (Op <> UINT#8) AND (Op <> UINT#9) AND (Op <> UINT#10) AND (Op <> UINT#11) AND (Op <> UINT#12) AND (Op <> UINT#13) AND (Op <> UINT#14) AND (Op <> UINT#15) AND (Op <> UINT#16) THEN
        ResetOk := OPENOT_ResetScanRecordList(
            ScanRecordOffsets := ScanRecordOffsets,
            ScanRecordLengths := ScanRecordLengths,
            ScanWritePos := ScanWritePos,
            ScanRecordCount := ScanRecordCount);
    END_IF;

    IF Checkpoint THEN
        IF PendingOpen THEN
            Error := TRUE;
        ELSIF SourceId = UDINT#0 THEN
            Error := TRUE;
        ELSE
            Found := FALSE;
            EntryIndex := UINT#0;

            FOR Index := UINT#0 TO UINT#15 DO
                IF Sources[Index].Used AND (Sources[Index].SourceId = SourceId) THEN
                    IF NOT Found THEN
                        EntryIndex := Index;
                        Found := TRUE;
                    END_IF;
                END_IF;
            END_FOR;

            IF NOT Found THEN
                Error := TRUE;
            ELSE
                ProducedCount := Sources[EntryIndex].NextSeq;
                EmitSourceTime := OPENOT_NextSourceTime(
                    UseSourceTimeInput := UseSourceTimeInput,
                    SourceTimeInput := SourceTimeInput,
                    StateSourceTime := StateSourceTime);
                HighWaterEncoder(
                    SourceId := SourceId,
                    RunId := StateRunId,
                    ProducedCount := ProducedCount,
                    SourceTime := EmitSourceTime);
                RingProducer(
                    Execute := TRUE,
                    Record := HighWaterEncoder.Buffer,
                    Length := HighWaterEncoder.Length,
                    RunId := StateRunId,
                    SourceId := SourceId,
                    Seq := ProducedCount);

                IF RingProducer.Done AND (NOT RingProducer.Error) THEN
                    AppendOk := OPENOT_AppendScanRecord(
                        Record := HighWaterEncoder.Buffer,
                        Length := HighWaterEncoder.Length,
                        ScanRecords := ScanRecords,
                        ScanRecordOffsets := ScanRecordOffsets,
                        ScanRecordLengths := ScanRecordLengths,
                        ScanWritePos := ScanWritePos,
                        ScanRecordCount := ScanRecordCount);
                    IF AppendOk THEN
                        Sources[EntryIndex].NextSeq := ProducedCount + ULINT#1;
                        StatePublishedRecordCount := StatePublishedRecordCount + ULINT#1;
                        IF EmissionCount < UINT#32 THEN
                            Emissions[EmissionCount].EventTypeId := UDINT#16#0108;
                            Emissions[EmissionCount].RunId := StateRunId;
                            Emissions[EmissionCount].SourceId := SourceId;
                            Emissions[EmissionCount].Seq := ProducedCount;
                            EmissionCount := EmissionCount + UINT#1;
                        END_IF;
                        Done := TRUE;
                        CheckpointDone := TRUE;
                    ELSE
                        Error := TRUE;
                    END_IF;
                ELSE
                    Error := TRUE;
                END_IF;
            END_IF;
        END_IF;
    ELSIF Op = UINT#0 THEN
        IF PendingOpen THEN
            Error := TRUE;
        ELSIF SourceId = UDINT#0 THEN
            Error := TRUE;
        ELSE
            Found := FALSE;
            FreeFound := FALSE;
            EntryIndex := UINT#0;

            FOR Index := UINT#0 TO UINT#15 DO
                IF Sources[Index].Used AND (Sources[Index].SourceId = SourceId) THEN
                    IF NOT Found THEN
                        EntryIndex := Index;
                        Found := TRUE;
                    END_IF;
                END_IF;
            END_FOR;

            IF NOT Found THEN
                FOR Index := UINT#0 TO UINT#15 DO
                    IF (NOT Sources[Index].Used) AND (NOT FreeFound) THEN
                        EntryIndex := Index;
                        FreeFound := TRUE;
                    END_IF;
                END_FOR;

                IF FreeFound THEN
                    Sources[EntryIndex].Used := TRUE;
                    Sources[EntryIndex].SourceId := SourceId;
                    Sources[EntryIndex].NextSeq := ULINT#0;
                ELSE
                    SourceTableFull := TRUE;
                    Error := TRUE;
                END_IF;
            END_IF;

            IF NOT Error THEN
                AssignedSeq := Sources[EntryIndex].NextSeq;
                Sources[EntryIndex].NextSeq := Sources[EntryIndex].NextSeq + ULINT#1;
                EmitSourceTime := OPENOT_NextSourceTime(
                    UseSourceTimeInput := UseSourceTimeInput,
                    SourceTimeInput := SourceTimeInput,
                    StateSourceTime := StateSourceTime);

                Encoder(
                    SourceId := SourceId,
                    RunId := StateRunId,
                    Seq := AssignedSeq,
                    SourceTime := EmitSourceTime,
                    MessageTemplateId := MessageTemplateId,
                    HasSeverity := MessageHasSeverity,
                    Severity := MessageSeverity,
                    ArgCount := MessageArgCount,
                    Arg1TypeTag := MessageArg1TypeTag,
                    Arg1PayloadLength := MessageArg1PayloadLength,
                    Arg1Bits := MessageArg1Bits,
                    Arg1String := MessageArg1String,
                    Arg2TypeTag := MessageArg2TypeTag,
                    Arg2PayloadLength := MessageArg2PayloadLength,
                    Arg2Bits := MessageArg2Bits,
                    Arg2String := MessageArg2String,
                    Arg3TypeTag := MessageArg3TypeTag,
                    Arg3PayloadLength := MessageArg3PayloadLength,
                    Arg3Bits := MessageArg3Bits,
                    Arg3String := MessageArg3String,
                    Arg4TypeTag := MessageArg4TypeTag,
                    Arg4PayloadLength := MessageArg4PayloadLength,
                    Arg4Bits := MessageArg4Bits,
                    Arg4String := MessageArg4String);
                RingProducer(
                    Execute := TRUE,
                    Record := Encoder.Buffer,
                    Length := Encoder.Length,
                    RunId := StateRunId,
                    SourceId := SourceId,
                    Seq := AssignedSeq);

                IF RingProducer.Done AND (NOT RingProducer.Error) THEN
                    AppendOk := OPENOT_AppendScanRecord(
                        Record := Encoder.Buffer,
                        Length := Encoder.Length,
                        ScanRecords := ScanRecords,
                        ScanRecordOffsets := ScanRecordOffsets,
                        ScanRecordLengths := ScanRecordLengths,
                        ScanWritePos := ScanWritePos,
                        ScanRecordCount := ScanRecordCount);
                    IF AppendOk THEN
                        StatePublishedRecordCount := StatePublishedRecordCount + ULINT#1;
                        IF EmissionCount < UINT#32 THEN
                            Emissions[EmissionCount].EventTypeId := UDINT#16#0003;
                            Emissions[EmissionCount].RunId := StateRunId;
                            Emissions[EmissionCount].SourceId := SourceId;
                            Emissions[EmissionCount].Seq := AssignedSeq;
                            EmissionCount := EmissionCount + UINT#1;
                        END_IF;
                        AttestableRecordOk := OPENOT_RecordAttestableSeq(
                            AttestableId := AttestableId,
                            SourceId := SourceId,
                            Seq := AssignedSeq,
                            AttestableEvents := AttestableEvents);
                        Done := TRUE;
                    ELSE
                        Error := TRUE;
                    END_IF;
                ELSE
                    Error := TRUE;
                END_IF;
            END_IF;
        END_IF;
    ELSIF Op = UINT#5 THEN
        IF PendingOpen THEN
            Error := TRUE;
        ELSIF SourceId = UDINT#0 THEN
            Error := TRUE;
        ELSIF (StageLength = UINT#0) OR (StageLength > UINT#256) THEN
            Error := TRUE;
        ELSE
            Found := FALSE;
            FreeFound := FALSE;
            EntryIndex := UINT#0;

            FOR Index := UINT#0 TO UINT#15 DO
                IF Sources[Index].Used AND (Sources[Index].SourceId = SourceId) THEN
                    IF NOT Found THEN
                        EntryIndex := Index;
                        Found := TRUE;
                    END_IF;
                END_IF;
            END_FOR;

            IF NOT Found THEN
                FOR Index := UINT#0 TO UINT#15 DO
                    IF (NOT Sources[Index].Used) AND (NOT FreeFound) THEN
                        EntryIndex := Index;
                        FreeFound := TRUE;
                    END_IF;
                END_FOR;

                IF FreeFound THEN
                    Sources[EntryIndex].Used := TRUE;
                    Sources[EntryIndex].SourceId := SourceId;
                    Sources[EntryIndex].NextSeq := ULINT#0;
                ELSE
                    SourceTableFull := TRUE;
                    Error := TRUE;
                END_IF;
            END_IF;

            IF NOT Error THEN
                IF StageSeq <> Sources[EntryIndex].NextSeq THEN
                    Error := TRUE;
                ELSE
                    RingProducer(
                        Execute := TRUE,
                        Record := StageRecord,
                        Length := StageLength,
                        RunId := StateRunId,
                        SourceId := SourceId,
                        Seq := StageSeq);

                    IF RingProducer.Done AND (NOT RingProducer.Error) THEN
                        AppendOk := OPENOT_AppendScanRecord(
                            Record := StageRecord,
                            Length := StageLength,
                            ScanRecords := ScanRecords,
                            ScanRecordOffsets := ScanRecordOffsets,
                            ScanRecordLengths := ScanRecordLengths,
                            ScanWritePos := ScanWritePos,
                            ScanRecordCount := ScanRecordCount);
                        IF AppendOk THEN
                            AssignedSeq := StageSeq;
                            Sources[EntryIndex].NextSeq := StageSeq + ULINT#1;
                            StatePublishedRecordCount := StatePublishedRecordCount + ULINT#1;
                            IF EmissionCount < UINT#32 THEN
                                Emissions[EmissionCount].EventTypeId := StageEventTypeId;
                                Emissions[EmissionCount].RunId := StateRunId;
                                Emissions[EmissionCount].SourceId := SourceId;
                                Emissions[EmissionCount].Seq := StageSeq;
                                EmissionCount := EmissionCount + UINT#1;
                            END_IF;
                            Done := TRUE;
                        ELSE
                            Error := TRUE;
                        END_IF;
                    ELSE
                        Error := TRUE;
                    END_IF;
                END_IF;
            END_IF;
        END_IF;
    ELSIF (Op = UINT#6) OR (Op = UINT#7) OR (Op = UINT#8) OR (Op = UINT#10) OR (Op = UINT#11) THEN
        ActiveSourceId := SourceId;
        IF ActiveSourceId = UDINT#0 THEN
            ActiveSourceId := UDINT#1;
        END_IF;

        StateFound := FALSE;
        StateFreeFound := FALSE;
        StateIndex := UINT#0;
        ShouldEmit := FALSE;
        PolicySourceTime := SourceTimeInput;
        IF NOT UseSourceTimeInput THEN
            PolicySourceTime := StateSourceTime;
        END_IF;
        SamplingIntervalNs := SamplingIntervalMs * ULINT#1000000;
        PeriodicDue := FALSE;

        IF Op = UINT#6 THEN
            IF ValueId = UDINT#0 THEN
                Error := TRUE;
            ELSE
                FOR Index := UINT#0 TO UINT#15 DO
                    IF RealValueStates[Index].Used AND (RealValueStates[Index].ValueId = ValueId) THEN
                        IF NOT StateFound THEN
                            StateIndex := Index;
                            StateFound := TRUE;
                        END_IF;
                    ELSIF (NOT StateFound) AND (NOT RealValueStates[Index].Used) AND (NOT StateFreeFound) THEN
                        StateIndex := Index;
                        StateFreeFound := TRUE;
                    END_IF;
                END_FOR;

                IF StateFound THEN
                    IF SamplingMode = UINT#2 THEN
                        IF RealValueStates[StateIndex].HasBandCenter THEN
                            ShouldEmit := (ValueReal > (RealValueStates[StateIndex].BandCenter + DeadbandReal)) OR (ValueReal < (RealValueStates[StateIndex].BandCenter - DeadbandReal));
                        ELSE
                            ShouldEmit := TRUE;
                        END_IF;
                    ELSE
                        IF RealValueStates[StateIndex].HasLast THEN
                            DeltaReal := ValueReal - RealValueStates[StateIndex].LastValue;
                            IF DeltaReal < REAL#0.0 THEN
                                DeltaReal := REAL#0.0 - DeltaReal;
                            END_IF;
                            ShouldEmit := DeltaReal > DeadbandReal;
                        ELSE
                            ShouldEmit := TRUE;
                        END_IF;
                    END_IF;
                ELSIF StateFreeFound THEN
                    RealValueStates[StateIndex].Used := TRUE;
                    RealValueStates[StateIndex].ValueId := ValueId;
                    RealValueStates[StateIndex].HasLast := FALSE;
                    RealValueStates[StateIndex].HasLastEmitSourceTime := FALSE;
                    RealValueStates[StateIndex].HasBandCenter := FALSE;
                    ShouldEmit := TRUE;
                ELSE
                    Error := TRUE;
                END_IF;
            END_IF;
        ELSIF Op = UINT#7 THEN
            IF ValueId = UDINT#0 THEN
                Error := TRUE;
            ELSE
                FOR Index := UINT#0 TO UINT#15 DO
                    IF IntValueStates[Index].Used AND (IntValueStates[Index].ValueId = ValueId) THEN
                        IF NOT StateFound THEN
                            StateIndex := Index;
                            StateFound := TRUE;
                        END_IF;
                    ELSIF (NOT StateFound) AND (NOT IntValueStates[Index].Used) AND (NOT StateFreeFound) THEN
                        StateIndex := Index;
                        StateFreeFound := TRUE;
                    END_IF;
                END_FOR;

                IF StateFound THEN
                    IF IntValueStates[StateIndex].HasLast THEN
                        ShouldEmit := ValueInt <> IntValueStates[StateIndex].LastValue;
                    ELSE
                        ShouldEmit := TRUE;
                    END_IF;
                ELSIF StateFreeFound THEN
                    IntValueStates[StateIndex].Used := TRUE;
                    IntValueStates[StateIndex].ValueId := ValueId;
                    IntValueStates[StateIndex].HasLast := FALSE;
                    IntValueStates[StateIndex].HasLastEmitSourceTime := FALSE;
                    ShouldEmit := TRUE;
                ELSE
                    Error := TRUE;
                END_IF;
            END_IF;
        ELSIF Op = UINT#10 THEN
            IF (ValueId = UDINT#0) OR ((ValuePayloadLength <> UINT#1) AND (ValuePayloadLength <> UINT#2) AND (ValuePayloadLength <> UINT#4) AND (ValuePayloadLength <> UINT#8)) THEN
                Error := TRUE;
            ELSE
                FOR Index := UINT#0 TO UINT#15 DO
                    IF BitsValueStates[Index].Used AND (BitsValueStates[Index].ValueId = ValueId) THEN
                        IF NOT StateFound THEN
                            StateIndex := Index;
                            StateFound := TRUE;
                        END_IF;
                    ELSIF (NOT StateFound) AND (NOT BitsValueStates[Index].Used) AND (NOT StateFreeFound) THEN
                        StateIndex := Index;
                        StateFreeFound := TRUE;
                    END_IF;
                END_FOR;

                IF StateFound THEN
                    IF BitsValueStates[StateIndex].HasLast THEN
                        ShouldEmit := ValueBits <> BitsValueStates[StateIndex].LastBits;
                    ELSE
                        ShouldEmit := TRUE;
                    END_IF;
                ELSIF StateFreeFound THEN
                    BitsValueStates[StateIndex].Used := TRUE;
                    BitsValueStates[StateIndex].ValueId := ValueId;
                    BitsValueStates[StateIndex].HasLast := FALSE;
                    BitsValueStates[StateIndex].HasLastEmitSourceTime := FALSE;
                    ShouldEmit := TRUE;
                ELSE
                    Error := TRUE;
                END_IF;
            END_IF;
        ELSIF Op = UINT#11 THEN
            IF ValueId = UDINT#0 THEN
                Error := TRUE;
            ELSE
                FOR Index := UINT#0 TO UINT#15 DO
                    IF StringValueStates[Index].Used AND (StringValueStates[Index].ValueId = ValueId) THEN
                        IF NOT StateFound THEN
                            StateIndex := Index;
                            StateFound := TRUE;
                        END_IF;
                    ELSIF (NOT StateFound) AND (NOT StringValueStates[Index].Used) AND (NOT StateFreeFound) THEN
                        StateIndex := Index;
                        StateFreeFound := TRUE;
                    END_IF;
                END_FOR;

                IF StateFound THEN
                    IF StringValueStates[StateIndex].HasLast THEN
                        ShouldEmit := ValueString <> StringValueStates[StateIndex].LastValue;
                    ELSE
                        ShouldEmit := TRUE;
                    END_IF;
                ELSIF StateFreeFound THEN
                    StringValueStates[StateIndex].Used := TRUE;
                    StringValueStates[StateIndex].ValueId := ValueId;
                    StringValueStates[StateIndex].HasLast := FALSE;
                    StringValueStates[StateIndex].HasLastEmitSourceTime := FALSE;
                    ShouldEmit := TRUE;
                ELSE
                    Error := TRUE;
                END_IF;
            END_IF;
        ELSE
            IF StateMachineId = UDINT#0 THEN
                Error := TRUE;
            ELSE
                FOR Index := UINT#0 TO UINT#15 DO
                    IF StateMachineStates[Index].Used AND (StateMachineStates[Index].StateMachineId = StateMachineId) THEN
                        IF NOT StateFound THEN
                            StateIndex := Index;
                            StateFound := TRUE;
                        END_IF;
                    ELSIF (NOT StateFound) AND (NOT StateMachineStates[Index].Used) AND (NOT StateFreeFound) THEN
                        StateIndex := Index;
                        StateFreeFound := TRUE;
                    END_IF;
                END_FOR;

                IF PreviousState <> NewState THEN
                    IF StateFound THEN
                        ShouldEmit := (NOT StateMachineStates[StateIndex].HasLast) OR (StateMachineStates[StateIndex].LastState <> NewState);
                    ELSIF StateFreeFound THEN
                        StateMachineStates[StateIndex].Used := TRUE;
                        StateMachineStates[StateIndex].StateMachineId := StateMachineId;
                        StateMachineStates[StateIndex].HasLast := FALSE;
                        ShouldEmit := TRUE;
                    ELSE
                        Error := TRUE;
                    END_IF;
                ELSE
                    ShouldEmit := FALSE;
                END_IF;
            END_IF;
        END_IF;

        IF (NOT Error) AND (SamplingMode = UINT#1) AND ((Op = UINT#6) OR (Op = UINT#7) OR (Op = UINT#10) OR (Op = UINT#11)) THEN
            IF SamplingIntervalMs = ULINT#0 THEN
                Error := TRUE;
            ELSIF Op = UINT#6 THEN
                IF RealValueStates[StateIndex].HasLastEmitSourceTime THEN
                    IF PolicySourceTime >= RealValueStates[StateIndex].LastEmitSourceTime THEN
                        PeriodicDue := (PolicySourceTime - RealValueStates[StateIndex].LastEmitSourceTime) >= SamplingIntervalNs;
                    ELSE
                        PeriodicDue := TRUE;
                    END_IF;
                ELSE
                    PeriodicDue := TRUE;
                END_IF;
            ELSIF Op = UINT#7 THEN
                IF IntValueStates[StateIndex].HasLastEmitSourceTime THEN
                    IF PolicySourceTime >= IntValueStates[StateIndex].LastEmitSourceTime THEN
                        PeriodicDue := (PolicySourceTime - IntValueStates[StateIndex].LastEmitSourceTime) >= SamplingIntervalNs;
                    ELSE
                        PeriodicDue := TRUE;
                    END_IF;
                ELSE
                    PeriodicDue := TRUE;
                END_IF;
            ELSIF Op = UINT#10 THEN
                IF BitsValueStates[StateIndex].HasLastEmitSourceTime THEN
                    IF PolicySourceTime >= BitsValueStates[StateIndex].LastEmitSourceTime THEN
                        PeriodicDue := (PolicySourceTime - BitsValueStates[StateIndex].LastEmitSourceTime) >= SamplingIntervalNs;
                    ELSE
                        PeriodicDue := TRUE;
                    END_IF;
                ELSE
                    PeriodicDue := TRUE;
                END_IF;
            ELSE
                IF StringValueStates[StateIndex].HasLastEmitSourceTime THEN
                    IF PolicySourceTime >= StringValueStates[StateIndex].LastEmitSourceTime THEN
                        PeriodicDue := (PolicySourceTime - StringValueStates[StateIndex].LastEmitSourceTime) >= SamplingIntervalNs;
                    ELSE
                        PeriodicDue := TRUE;
                    END_IF;
                ELSE
                    PeriodicDue := TRUE;
                END_IF;
            END_IF;
            ShouldEmit := ShouldEmit OR PeriodicDue;
        END_IF;

        IF (NOT Error) AND (NOT ShouldEmit) THEN
            Done := TRUE;
        ELSIF NOT Error THEN
            Found := FALSE;
            FreeFound := FALSE;
            EntryIndex := UINT#0;

            FOR Index := UINT#0 TO UINT#15 DO
                IF Sources[Index].Used AND (Sources[Index].SourceId = ActiveSourceId) THEN
                    IF NOT Found THEN
                        EntryIndex := Index;
                        Found := TRUE;
                    END_IF;
                END_IF;
            END_FOR;

            IF NOT Found THEN
                FOR Index := UINT#0 TO UINT#15 DO
                    IF (NOT Sources[Index].Used) AND (NOT FreeFound) THEN
                        EntryIndex := Index;
                        FreeFound := TRUE;
                    END_IF;
                END_FOR;

                IF FreeFound THEN
                    Sources[EntryIndex].Used := TRUE;
                    Sources[EntryIndex].SourceId := ActiveSourceId;
                    Sources[EntryIndex].NextSeq := ULINT#0;
                ELSE
                    SourceTableFull := TRUE;
                    Error := TRUE;
                END_IF;
            END_IF;

            IF NOT Error THEN
                AssignedSeq := Sources[EntryIndex].NextSeq;
                EmitSourceTime := OPENOT_NextSourceTime(
                    UseSourceTimeInput := UseSourceTimeInput,
                    SourceTimeInput := SourceTimeInput,
                    StateSourceTime := StateSourceTime);

                IF Op = UINT#6 THEN
                    ValueRealEncoder(
                        SourceId := ActiveSourceId,
                        RunId := StateRunId,
                        Seq := AssignedSeq,
                        SourceTime := EmitSourceTime,
                        ValueId := ValueId,
                        NewValue := ValueReal,
                        HasPrevious := RealValueStates[StateIndex].HasLast AND (NOT SuppressPrevious),
                        PreviousValue := RealValueStates[StateIndex].LastValue,
                        HasQuality := HasQuality,
                        Quality := Quality);
                    RingProducer(
                        Execute := TRUE,
                        Record := ValueRealEncoder.Buffer,
                        Length := ValueRealEncoder.Length,
                        RunId := StateRunId,
                        SourceId := ActiveSourceId,
                        Seq := AssignedSeq);

                    IF RingProducer.Done AND (NOT RingProducer.Error) THEN
                        AppendOk := OPENOT_AppendScanRecord(
                            Record := ValueRealEncoder.Buffer,
                            Length := ValueRealEncoder.Length,
                            ScanRecords := ScanRecords,
                            ScanRecordOffsets := ScanRecordOffsets,
                            ScanRecordLengths := ScanRecordLengths,
                            ScanWritePos := ScanWritePos,
                            ScanRecordCount := ScanRecordCount);
                    ELSE
                        AppendOk := FALSE;
                    END_IF;
                ELSIF Op = UINT#7 THEN
                    ValueIntEncoder(
                        SourceId := ActiveSourceId,
                        RunId := StateRunId,
                        Seq := AssignedSeq,
                        SourceTime := EmitSourceTime,
                        ValueId := ValueId,
                        NewValue := ValueInt,
                        HasPrevious := IntValueStates[StateIndex].HasLast AND (NOT SuppressPrevious),
                        PreviousValue := IntValueStates[StateIndex].LastValue,
                        HasQuality := HasQuality,
                        Quality := Quality);
                    RingProducer(
                        Execute := TRUE,
                        Record := ValueIntEncoder.Buffer,
                        Length := ValueIntEncoder.Length,
                        RunId := StateRunId,
                        SourceId := ActiveSourceId,
                        Seq := AssignedSeq);

                    IF RingProducer.Done AND (NOT RingProducer.Error) THEN
                        AppendOk := OPENOT_AppendScanRecord(
                            Record := ValueIntEncoder.Buffer,
                            Length := ValueIntEncoder.Length,
                            ScanRecords := ScanRecords,
                            ScanRecordOffsets := ScanRecordOffsets,
                            ScanRecordLengths := ScanRecordLengths,
                            ScanWritePos := ScanWritePos,
                            ScanRecordCount := ScanRecordCount);
                    ELSE
                        AppendOk := FALSE;
                    END_IF;
                ELSIF Op = UINT#10 THEN
                    ValueBitsEncoder(
                        SourceId := ActiveSourceId,
                        RunId := StateRunId,
                        Seq := AssignedSeq,
                        SourceTime := EmitSourceTime,
                        ValueId := ValueId,
                        TypeTag := ValueTypeTag,
                        PayloadLength := ValuePayloadLength,
                        NewBits := ValueBits,
                        HasPrevious := BitsValueStates[StateIndex].HasLast AND (NOT SuppressPrevious),
                        PreviousBits := BitsValueStates[StateIndex].LastBits,
                        HasQuality := HasQuality,
                        Quality := Quality);
                    RingProducer(
                        Execute := TRUE,
                        Record := ValueBitsEncoder.Buffer,
                        Length := ValueBitsEncoder.Length,
                        RunId := StateRunId,
                        SourceId := ActiveSourceId,
                        Seq := AssignedSeq);

                    IF RingProducer.Done AND (NOT RingProducer.Error) THEN
                        AppendOk := OPENOT_AppendScanRecord(
                            Record := ValueBitsEncoder.Buffer,
                            Length := ValueBitsEncoder.Length,
                            ScanRecords := ScanRecords,
                            ScanRecordOffsets := ScanRecordOffsets,
                            ScanRecordLengths := ScanRecordLengths,
                            ScanWritePos := ScanWritePos,
                            ScanRecordCount := ScanRecordCount);
                    ELSE
                        AppendOk := FALSE;
                    END_IF;
                ELSIF Op = UINT#11 THEN
                    ValueStringEncoder(
                        SourceId := ActiveSourceId,
                        RunId := StateRunId,
                        Seq := AssignedSeq,
                        SourceTime := EmitSourceTime,
                        ValueId := ValueId,
                        NewValue := ValueString,
                        HasPrevious := StringValueStates[StateIndex].HasLast AND (NOT SuppressPrevious),
                        PreviousValue := StringValueStates[StateIndex].LastValue,
                        HasQuality := HasQuality,
                        Quality := Quality);
                    RingProducer(
                        Execute := TRUE,
                        Record := ValueStringEncoder.Buffer,
                        Length := ValueStringEncoder.Length,
                        RunId := StateRunId,
                        SourceId := ActiveSourceId,
                        Seq := AssignedSeq);

                    IF RingProducer.Done AND (NOT RingProducer.Error) THEN
                        AppendOk := OPENOT_AppendScanRecord(
                            Record := ValueStringEncoder.Buffer,
                            Length := ValueStringEncoder.Length,
                            ScanRecords := ScanRecords,
                            ScanRecordOffsets := ScanRecordOffsets,
                            ScanRecordLengths := ScanRecordLengths,
                            ScanWritePos := ScanWritePos,
                            ScanRecordCount := ScanRecordCount);
                    ELSE
                        AppendOk := FALSE;
                    END_IF;
                ELSE
                    StateTransitionEncoder(
                        SourceId := ActiveSourceId,
                        RunId := StateRunId,
                        Seq := AssignedSeq,
                        SourceTime := EmitSourceTime,
                        StateMachineId := StateMachineId,
                        Category := Category,
                        PreviousState := PreviousState,
                        NewState := NewState);
                    RingProducer(
                        Execute := TRUE,
                        Record := StateTransitionEncoder.Buffer,
                        Length := StateTransitionEncoder.Length,
                        RunId := StateRunId,
                        SourceId := ActiveSourceId,
                        Seq := AssignedSeq);

                    IF RingProducer.Done AND (NOT RingProducer.Error) THEN
                        AppendOk := OPENOT_AppendScanRecord(
                            Record := StateTransitionEncoder.Buffer,
                            Length := StateTransitionEncoder.Length,
                            ScanRecords := ScanRecords,
                            ScanRecordOffsets := ScanRecordOffsets,
                            ScanRecordLengths := ScanRecordLengths,
                            ScanWritePos := ScanWritePos,
                            ScanRecordCount := ScanRecordCount);
                    ELSE
                        AppendOk := FALSE;
                    END_IF;
                END_IF;

                IF AppendOk THEN
                    Sources[EntryIndex].NextSeq := AssignedSeq + ULINT#1;
                    StatePublishedRecordCount := StatePublishedRecordCount + ULINT#1;
                    IF EmissionCount < UINT#32 THEN
                        IF Op = UINT#8 THEN
                            Emissions[EmissionCount].EventTypeId := UDINT#16#0001;
                        ELSE
                            Emissions[EmissionCount].EventTypeId := UDINT#16#0002;
                        END_IF;
                        Emissions[EmissionCount].RunId := StateRunId;
                        Emissions[EmissionCount].SourceId := ActiveSourceId;
                        Emissions[EmissionCount].Seq := AssignedSeq;
                        EmissionCount := EmissionCount + UINT#1;
                    END_IF;
                    AttestableRecordOk := OPENOT_RecordAttestableSeq(
                        AttestableId := AttestableId,
                        SourceId := ActiveSourceId,
                        Seq := AssignedSeq,
                        AttestableEvents := AttestableEvents);

                    IF Op = UINT#6 THEN
                        RealValueStates[StateIndex].HasLast := TRUE;
                        RealValueStates[StateIndex].LastValue := ValueReal;
                        RealValueStates[StateIndex].HasLastEmitSourceTime := TRUE;
                        RealValueStates[StateIndex].LastEmitSourceTime := EmitSourceTime;
                        RealValueStates[StateIndex].HasBandCenter := TRUE;
                        RealValueStates[StateIndex].BandCenter := ValueReal;
                    ELSIF Op = UINT#7 THEN
                        IntValueStates[StateIndex].HasLast := TRUE;
                        IntValueStates[StateIndex].LastValue := ValueInt;
                        IntValueStates[StateIndex].HasLastEmitSourceTime := TRUE;
                        IntValueStates[StateIndex].LastEmitSourceTime := EmitSourceTime;
                    ELSIF Op = UINT#10 THEN
                        BitsValueStates[StateIndex].HasLast := TRUE;
                        BitsValueStates[StateIndex].LastBits := ValueBits;
                        BitsValueStates[StateIndex].HasLastEmitSourceTime := TRUE;
                        BitsValueStates[StateIndex].LastEmitSourceTime := EmitSourceTime;
                    ELSIF Op = UINT#11 THEN
                        StringValueStates[StateIndex].HasLast := TRUE;
                        StringValueStates[StateIndex].LastValue := ValueString;
                        StringValueStates[StateIndex].HasLastEmitSourceTime := TRUE;
                        StringValueStates[StateIndex].LastEmitSourceTime := EmitSourceTime;
                    ELSE
                        StateMachineStates[StateIndex].HasLast := TRUE;
                        StateMachineStates[StateIndex].LastState := NewState;
                    END_IF;
                    Done := TRUE;
                ELSE
                    Error := TRUE;
                END_IF;
            END_IF;
        END_IF;
    ELSIF Op = UINT#9 THEN
        ActiveSourceId := SourceId;
        IF ActiveSourceId = UDINT#0 THEN
            ActiveSourceId := UDINT#1;
        END_IF;

        StateFound := FALSE;
        StateFreeFound := FALSE;
        StateIndex := UINT#0;
        ShouldEmit := FALSE;

        IF ConditionId = UDINT#0 THEN
            Error := TRUE;
        ELSE
            FOR Index := UINT#0 TO UINT#15 DO
                IF ConditionStates[Index].Used AND (ConditionStates[Index].SourceId = ActiveSourceId) AND (ConditionStates[Index].ConditionId = ConditionId) THEN
                    IF NOT StateFound THEN
                        StateIndex := Index;
                        StateFound := TRUE;
                    END_IF;
                ELSIF (NOT StateFound) AND (NOT ConditionStates[Index].Used) AND (NOT StateFreeFound) THEN
                    StateIndex := Index;
                    StateFreeFound := TRUE;
                END_IF;
            END_FOR;

            IF StateFound THEN
                IF ConditionStates[StateIndex].HasLast THEN
                    ShouldEmit := ConditionStates[StateIndex].LastActive <> ConditionActive;
                ELSE
                    ShouldEmit := ConditionActive;
                END_IF;
            ELSIF StateFreeFound THEN
                ConditionStates[StateIndex].Used := TRUE;
                ConditionStates[StateIndex].SourceId := ActiveSourceId;
                ConditionStates[StateIndex].ConditionId := ConditionId;
                ConditionStates[StateIndex].HasLast := FALSE;
                ConditionStates[StateIndex].CorrelationId := UDINT#0;
                ShouldEmit := ConditionActive;
            ELSE
                Error := TRUE;
            END_IF;
        END_IF;

        IF (NOT Error) AND (NOT ShouldEmit) THEN
            IF StateFound OR StateFreeFound THEN
                ConditionStates[StateIndex].HasLast := TRUE;
                ConditionStates[StateIndex].LastActive := ConditionActive;
            END_IF;
            Done := TRUE;
        ELSIF NOT Error THEN
            Found := FALSE;
            FreeFound := FALSE;
            EntryIndex := UINT#0;

            FOR Index := UINT#0 TO UINT#15 DO
                IF Sources[Index].Used AND (Sources[Index].SourceId = ActiveSourceId) THEN
                    IF NOT Found THEN
                        EntryIndex := Index;
                        Found := TRUE;
                    END_IF;
                END_IF;
            END_FOR;

            IF NOT Found THEN
                FOR Index := UINT#0 TO UINT#15 DO
                    IF (NOT Sources[Index].Used) AND (NOT FreeFound) THEN
                        EntryIndex := Index;
                        FreeFound := TRUE;
                    END_IF;
                END_FOR;

                IF FreeFound THEN
                    Sources[EntryIndex].Used := TRUE;
                    Sources[EntryIndex].SourceId := ActiveSourceId;
                    Sources[EntryIndex].NextSeq := ULINT#0;
                ELSE
                    SourceTableFull := TRUE;
                    Error := TRUE;
                END_IF;
            END_IF;

            IF NOT Error THEN
                AssignedSeq := Sources[EntryIndex].NextSeq;
                EmitSourceTime := OPENOT_NextSourceTime(
                    UseSourceTimeInput := UseSourceTimeInput,
                    SourceTimeInput := SourceTimeInput,
                    StateSourceTime := StateSourceTime);
                IF ConditionActive THEN
                    ConditionEventTypeId := UDINT#16#0200;
                    ConditionCorrelationId := StateNextConditionCorrelationId;
                    StateNextConditionCorrelationId := StateNextConditionCorrelationId + UDINT#1;
                    ConditionStates[StateIndex].CorrelationId := ConditionCorrelationId;
                ELSE
                    ConditionEventTypeId := UDINT#16#0201;
                    ConditionCorrelationId := ConditionStates[StateIndex].CorrelationId;
                END_IF;

                ConditionEncoder(
                    SourceId := ActiveSourceId,
                    RunId := StateRunId,
                    Seq := AssignedSeq,
                    SourceTime := EmitSourceTime,
                    EventTypeId := ConditionEventTypeId,
                    ConditionId := ConditionId,
                    ConditionClass := ConditionClass,
                    Severity := Severity,
                    HasCorrelationId := ConditionCorrelationId <> UDINT#0,
                    CorrelationId := ConditionCorrelationId,
                    HasCauseOperand := HasCauseOperand,
                    CauseOperand := CauseOperand);
                RingProducer(
                    Execute := TRUE,
                    Record := ConditionEncoder.Buffer,
                    Length := ConditionEncoder.Length,
                    RunId := StateRunId,
                    SourceId := ActiveSourceId,
                    Seq := AssignedSeq);

                IF RingProducer.Done AND (NOT RingProducer.Error) THEN
                    AppendOk := OPENOT_AppendScanRecord(
                        Record := ConditionEncoder.Buffer,
                        Length := ConditionEncoder.Length,
                        ScanRecords := ScanRecords,
                        ScanRecordOffsets := ScanRecordOffsets,
                        ScanRecordLengths := ScanRecordLengths,
                        ScanWritePos := ScanWritePos,
                        ScanRecordCount := ScanRecordCount);
                ELSE
                    AppendOk := FALSE;
                END_IF;

                IF AppendOk THEN
                    Sources[EntryIndex].NextSeq := AssignedSeq + ULINT#1;
                    StatePublishedRecordCount := StatePublishedRecordCount + ULINT#1;
                    IF EmissionCount < UINT#32 THEN
                        Emissions[EmissionCount].EventTypeId := ConditionEventTypeId;
                        Emissions[EmissionCount].RunId := StateRunId;
                        Emissions[EmissionCount].SourceId := ActiveSourceId;
                        Emissions[EmissionCount].Seq := AssignedSeq;
                        EmissionCount := EmissionCount + UINT#1;
                    END_IF;
                    ConditionStates[StateIndex].HasLast := TRUE;
                    ConditionStates[StateIndex].LastActive := ConditionActive;
                    Done := TRUE;
                ELSE
                    Error := TRUE;
                END_IF;
            END_IF;
        END_IF;
    ELSIF Op = UINT#12 THEN
        ActiveSourceId := SourceId;
        IF ActiveSourceId = UDINT#0 THEN
            ActiveSourceId := UDINT#1;
        END_IF;

        ConditionEventTypeId := ConditionLifecycleEventTypeId;
        ConditionCorrelationId := UDINT#0;
        LifecycleShouldEmit := FALSE;
        LifecycleActivationScoped := (ConditionEventTypeId = UDINT#16#0202) OR (ConditionEventTypeId = UDINT#16#0203) OR (ConditionEventTypeId = UDINT#16#0204) OR (ConditionEventTypeId = UDINT#16#0205) OR (ConditionEventTypeId = UDINT#16#020A) OR (ConditionEventTypeId = UDINT#16#020B);
        LifecycleHasAllowedAckBy := LifecycleHasAckBy AND ((ConditionEventTypeId = UDINT#16#0202) OR (ConditionEventTypeId = UDINT#16#0203) OR (ConditionEventTypeId = UDINT#16#0204) OR (ConditionEventTypeId = UDINT#16#0208) OR (ConditionEventTypeId = UDINT#16#020A) OR (ConditionEventTypeId = UDINT#16#020B) OR (ConditionEventTypeId = UDINT#16#020C));
        LifecycleHasAllowedShelveSecs := LifecycleHasShelveSecs AND (ConditionEventTypeId = UDINT#16#0204);
        LifecycleHasAllowedReason := LifecycleHasReason AND (ConditionEventTypeId = UDINT#16#0206);
        LifecycleHasAllowedComment := LifecycleHasComment AND (ConditionEventTypeId = UDINT#16#020A);
        LifecycleHasAllowedPreviousPriority := LifecycleHasPreviousPriority AND (ConditionEventTypeId = UDINT#16#020C);
        LifecycleHasAllowedNewPriority := LifecycleHasNewPriority AND (ConditionEventTypeId = UDINT#16#020C);
        LifecycleAckByLength := INT_TO_UINT(LEN(LifecycleAckBy));
        LifecycleReasonLength := INT_TO_UINT(LEN(LifecycleReason));
        LifecycleCommentLength := INT_TO_UINT(LEN(LifecycleComment));
        IF LifecycleAckByLength > UINT#96 THEN
            LifecycleAckByLength := UINT#96;
        END_IF;
        IF LifecycleReasonLength > UINT#96 THEN
            LifecycleReasonLength := UINT#96;
        END_IF;
        IF LifecycleCommentLength > UINT#96 THEN
            LifecycleCommentLength := UINT#96;
        END_IF;

        IF ConditionId = UDINT#0 THEN
            StateLastLifecycleError := UINT#2;
            Error := TRUE;
        ELSIF (ConditionEventTypeId <> UDINT#16#0202) AND (ConditionEventTypeId <> UDINT#16#0203) AND (ConditionEventTypeId <> UDINT#16#0204) AND (ConditionEventTypeId <> UDINT#16#0205) AND (ConditionEventTypeId <> UDINT#16#0206) AND (ConditionEventTypeId <> UDINT#16#0207) AND (ConditionEventTypeId <> UDINT#16#0208) AND (ConditionEventTypeId <> UDINT#16#0209) AND (ConditionEventTypeId <> UDINT#16#020A) AND (ConditionEventTypeId <> UDINT#16#020B) AND (ConditionEventTypeId <> UDINT#16#020C) THEN
            StateLastLifecycleError := UINT#3;
            Error := TRUE;
        ELSIF LifecycleActivationScoped THEN
            StateFound := FALSE;
            StateIndex := UINT#0;

            FOR Index := UINT#0 TO UINT#15 DO
                IF ConditionStates[Index].Used AND (ConditionStates[Index].SourceId = ActiveSourceId) AND (ConditionStates[Index].ConditionId = ConditionId) THEN
                    IF NOT StateFound THEN
                        StateIndex := Index;
                        StateFound := TRUE;
                    END_IF;
                END_IF;
            END_FOR;

            IF StateFound AND ConditionStates[StateIndex].HasLast AND ConditionStates[StateIndex].LastActive AND (ConditionStates[StateIndex].CorrelationId <> UDINT#0) THEN
                ConditionCorrelationId := ConditionStates[StateIndex].CorrelationId;
                LifecycleShouldEmit := TRUE;
            ELSE
                StateDroppedLifecycleCount := StateDroppedLifecycleCount + ULINT#1;
                StateLastLifecycleError := UINT#1;
                Error := TRUE;
            END_IF;
        ELSE
            LifecycleShouldEmit := TRUE;
        END_IF;

        IF LifecycleShouldEmit THEN
            LifecycleRecordLength := UINT#52;
            IF LifecycleActivationScoped THEN
                LifecycleRecordLength := LifecycleRecordLength + UINT#8;
            END_IF;
            IF LifecycleHasAllowedComment THEN
                LifecycleRecordLength := LifecycleRecordLength + OPENOT_ValueSlotEncodedLength(PayloadLength := LifecycleCommentLength);
            END_IF;
            IF LifecycleHasAllowedPreviousPriority THEN
                LifecycleRecordLength := LifecycleRecordLength + UINT#8;
            END_IF;
            IF LifecycleHasAllowedNewPriority THEN
                LifecycleRecordLength := LifecycleRecordLength + UINT#8;
            END_IF;
            IF LifecycleHasAllowedAckBy THEN
                LifecycleRecordLength := LifecycleRecordLength + OPENOT_ValueSlotEncodedLength(PayloadLength := LifecycleAckByLength);
            END_IF;
            IF LifecycleHasAllowedShelveSecs THEN
                LifecycleRecordLength := LifecycleRecordLength + UINT#8;
            END_IF;
            IF LifecycleHasAllowedReason THEN
                LifecycleRecordLength := LifecycleRecordLength + OPENOT_ValueSlotEncodedLength(PayloadLength := LifecycleReasonLength);
            END_IF;

            IF ((ConditionEventTypeId = UDINT#16#020A) AND (NOT LifecycleHasAllowedComment)) OR ((ConditionEventTypeId = UDINT#16#020C) AND ((NOT LifecycleHasAllowedPreviousPriority) OR (NOT LifecycleHasAllowedNewPriority))) THEN
                StateDroppedLifecycleCount := StateDroppedLifecycleCount + ULINT#1;
                StateLastLifecycleError := UINT#5;
                LifecycleShouldEmit := FALSE;
                Error := TRUE;
            ELSIF LifecycleRecordLength > UINT#256 THEN
                StateDroppedLifecycleCount := StateDroppedLifecycleCount + ULINT#1;
                StateLastLifecycleError := UINT#6;
                LifecycleShouldEmit := FALSE;
                Error := TRUE;
            END_IF;
        END_IF;

        IF LifecycleShouldEmit THEN
            Found := FALSE;
            FreeFound := FALSE;
            EntryIndex := UINT#0;

            FOR Index := UINT#0 TO UINT#15 DO
                IF Sources[Index].Used AND (Sources[Index].SourceId = ActiveSourceId) THEN
                    IF NOT Found THEN
                        EntryIndex := Index;
                        Found := TRUE;
                    END_IF;
                END_IF;
            END_FOR;

            IF NOT Found THEN
                FOR Index := UINT#0 TO UINT#15 DO
                    IF (NOT Sources[Index].Used) AND (NOT FreeFound) THEN
                        EntryIndex := Index;
                        FreeFound := TRUE;
                    END_IF;
                END_FOR;

                IF FreeFound THEN
                    Sources[EntryIndex].Used := TRUE;
                    Sources[EntryIndex].SourceId := ActiveSourceId;
                    Sources[EntryIndex].NextSeq := ULINT#0;
                ELSE
                    SourceTableFull := TRUE;
                    StateLastLifecycleError := UINT#4;
                    Error := TRUE;
                END_IF;
            END_IF;

            IF NOT Error THEN
                AssignedSeq := Sources[EntryIndex].NextSeq;
                EmitSourceTime := OPENOT_NextSourceTime(
                    UseSourceTimeInput := UseSourceTimeInput,
                    SourceTimeInput := SourceTimeInput,
                    StateSourceTime := StateSourceTime);
                ConditionLifecycleEncoder(
                    SourceId := ActiveSourceId,
                    RunId := StateRunId,
                    Seq := AssignedSeq,
                    SourceTime := EmitSourceTime,
                    EventTypeId := ConditionEventTypeId,
                    ConditionId := ConditionId,
                    CorrelationId := ConditionCorrelationId,
                    HasAckBy := LifecycleHasAllowedAckBy,
                    AckBy := LifecycleAckBy,
                    HasShelveSecs := LifecycleHasAllowedShelveSecs,
                    ShelveSecs := LifecycleShelveSecs,
                    HasReason := LifecycleHasAllowedReason,
                    Reason := LifecycleReason,
                    HasComment := LifecycleHasAllowedComment,
                    Comment := LifecycleComment,
                    HasPreviousPriority := LifecycleHasAllowedPreviousPriority,
                    PreviousPriority := LifecyclePreviousPriority,
                    HasNewPriority := LifecycleHasAllowedNewPriority,
                    NewPriority := LifecycleNewPriority,
                    HasRefreshId := FALSE,
                    RefreshId := UDINT#0);
                RingProducer(
                    Execute := TRUE,
                    Record := ConditionLifecycleEncoder.Buffer,
                    Length := ConditionLifecycleEncoder.Length,
                    RunId := StateRunId,
                    SourceId := ActiveSourceId,
                    Seq := AssignedSeq);

                IF RingProducer.Done AND (NOT RingProducer.Error) THEN
                    AppendOk := OPENOT_AppendScanRecord(
                        Record := ConditionLifecycleEncoder.Buffer,
                        Length := ConditionLifecycleEncoder.Length,
                        ScanRecords := ScanRecords,
                        ScanRecordOffsets := ScanRecordOffsets,
                        ScanRecordLengths := ScanRecordLengths,
                        ScanWritePos := ScanWritePos,
                        ScanRecordCount := ScanRecordCount);
                ELSE
                    AppendOk := FALSE;
                END_IF;

                IF AppendOk THEN
                    Sources[EntryIndex].NextSeq := AssignedSeq + ULINT#1;
                    StatePublishedRecordCount := StatePublishedRecordCount + ULINT#1;
                    IF EmissionCount < UINT#32 THEN
                        Emissions[EmissionCount].EventTypeId := ConditionEventTypeId;
                        Emissions[EmissionCount].RunId := StateRunId;
                        Emissions[EmissionCount].SourceId := ActiveSourceId;
                        Emissions[EmissionCount].Seq := AssignedSeq;
                        EmissionCount := EmissionCount + UINT#1;
                    END_IF;
                    Done := TRUE;
                ELSE
                    Error := TRUE;
                END_IF;
            END_IF;
        END_IF;
    ELSIF Op = UINT#13 THEN
        ActiveSourceId := SourceId;
        IF ActiveSourceId = UDINT#0 THEN
            ActiveSourceId := UDINT#1;
        END_IF;

        ProcedureRecipeVersionLength := INT_TO_UINT(LEN(ProcedureRecipeVersion));
        ProcedureAckByLength := INT_TO_UINT(LEN(ProcedureAckBy));
        IF ProcedureRecipeVersionLength > UINT#96 THEN
            ProcedureRecipeVersionLength := UINT#96;
        END_IF;
        IF ProcedureAckByLength > UINT#96 THEN
            ProcedureAckByLength := UINT#96;
        END_IF;

        ProcedureShouldEmit := FALSE;
        ProcedureRecordLength := UINT#0;

        IF ProcedureEventTypeId = UDINT#16#0301 THEN
            ProcedureRecordLength := UINT#52 + OPENOT_ValueSlotEncodedLength(PayloadLength := ProcedureRecipeVersionLength);
            IF ProcedureHasBatchId THEN
                ProcedureRecordLength := ProcedureRecordLength + UINT#8;
            END_IF;
            IF ProcedureRecipeVersionLength = UINT#0 THEN
                StateDroppedCommandCount := StateDroppedCommandCount + ULINT#1;
                StateLastCommandError := UINT#2;
                Error := TRUE;
            ELSE
                ProcedureShouldEmit := TRUE;
            END_IF;
        ELSIF ProcedureEventTypeId = UDINT#16#0302 THEN
            ProcedureRecordLength := UINT#52 + OPENOT_ValueSlotEncodedLength(PayloadLength := ProcedureRecipeVersionLength);
            IF ProcedureHasAuthResult THEN
                ProcedureRecordLength := ProcedureRecordLength + UINT#8;
            END_IF;
            IF ProcedureHasAckBy THEN
                ProcedureRecordLength := ProcedureRecordLength + OPENOT_ValueSlotEncodedLength(PayloadLength := ProcedureAckByLength);
            END_IF;
            IF ProcedureRecipeVersionLength = UINT#0 THEN
                StateDroppedCommandCount := StateDroppedCommandCount + ULINT#1;
                StateLastCommandError := UINT#2;
                Error := TRUE;
            ELSE
                ProcedureShouldEmit := TRUE;
            END_IF;
        ELSIF ProcedureEventTypeId = UDINT#16#0303 THEN
            ProcedureRecordLength := UINT#60;
            IF ProcedureHasRecipeId THEN
                ProcedureRecordLength := ProcedureRecordLength + UINT#8;
            END_IF;
            ProcedureShouldEmit := TRUE;
        ELSIF ProcedureEventTypeId = UDINT#16#0304 THEN
            ProcedureRecordLength := UINT#72;
            IF ProcedureHasUnit THEN
                ProcedureRecordLength := ProcedureRecordLength + UINT#8;
            END_IF;
            ProcedureShouldEmit := TRUE;
        ELSE
            StateDroppedCommandCount := StateDroppedCommandCount + ULINT#1;
            StateLastCommandError := UINT#1;
            Error := TRUE;
        END_IF;

        IF ProcedureShouldEmit AND (ProcedureRecordLength > UINT#256) THEN
            StateDroppedCommandCount := StateDroppedCommandCount + ULINT#1;
            StateLastCommandError := UINT#3;
            ProcedureShouldEmit := FALSE;
            Error := TRUE;
        END_IF;

        IF ProcedureShouldEmit THEN
            Found := FALSE;
            FreeFound := FALSE;
            EntryIndex := UINT#0;

            FOR Index := UINT#0 TO UINT#15 DO
                IF Sources[Index].Used AND (Sources[Index].SourceId = ActiveSourceId) THEN
                    IF NOT Found THEN
                        EntryIndex := Index;
                        Found := TRUE;
                    END_IF;
                END_IF;
            END_FOR;

            IF NOT Found THEN
                FOR Index := UINT#0 TO UINT#15 DO
                    IF (NOT Sources[Index].Used) AND (NOT FreeFound) THEN
                        EntryIndex := Index;
                        FreeFound := TRUE;
                    END_IF;
                END_FOR;

                IF FreeFound THEN
                    Sources[EntryIndex].Used := TRUE;
                    Sources[EntryIndex].SourceId := ActiveSourceId;
                    Sources[EntryIndex].NextSeq := ULINT#0;
                ELSE
                    SourceTableFull := TRUE;
                    StateDroppedCommandCount := StateDroppedCommandCount + ULINT#1;
                    StateLastCommandError := UINT#4;
                    ProcedureShouldEmit := FALSE;
                    Error := TRUE;
                END_IF;
            END_IF;
        END_IF;

        IF ProcedureShouldEmit THEN
            AssignedSeq := Sources[EntryIndex].NextSeq;
            EmitSourceTime := OPENOT_NextSourceTime(
                UseSourceTimeInput := UseSourceTimeInput,
                SourceTimeInput := SourceTimeInput,
                StateSourceTime := StateSourceTime);
            BatchRecipeEncoder(
                SourceId := ActiveSourceId,
                RunId := StateRunId,
                Seq := AssignedSeq,
                SourceTime := EmitSourceTime,
                EventTypeId := ProcedureEventTypeId,
                RecipeId := ProcedureRecipeId,
                HasRecipeId := ProcedureHasRecipeId,
                RecipeVersion := ProcedureRecipeVersion,
                HasBatchId := ProcedureHasBatchId,
                BatchId := ProcedureBatchId,
                HasAuthResult := ProcedureHasAuthResult,
                AuthResult := ProcedureAuthResult,
                HasAckBy := ProcedureHasAckBy,
                AckBy := ProcedureAckBy,
                NewState := ProcedureNewState,
                MaterialId := ProcedureMaterialId,
                Quantity := ProcedureQuantity,
                HasUnit := ProcedureHasUnit,
                UnitId := ProcedureUnitId);
            RingProducer(
                Execute := TRUE,
                Record := BatchRecipeEncoder.Buffer,
                Length := BatchRecipeEncoder.Length,
                RunId := StateRunId,
                SourceId := ActiveSourceId,
                Seq := AssignedSeq);

            IF RingProducer.Done AND (NOT RingProducer.Error) THEN
                AppendOk := OPENOT_AppendScanRecord(
                    Record := BatchRecipeEncoder.Buffer,
                    Length := BatchRecipeEncoder.Length,
                    ScanRecords := ScanRecords,
                    ScanRecordOffsets := ScanRecordOffsets,
                    ScanRecordLengths := ScanRecordLengths,
                    ScanWritePos := ScanWritePos,
                    ScanRecordCount := ScanRecordCount);
            ELSE
                AppendOk := FALSE;
            END_IF;

            IF AppendOk THEN
                Sources[EntryIndex].NextSeq := AssignedSeq + ULINT#1;
                StatePublishedRecordCount := StatePublishedRecordCount + ULINT#1;
                IF EmissionCount < UINT#32 THEN
                    Emissions[EmissionCount].EventTypeId := ProcedureEventTypeId;
                    Emissions[EmissionCount].RunId := StateRunId;
                    Emissions[EmissionCount].SourceId := ActiveSourceId;
                    Emissions[EmissionCount].Seq := AssignedSeq;
                    EmissionCount := EmissionCount + UINT#1;
                END_IF;
                AttestableRecordOk := OPENOT_RecordAttestableSeq(
                    AttestableId := AttestableId,
                    SourceId := ActiveSourceId,
                    Seq := AssignedSeq,
                    AttestableEvents := AttestableEvents);
                Done := TRUE;
            ELSE
                StateDroppedCommandCount := StateDroppedCommandCount + ULINT#1;
                StateLastCommandError := UINT#5;
                Error := TRUE;
            END_IF;
        END_IF;
    ELSIF Op = UINT#14 THEN
        ActiveSourceId := SourceId;
        IF ActiveSourceId = UDINT#0 THEN
            ActiveSourceId := UDINT#1;
        END_IF;

        RegulatedActorLength := INT_TO_UINT(LEN(RegulatedActor));
        RegulatedWorkstationLength := INT_TO_UINT(LEN(RegulatedWorkstation));
        RegulatedReasonLength := INT_TO_UINT(LEN(RegulatedReason));
        IF RegulatedActorLength > UINT#96 THEN
            RegulatedActorLength := UINT#96;
        END_IF;
        IF RegulatedWorkstationLength > UINT#96 THEN
            RegulatedWorkstationLength := UINT#96;
        END_IF;
        IF RegulatedReasonLength > UINT#96 THEN
            RegulatedReasonLength := UINT#96;
        END_IF;

        RegulatedShouldEmit := FALSE;
        RegulatedRecordLength := UINT#0;

        IF RegulatedEventTypeId = UDINT#16#0400 THEN
            RegulatedRecordLength := UINT#52 + OPENOT_ValueSlotEncodedLength(PayloadLength := RegulatedActorLength);
            IF RegulatedHasContext1 THEN
                RegulatedRecordLength := RegulatedRecordLength + UINT#8;
            END_IF;
            IF RegulatedHasContext2 THEN
                RegulatedRecordLength := RegulatedRecordLength + UINT#8;
            END_IF;
            IF RegulatedHasContext3 THEN
                RegulatedRecordLength := RegulatedRecordLength + UINT#8;
            END_IF;
            IF RegulatedHasContext4 THEN
                RegulatedRecordLength := RegulatedRecordLength + UINT#8;
            END_IF;
            IF RegulatedHasAuthResult THEN
                RegulatedRecordLength := RegulatedRecordLength + UINT#8;
            END_IF;
            IF RegulatedHasWorkstation THEN
                RegulatedRecordLength := RegulatedRecordLength + OPENOT_ValueSlotEncodedLength(PayloadLength := RegulatedWorkstationLength);
            END_IF;
            IF RegulatedActorLength = UINT#0 THEN
                StateDroppedCommandCount := StateDroppedCommandCount + ULINT#1;
                StateLastCommandError := UINT#2;
                Error := TRUE;
            ELSE
                RegulatedShouldEmit := TRUE;
            END_IF;
        ELSIF RegulatedEventTypeId = UDINT#16#0401 THEN
            RegulatedRecordLength := UINT#44 + OPENOT_ValueSlotEncodedLength(PayloadLength := RegulatedActorLength) + UINT#8;
            IF RegulatedHasWorkstation THEN
                RegulatedRecordLength := RegulatedRecordLength + OPENOT_ValueSlotEncodedLength(PayloadLength := RegulatedWorkstationLength);
            END_IF;
            IF RegulatedHasRole THEN
                RegulatedRecordLength := RegulatedRecordLength + UINT#8;
            END_IF;
            IF (RegulatedActorLength = UINT#0) OR (NOT RegulatedHasAuthResult) THEN
                StateDroppedCommandCount := StateDroppedCommandCount + ULINT#1;
                StateLastCommandError := UINT#2;
                Error := TRUE;
            ELSE
                RegulatedShouldEmit := TRUE;
            END_IF;
        ELSIF RegulatedEventTypeId = UDINT#16#0402 THEN
            RegulatedRecordLength := UINT#44 + OPENOT_ValueSlotEncodedLength(PayloadLength := RegulatedActorLength);
            IF RegulatedHasWorkstation THEN
                RegulatedRecordLength := RegulatedRecordLength + OPENOT_ValueSlotEncodedLength(PayloadLength := RegulatedWorkstationLength);
            END_IF;
            IF RegulatedActorLength = UINT#0 THEN
                StateDroppedCommandCount := StateDroppedCommandCount + ULINT#1;
                StateLastCommandError := UINT#2;
                Error := TRUE;
            ELSE
                RegulatedShouldEmit := TRUE;
            END_IF;
        ELSIF RegulatedEventTypeId = UDINT#16#0405 THEN
            RegulatedRecordLength := UINT#44 + OPENOT_ValueSlotEncodedLength(PayloadLength := RegulatedActorLength);
            IF RegulatedHasWorkstation THEN
                RegulatedRecordLength := RegulatedRecordLength + OPENOT_ValueSlotEncodedLength(PayloadLength := RegulatedWorkstationLength);
            END_IF;
            IF RegulatedHasReason THEN
                RegulatedRecordLength := RegulatedRecordLength + OPENOT_ValueSlotEncodedLength(PayloadLength := RegulatedReasonLength);
            END_IF;
            IF RegulatedActorLength = UINT#0 THEN
                StateDroppedCommandCount := StateDroppedCommandCount + ULINT#1;
                StateLastCommandError := UINT#2;
                Error := TRUE;
            ELSE
                RegulatedShouldEmit := TRUE;
            END_IF;
        ELSE
            StateDroppedCommandCount := StateDroppedCommandCount + ULINT#1;
            StateLastCommandError := UINT#1;
            Error := TRUE;
        END_IF;

        IF RegulatedShouldEmit AND (RegulatedRecordLength > UINT#256) THEN
            StateDroppedCommandCount := StateDroppedCommandCount + ULINT#1;
            StateLastCommandError := UINT#3;
            RegulatedShouldEmit := FALSE;
            Error := TRUE;
        END_IF;

        IF RegulatedShouldEmit THEN
            Found := FALSE;
            FreeFound := FALSE;
            EntryIndex := UINT#0;

            FOR Index := UINT#0 TO UINT#15 DO
                IF Sources[Index].Used AND (Sources[Index].SourceId = ActiveSourceId) THEN
                    IF NOT Found THEN
                        EntryIndex := Index;
                        Found := TRUE;
                    END_IF;
                END_IF;
            END_FOR;

            IF NOT Found THEN
                FOR Index := UINT#0 TO UINT#15 DO
                    IF (NOT Sources[Index].Used) AND (NOT FreeFound) THEN
                        EntryIndex := Index;
                        FreeFound := TRUE;
                    END_IF;
                END_FOR;

                IF FreeFound THEN
                    Sources[EntryIndex].Used := TRUE;
                    Sources[EntryIndex].SourceId := ActiveSourceId;
                    Sources[EntryIndex].NextSeq := ULINT#0;
                ELSE
                    SourceTableFull := TRUE;
                    StateDroppedCommandCount := StateDroppedCommandCount + ULINT#1;
                    StateLastCommandError := UINT#4;
                    RegulatedShouldEmit := FALSE;
                    Error := TRUE;
                END_IF;
            END_IF;
        END_IF;

        IF RegulatedShouldEmit THEN
            AssignedSeq := Sources[EntryIndex].NextSeq;
            EmitSourceTime := OPENOT_NextSourceTime(
                UseSourceTimeInput := UseSourceTimeInput,
                SourceTimeInput := SourceTimeInput,
                StateSourceTime := StateSourceTime);
            RegulatedEncoder(
                SourceId := ActiveSourceId,
                RunId := StateRunId,
                Seq := AssignedSeq,
                SourceTime := EmitSourceTime,
                EventTypeId := RegulatedEventTypeId,
                ActionId := RegulatedActionId,
                Actor := RegulatedActor,
                HasContext1 := RegulatedHasContext1,
                Context1 := RegulatedContext1,
                HasContext2 := RegulatedHasContext2,
                Context2 := RegulatedContext2,
                HasContext3 := RegulatedHasContext3,
                Context3 := RegulatedContext3,
                HasContext4 := RegulatedHasContext4,
                Context4 := RegulatedContext4,
                HasAuthResult := RegulatedHasAuthResult,
                AuthResult := RegulatedAuthResult,
                HasWorkstation := RegulatedHasWorkstation,
                Workstation := RegulatedWorkstation,
                HasRole := RegulatedHasRole,
                Role := RegulatedRole,
                HasReason := RegulatedHasReason,
                Reason := RegulatedReason);
            RingProducer(
                Execute := TRUE,
                Record := RegulatedEncoder.Buffer,
                Length := RegulatedEncoder.Length,
                RunId := StateRunId,
                SourceId := ActiveSourceId,
                Seq := AssignedSeq);

            IF RingProducer.Done AND (NOT RingProducer.Error) THEN
                AppendOk := OPENOT_AppendScanRecord(
                    Record := RegulatedEncoder.Buffer,
                    Length := RegulatedEncoder.Length,
                    ScanRecords := ScanRecords,
                    ScanRecordOffsets := ScanRecordOffsets,
                    ScanRecordLengths := ScanRecordLengths,
                    ScanWritePos := ScanWritePos,
                    ScanRecordCount := ScanRecordCount);
            ELSE
                AppendOk := FALSE;
            END_IF;

            IF AppendOk THEN
                Sources[EntryIndex].NextSeq := AssignedSeq + ULINT#1;
                StatePublishedRecordCount := StatePublishedRecordCount + ULINT#1;
                IF EmissionCount < UINT#32 THEN
                    Emissions[EmissionCount].EventTypeId := RegulatedEventTypeId;
                    Emissions[EmissionCount].RunId := StateRunId;
                    Emissions[EmissionCount].SourceId := ActiveSourceId;
                    Emissions[EmissionCount].Seq := AssignedSeq;
                    EmissionCount := EmissionCount + UINT#1;
                END_IF;
                AttestableRecordOk := OPENOT_RecordAttestableSeq(
                    AttestableId := AttestableId,
                    SourceId := ActiveSourceId,
                    Seq := AssignedSeq,
                    AttestableEvents := AttestableEvents);
                Done := TRUE;
            ELSE
                StateDroppedCommandCount := StateDroppedCommandCount + ULINT#1;
                StateLastCommandError := UINT#5;
                Error := TRUE;
            END_IF;
        END_IF;
    ELSIF Op = UINT#15 THEN
        ActiveSourceId := SourceId;
        IF ActiveSourceId = UDINT#0 THEN
            ActiveSourceId := UDINT#1;
        END_IF;

        StateFound := FALSE;
        StateFreeFound := FALSE;
        StateIndex := UINT#0;
        AuditShouldEmit := FALSE;

        IF ValueId = UDINT#0 THEN
            StateDroppedCommandCount := StateDroppedCommandCount + ULINT#1;
            StateLastCommandError := UINT#1;
            Error := TRUE;
        ELSIF (ValueTypeTag = BYTE#16#09) AND (ValuePayloadLength = UINT#4) THEN
            FOR Index := UINT#0 TO UINT#15 DO
                IF RealValueStates[Index].Used AND (RealValueStates[Index].ValueId = ValueId) THEN
                    IF NOT StateFound THEN
                        StateIndex := Index;
                        StateFound := TRUE;
                    END_IF;
                ELSIF (NOT StateFound) AND (NOT RealValueStates[Index].Used) AND (NOT StateFreeFound) THEN
                    StateIndex := Index;
                    StateFreeFound := TRUE;
                END_IF;
            END_FOR;

            IF StateFound THEN
                IF RealValueStates[StateIndex].HasLast THEN
                    AuditShouldEmit := ValueReal <> RealValueStates[StateIndex].LastValue;
                    IF NOT AuditShouldEmit THEN
                        Done := TRUE;
                    END_IF;
                ELSE
                    RealValueStates[StateIndex].HasLast := TRUE;
                    RealValueStates[StateIndex].LastValue := ValueReal;
                    Done := TRUE;
                END_IF;
            ELSIF StateFreeFound THEN
                RealValueStates[StateIndex].Used := TRUE;
                RealValueStates[StateIndex].ValueId := ValueId;
                RealValueStates[StateIndex].HasLast := TRUE;
                RealValueStates[StateIndex].LastValue := ValueReal;
                RealValueStates[StateIndex].HasLastEmitSourceTime := FALSE;
                RealValueStates[StateIndex].HasBandCenter := FALSE;
                Done := TRUE;
            ELSE
                StateDroppedCommandCount := StateDroppedCommandCount + ULINT#1;
                StateLastCommandError := UINT#4;
                Error := TRUE;
            END_IF;
        ELSIF (ValueTypeTag = BYTE#16#06) AND (ValuePayloadLength = UINT#4) THEN
            FOR Index := UINT#0 TO UINT#15 DO
                IF IntValueStates[Index].Used AND (IntValueStates[Index].ValueId = ValueId) THEN
                    IF NOT StateFound THEN
                        StateIndex := Index;
                        StateFound := TRUE;
                    END_IF;
                ELSIF (NOT StateFound) AND (NOT IntValueStates[Index].Used) AND (NOT StateFreeFound) THEN
                    StateIndex := Index;
                    StateFreeFound := TRUE;
                END_IF;
            END_FOR;

            IF StateFound THEN
                IF IntValueStates[StateIndex].HasLast THEN
                    AuditShouldEmit := ValueInt <> IntValueStates[StateIndex].LastValue;
                    IF NOT AuditShouldEmit THEN
                        Done := TRUE;
                    END_IF;
                ELSE
                    IntValueStates[StateIndex].HasLast := TRUE;
                    IntValueStates[StateIndex].LastValue := ValueInt;
                    Done := TRUE;
                END_IF;
            ELSIF StateFreeFound THEN
                IntValueStates[StateIndex].Used := TRUE;
                IntValueStates[StateIndex].ValueId := ValueId;
                IntValueStates[StateIndex].HasLast := TRUE;
                IntValueStates[StateIndex].LastValue := ValueInt;
                IntValueStates[StateIndex].HasLastEmitSourceTime := FALSE;
                Done := TRUE;
            ELSE
                StateDroppedCommandCount := StateDroppedCommandCount + ULINT#1;
                StateLastCommandError := UINT#4;
                Error := TRUE;
            END_IF;
        ELSIF ValueTypeTag = BYTE#16#0C THEN
            FOR Index := UINT#0 TO UINT#15 DO
                IF StringValueStates[Index].Used AND (StringValueStates[Index].ValueId = ValueId) THEN
                    IF NOT StateFound THEN
                        StateIndex := Index;
                        StateFound := TRUE;
                    END_IF;
                ELSIF (NOT StateFound) AND (NOT StringValueStates[Index].Used) AND (NOT StateFreeFound) THEN
                    StateIndex := Index;
                    StateFreeFound := TRUE;
                END_IF;
            END_FOR;

            IF StateFound THEN
                IF StringValueStates[StateIndex].HasLast THEN
                    AuditShouldEmit := ValueString <> StringValueStates[StateIndex].LastValue;
                    IF NOT AuditShouldEmit THEN
                        Done := TRUE;
                    END_IF;
                ELSE
                    StringValueStates[StateIndex].HasLast := TRUE;
                    StringValueStates[StateIndex].LastValue := ValueString;
                    Done := TRUE;
                END_IF;
            ELSIF StateFreeFound THEN
                StringValueStates[StateIndex].Used := TRUE;
                StringValueStates[StateIndex].ValueId := ValueId;
                StringValueStates[StateIndex].HasLast := TRUE;
                StringValueStates[StateIndex].LastValue := ValueString;
                StringValueStates[StateIndex].HasLastEmitSourceTime := FALSE;
                Done := TRUE;
            ELSE
                StateDroppedCommandCount := StateDroppedCommandCount + ULINT#1;
                StateLastCommandError := UINT#4;
                Error := TRUE;
            END_IF;
        ELSIF (ValuePayloadLength = UINT#1) OR (ValuePayloadLength = UINT#2) OR (ValuePayloadLength = UINT#4) OR (ValuePayloadLength = UINT#8) THEN
            FOR Index := UINT#0 TO UINT#15 DO
                IF BitsValueStates[Index].Used AND (BitsValueStates[Index].ValueId = ValueId) THEN
                    IF NOT StateFound THEN
                        StateIndex := Index;
                        StateFound := TRUE;
                    END_IF;
                ELSIF (NOT StateFound) AND (NOT BitsValueStates[Index].Used) AND (NOT StateFreeFound) THEN
                    StateIndex := Index;
                    StateFreeFound := TRUE;
                END_IF;
            END_FOR;

            IF StateFound THEN
                IF BitsValueStates[StateIndex].HasLast THEN
                    AuditShouldEmit := ValueBits <> BitsValueStates[StateIndex].LastBits;
                    IF NOT AuditShouldEmit THEN
                        Done := TRUE;
                    END_IF;
                ELSE
                    BitsValueStates[StateIndex].HasLast := TRUE;
                    BitsValueStates[StateIndex].LastBits := ValueBits;
                    Done := TRUE;
                END_IF;
            ELSIF StateFreeFound THEN
                BitsValueStates[StateIndex].Used := TRUE;
                BitsValueStates[StateIndex].ValueId := ValueId;
                BitsValueStates[StateIndex].HasLast := TRUE;
                BitsValueStates[StateIndex].LastBits := ValueBits;
                BitsValueStates[StateIndex].HasLastEmitSourceTime := FALSE;
                Done := TRUE;
            ELSE
                StateDroppedCommandCount := StateDroppedCommandCount + ULINT#1;
                StateLastCommandError := UINT#4;
                Error := TRUE;
            END_IF;
        ELSE
            StateDroppedCommandCount := StateDroppedCommandCount + ULINT#1;
            StateLastCommandError := UINT#1;
            Error := TRUE;
        END_IF;

        IF AuditShouldEmit AND (NOT Error) THEN
            AuditActorLength := INT_TO_UINT(LEN(AuditActor));
            AuditReasonLength := INT_TO_UINT(LEN(AuditReason));
            IF AuditActorLength > UINT#96 THEN
                AuditActorLength := UINT#96;
            END_IF;
            IF AuditReasonLength > UINT#96 THEN
                AuditReasonLength := UINT#96;
            END_IF;

            AuditRecordLength := UINT#52 + OPENOT_ValueSlotEncodedLength(PayloadLength := AuditActorLength) + OPENOT_ValueSlotEncodedLength(PayloadLength := AuditReasonLength);
            IF (ValueTypeTag = BYTE#16#09) OR (ValueTypeTag = BYTE#16#06) THEN
                AuditRecordLength := AuditRecordLength + UINT#16;
            ELSIF ValueTypeTag = BYTE#16#0C THEN
                AuditPreviousStringLength := INT_TO_UINT(LEN(StringValueStates[StateIndex].LastValue));
                AuditNewStringLength := INT_TO_UINT(LEN(ValueString));
                IF AuditPreviousStringLength > UINT#96 THEN
                    AuditPreviousStringLength := UINT#96;
                END_IF;
                IF AuditNewStringLength > UINT#96 THEN
                    AuditNewStringLength := UINT#96;
                END_IF;
                AuditRecordLength := AuditRecordLength + OPENOT_ValueSlotEncodedLength(PayloadLength := AuditPreviousStringLength) + OPENOT_ValueSlotEncodedLength(PayloadLength := AuditNewStringLength);
            ELSE
                AuditRecordLength := AuditRecordLength + OPENOT_ValueSlotEncodedLength(PayloadLength := ValuePayloadLength) + OPENOT_ValueSlotEncodedLength(PayloadLength := ValuePayloadLength);
            END_IF;
            IF AuditHasAuthResult THEN
                AuditRecordLength := AuditRecordLength + UINT#8;
            END_IF;

            IF (AuditActorLength = UINT#0) OR (AuditReasonLength = UINT#0) THEN
                StateDroppedCommandCount := StateDroppedCommandCount + ULINT#1;
                StateLastCommandError := UINT#2;
                AuditShouldEmit := FALSE;
                Error := TRUE;
            ELSIF AuditRecordLength > UINT#256 THEN
                StateDroppedCommandCount := StateDroppedCommandCount + ULINT#1;
                StateLastCommandError := UINT#3;
                AuditShouldEmit := FALSE;
                Error := TRUE;
            END_IF;
        END_IF;

        IF AuditShouldEmit AND (NOT Error) THEN
            Found := FALSE;
            FreeFound := FALSE;
            EntryIndex := UINT#0;

            FOR Index := UINT#0 TO UINT#15 DO
                IF Sources[Index].Used AND (Sources[Index].SourceId = ActiveSourceId) THEN
                    IF NOT Found THEN
                        EntryIndex := Index;
                        Found := TRUE;
                    END_IF;
                END_IF;
            END_FOR;

            IF NOT Found THEN
                FOR Index := UINT#0 TO UINT#15 DO
                    IF (NOT Sources[Index].Used) AND (NOT FreeFound) THEN
                        EntryIndex := Index;
                        FreeFound := TRUE;
                    END_IF;
                END_FOR;

                IF FreeFound THEN
                    Sources[EntryIndex].Used := TRUE;
                    Sources[EntryIndex].SourceId := ActiveSourceId;
                    Sources[EntryIndex].NextSeq := ULINT#0;
                ELSE
                    SourceTableFull := TRUE;
                    StateDroppedCommandCount := StateDroppedCommandCount + ULINT#1;
                    StateLastCommandError := UINT#4;
                    AuditShouldEmit := FALSE;
                    Error := TRUE;
                END_IF;
            END_IF;
        END_IF;

        IF AuditShouldEmit AND (NOT Error) THEN
            AssignedSeq := Sources[EntryIndex].NextSeq;
            EmitSourceTime := OPENOT_NextSourceTime(
                UseSourceTimeInput := UseSourceTimeInput,
                SourceTimeInput := SourceTimeInput,
                StateSourceTime := StateSourceTime);

            IF ValueTypeTag = BYTE#16#09 THEN
                ParameterRealEncoder(
                    SourceId := ActiveSourceId,
                    RunId := StateRunId,
                    Seq := AssignedSeq,
                    SourceTime := EmitSourceTime,
                    ValueId := ValueId,
                    PreviousValue := RealValueStates[StateIndex].LastValue,
                    NewValue := ValueReal,
                    Actor := AuditActor,
                    Reason := AuditReason,
                    HasAuthResult := AuditHasAuthResult,
                    AuthResult := AuditAuthResult);
                RingProducer(
                    Execute := TRUE,
                    Record := ParameterRealEncoder.Buffer,
                    Length := ParameterRealEncoder.Length,
                    RunId := StateRunId,
                    SourceId := ActiveSourceId,
                    Seq := AssignedSeq);

                IF RingProducer.Done AND (NOT RingProducer.Error) THEN
                    AppendOk := OPENOT_AppendScanRecord(
                        Record := ParameterRealEncoder.Buffer,
                        Length := ParameterRealEncoder.Length,
                        ScanRecords := ScanRecords,
                        ScanRecordOffsets := ScanRecordOffsets,
                        ScanRecordLengths := ScanRecordLengths,
                        ScanWritePos := ScanWritePos,
                        ScanRecordCount := ScanRecordCount);
                ELSE
                    AppendOk := FALSE;
                END_IF;
            ELSIF ValueTypeTag = BYTE#16#06 THEN
                ParameterIntEncoder(
                    SourceId := ActiveSourceId,
                    RunId := StateRunId,
                    Seq := AssignedSeq,
                    SourceTime := EmitSourceTime,
                    ValueId := ValueId,
                    PreviousValue := IntValueStates[StateIndex].LastValue,
                    NewValue := ValueInt,
                    Actor := AuditActor,
                    Reason := AuditReason,
                    HasAuthResult := AuditHasAuthResult,
                    AuthResult := AuditAuthResult);
                RingProducer(
                    Execute := TRUE,
                    Record := ParameterIntEncoder.Buffer,
                    Length := ParameterIntEncoder.Length,
                    RunId := StateRunId,
                    SourceId := ActiveSourceId,
                    Seq := AssignedSeq);

                IF RingProducer.Done AND (NOT RingProducer.Error) THEN
                    AppendOk := OPENOT_AppendScanRecord(
                        Record := ParameterIntEncoder.Buffer,
                        Length := ParameterIntEncoder.Length,
                        ScanRecords := ScanRecords,
                        ScanRecordOffsets := ScanRecordOffsets,
                        ScanRecordLengths := ScanRecordLengths,
                        ScanWritePos := ScanWritePos,
                        ScanRecordCount := ScanRecordCount);
                ELSE
                    AppendOk := FALSE;
                END_IF;
            ELSIF ValueTypeTag = BYTE#16#0C THEN
                ParameterStringEncoder(
                    SourceId := ActiveSourceId,
                    RunId := StateRunId,
                    Seq := AssignedSeq,
                    SourceTime := EmitSourceTime,
                    ValueId := ValueId,
                    PreviousValue := StringValueStates[StateIndex].LastValue,
                    NewValue := ValueString,
                    Actor := AuditActor,
                    Reason := AuditReason,
                    HasAuthResult := AuditHasAuthResult,
                    AuthResult := AuditAuthResult);
                RingProducer(
                    Execute := TRUE,
                    Record := ParameterStringEncoder.Buffer,
                    Length := ParameterStringEncoder.Length,
                    RunId := StateRunId,
                    SourceId := ActiveSourceId,
                    Seq := AssignedSeq);

                IF RingProducer.Done AND (NOT RingProducer.Error) THEN
                    AppendOk := OPENOT_AppendScanRecord(
                        Record := ParameterStringEncoder.Buffer,
                        Length := ParameterStringEncoder.Length,
                        ScanRecords := ScanRecords,
                        ScanRecordOffsets := ScanRecordOffsets,
                        ScanRecordLengths := ScanRecordLengths,
                        ScanWritePos := ScanWritePos,
                        ScanRecordCount := ScanRecordCount);
                ELSE
                    AppendOk := FALSE;
                END_IF;
            ELSE
                ParameterBitsEncoder(
                    SourceId := ActiveSourceId,
                    RunId := StateRunId,
                    Seq := AssignedSeq,
                    SourceTime := EmitSourceTime,
                    ValueId := ValueId,
                    TypeTag := ValueTypeTag,
                    PayloadLength := ValuePayloadLength,
                    PreviousBits := BitsValueStates[StateIndex].LastBits,
                    NewBits := ValueBits,
                    Actor := AuditActor,
                    Reason := AuditReason,
                    HasAuthResult := AuditHasAuthResult,
                    AuthResult := AuditAuthResult);
                RingProducer(
                    Execute := TRUE,
                    Record := ParameterBitsEncoder.Buffer,
                    Length := ParameterBitsEncoder.Length,
                    RunId := StateRunId,
                    SourceId := ActiveSourceId,
                    Seq := AssignedSeq);

                IF RingProducer.Done AND (NOT RingProducer.Error) THEN
                    AppendOk := OPENOT_AppendScanRecord(
                        Record := ParameterBitsEncoder.Buffer,
                        Length := ParameterBitsEncoder.Length,
                        ScanRecords := ScanRecords,
                        ScanRecordOffsets := ScanRecordOffsets,
                        ScanRecordLengths := ScanRecordLengths,
                        ScanWritePos := ScanWritePos,
                        ScanRecordCount := ScanRecordCount);
                ELSE
                    AppendOk := FALSE;
                END_IF;
            END_IF;

            IF AppendOk THEN
                Sources[EntryIndex].NextSeq := AssignedSeq + ULINT#1;
                StatePublishedRecordCount := StatePublishedRecordCount + ULINT#1;
                IF EmissionCount < UINT#32 THEN
                    Emissions[EmissionCount].EventTypeId := UDINT#16#0403;
                    Emissions[EmissionCount].RunId := StateRunId;
                    Emissions[EmissionCount].SourceId := ActiveSourceId;
                    Emissions[EmissionCount].Seq := AssignedSeq;
                    EmissionCount := EmissionCount + UINT#1;
                END_IF;
                AttestableRecordOk := OPENOT_RecordAttestableSeq(
                    AttestableId := AttestableId,
                    SourceId := ActiveSourceId,
                    Seq := AssignedSeq,
                    AttestableEvents := AttestableEvents);

                IF ValueTypeTag = BYTE#16#09 THEN
                    RealValueStates[StateIndex].HasLast := TRUE;
                    RealValueStates[StateIndex].LastValue := ValueReal;
                ELSIF ValueTypeTag = BYTE#16#06 THEN
                    IntValueStates[StateIndex].HasLast := TRUE;
                    IntValueStates[StateIndex].LastValue := ValueInt;
                ELSIF ValueTypeTag = BYTE#16#0C THEN
                    StringValueStates[StateIndex].HasLast := TRUE;
                    StringValueStates[StateIndex].LastValue := ValueString;
                ELSE
                    BitsValueStates[StateIndex].HasLast := TRUE;
                    BitsValueStates[StateIndex].LastBits := ValueBits;
                END_IF;
                Done := TRUE;
            ELSE
                StateDroppedCommandCount := StateDroppedCommandCount + ULINT#1;
                StateLastCommandError := UINT#5;
                Error := TRUE;
            END_IF;
        END_IF;
    ELSIF Op = UINT#16 THEN
        ActiveSourceId := SourceId;
        IF ActiveSourceId = UDINT#0 THEN
            ActiveSourceId := UDINT#1;
        END_IF;

        SignatureShouldEmit := TRUE;
        Found := FALSE;
        FreeFound := FALSE;
        EntryIndex := UINT#0;

        IF (SignatureAttestsId = UDINT#0) OR (SignatureAttestsId > UDINT#32) THEN
            StateDroppedCommandCount := StateDroppedCommandCount + ULINT#1;
            StateLastCommandError := UINT#1;
            SignatureShouldEmit := FALSE;
            Error := TRUE;
        ELSE
            FOR Index := UINT#0 TO UINT#31 DO
                IF AttestableEvents[Index].Used AND (AttestableEvents[Index].AttestableId = SignatureAttestsId) THEN
                    IF (NOT Found) AND AttestableEvents[Index].HasSeq AND (AttestableEvents[Index].SourceId = ActiveSourceId) THEN
                        SignatureSeq := AttestableEvents[Index].LastSeq;
                        Found := TRUE;
                    END_IF;
                END_IF;
            END_FOR;

            IF NOT Found THEN
                StateDroppedCommandCount := StateDroppedCommandCount + ULINT#1;
                StateLastCommandError := UINT#6;
                SignatureShouldEmit := FALSE;
                Error := TRUE;
            END_IF;
        END_IF;

        IF SignatureShouldEmit THEN
            SignatureActorLength := INT_TO_UINT(LEN(SignatureActor));
            IF SignatureActorLength > UINT#96 THEN
                SignatureActorLength := UINT#96;
            END_IF;

            SignatureRecordLength := UINT#72 + OPENOT_ValueSlotEncodedLength(PayloadLength := SignatureActorLength);
            IF SignatureHasAuthResult THEN
                SignatureRecordLength := SignatureRecordLength + UINT#8;
            END_IF;

            IF SignatureActorLength = UINT#0 THEN
                StateDroppedCommandCount := StateDroppedCommandCount + ULINT#1;
                StateLastCommandError := UINT#2;
                SignatureShouldEmit := FALSE;
                Error := TRUE;
            ELSIF SignatureRecordLength > UINT#256 THEN
                StateDroppedCommandCount := StateDroppedCommandCount + ULINT#1;
                StateLastCommandError := UINT#3;
                SignatureShouldEmit := FALSE;
                Error := TRUE;
            END_IF;
        END_IF;

        IF SignatureShouldEmit THEN
            Found := FALSE;
            FreeFound := FALSE;
            EntryIndex := UINT#0;

            FOR Index := UINT#0 TO UINT#15 DO
                IF Sources[Index].Used AND (Sources[Index].SourceId = ActiveSourceId) THEN
                    IF NOT Found THEN
                        EntryIndex := Index;
                        Found := TRUE;
                    END_IF;
                END_IF;
            END_FOR;

            IF NOT Found THEN
                FOR Index := UINT#0 TO UINT#15 DO
                    IF (NOT Sources[Index].Used) AND (NOT FreeFound) THEN
                        EntryIndex := Index;
                        FreeFound := TRUE;
                    END_IF;
                END_FOR;

                IF FreeFound THEN
                    Sources[EntryIndex].Used := TRUE;
                    Sources[EntryIndex].SourceId := ActiveSourceId;
                    Sources[EntryIndex].NextSeq := ULINT#0;
                ELSE
                    SourceTableFull := TRUE;
                    StateDroppedCommandCount := StateDroppedCommandCount + ULINT#1;
                    StateLastCommandError := UINT#4;
                    SignatureShouldEmit := FALSE;
                    Error := TRUE;
                END_IF;
            END_IF;
        END_IF;

        IF SignatureShouldEmit THEN
            AssignedSeq := Sources[EntryIndex].NextSeq;
            EmitSourceTime := OPENOT_NextSourceTime(
                UseSourceTimeInput := UseSourceTimeInput,
                SourceTimeInput := SourceTimeInput,
                StateSourceTime := StateSourceTime);
            ESignatureEncoder(
                SourceId := ActiveSourceId,
                RunId := StateRunId,
                Seq := AssignedSeq,
                SourceTime := EmitSourceTime,
                ActionId := SignatureActionId,
                Actor := SignatureActor,
                SignatureMeaning := SignatureMeaning,
                SignedEventSeq := SignatureSeq,
                HasAuthResult := SignatureHasAuthResult,
                AuthResult := SignatureAuthResult);
            RingProducer(
                Execute := TRUE,
                Record := ESignatureEncoder.Buffer,
                Length := ESignatureEncoder.Length,
                RunId := StateRunId,
                SourceId := ActiveSourceId,
                Seq := AssignedSeq);

            IF RingProducer.Done AND (NOT RingProducer.Error) THEN
                AppendOk := OPENOT_AppendScanRecord(
                    Record := ESignatureEncoder.Buffer,
                    Length := ESignatureEncoder.Length,
                    ScanRecords := ScanRecords,
                    ScanRecordOffsets := ScanRecordOffsets,
                    ScanRecordLengths := ScanRecordLengths,
                    ScanWritePos := ScanWritePos,
                    ScanRecordCount := ScanRecordCount);
            ELSE
                AppendOk := FALSE;
            END_IF;

            IF AppendOk THEN
                Sources[EntryIndex].NextSeq := AssignedSeq + ULINT#1;
                StatePublishedRecordCount := StatePublishedRecordCount + ULINT#1;
                IF EmissionCount < UINT#32 THEN
                    Emissions[EmissionCount].EventTypeId := UDINT#16#0404;
                    Emissions[EmissionCount].RunId := StateRunId;
                    Emissions[EmissionCount].SourceId := ActiveSourceId;
                    Emissions[EmissionCount].Seq := AssignedSeq;
                    EmissionCount := EmissionCount + UINT#1;
                END_IF;
                Done := TRUE;
            ELSE
                StateDroppedCommandCount := StateDroppedCommandCount + ULINT#1;
                StateLastCommandError := UINT#5;
                Error := TRUE;
            END_IF;
        END_IF;
    ELSIF Op = UINT#1 THEN
        IF PendingOpen THEN
            Error := TRUE;
        ELSE
            EmitSeq := StateSystemSeq;
            StateSystemSeq := StateSystemSeq + ULINT#1;
            EmitSourceTime := OPENOT_NextSourceTime(
                UseSourceTimeInput := UseSourceTimeInput,
                SourceTimeInput := SourceTimeInput,
                StateSourceTime := StateSourceTime);
            StoppedEncoder(RunId := StateRunId, Seq := EmitSeq, SourceTime := EmitSourceTime);
            RingProducer(
                Execute := TRUE,
                Record := StoppedEncoder.Buffer,
                Length := StoppedEncoder.Length,
                RunId := StateRunId,
                SourceId := UDINT#0,
                Seq := EmitSeq);

            IF RingProducer.Done AND (NOT RingProducer.Error) THEN
                AppendOk := OPENOT_AppendScanRecord(
                    Record := StoppedEncoder.Buffer,
                    Length := StoppedEncoder.Length,
                    ScanRecords := ScanRecords,
                    ScanRecordOffsets := ScanRecordOffsets,
                    ScanRecordLengths := ScanRecordLengths,
                    ScanWritePos := ScanWritePos,
                    ScanRecordCount := ScanRecordCount);
                IF AppendOk THEN
                    StatePublishedRecordCount := StatePublishedRecordCount + ULINT#1;
                    IF EmissionCount < UINT#32 THEN
                        Emissions[EmissionCount].EventTypeId := UDINT#16#0102;
                        Emissions[EmissionCount].RunId := StateRunId;
                        Emissions[EmissionCount].SourceId := UDINT#0;
                        Emissions[EmissionCount].Seq := EmitSeq;
                        EmissionCount := EmissionCount + UINT#1;
                    END_IF;
                ELSE
                    Error := TRUE;
                END_IF;
            ELSE
                Error := TRUE;
            END_IF;

            SourceLoopActive := NOT Error;
            HasLastSourceId := FALSE;
            LastSourceId := UDINT#0;

            WHILE SourceLoopActive DO
                CandidateFound := FALSE;
                SelectedIndex := UINT#0;
                NextSourceId := UDINT#0;

                FOR Index := UINT#0 TO UINT#15 DO
                    IF Sources[Index].Used AND ((NOT HasLastSourceId) OR (Sources[Index].SourceId > LastSourceId)) THEN
                        IF (NOT CandidateFound) OR (Sources[Index].SourceId < NextSourceId) THEN
                            CandidateFound := TRUE;
                            SelectedIndex := Index;
                            NextSourceId := Sources[Index].SourceId;
                        END_IF;
                    END_IF;
                END_FOR;

                IF CandidateFound THEN
                    ProducedCount := Sources[SelectedIndex].NextSeq;
                    EmitSourceTime := OPENOT_NextSourceTime(
                        UseSourceTimeInput := UseSourceTimeInput,
                        SourceTimeInput := SourceTimeInput,
                        StateSourceTime := StateSourceTime);
                    HighWaterEncoder(
                        SourceId := Sources[SelectedIndex].SourceId,
                        RunId := StateRunId,
                        ProducedCount := ProducedCount,
                        SourceTime := EmitSourceTime);
                    RingProducer(
                        Execute := TRUE,
                        Record := HighWaterEncoder.Buffer,
                        Length := HighWaterEncoder.Length,
                        RunId := StateRunId,
                        SourceId := Sources[SelectedIndex].SourceId,
                        Seq := ProducedCount);

                    IF RingProducer.Done AND (NOT RingProducer.Error) THEN
                        AppendOk := OPENOT_AppendScanRecord(
                            Record := HighWaterEncoder.Buffer,
                            Length := HighWaterEncoder.Length,
                            ScanRecords := ScanRecords,
                            ScanRecordOffsets := ScanRecordOffsets,
                            ScanRecordLengths := ScanRecordLengths,
                            ScanWritePos := ScanWritePos,
                            ScanRecordCount := ScanRecordCount);
                        IF AppendOk THEN
                            Sources[SelectedIndex].NextSeq := ProducedCount + ULINT#1;
                            StatePublishedRecordCount := StatePublishedRecordCount + ULINT#1;
                            IF EmissionCount < UINT#32 THEN
                                Emissions[EmissionCount].EventTypeId := UDINT#16#0108;
                                Emissions[EmissionCount].RunId := StateRunId;
                                Emissions[EmissionCount].SourceId := Sources[SelectedIndex].SourceId;
                                Emissions[EmissionCount].Seq := ProducedCount;
                                EmissionCount := EmissionCount + UINT#1;
                            END_IF;
                            LastSourceId := NextSourceId;
                            HasLastSourceId := TRUE;
                        ELSE
                            Error := TRUE;
                            SourceLoopActive := FALSE;
                        END_IF;
                    ELSE
                        Error := TRUE;
                        SourceLoopActive := FALSE;
                    END_IF;
                ELSE
                    SourceLoopActive := FALSE;
                END_IF;
            END_WHILE;

            IF NOT Error THEN
                FOR Index := UINT#0 TO UINT#7 DO
                    PendingDefHashOld[Index] := DefHashOld[Index];
                    PendingDefHashNew[Index] := DefHashNew[Index];
                END_FOR;
                PendingEpochId := TransitionEpochId;
                PendingDefChangedEmitted := FALSE;
                PendingOpen := TRUE;
                Done := TRUE;
            END_IF;
        END_IF;
    ELSIF Op = UINT#2 THEN
        IF NOT PendingOpen THEN
            Error := TRUE;
        ELSIF PendingDefChangedEmitted THEN
            Error := TRUE;
        ELSE
            EmitSeq := StateSystemSeq;
            StateSystemSeq := StateSystemSeq + ULINT#1;
            EmitSourceTime := OPENOT_NextSourceTime(
                UseSourceTimeInput := UseSourceTimeInput,
                SourceTimeInput := SourceTimeInput,
                StateSourceTime := StateSourceTime);
            DefinitionEncoder(
                RunId := StateRunId,
                Seq := EmitSeq,
                DefHashOld := PendingDefHashOld,
                DefHashNew := PendingDefHashNew,
                EpochId := PendingEpochId,
                SourceTime := EmitSourceTime);
            RingProducer(
                Execute := TRUE,
                Record := DefinitionEncoder.Buffer,
                Length := DefinitionEncoder.Length,
                RunId := StateRunId,
                SourceId := UDINT#0,
                Seq := EmitSeq);

            IF RingProducer.Done AND (NOT RingProducer.Error) THEN
                AppendOk := OPENOT_AppendScanRecord(
                    Record := DefinitionEncoder.Buffer,
                    Length := DefinitionEncoder.Length,
                    ScanRecords := ScanRecords,
                    ScanRecordOffsets := ScanRecordOffsets,
                    ScanRecordLengths := ScanRecordLengths,
                    ScanWritePos := ScanWritePos,
                    ScanRecordCount := ScanRecordCount);
                IF AppendOk THEN
                    StatePublishedRecordCount := StatePublishedRecordCount + ULINT#1;
                    IF EmissionCount < UINT#32 THEN
                        Emissions[EmissionCount].EventTypeId := UDINT#16#0106;
                        Emissions[EmissionCount].RunId := StateRunId;
                        Emissions[EmissionCount].SourceId := UDINT#0;
                        Emissions[EmissionCount].Seq := EmitSeq;
                        EmissionCount := EmissionCount + UINT#1;
                    END_IF;
                    PendingDefChangedEmitted := TRUE;
                    Done := TRUE;
                ELSE
                    Error := TRUE;
                END_IF;
            ELSE
                Error := TRUE;
            END_IF;
        END_IF;
    ELSIF Op = UINT#3 THEN
        IF NOT PendingOpen THEN
            Error := TRUE;
        ELSIF NOT PendingDefChangedEmitted THEN
            Error := TRUE;
        ELSE
            StateEpochId := PendingEpochId;
            EmitSeq := StateSystemSeq;
            StateSystemSeq := StateSystemSeq + ULINT#1;
            EmitSourceTime := OPENOT_NextSourceTime(
                UseSourceTimeInput := UseSourceTimeInput,
                SourceTimeInput := SourceTimeInput,
                StateSourceTime := StateSourceTime);
            StartedEncoder(
                RunId := StateRunId,
                Seq := EmitSeq,
                ColdStart := FALSE,
                SourceTime := EmitSourceTime);
            RingProducer(
                Execute := TRUE,
                Record := StartedEncoder.Buffer,
                Length := StartedEncoder.Length,
                RunId := StateRunId,
                SourceId := UDINT#0,
                Seq := EmitSeq);

            IF RingProducer.Done AND (NOT RingProducer.Error) THEN
                AppendOk := OPENOT_AppendScanRecord(
                    Record := StartedEncoder.Buffer,
                    Length := StartedEncoder.Length,
                    ScanRecords := ScanRecords,
                    ScanRecordOffsets := ScanRecordOffsets,
                    ScanRecordLengths := ScanRecordLengths,
                    ScanWritePos := ScanWritePos,
                    ScanRecordCount := ScanRecordCount);
	                IF AppendOk THEN
	                    StateEpochFirstAbs := RingProducer.LastStartAbs;
	                    FOR Index := UINT#0 TO UINT#7 DO
	                        StatePrevDefinitionHash[Index] := PendingDefHashOld[Index];
	                        StateDefinitionHash[Index] := PendingDefHashNew[Index];
	                    END_FOR;
	                    FOR Index := UINT#0 TO UINT#31 DO
	                        AttestableEvents[Index].Used := FALSE;
	                        AttestableEvents[Index].AttestableId := UDINT#0;
	                        AttestableEvents[Index].SourceId := UDINT#0;
	                        AttestableEvents[Index].HasSeq := FALSE;
	                        AttestableEvents[Index].LastSeq := ULINT#0;
	                    END_FOR;
	                    PendingOpen := FALSE;
	                    PendingDefChangedEmitted := FALSE;
	                    StatePublishedRecordCount := StatePublishedRecordCount + ULINT#1;
                    IF EmissionCount < UINT#32 THEN
                        Emissions[EmissionCount].EventTypeId := UDINT#16#0101;
                        Emissions[EmissionCount].RunId := StateRunId;
                        Emissions[EmissionCount].SourceId := UDINT#0;
                        Emissions[EmissionCount].Seq := EmitSeq;
                        EmissionCount := EmissionCount + UINT#1;
                    END_IF;
                    Done := TRUE;
                ELSE
                    Error := TRUE;
                END_IF;
            ELSE
                Error := TRUE;
            END_IF;
        END_IF;
    ELSIF Op = UINT#4 THEN
        IF NOT PendingOpen THEN
            Error := TRUE;
        ELSIF NOT PendingDefChangedEmitted THEN
            Error := TRUE;
        ELSE
            StateRunId := StateRunId + ULINT#1;
            StateEpochId := PendingEpochId;
            StateSystemSeq := ULINT#0;

            FOR Index := UINT#0 TO UINT#15 DO
                Sources[Index].Used := FALSE;
                Sources[Index].SourceId := UDINT#0;
                Sources[Index].NextSeq := ULINT#0;
            END_FOR;
            FOR Index := UINT#0 TO UINT#31 DO
                AttestableEvents[Index].Used := FALSE;
                AttestableEvents[Index].AttestableId := UDINT#0;
                AttestableEvents[Index].SourceId := UDINT#0;
                AttestableEvents[Index].HasSeq := FALSE;
                AttestableEvents[Index].LastSeq := ULINT#0;
            END_FOR;

            EmitSeq := StateSystemSeq;
            StateSystemSeq := StateSystemSeq + ULINT#1;
            EmitSourceTime := OPENOT_NextSourceTime(
                UseSourceTimeInput := UseSourceTimeInput,
                SourceTimeInput := SourceTimeInput,
                StateSourceTime := StateSourceTime);
            StartedEncoder(
                RunId := StateRunId,
                Seq := EmitSeq,
                ColdStart := TRUE,
                SourceTime := EmitSourceTime);
            RingProducer(
                Execute := TRUE,
                Record := StartedEncoder.Buffer,
                Length := StartedEncoder.Length,
                RunId := StateRunId,
                SourceId := UDINT#0,
                Seq := EmitSeq);

            IF RingProducer.Done AND (NOT RingProducer.Error) THEN
                AppendOk := OPENOT_AppendScanRecord(
                    Record := StartedEncoder.Buffer,
                    Length := StartedEncoder.Length,
                    ScanRecords := ScanRecords,
                    ScanRecordOffsets := ScanRecordOffsets,
                    ScanRecordLengths := ScanRecordLengths,
                    ScanWritePos := ScanWritePos,
                    ScanRecordCount := ScanRecordCount);
                IF AppendOk THEN
                    StateEpochFirstAbs := RingProducer.LastStartAbs;
                    FOR Index := UINT#0 TO UINT#7 DO
                        StatePrevDefinitionHash[Index] := PendingDefHashOld[Index];
                        StateDefinitionHash[Index] := PendingDefHashNew[Index];
                    END_FOR;
                    PendingOpen := FALSE;
                    PendingDefChangedEmitted := FALSE;
                    StatePublishedRecordCount := StatePublishedRecordCount + ULINT#1;
                    IF EmissionCount < UINT#32 THEN
                        Emissions[EmissionCount].EventTypeId := UDINT#16#0101;
                        Emissions[EmissionCount].RunId := StateRunId;
                        Emissions[EmissionCount].SourceId := UDINT#0;
                        Emissions[EmissionCount].Seq := EmitSeq;
                        EmissionCount := EmissionCount + UINT#1;
                    END_IF;
                    Done := TRUE;
                ELSE
                    Error := TRUE;
                END_IF;
            ELSE
                Error := TRUE;
            END_IF;
        END_IF;
    ELSE
        Error := TRUE;
    END_IF;
END_IF;

PreviousExecute := Execute;

SourceCount := UINT#0;
FOR Index := UINT#0 TO UINT#15 DO
    SourceUsed[Index] := Sources[Index].Used;
    SourceIds[Index] := Sources[Index].SourceId;
    SourceNextSeqs[Index] := Sources[Index].NextSeq;
    IF Sources[Index].Used THEN
        SourceCount := SourceCount + UINT#1;
    END_IF;
END_FOR;

RunId := StateRunId;
IF UseSourceTimeInput THEN
    SourceTime := SourceTimeInput;
ELSE
    SourceTime := StateSourceTime;
END_IF;
SystemSeq := StateSystemSeq;
PublishedRecordCount := StatePublishedRecordCount;
DroppedLifecycleCount := StateDroppedLifecycleCount;
LastLifecycleError := StateLastLifecycleError;
DroppedCommandCount := StateDroppedCommandCount;
LastCommandError := StateLastCommandError;
EpochId := StateEpochId;
EpochFirstAbs := StateEpochFirstAbs;
TransitionOpen := PendingOpen;
DefinitionChangedEmitted := PendingDefChangedEmitted;

FOR Index := UINT#0 TO UINT#7 DO
    DefinitionHash[Index] := StateDefinitionHash[Index];
    PrevDefinitionHash[Index] := StatePrevDefinitionHash[Index];
END_FOR;

IF ExecuteEdge THEN
    FOR ByteIndex := UINT#0 TO UINT#255 DO
        Ring[ByteIndex] := RingProducer.Ring[ByteIndex];
    END_FOR;

    FOR LossIndex := UINT#0 TO UINT#15 DO
        LossRanges[LossIndex].RunId := RingProducer.LossRanges[LossIndex].RunId;
        LossRanges[LossIndex].SourceId := RingProducer.LossRanges[LossIndex].SourceId;
        LossRanges[LossIndex].FirstSeq := RingProducer.LossRanges[LossIndex].FirstSeq;
        LossRanges[LossIndex].LastSeq := RingProducer.LossRanges[LossIndex].LastSeq;
    END_FOR;

    HeadAbs := RingProducer.HeadAbs;
    OldestAbs := RingProducer.OldestAbs;
    LostCount := RingProducer.LostCount;
    LastStartAbs := RingProducer.LastStartAbs;
    LastEndAbs := RingProducer.LastEndAbs;
    LossRangeCount := RingProducer.LossRangeCount;
    LossRangeOverflow := RingProducer.LossRangeOverflow;
    RingError := RingProducer.Error;
END_IF;
END_FUNCTION_BLOCK
