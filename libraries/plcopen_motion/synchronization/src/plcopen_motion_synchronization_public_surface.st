TYPE MC_START_MODE : (
    mcAbsolute,
    mcRelative,
    mcRampIn
);
END_TYPE

TYPE MC_SYNC_MODE : (
    mcShortest,
    mcCatchUp,
    mcSlowDown
);
END_TYPE

TYPE MC_CAM_ID : UINT;
END_TYPE

TYPE MC_CAM_REF :
STRUCT
    CamId : MC_CAM_ID;
    NumberOfPairs : UINT;
    IsAbsolute : BOOL;
    MasterPosition0 : REAL;
    MasterPosition1 : REAL;
    MasterPosition2 : REAL;
    MasterPosition3 : REAL;
    MasterPosition4 : REAL;
    MasterPosition5 : REAL;
    MasterPosition6 : REAL;
    MasterPosition7 : REAL;
    SlavePosition0 : REAL;
    SlavePosition1 : REAL;
    SlavePosition2 : REAL;
    SlavePosition3 : REAL;
    SlavePosition4 : REAL;
    SlavePosition5 : REAL;
    SlavePosition6 : REAL;
    SlavePosition7 : REAL;
END_STRUCT
END_TYPE

FUNCTION_BLOCK MC_CamTableSelect
VAR_IN_OUT
    Master : AXIS_REF;
    Slave : AXIS_REF;
    CamTable : MC_CAM_REF;
END_VAR
VAR_INPUT
    Execute : BOOL;
    Periodic : BOOL;
    MasterAbsolute : BOOL;
    SlaveAbsolute : BOOL;
    ExecutionMode : MC_EXECUTION_MODE;
END_VAR
VAR_OUTPUT
    Done : BOOL;
    Busy : BOOL;
    Error : BOOL;
    ErrorID : WORD;
    CamTableID : MC_CAM_ID;
END_VAR
VAR
    MasterIndex : UINT;
    SlaveIndex : UINT;
    LastExecute : BOOL;
    PendingSelection : BOOL;
    PendingPeriodic : BOOL;
    PendingMasterAbsolute : BOOL;
    PendingSlaveAbsolute : BOOL;
END_VAR
MasterIndex := TRUST_SYNC_AxisIndex(Axis := Master);
SlaveIndex := TRUST_SYNC_AxisIndex(Axis := Slave);
IF NOT g_trust_sync_axis_initialized[MasterIndex] THEN
    TRUST_SYNC_ResetAxis(AxisIndex := MasterIndex);
END_IF;
IF NOT g_trust_sync_axis_initialized[SlaveIndex] THEN
    TRUST_SYNC_ResetAxis(AxisIndex := SlaveIndex);
END_IF;
IF Execute AND (NOT LastExecute) THEN
    Done := FALSE;
    Busy := FALSE;
    Error := FALSE;
    ErrorID := WORD#16#0000;
    CamTableID := UINT#0;
    IF ExecutionMode = MC_EXECUTION_MODE#mcDelayed THEN
        Error := TRUE;
        ErrorID := WORD#16#0500;
        PendingSelection := FALSE;
    ELSE
        Busy := TRUE;
        PendingSelection := TRUE;
        PendingPeriodic := Periodic;
        PendingMasterAbsolute := MasterAbsolute;
        PendingSlaveAbsolute := SlaveAbsolute;
    END_IF;
ELSIF PendingSelection THEN
    TRUST_SYNC_LoadSelectedCamFromRef(
        SlaveIndex := SlaveIndex,
        CamTable := CamTable,
        Periodic := PendingPeriodic,
        MasterAbsolute := PendingMasterAbsolute,
        SlaveAbsolute := PendingSlaveAbsolute
    );
    Busy := FALSE;
    Done := TRUE;
    Error := FALSE;
    ErrorID := WORD#16#0000;
    CamTableID := g_trust_sync_selected_cam_id[SlaveIndex];
    PendingSelection := FALSE;
ELSIF NOT Execute THEN
    Done := FALSE;
    Busy := FALSE;
    Error := FALSE;
    ErrorID := WORD#16#0000;
END_IF;
LastExecute := Execute;
END_FUNCTION_BLOCK

FUNCTION_BLOCK MC_CamIn
VAR_IN_OUT
    Master : AXIS_REF;
    Slave : AXIS_REF;
END_VAR
VAR_INPUT
    Execute : BOOL;
    ContinuousUpdate : BOOL;
    MasterOffset : REAL;
    SlaveOffset : REAL;
    MasterScaling : REAL;
    SlaveScaling : REAL;
    MasterStartDistance : REAL;
    MasterSyncPosition : REAL;
    StartMode : MC_START_MODE;
    MasterValueSource : MC_SOURCE;
    CamTableID : MC_CAM_ID;
    BufferMode : MC_BUFFER_MODE;
END_VAR
VAR_OUTPUT
    InSync : BOOL;
    Busy : BOOL;
    Active : BOOL;
    CommandAborted : BOOL;
    Error : BOOL;
    ErrorID : WORD;
    EndOfProfile : BOOL;
END_VAR
VAR
    SlaveIndex : UINT;
    MasterIndex : UINT;
    LastExecute : BOOL;
    AcceptedCommandID : UINT;
    WasActive : BOOL;
    Request : TRUST_SYNC_RequestBufferedCommand;
END_VAR
SlaveIndex := TRUST_SYNC_AxisIndex(Axis := Slave);
MasterIndex := TRUST_SYNC_AxisIndex(Axis := Master);
IF NOT g_trust_sync_axis_initialized[MasterIndex] THEN
    TRUST_SYNC_ResetAxis(AxisIndex := MasterIndex);
END_IF;
IF NOT g_trust_sync_axis_initialized[SlaveIndex] THEN
    TRUST_SYNC_ResetAxis(AxisIndex := SlaveIndex);
END_IF;
TRUST_SYNC_PromoteQueuedCommand(SlaveIndex := SlaveIndex);
EndOfProfile := FALSE;
IF Execute AND (NOT LastExecute) THEN
    InSync := FALSE;
    Busy := FALSE;
    Active := FALSE;
    CommandAborted := FALSE;
    Error := FALSE;
    ErrorID := WORD#16#0000;
    IF NOT g_trust_sync_selected_cam_ready[SlaveIndex] THEN
        Error := TRUE;
        ErrorID := WORD#16#0102;
    ELSIF g_trust_sync_selected_cam_id[SlaveIndex] <> CamTableID THEN
        Error := TRUE;
        ErrorID := WORD#16#0102;
    ELSIF NOT TRUST_SYNC_SupportsSyncBufferMode(BufferMode := BufferMode) THEN
        Error := TRUE;
        ErrorID := WORD#16#0500;
    ELSE
        Request(AxisIndex := SlaveIndex, Kind := TRUST_SYNC_COMMAND_KIND#trustSyncCamIn, BufferMode := BufferMode);
        IF Request.Error THEN
            Error := TRUE;
            ErrorID := Request.ErrorID;
        ELSE
            AcceptedCommandID := Request.CommandID;
            Busy := TRUE;
            Active := Request.ActiveNow;
            IF Request.ActiveNow THEN
                WasActive := TRUE;
                TRUST_SYNC_StoreActiveCamParameters(
                    SlaveIndex := SlaveIndex,
                    MasterIndex := MasterIndex,
                    MasterOffset := MasterOffset,
                    SlaveOffset := SlaveOffset,
                    MasterScaling := MasterScaling,
                    SlaveScaling := SlaveScaling,
                    MasterStartDistance := MasterStartDistance,
                    MasterSyncPosition := MasterSyncPosition,
                    StartMode := StartMode,
                    MasterValueSource := MasterValueSource,
                    CamTableID := CamTableID
                );
                TRUST_SYNC_BeginActiveCommand(SlaveIndex := SlaveIndex);
            ELSE
                TRUST_SYNC_StoreQueuedCamParameters(
                    SlaveIndex := SlaveIndex,
                    MasterIndex := MasterIndex,
                    MasterOffset := MasterOffset,
                    SlaveOffset := SlaveOffset,
                    MasterScaling := MasterScaling,
                    SlaveScaling := SlaveScaling,
                    MasterStartDistance := MasterStartDistance,
                    MasterSyncPosition := MasterSyncPosition,
                    StartMode := StartMode,
                    MasterValueSource := MasterValueSource,
                    CamTableID := CamTableID
                );
            END_IF;
        END_IF;
    END_IF;
END_IF;
IF AcceptedCommandID <> UINT#0 THEN
    IF TRUST_SYNC_CommandWasAborted(AxisIndex := SlaveIndex, CommandID := AcceptedCommandID) THEN
        Busy := FALSE;
        Active := FALSE;
        InSync := FALSE;
        EndOfProfile := FALSE;
        Error := FALSE;
        ErrorID := WORD#16#0000;
        CommandAborted := TRUE;
        TRUST_SYNC_ClearAbortedCommand(AxisIndex := SlaveIndex, CommandID := AcceptedCommandID);
        AcceptedCommandID := UINT#0;
        WasActive := FALSE;
    ELSIF g_trust_sync_active_command_id[SlaveIndex] = AcceptedCommandID THEN
        IF (NOT WasActive) THEN
            WasActive := TRUE;
            TRUST_SYNC_StoreActiveCamParameters(
                SlaveIndex := SlaveIndex,
                MasterIndex := MasterIndex,
                MasterOffset := MasterOffset,
                SlaveOffset := SlaveOffset,
                MasterScaling := MasterScaling,
                SlaveScaling := SlaveScaling,
                MasterStartDistance := MasterStartDistance,
                MasterSyncPosition := MasterSyncPosition,
                StartMode := StartMode,
                MasterValueSource := MasterValueSource,
                CamTableID := CamTableID
            );
            TRUST_SYNC_BeginActiveCommand(SlaveIndex := SlaveIndex);
        ELSIF ContinuousUpdate THEN
            TRUST_SYNC_StoreActiveCamParameters(
                SlaveIndex := SlaveIndex,
                MasterIndex := MasterIndex,
                MasterOffset := MasterOffset,
                SlaveOffset := SlaveOffset,
                MasterScaling := MasterScaling,
                SlaveScaling := SlaveScaling,
                MasterStartDistance := MasterStartDistance,
                MasterSyncPosition := MasterSyncPosition,
                StartMode := StartMode,
                MasterValueSource := MasterValueSource,
                CamTableID := CamTableID
            );
        END_IF;
        Busy := TRUE;
        Active := TRUE;
        InSync := g_trust_sync_active_in_sync[SlaveIndex];
        EndOfProfile := g_trust_sync_active_end_of_profile_pulse[SlaveIndex];
        CommandAborted := FALSE;
        Error := FALSE;
        ErrorID := WORD#16#0000;
    ELSIF g_trust_sync_queued_command_id[SlaveIndex] = AcceptedCommandID THEN
        IF ContinuousUpdate THEN
            TRUST_SYNC_StoreQueuedCamParameters(
                SlaveIndex := SlaveIndex,
                MasterIndex := MasterIndex,
                MasterOffset := MasterOffset,
                SlaveOffset := SlaveOffset,
                MasterScaling := MasterScaling,
                SlaveScaling := SlaveScaling,
                MasterStartDistance := MasterStartDistance,
                MasterSyncPosition := MasterSyncPosition,
                StartMode := StartMode,
                MasterValueSource := MasterValueSource,
                CamTableID := CamTableID
            );
        END_IF;
        Busy := TRUE;
        Active := FALSE;
        InSync := FALSE;
        CommandAborted := FALSE;
        Error := FALSE;
        ErrorID := WORD#16#0000;
    ELSE
        Busy := FALSE;
        Active := FALSE;
        InSync := FALSE;
        EndOfProfile := FALSE;
        AcceptedCommandID := UINT#0;
        WasActive := FALSE;
    END_IF;
END_IF;
LastExecute := Execute;
END_FUNCTION_BLOCK

FUNCTION_BLOCK MC_CamOut
VAR_IN_OUT
    Slave : AXIS_REF;
END_VAR
VAR_INPUT
    Execute : BOOL;
END_VAR
VAR_OUTPUT
    Done : BOOL;
    Busy : BOOL;
    Error : BOOL;
    ErrorID : WORD;
END_VAR
VAR
    SlaveIndex : UINT;
    LastExecute : BOOL;
END_VAR
SlaveIndex := TRUST_SYNC_AxisIndex(Axis := Slave);
IF NOT g_trust_sync_axis_initialized[SlaveIndex] THEN
    TRUST_SYNC_ResetAxis(AxisIndex := SlaveIndex);
END_IF;
IF Execute AND (NOT LastExecute) THEN
    Done := FALSE;
    Busy := FALSE;
    Error := FALSE;
    ErrorID := WORD#16#0000;
    IF (g_trust_sync_axis_state[SlaveIndex] <> MC_AXIS_STATUS#mcSynchronizedMotion) OR
       (g_trust_sync_active_command_kind[SlaveIndex] <> TRUST_SYNC_COMMAND_KIND#trustSyncCamIn) THEN
        Error := TRUE;
        ErrorID := WORD#16#0200;
    ELSE
        IF g_trust_sync_active_command_id[SlaveIndex] <> UINT#0 THEN
            TRUST_SYNC_RecordAbortedCommand(AxisIndex := SlaveIndex, CommandID := g_trust_sync_active_command_id[SlaveIndex]);
        END_IF;
        TRUST_SYNC_ClearActiveCommandSlot(AxisIndex := SlaveIndex);
        g_trust_sync_axis_state[SlaveIndex] := MC_AXIS_STATUS#mcContinuousMotion;
        TRUST_SYNC_PromoteQueuedCommand(SlaveIndex := SlaveIndex);
        Done := TRUE;
    END_IF;
ELSIF NOT Execute THEN
    Done := FALSE;
    Busy := FALSE;
    Error := FALSE;
    ErrorID := WORD#16#0000;
END_IF;
LastExecute := Execute;
END_FUNCTION_BLOCK

FUNCTION_BLOCK MC_GearIn
VAR_IN_OUT
    Master : AXIS_REF;
    Slave : AXIS_REF;
END_VAR
VAR_INPUT
    Execute : BOOL;
    ContinuousUpdate : BOOL;
    RatioNumerator : INT;
    RatioDenominator : UINT;
    MasterValueSource : MC_SOURCE;
    Acceleration : REAL;
    Deceleration : REAL;
    Jerk : REAL;
    BufferMode : MC_BUFFER_MODE;
END_VAR
VAR_OUTPUT
    InGear : BOOL;
    Busy : BOOL;
    Active : BOOL;
    CommandAborted : BOOL;
    Error : BOOL;
    ErrorID : WORD;
END_VAR
VAR
    SlaveIndex : UINT;
    MasterIndex : UINT;
    LastExecute : BOOL;
    AcceptedCommandID : UINT;
    WasActive : BOOL;
    Request : TRUST_SYNC_RequestBufferedCommand;
END_VAR
SlaveIndex := TRUST_SYNC_AxisIndex(Axis := Slave);
MasterIndex := TRUST_SYNC_AxisIndex(Axis := Master);
IF NOT g_trust_sync_axis_initialized[MasterIndex] THEN
    TRUST_SYNC_ResetAxis(AxisIndex := MasterIndex);
END_IF;
IF NOT g_trust_sync_axis_initialized[SlaveIndex] THEN
    TRUST_SYNC_ResetAxis(AxisIndex := SlaveIndex);
END_IF;
TRUST_SYNC_PromoteQueuedCommand(SlaveIndex := SlaveIndex);
IF Execute AND (NOT LastExecute) THEN
    InGear := FALSE;
    Busy := FALSE;
    Active := FALSE;
    CommandAborted := FALSE;
    Error := FALSE;
    ErrorID := WORD#16#0000;
    IF RatioDenominator = UINT#0 THEN
        Error := TRUE;
        ErrorID := WORD#16#0100;
    ELSIF NOT TRUST_SYNC_SupportsSyncBufferMode(BufferMode := BufferMode) THEN
        Error := TRUE;
        ErrorID := WORD#16#0500;
    ELSE
        Request(AxisIndex := SlaveIndex, Kind := TRUST_SYNC_COMMAND_KIND#trustSyncGearIn, BufferMode := BufferMode);
        IF Request.Error THEN
            Error := TRUE;
            ErrorID := Request.ErrorID;
        ELSE
            AcceptedCommandID := Request.CommandID;
            Busy := TRUE;
            Active := Request.ActiveNow;
            IF Request.ActiveNow THEN
                WasActive := TRUE;
                TRUST_SYNC_StoreActiveGearParameters(
                    SlaveIndex := SlaveIndex,
                    MasterIndex := MasterIndex,
                    RatioNumerator := RatioNumerator,
                    RatioDenominator := RatioDenominator,
                    MasterValueSource := MasterValueSource,
                    Acceleration := Acceleration,
                    Deceleration := Deceleration,
                    Jerk := Jerk
                );
                TRUST_SYNC_BeginActiveCommand(SlaveIndex := SlaveIndex);
            ELSE
                TRUST_SYNC_StoreQueuedGearParameters(
                    SlaveIndex := SlaveIndex,
                    MasterIndex := MasterIndex,
                    RatioNumerator := RatioNumerator,
                    RatioDenominator := RatioDenominator,
                    MasterValueSource := MasterValueSource,
                    Acceleration := Acceleration,
                    Deceleration := Deceleration,
                    Jerk := Jerk
                );
            END_IF;
        END_IF;
    END_IF;
END_IF;
IF AcceptedCommandID <> UINT#0 THEN
    IF TRUST_SYNC_CommandWasAborted(AxisIndex := SlaveIndex, CommandID := AcceptedCommandID) THEN
        Busy := FALSE;
        Active := FALSE;
        InGear := FALSE;
        Error := FALSE;
        ErrorID := WORD#16#0000;
        CommandAborted := TRUE;
        TRUST_SYNC_ClearAbortedCommand(AxisIndex := SlaveIndex, CommandID := AcceptedCommandID);
        AcceptedCommandID := UINT#0;
        WasActive := FALSE;
    ELSIF g_trust_sync_active_command_id[SlaveIndex] = AcceptedCommandID THEN
        IF (NOT WasActive) THEN
            WasActive := TRUE;
            TRUST_SYNC_StoreActiveGearParameters(
                SlaveIndex := SlaveIndex,
                MasterIndex := MasterIndex,
                RatioNumerator := RatioNumerator,
                RatioDenominator := RatioDenominator,
                MasterValueSource := MasterValueSource,
                Acceleration := Acceleration,
                Deceleration := Deceleration,
                Jerk := Jerk
            );
            TRUST_SYNC_BeginActiveCommand(SlaveIndex := SlaveIndex);
        ELSIF ContinuousUpdate THEN
            TRUST_SYNC_StoreActiveGearParameters(
                SlaveIndex := SlaveIndex,
                MasterIndex := MasterIndex,
                RatioNumerator := RatioNumerator,
                RatioDenominator := RatioDenominator,
                MasterValueSource := MasterValueSource,
                Acceleration := Acceleration,
                Deceleration := Deceleration,
                Jerk := Jerk
            );
        END_IF;
        Busy := TRUE;
        Active := TRUE;
        InGear := g_trust_sync_active_in_gear[SlaveIndex];
        CommandAborted := FALSE;
        Error := FALSE;
        ErrorID := WORD#16#0000;
    ELSIF g_trust_sync_queued_command_id[SlaveIndex] = AcceptedCommandID THEN
        IF ContinuousUpdate THEN
            TRUST_SYNC_StoreQueuedGearParameters(
                SlaveIndex := SlaveIndex,
                MasterIndex := MasterIndex,
                RatioNumerator := RatioNumerator,
                RatioDenominator := RatioDenominator,
                MasterValueSource := MasterValueSource,
                Acceleration := Acceleration,
                Deceleration := Deceleration,
                Jerk := Jerk
            );
        END_IF;
        Busy := TRUE;
        Active := FALSE;
        InGear := FALSE;
        CommandAborted := FALSE;
        Error := FALSE;
        ErrorID := WORD#16#0000;
    ELSE
        Busy := FALSE;
        Active := FALSE;
        InGear := FALSE;
        AcceptedCommandID := UINT#0;
        WasActive := FALSE;
    END_IF;
END_IF;
LastExecute := Execute;
END_FUNCTION_BLOCK

FUNCTION_BLOCK MC_GearOut
VAR_IN_OUT
    Slave : AXIS_REF;
END_VAR
VAR_INPUT
    Execute : BOOL;
END_VAR
VAR_OUTPUT
    Done : BOOL;
    Busy : BOOL;
    Error : BOOL;
    ErrorID : WORD;
END_VAR
VAR
    SlaveIndex : UINT;
    LastExecute : BOOL;
END_VAR
SlaveIndex := TRUST_SYNC_AxisIndex(Axis := Slave);
IF NOT g_trust_sync_axis_initialized[SlaveIndex] THEN
    TRUST_SYNC_ResetAxis(AxisIndex := SlaveIndex);
END_IF;
IF Execute AND (NOT LastExecute) THEN
    Done := FALSE;
    Busy := FALSE;
    Error := FALSE;
    ErrorID := WORD#16#0000;
    IF (g_trust_sync_axis_state[SlaveIndex] <> MC_AXIS_STATUS#mcSynchronizedMotion) OR
       ((g_trust_sync_active_command_kind[SlaveIndex] <> TRUST_SYNC_COMMAND_KIND#trustSyncGearIn) AND
        (g_trust_sync_active_command_kind[SlaveIndex] <> TRUST_SYNC_COMMAND_KIND#trustSyncGearInPos)) THEN
        Error := TRUE;
        ErrorID := WORD#16#0200;
    ELSE
        IF g_trust_sync_active_command_id[SlaveIndex] <> UINT#0 THEN
            TRUST_SYNC_RecordAbortedCommand(AxisIndex := SlaveIndex, CommandID := g_trust_sync_active_command_id[SlaveIndex]);
        END_IF;
        TRUST_SYNC_ClearActiveCommandSlot(AxisIndex := SlaveIndex);
        g_trust_sync_axis_state[SlaveIndex] := MC_AXIS_STATUS#mcContinuousMotion;
        TRUST_SYNC_PromoteQueuedCommand(SlaveIndex := SlaveIndex);
        Done := TRUE;
    END_IF;
ELSIF NOT Execute THEN
    Done := FALSE;
    Busy := FALSE;
    Error := FALSE;
    ErrorID := WORD#16#0000;
END_IF;
LastExecute := Execute;
END_FUNCTION_BLOCK

FUNCTION_BLOCK MC_GearInPos
VAR_IN_OUT
    Master : AXIS_REF;
    Slave : AXIS_REF;
END_VAR
VAR_INPUT
    Execute : BOOL;
    RatioNumerator : INT;
    RatioDenominator : UINT;
    MasterValueSource : MC_SOURCE;
    MasterSyncPosition : REAL;
    SlaveSyncPosition : REAL;
    SyncMode : MC_SYNC_MODE;
    MasterStartDistance : REAL;
    Velocity : REAL;
    Acceleration : REAL;
    Deceleration : REAL;
    Jerk : REAL;
    BufferMode : MC_BUFFER_MODE;
END_VAR
VAR_OUTPUT
    StartSync : BOOL;
    InSync : BOOL;
    Busy : BOOL;
    Active : BOOL;
    CommandAborted : BOOL;
    Error : BOOL;
    ErrorID : WORD;
END_VAR
VAR
    SlaveIndex : UINT;
    MasterIndex : UINT;
    LastExecute : BOOL;
    AcceptedCommandID : UINT;
    WasActive : BOOL;
    Request : TRUST_SYNC_RequestBufferedCommand;
END_VAR
SlaveIndex := TRUST_SYNC_AxisIndex(Axis := Slave);
MasterIndex := TRUST_SYNC_AxisIndex(Axis := Master);
IF NOT g_trust_sync_axis_initialized[MasterIndex] THEN
    TRUST_SYNC_ResetAxis(AxisIndex := MasterIndex);
END_IF;
IF NOT g_trust_sync_axis_initialized[SlaveIndex] THEN
    TRUST_SYNC_ResetAxis(AxisIndex := SlaveIndex);
END_IF;
TRUST_SYNC_PromoteQueuedCommand(SlaveIndex := SlaveIndex);
StartSync := FALSE;
IF Execute AND (NOT LastExecute) THEN
    InSync := FALSE;
    Busy := FALSE;
    Active := FALSE;
    CommandAborted := FALSE;
    Error := FALSE;
    ErrorID := WORD#16#0000;
    IF RatioDenominator = UINT#0 THEN
        Error := TRUE;
        ErrorID := WORD#16#0100;
    ELSIF NOT TRUST_SYNC_SupportsSyncBufferMode(BufferMode := BufferMode) THEN
        Error := TRUE;
        ErrorID := WORD#16#0500;
    ELSE
        Request(AxisIndex := SlaveIndex, Kind := TRUST_SYNC_COMMAND_KIND#trustSyncGearInPos, BufferMode := BufferMode);
        IF Request.Error THEN
            Error := TRUE;
            ErrorID := Request.ErrorID;
        ELSE
            AcceptedCommandID := Request.CommandID;
            Busy := TRUE;
            Active := Request.ActiveNow;
            IF Request.ActiveNow THEN
                WasActive := TRUE;
                TRUST_SYNC_StoreActiveGearInPosParameters(
                    SlaveIndex := SlaveIndex,
                    MasterIndex := MasterIndex,
                    RatioNumerator := RatioNumerator,
                    RatioDenominator := RatioDenominator,
                    MasterValueSource := MasterValueSource,
                    MasterSyncPosition := MasterSyncPosition,
                    SlaveSyncPosition := SlaveSyncPosition,
                    SyncMode := SyncMode,
                    MasterStartDistance := MasterStartDistance,
                    Velocity := Velocity,
                    Acceleration := Acceleration,
                    Deceleration := Deceleration,
                    Jerk := Jerk
                );
                TRUST_SYNC_BeginActiveCommand(SlaveIndex := SlaveIndex);
            ELSE
                TRUST_SYNC_StoreQueuedGearInPosParameters(
                    SlaveIndex := SlaveIndex,
                    MasterIndex := MasterIndex,
                    RatioNumerator := RatioNumerator,
                    RatioDenominator := RatioDenominator,
                    MasterValueSource := MasterValueSource,
                    MasterSyncPosition := MasterSyncPosition,
                    SlaveSyncPosition := SlaveSyncPosition,
                    SyncMode := SyncMode,
                    MasterStartDistance := MasterStartDistance,
                    Velocity := Velocity,
                    Acceleration := Acceleration,
                    Deceleration := Deceleration,
                    Jerk := Jerk
                );
            END_IF;
        END_IF;
    END_IF;
END_IF;
IF AcceptedCommandID <> UINT#0 THEN
    IF TRUST_SYNC_CommandWasAborted(AxisIndex := SlaveIndex, CommandID := AcceptedCommandID) THEN
        Busy := FALSE;
        Active := FALSE;
        StartSync := FALSE;
        InSync := FALSE;
        Error := FALSE;
        ErrorID := WORD#16#0000;
        CommandAborted := TRUE;
        TRUST_SYNC_ClearAbortedCommand(AxisIndex := SlaveIndex, CommandID := AcceptedCommandID);
        AcceptedCommandID := UINT#0;
        WasActive := FALSE;
    ELSIF g_trust_sync_active_command_id[SlaveIndex] = AcceptedCommandID THEN
        IF (NOT WasActive) THEN
            WasActive := TRUE;
            TRUST_SYNC_StoreActiveGearInPosParameters(
                SlaveIndex := SlaveIndex,
                MasterIndex := MasterIndex,
                RatioNumerator := RatioNumerator,
                RatioDenominator := RatioDenominator,
                MasterValueSource := MasterValueSource,
                MasterSyncPosition := MasterSyncPosition,
                SlaveSyncPosition := SlaveSyncPosition,
                SyncMode := SyncMode,
                MasterStartDistance := MasterStartDistance,
                Velocity := Velocity,
                Acceleration := Acceleration,
                Deceleration := Deceleration,
                Jerk := Jerk
            );
            TRUST_SYNC_BeginActiveCommand(SlaveIndex := SlaveIndex);
        END_IF;
        Busy := TRUE;
        Active := TRUE;
        StartSync := g_trust_sync_active_start_sync_pulse[SlaveIndex];
        InSync := g_trust_sync_active_in_sync[SlaveIndex];
        CommandAborted := FALSE;
        Error := FALSE;
        ErrorID := WORD#16#0000;
    ELSIF g_trust_sync_queued_command_id[SlaveIndex] = AcceptedCommandID THEN
        Busy := TRUE;
        Active := FALSE;
        StartSync := FALSE;
        InSync := FALSE;
        CommandAborted := FALSE;
        Error := FALSE;
        ErrorID := WORD#16#0000;
    ELSE
        Busy := FALSE;
        Active := FALSE;
        StartSync := FALSE;
        InSync := FALSE;
        AcceptedCommandID := UINT#0;
        WasActive := FALSE;
    END_IF;
END_IF;
LastExecute := Execute;
END_FUNCTION_BLOCK
