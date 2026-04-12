TYPE TRUST_SYNC_COMMAND_KIND : (
    trustSyncNone,
    trustSyncCamIn,
    trustSyncGearIn,
    trustSyncGearInPos
);
END_TYPE

FUNCTION TRUST_SYNC_AxisIndex : UINT
VAR_INPUT
    Axis : AXIS_REF;
END_VAR
TRUST_SYNC_AxisIndex := Axis.InternalIndex;
END_FUNCTION

FUNCTION TRUST_SYNC_SupportsSyncBufferMode : BOOL
VAR_INPUT
    BufferMode : MC_BUFFER_MODE;
END_VAR
TRUST_SYNC_SupportsSyncBufferMode :=
    (BufferMode = MC_BUFFER_MODE#mcAborting) OR
    (BufferMode = MC_BUFFER_MODE#mcBuffered);
END_FUNCTION

FUNCTION TRUST_SYNC_ResetAxis : BOOL
VAR_INPUT
    AxisIndex : UINT;
END_VAR
g_trust_sync_axis_initialized[AxisIndex] := TRUE;
g_trust_sync_axis_state[AxisIndex] := MC_AXIS_STATUS#mcStandstill;
g_trust_sync_axis_commanded_position[AxisIndex] := REAL#0.0;
g_trust_sync_axis_actual_position[AxisIndex] := REAL#0.0;
g_trust_sync_axis_last_commanded_position[AxisIndex] := REAL#0.0;
g_trust_sync_axis_velocity[AxisIndex] := REAL#0.0;
TRUST_SYNC_ResetAxis := TRUE;
END_FUNCTION

FUNCTION TRUST_SYNC_ClearActiveCommandSlot : BOOL
VAR_INPUT
    AxisIndex : UINT;
END_VAR
g_trust_sync_active_command_id[AxisIndex] := UINT#0;
g_trust_sync_active_command_kind[AxisIndex] := TRUST_SYNC_COMMAND_KIND#trustSyncNone;
g_trust_sync_active_buffer_mode[AxisIndex] := MC_BUFFER_MODE#mcAborting;
g_trust_sync_active_master_axis_index[AxisIndex] := UINT#0;
g_trust_sync_active_ratio_numerator[AxisIndex] := INT#0;
g_trust_sync_active_ratio_denominator[AxisIndex] := UINT#1;
g_trust_sync_active_master_offset[AxisIndex] := REAL#0.0;
g_trust_sync_active_slave_offset[AxisIndex] := REAL#0.0;
g_trust_sync_active_master_scaling[AxisIndex] := REAL#1.0;
g_trust_sync_active_slave_scaling[AxisIndex] := REAL#1.0;
g_trust_sync_active_master_start_distance[AxisIndex] := REAL#0.0;
g_trust_sync_active_master_sync_position[AxisIndex] := REAL#0.0;
g_trust_sync_active_slave_sync_position[AxisIndex] := REAL#0.0;
g_trust_sync_active_velocity[AxisIndex] := REAL#0.0;
g_trust_sync_active_acceleration[AxisIndex] := REAL#0.0;
g_trust_sync_active_deceleration[AxisIndex] := REAL#0.0;
g_trust_sync_active_jerk[AxisIndex] := REAL#0.0;
g_trust_sync_active_start_mode[AxisIndex] := MC_START_MODE#mcAbsolute;
g_trust_sync_active_sync_mode[AxisIndex] := MC_SYNC_MODE#mcShortest;
g_trust_sync_active_master_value_source[AxisIndex] := MC_SOURCE#mcSetValue;
g_trust_sync_active_cam_table_id[AxisIndex] := UINT#0;
g_trust_sync_active_command_start_master_position[AxisIndex] := REAL#0.0;
g_trust_sync_active_command_start_slave_position[AxisIndex] := REAL#0.0;
g_trust_sync_active_start_sync_seen[AxisIndex] := FALSE;
g_trust_sync_active_start_sync_pulse[AxisIndex] := FALSE;
g_trust_sync_active_end_of_profile_pulse[AxisIndex] := FALSE;
g_trust_sync_active_in_sync[AxisIndex] := FALSE;
g_trust_sync_active_in_gear[AxisIndex] := FALSE;
g_trust_sync_active_previous_source_position[AxisIndex] := REAL#0.0;
TRUST_SYNC_ClearActiveCommandSlot := TRUE;
END_FUNCTION

FUNCTION TRUST_SYNC_ClearQueuedCommandSlot : BOOL
VAR_INPUT
    AxisIndex : UINT;
END_VAR
g_trust_sync_queued_command_id[AxisIndex] := UINT#0;
g_trust_sync_queued_command_kind[AxisIndex] := TRUST_SYNC_COMMAND_KIND#trustSyncNone;
g_trust_sync_queued_buffer_mode[AxisIndex] := MC_BUFFER_MODE#mcAborting;
g_trust_sync_queued_master_axis_index[AxisIndex] := UINT#0;
g_trust_sync_queued_ratio_numerator[AxisIndex] := INT#0;
g_trust_sync_queued_ratio_denominator[AxisIndex] := UINT#1;
g_trust_sync_queued_master_offset[AxisIndex] := REAL#0.0;
g_trust_sync_queued_slave_offset[AxisIndex] := REAL#0.0;
g_trust_sync_queued_master_scaling[AxisIndex] := REAL#1.0;
g_trust_sync_queued_slave_scaling[AxisIndex] := REAL#1.0;
g_trust_sync_queued_master_start_distance[AxisIndex] := REAL#0.0;
g_trust_sync_queued_master_sync_position[AxisIndex] := REAL#0.0;
g_trust_sync_queued_slave_sync_position[AxisIndex] := REAL#0.0;
g_trust_sync_queued_velocity[AxisIndex] := REAL#0.0;
g_trust_sync_queued_acceleration[AxisIndex] := REAL#0.0;
g_trust_sync_queued_deceleration[AxisIndex] := REAL#0.0;
g_trust_sync_queued_jerk[AxisIndex] := REAL#0.0;
g_trust_sync_queued_start_mode[AxisIndex] := MC_START_MODE#mcAbsolute;
g_trust_sync_queued_sync_mode[AxisIndex] := MC_SYNC_MODE#mcShortest;
g_trust_sync_queued_master_value_source[AxisIndex] := MC_SOURCE#mcSetValue;
g_trust_sync_queued_cam_table_id[AxisIndex] := UINT#0;
TRUST_SYNC_ClearQueuedCommandSlot := TRUE;
END_FUNCTION

FUNCTION TRUST_SYNC_ResetSelection : BOOL
VAR_INPUT
    AxisIndex : UINT;
END_VAR
VAR
    PointIndex : INT;
END_VAR
g_trust_sync_selected_cam_ready[AxisIndex] := FALSE;
g_trust_sync_selected_cam_id[AxisIndex] := UINT#0;
g_trust_sync_selected_cam_periodic[AxisIndex] := FALSE;
g_trust_sync_selected_cam_master_absolute[AxisIndex] := TRUE;
g_trust_sync_selected_cam_slave_absolute[AxisIndex] := TRUE;
g_trust_sync_selected_cam_point_count[AxisIndex] := UINT#0;
FOR PointIndex := INT#0 TO INT#7 DO
    g_trust_sync_selected_cam_master[AxisIndex, PointIndex] := REAL#0.0;
    g_trust_sync_selected_cam_slave[AxisIndex, PointIndex] := REAL#0.0;
END_FOR;
TRUST_SYNC_ResetSelection := TRUE;
END_FUNCTION

FUNCTION TRUST_SYNC_ResetSlaveKernel : BOOL
VAR_INPUT
    AxisIndex : UINT;
END_VAR
TRUST_SYNC_ResetAxis(AxisIndex := AxisIndex);
TRUST_SYNC_ClearActiveCommandSlot(AxisIndex := AxisIndex);
TRUST_SYNC_ClearQueuedCommandSlot(AxisIndex := AxisIndex);
TRUST_SYNC_ResetSelection(AxisIndex := AxisIndex);
g_trust_sync_next_command_id[AxisIndex] := UINT#0;
g_trust_sync_aborted0_command_id[AxisIndex] := UINT#0;
g_trust_sync_aborted1_command_id[AxisIndex] := UINT#0;
g_trust_sync_seed_start_position[AxisIndex] := REAL#0.0;
g_trust_sync_seed_exit_position[AxisIndex] := REAL#0.0;
g_trust_sync_seed_ratio_numerator[AxisIndex] := REAL#1.0;
g_trust_sync_seed_ratio_denominator[AxisIndex] := REAL#1.0;
g_trust_sync_seed_phase_offset[AxisIndex] := REAL#0.0;
g_trust_sync_seed_break_requested[AxisIndex] := FALSE;
TRUST_SYNC_ResetSlaveKernel := TRUE;
END_FUNCTION

FUNCTION TRUST_SYNC_NextCommandId : UINT
VAR_INPUT
    AxisIndex : UINT;
END_VAR
g_trust_sync_next_command_id[AxisIndex] := g_trust_sync_next_command_id[AxisIndex] + UINT#1;
IF g_trust_sync_next_command_id[AxisIndex] = UINT#0 THEN
    g_trust_sync_next_command_id[AxisIndex] := UINT#1;
END_IF;
TRUST_SYNC_NextCommandId := g_trust_sync_next_command_id[AxisIndex];
END_FUNCTION

FUNCTION TRUST_SYNC_CommandWasAborted : BOOL
VAR_INPUT
    AxisIndex : UINT;
    CommandID : UINT;
END_VAR
TRUST_SYNC_CommandWasAborted :=
    (g_trust_sync_aborted0_command_id[AxisIndex] = CommandID) OR
    (g_trust_sync_aborted1_command_id[AxisIndex] = CommandID);
END_FUNCTION

FUNCTION TRUST_SYNC_ClearAbortedCommand : BOOL
VAR_INPUT
    AxisIndex : UINT;
    CommandID : UINT;
END_VAR
TRUST_SYNC_ClearAbortedCommand := FALSE;
IF g_trust_sync_aborted0_command_id[AxisIndex] = CommandID THEN
    g_trust_sync_aborted0_command_id[AxisIndex] := UINT#0;
    TRUST_SYNC_ClearAbortedCommand := TRUE;
END_IF;
IF g_trust_sync_aborted1_command_id[AxisIndex] = CommandID THEN
    g_trust_sync_aborted1_command_id[AxisIndex] := UINT#0;
    TRUST_SYNC_ClearAbortedCommand := TRUE;
END_IF;
END_FUNCTION

FUNCTION TRUST_SYNC_RecordAbortedCommand : BOOL
VAR_INPUT
    AxisIndex : UINT;
    CommandID : UINT;
END_VAR
TRUST_SYNC_RecordAbortedCommand := FALSE;
IF CommandID <> UINT#0 THEN
    IF g_trust_sync_aborted0_command_id[AxisIndex] = UINT#0 THEN
        g_trust_sync_aborted0_command_id[AxisIndex] := CommandID;
        TRUST_SYNC_RecordAbortedCommand := TRUE;
    ELSIF g_trust_sync_aborted1_command_id[AxisIndex] = UINT#0 THEN
        g_trust_sync_aborted1_command_id[AxisIndex] := CommandID;
        TRUST_SYNC_RecordAbortedCommand := TRUE;
    END_IF;
END_IF;
END_FUNCTION

FUNCTION TRUST_SYNC_PromoteQueuedCommand : BOOL
VAR_INPUT
    SlaveIndex : UINT;
END_VAR
TRUST_SYNC_PromoteQueuedCommand := FALSE;
IF (g_trust_sync_active_command_id[SlaveIndex] = UINT#0) AND (g_trust_sync_queued_command_id[SlaveIndex] <> UINT#0) THEN
    g_trust_sync_active_command_id[SlaveIndex] := g_trust_sync_queued_command_id[SlaveIndex];
    g_trust_sync_active_command_kind[SlaveIndex] := g_trust_sync_queued_command_kind[SlaveIndex];
    g_trust_sync_active_buffer_mode[SlaveIndex] := g_trust_sync_queued_buffer_mode[SlaveIndex];
    g_trust_sync_active_master_axis_index[SlaveIndex] := g_trust_sync_queued_master_axis_index[SlaveIndex];
    g_trust_sync_active_ratio_numerator[SlaveIndex] := g_trust_sync_queued_ratio_numerator[SlaveIndex];
    g_trust_sync_active_ratio_denominator[SlaveIndex] := g_trust_sync_queued_ratio_denominator[SlaveIndex];
    g_trust_sync_active_master_offset[SlaveIndex] := g_trust_sync_queued_master_offset[SlaveIndex];
    g_trust_sync_active_slave_offset[SlaveIndex] := g_trust_sync_queued_slave_offset[SlaveIndex];
    g_trust_sync_active_master_scaling[SlaveIndex] := g_trust_sync_queued_master_scaling[SlaveIndex];
    g_trust_sync_active_slave_scaling[SlaveIndex] := g_trust_sync_queued_slave_scaling[SlaveIndex];
    g_trust_sync_active_master_start_distance[SlaveIndex] := g_trust_sync_queued_master_start_distance[SlaveIndex];
    g_trust_sync_active_master_sync_position[SlaveIndex] := g_trust_sync_queued_master_sync_position[SlaveIndex];
    g_trust_sync_active_slave_sync_position[SlaveIndex] := g_trust_sync_queued_slave_sync_position[SlaveIndex];
    g_trust_sync_active_velocity[SlaveIndex] := g_trust_sync_queued_velocity[SlaveIndex];
    g_trust_sync_active_acceleration[SlaveIndex] := g_trust_sync_queued_acceleration[SlaveIndex];
    g_trust_sync_active_deceleration[SlaveIndex] := g_trust_sync_queued_deceleration[SlaveIndex];
    g_trust_sync_active_jerk[SlaveIndex] := g_trust_sync_queued_jerk[SlaveIndex];
    g_trust_sync_active_start_mode[SlaveIndex] := g_trust_sync_queued_start_mode[SlaveIndex];
    g_trust_sync_active_sync_mode[SlaveIndex] := g_trust_sync_queued_sync_mode[SlaveIndex];
    g_trust_sync_active_master_value_source[SlaveIndex] := g_trust_sync_queued_master_value_source[SlaveIndex];
    g_trust_sync_active_cam_table_id[SlaveIndex] := g_trust_sync_queued_cam_table_id[SlaveIndex];
    TRUST_SYNC_ClearQueuedCommandSlot(AxisIndex := SlaveIndex);
    TRUST_SYNC_PromoteQueuedCommand := TRUE;
END_IF;
END_FUNCTION

FUNCTION_BLOCK TRUST_SYNC_RequestBufferedCommand
VAR_INPUT
    AxisIndex : UINT;
    Kind : TRUST_SYNC_COMMAND_KIND;
    BufferMode : MC_BUFFER_MODE;
END_VAR
VAR_OUTPUT
    Accepted : BOOL;
    ActiveNow : BOOL;
    Error : BOOL;
    ErrorID : WORD;
    CommandID : UINT;
END_VAR
IF NOT TRUST_SYNC_SupportsSyncBufferMode(BufferMode := BufferMode) THEN
    Error := TRUE;
    ErrorID := WORD#16#0500;
ELSIF BufferMode = MC_BUFFER_MODE#mcAborting THEN
    IF g_trust_sync_active_command_id[AxisIndex] <> UINT#0 THEN
        TRUST_SYNC_RecordAbortedCommand(AxisIndex := AxisIndex, CommandID := g_trust_sync_active_command_id[AxisIndex]);
    END_IF;
    IF g_trust_sync_queued_command_id[AxisIndex] <> UINT#0 THEN
        TRUST_SYNC_RecordAbortedCommand(AxisIndex := AxisIndex, CommandID := g_trust_sync_queued_command_id[AxisIndex]);
    END_IF;
    TRUST_SYNC_ClearActiveCommandSlot(AxisIndex := AxisIndex);
    TRUST_SYNC_ClearQueuedCommandSlot(AxisIndex := AxisIndex);
    CommandID := TRUST_SYNC_NextCommandId(AxisIndex := AxisIndex);
    g_trust_sync_active_command_id[AxisIndex] := CommandID;
    g_trust_sync_active_command_kind[AxisIndex] := Kind;
    g_trust_sync_active_buffer_mode[AxisIndex] := BufferMode;
    Accepted := TRUE;
    ActiveNow := TRUE;
ELSIF g_trust_sync_active_command_id[AxisIndex] = UINT#0 THEN
    CommandID := TRUST_SYNC_NextCommandId(AxisIndex := AxisIndex);
    g_trust_sync_active_command_id[AxisIndex] := CommandID;
    g_trust_sync_active_command_kind[AxisIndex] := Kind;
    g_trust_sync_active_buffer_mode[AxisIndex] := BufferMode;
    Accepted := TRUE;
    ActiveNow := TRUE;
ELSIF g_trust_sync_queued_command_id[AxisIndex] = UINT#0 THEN
    CommandID := TRUST_SYNC_NextCommandId(AxisIndex := AxisIndex);
    g_trust_sync_queued_command_id[AxisIndex] := CommandID;
    g_trust_sync_queued_command_kind[AxisIndex] := Kind;
    g_trust_sync_queued_buffer_mode[AxisIndex] := BufferMode;
    Accepted := TRUE;
    ActiveNow := FALSE;
ELSE
    Error := TRUE;
    ErrorID := WORD#16#0400;
END_IF;
END_FUNCTION_BLOCK

FUNCTION TRUST_SYNC_ReadMasterSourcePosition : REAL
VAR_INPUT
    MasterAxisIndex : UINT;
    Source : MC_SOURCE;
END_VAR
IF Source = MC_SOURCE#mcActualValue THEN
    TRUST_SYNC_ReadMasterSourcePosition := g_trust_sync_axis_actual_position[MasterAxisIndex];
ELSE
    TRUST_SYNC_ReadMasterSourcePosition := g_trust_sync_axis_commanded_position[MasterAxisIndex];
END_IF;
END_FUNCTION

FUNCTION TRUST_SYNC_CamLastPointIndex : INT
VAR_INPUT
    SlaveIndex : UINT;
END_VAR
TRUST_SYNC_CamLastPointIndex := INT#0;
IF g_trust_sync_selected_cam_point_count[SlaveIndex] > UINT#0 THEN
    TRUST_SYNC_CamLastPointIndex := UINT_TO_INT(g_trust_sync_selected_cam_point_count[SlaveIndex]) - INT#1;
END_IF;
END_FUNCTION

FUNCTION TRUST_SYNC_CamSlavePositionAt : REAL
VAR_INPUT
    SlaveIndex : UINT;
    SampleMaster : REAL;
END_VAR
VAR
    Index : INT;
    LastPoint : INT;
END_VAR
TRUST_SYNC_CamSlavePositionAt := REAL#0.0;
LastPoint := TRUST_SYNC_CamLastPointIndex(SlaveIndex := SlaveIndex);
IF g_trust_sync_selected_cam_point_count[SlaveIndex] > UINT#0 THEN
    TRUST_SYNC_CamSlavePositionAt := g_trust_sync_selected_cam_slave[SlaveIndex, 0];
END_IF;
FOR Index := INT#1 TO LastPoint DO
    IF SampleMaster >= g_trust_sync_selected_cam_master[SlaveIndex, Index] THEN
        TRUST_SYNC_CamSlavePositionAt := g_trust_sync_selected_cam_slave[SlaveIndex, Index];
    END_IF;
END_FOR;
END_FUNCTION

FUNCTION TRUST_SYNC_ApplySlaveTarget : BOOL
VAR_INPUT
    SlaveIndex : UINT;
    TargetPosition : REAL;
END_VAR
g_trust_sync_axis_velocity[SlaveIndex] := TargetPosition - g_trust_sync_axis_commanded_position[SlaveIndex];
g_trust_sync_axis_last_commanded_position[SlaveIndex] := g_trust_sync_axis_commanded_position[SlaveIndex];
g_trust_sync_axis_commanded_position[SlaveIndex] := TargetPosition;
g_trust_sync_axis_actual_position[SlaveIndex] := TargetPosition;
TRUST_SYNC_ApplySlaveTarget := TRUE;
END_FUNCTION

FUNCTION TRUST_SYNC_BeginActiveCommand : BOOL
VAR_INPUT
    SlaveIndex : UINT;
END_VAR
VAR
    MasterAxisIndex : UINT;
END_VAR
MasterAxisIndex := g_trust_sync_active_master_axis_index[SlaveIndex];
g_trust_sync_active_command_start_master_position[SlaveIndex] := TRUST_SYNC_ReadMasterSourcePosition(
    MasterAxisIndex := MasterAxisIndex,
    Source := g_trust_sync_active_master_value_source[SlaveIndex]
);
g_trust_sync_active_command_start_slave_position[SlaveIndex] := g_trust_sync_axis_commanded_position[SlaveIndex];
g_trust_sync_active_start_sync_seen[SlaveIndex] := FALSE;
g_trust_sync_active_start_sync_pulse[SlaveIndex] := FALSE;
g_trust_sync_active_end_of_profile_pulse[SlaveIndex] := FALSE;
g_trust_sync_active_in_sync[SlaveIndex] := FALSE;
g_trust_sync_active_in_gear[SlaveIndex] := FALSE;
g_trust_sync_active_previous_source_position[SlaveIndex] := g_trust_sync_active_command_start_master_position[SlaveIndex];
g_trust_sync_axis_state[SlaveIndex] := MC_AXIS_STATUS#mcContinuousMotion;
TRUST_SYNC_BeginActiveCommand := TRUE;
END_FUNCTION

FUNCTION TRUST_SYNC_CamRefMasterPoint : REAL
VAR_INPUT
    CamTable : MC_CAM_REF;
    PointIndex : INT;
END_VAR
CASE PointIndex OF
    INT#0: TRUST_SYNC_CamRefMasterPoint := CamTable.MasterPosition0;
    INT#1: TRUST_SYNC_CamRefMasterPoint := CamTable.MasterPosition1;
    INT#2: TRUST_SYNC_CamRefMasterPoint := CamTable.MasterPosition2;
    INT#3: TRUST_SYNC_CamRefMasterPoint := CamTable.MasterPosition3;
    INT#4: TRUST_SYNC_CamRefMasterPoint := CamTable.MasterPosition4;
    INT#5: TRUST_SYNC_CamRefMasterPoint := CamTable.MasterPosition5;
    INT#6: TRUST_SYNC_CamRefMasterPoint := CamTable.MasterPosition6;
    INT#7: TRUST_SYNC_CamRefMasterPoint := CamTable.MasterPosition7;
ELSE
    TRUST_SYNC_CamRefMasterPoint := REAL#0.0;
END_CASE;
END_FUNCTION

FUNCTION TRUST_SYNC_CamRefSlavePoint : REAL
VAR_INPUT
    CamTable : MC_CAM_REF;
    PointIndex : INT;
END_VAR
CASE PointIndex OF
    INT#0: TRUST_SYNC_CamRefSlavePoint := CamTable.SlavePosition0;
    INT#1: TRUST_SYNC_CamRefSlavePoint := CamTable.SlavePosition1;
    INT#2: TRUST_SYNC_CamRefSlavePoint := CamTable.SlavePosition2;
    INT#3: TRUST_SYNC_CamRefSlavePoint := CamTable.SlavePosition3;
    INT#4: TRUST_SYNC_CamRefSlavePoint := CamTable.SlavePosition4;
    INT#5: TRUST_SYNC_CamRefSlavePoint := CamTable.SlavePosition5;
    INT#6: TRUST_SYNC_CamRefSlavePoint := CamTable.SlavePosition6;
    INT#7: TRUST_SYNC_CamRefSlavePoint := CamTable.SlavePosition7;
ELSE
    TRUST_SYNC_CamRefSlavePoint := REAL#0.0;
END_CASE;
END_FUNCTION

FUNCTION TRUST_SYNC_LoadSelectedCamFromRef : BOOL
VAR_INPUT
    SlaveIndex : UINT;
    CamTable : MC_CAM_REF;
    Periodic : BOOL;
    MasterAbsolute : BOOL;
    SlaveAbsolute : BOOL;
END_VAR
VAR
    PointIndex : INT;
    PointCount : INT;
END_VAR
TRUST_SYNC_ResetSelection(AxisIndex := SlaveIndex);
PointCount := UINT_TO_INT(CamTable.NumberOfPairs);
IF PointCount > INT#8 THEN
    PointCount := INT#8;
END_IF;
IF PointCount < INT#0 THEN
    PointCount := INT#0;
END_IF;
g_trust_sync_selected_cam_ready[SlaveIndex] := PointCount > INT#0;
g_trust_sync_selected_cam_id[SlaveIndex] := CamTable.CamId;
g_trust_sync_selected_cam_periodic[SlaveIndex] := Periodic;
g_trust_sync_selected_cam_master_absolute[SlaveIndex] := MasterAbsolute;
g_trust_sync_selected_cam_slave_absolute[SlaveIndex] := SlaveAbsolute;
g_trust_sync_selected_cam_point_count[SlaveIndex] := INT_TO_UINT(PointCount);
FOR PointIndex := INT#0 TO PointCount - INT#1 DO
    g_trust_sync_selected_cam_master[SlaveIndex, PointIndex] := TRUST_SYNC_CamRefMasterPoint(CamTable := CamTable, PointIndex := PointIndex);
    g_trust_sync_selected_cam_slave[SlaveIndex, PointIndex] := TRUST_SYNC_CamRefSlavePoint(CamTable := CamTable, PointIndex := PointIndex);
END_FOR;
TRUST_SYNC_LoadSelectedCamFromRef := TRUE;
END_FUNCTION

FUNCTION TRUST_SYNC_LoadDefaultCamTable : BOOL
VAR_INPUT
    SlaveIndex : UINT;
END_VAR
g_trust_sync_selected_cam_ready[SlaveIndex] := TRUE;
g_trust_sync_selected_cam_id[SlaveIndex] := SlaveIndex + UINT#1;
g_trust_sync_selected_cam_periodic[SlaveIndex] := TRUE;
g_trust_sync_selected_cam_master_absolute[SlaveIndex] := TRUE;
g_trust_sync_selected_cam_slave_absolute[SlaveIndex] := TRUE;
g_trust_sync_selected_cam_point_count[SlaveIndex] := UINT#4;
g_trust_sync_selected_cam_master[SlaveIndex, 0] := REAL#0.0;
g_trust_sync_selected_cam_slave[SlaveIndex, 0] := REAL#0.0;
g_trust_sync_selected_cam_master[SlaveIndex, 1] := REAL#10.0;
g_trust_sync_selected_cam_slave[SlaveIndex, 1] := REAL#5.0;
g_trust_sync_selected_cam_master[SlaveIndex, 2] := REAL#20.0;
g_trust_sync_selected_cam_slave[SlaveIndex, 2] := REAL#15.0;
g_trust_sync_selected_cam_master[SlaveIndex, 3] := REAL#30.0;
g_trust_sync_selected_cam_slave[SlaveIndex, 3] := REAL#30.0;
TRUST_SYNC_LoadDefaultCamTable := TRUE;
END_FUNCTION

FUNCTION TRUST_SYNC_StoreActiveCamParameters : BOOL
VAR_INPUT
    SlaveIndex : UINT;
    MasterIndex : UINT;
    MasterOffset : REAL;
    SlaveOffset : REAL;
    MasterScaling : REAL;
    SlaveScaling : REAL;
    MasterStartDistance : REAL;
    MasterSyncPosition : REAL;
    StartMode : MC_START_MODE;
    MasterValueSource : MC_SOURCE;
    CamTableID : MC_CAM_ID;
END_VAR
g_trust_sync_active_master_axis_index[SlaveIndex] := MasterIndex;
g_trust_sync_active_master_offset[SlaveIndex] := MasterOffset;
g_trust_sync_active_slave_offset[SlaveIndex] := SlaveOffset;
g_trust_sync_active_master_scaling[SlaveIndex] := MasterScaling;
g_trust_sync_active_slave_scaling[SlaveIndex] := SlaveScaling;
g_trust_sync_active_master_start_distance[SlaveIndex] := MasterStartDistance;
g_trust_sync_active_master_sync_position[SlaveIndex] := MasterSyncPosition;
g_trust_sync_active_start_mode[SlaveIndex] := StartMode;
g_trust_sync_active_master_value_source[SlaveIndex] := MasterValueSource;
g_trust_sync_active_cam_table_id[SlaveIndex] := CamTableID;
TRUST_SYNC_StoreActiveCamParameters := TRUE;
END_FUNCTION

FUNCTION TRUST_SYNC_StoreQueuedCamParameters : BOOL
VAR_INPUT
    SlaveIndex : UINT;
    MasterIndex : UINT;
    MasterOffset : REAL;
    SlaveOffset : REAL;
    MasterScaling : REAL;
    SlaveScaling : REAL;
    MasterStartDistance : REAL;
    MasterSyncPosition : REAL;
    StartMode : MC_START_MODE;
    MasterValueSource : MC_SOURCE;
    CamTableID : MC_CAM_ID;
END_VAR
g_trust_sync_queued_master_axis_index[SlaveIndex] := MasterIndex;
g_trust_sync_queued_master_offset[SlaveIndex] := MasterOffset;
g_trust_sync_queued_slave_offset[SlaveIndex] := SlaveOffset;
g_trust_sync_queued_master_scaling[SlaveIndex] := MasterScaling;
g_trust_sync_queued_slave_scaling[SlaveIndex] := SlaveScaling;
g_trust_sync_queued_master_start_distance[SlaveIndex] := MasterStartDistance;
g_trust_sync_queued_master_sync_position[SlaveIndex] := MasterSyncPosition;
g_trust_sync_queued_start_mode[SlaveIndex] := StartMode;
g_trust_sync_queued_master_value_source[SlaveIndex] := MasterValueSource;
g_trust_sync_queued_cam_table_id[SlaveIndex] := CamTableID;
TRUST_SYNC_StoreQueuedCamParameters := TRUE;
END_FUNCTION

FUNCTION TRUST_SYNC_StoreActiveGearParameters : BOOL
VAR_INPUT
    SlaveIndex : UINT;
    MasterIndex : UINT;
    RatioNumerator : INT;
    RatioDenominator : UINT;
    MasterValueSource : MC_SOURCE;
    Acceleration : REAL;
    Deceleration : REAL;
    Jerk : REAL;
END_VAR
g_trust_sync_active_master_axis_index[SlaveIndex] := MasterIndex;
g_trust_sync_active_ratio_numerator[SlaveIndex] := RatioNumerator;
g_trust_sync_active_ratio_denominator[SlaveIndex] := RatioDenominator;
g_trust_sync_active_master_value_source[SlaveIndex] := MasterValueSource;
g_trust_sync_active_acceleration[SlaveIndex] := Acceleration;
g_trust_sync_active_deceleration[SlaveIndex] := Deceleration;
g_trust_sync_active_jerk[SlaveIndex] := Jerk;
TRUST_SYNC_StoreActiveGearParameters := TRUE;
END_FUNCTION

FUNCTION TRUST_SYNC_StoreQueuedGearParameters : BOOL
VAR_INPUT
    SlaveIndex : UINT;
    MasterIndex : UINT;
    RatioNumerator : INT;
    RatioDenominator : UINT;
    MasterValueSource : MC_SOURCE;
    Acceleration : REAL;
    Deceleration : REAL;
    Jerk : REAL;
END_VAR
g_trust_sync_queued_master_axis_index[SlaveIndex] := MasterIndex;
g_trust_sync_queued_ratio_numerator[SlaveIndex] := RatioNumerator;
g_trust_sync_queued_ratio_denominator[SlaveIndex] := RatioDenominator;
g_trust_sync_queued_master_value_source[SlaveIndex] := MasterValueSource;
g_trust_sync_queued_acceleration[SlaveIndex] := Acceleration;
g_trust_sync_queued_deceleration[SlaveIndex] := Deceleration;
g_trust_sync_queued_jerk[SlaveIndex] := Jerk;
TRUST_SYNC_StoreQueuedGearParameters := TRUE;
END_FUNCTION

FUNCTION TRUST_SYNC_StoreActiveGearInPosParameters : BOOL
VAR_INPUT
    SlaveIndex : UINT;
    MasterIndex : UINT;
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
END_VAR
g_trust_sync_active_master_axis_index[SlaveIndex] := MasterIndex;
g_trust_sync_active_ratio_numerator[SlaveIndex] := RatioNumerator;
g_trust_sync_active_ratio_denominator[SlaveIndex] := RatioDenominator;
g_trust_sync_active_master_value_source[SlaveIndex] := MasterValueSource;
g_trust_sync_active_master_sync_position[SlaveIndex] := MasterSyncPosition;
g_trust_sync_active_slave_sync_position[SlaveIndex] := SlaveSyncPosition;
g_trust_sync_active_sync_mode[SlaveIndex] := SyncMode;
g_trust_sync_active_master_start_distance[SlaveIndex] := MasterStartDistance;
g_trust_sync_active_velocity[SlaveIndex] := Velocity;
g_trust_sync_active_acceleration[SlaveIndex] := Acceleration;
g_trust_sync_active_deceleration[SlaveIndex] := Deceleration;
g_trust_sync_active_jerk[SlaveIndex] := Jerk;
TRUST_SYNC_StoreActiveGearInPosParameters := TRUE;
END_FUNCTION

FUNCTION TRUST_SYNC_StoreQueuedGearInPosParameters : BOOL
VAR_INPUT
    SlaveIndex : UINT;
    MasterIndex : UINT;
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
END_VAR
g_trust_sync_queued_master_axis_index[SlaveIndex] := MasterIndex;
g_trust_sync_queued_ratio_numerator[SlaveIndex] := RatioNumerator;
g_trust_sync_queued_ratio_denominator[SlaveIndex] := RatioDenominator;
g_trust_sync_queued_master_value_source[SlaveIndex] := MasterValueSource;
g_trust_sync_queued_master_sync_position[SlaveIndex] := MasterSyncPosition;
g_trust_sync_queued_slave_sync_position[SlaveIndex] := SlaveSyncPosition;
g_trust_sync_queued_sync_mode[SlaveIndex] := SyncMode;
g_trust_sync_queued_master_start_distance[SlaveIndex] := MasterStartDistance;
g_trust_sync_queued_velocity[SlaveIndex] := Velocity;
g_trust_sync_queued_acceleration[SlaveIndex] := Acceleration;
g_trust_sync_queued_deceleration[SlaveIndex] := Deceleration;
g_trust_sync_queued_jerk[SlaveIndex] := Jerk;
TRUST_SYNC_StoreQueuedGearInPosParameters := TRUE;
END_FUNCTION

FUNCTION TRUST_SYNC_ProcessActiveCommand : BOOL
VAR_INPUT
    SlaveIndex : UINT;
END_VAR
VAR
    MasterIndex : UINT;
    SourcePosition : REAL;
    PreviousSource : REAL;
    StartThreshold : REAL;
    TargetPosition : REAL;
    Ratio : REAL;
    LastPoint : INT;
    EffectiveMaster : REAL;
    ProfileEnd : REAL;
END_VAR
TRUST_SYNC_ProcessActiveCommand := FALSE;
g_trust_sync_active_start_sync_pulse[SlaveIndex] := FALSE;
g_trust_sync_active_end_of_profile_pulse[SlaveIndex] := FALSE;
IF g_trust_sync_active_command_id[SlaveIndex] <> UINT#0 THEN
MasterIndex := g_trust_sync_active_master_axis_index[SlaveIndex];
SourcePosition := TRUST_SYNC_ReadMasterSourcePosition(
    MasterAxisIndex := MasterIndex,
    Source := g_trust_sync_active_master_value_source[SlaveIndex]
);
PreviousSource := g_trust_sync_active_previous_source_position[SlaveIndex];
CASE g_trust_sync_active_command_kind[SlaveIndex] OF
    TRUST_SYNC_COMMAND_KIND#trustSyncCamIn:
        StartThreshold := g_trust_sync_active_master_sync_position[SlaveIndex] - g_trust_sync_active_master_start_distance[SlaveIndex];
        IF (NOT g_trust_sync_active_start_sync_seen[SlaveIndex]) AND (SourcePosition >= StartThreshold) THEN
            g_trust_sync_active_start_sync_seen[SlaveIndex] := TRUE;
        END_IF;
        IF SourcePosition >= g_trust_sync_active_master_sync_position[SlaveIndex] THEN
            g_trust_sync_active_in_sync[SlaveIndex] := TRUE;
            g_trust_sync_axis_state[SlaveIndex] := MC_AXIS_STATUS#mcSynchronizedMotion;
        ELSE
            g_trust_sync_axis_state[SlaveIndex] := MC_AXIS_STATUS#mcContinuousMotion;
        END_IF;
        IF g_trust_sync_active_start_mode[SlaveIndex] = MC_START_MODE#mcAbsolute THEN
            EffectiveMaster := (SourcePosition * g_trust_sync_active_master_scaling[SlaveIndex]) + g_trust_sync_active_master_offset[SlaveIndex];
        ELSE
            EffectiveMaster := ((SourcePosition - g_trust_sync_active_command_start_master_position[SlaveIndex]) * g_trust_sync_active_master_scaling[SlaveIndex]) + g_trust_sync_active_master_offset[SlaveIndex];
        END_IF;
        LastPoint := TRUST_SYNC_CamLastPointIndex(SlaveIndex := SlaveIndex);
        ProfileEnd := REAL#0.0;
        IF LastPoint >= INT#0 THEN
            ProfileEnd := g_trust_sync_selected_cam_master[SlaveIndex, LastPoint];
        END_IF;
        IF g_trust_sync_selected_cam_periodic[SlaveIndex] AND (PreviousSource < ProfileEnd) AND (SourcePosition >= ProfileEnd) THEN
            g_trust_sync_active_end_of_profile_pulse[SlaveIndex] := TRUE;
            IF EffectiveMaster > ProfileEnd THEN
                EffectiveMaster := EffectiveMaster - ProfileEnd;
            END_IF;
        ELSIF (NOT g_trust_sync_selected_cam_periodic[SlaveIndex]) AND (PreviousSource < ProfileEnd) AND (SourcePosition >= ProfileEnd) THEN
            g_trust_sync_active_end_of_profile_pulse[SlaveIndex] := TRUE;
            EffectiveMaster := ProfileEnd;
        END_IF;
        TargetPosition := TRUST_SYNC_CamSlavePositionAt(SlaveIndex := SlaveIndex, SampleMaster := EffectiveMaster);
        TargetPosition := (TargetPosition * g_trust_sync_active_slave_scaling[SlaveIndex]) + g_trust_sync_active_slave_offset[SlaveIndex];
        IF g_trust_sync_active_in_sync[SlaveIndex] THEN
            TRUST_SYNC_ApplySlaveTarget(SlaveIndex := SlaveIndex, TargetPosition := TargetPosition);
        END_IF;
    TRUST_SYNC_COMMAND_KIND#trustSyncGearIn:
        Ratio := REAL#0.0;
        IF g_trust_sync_active_ratio_denominator[SlaveIndex] <> UINT#0 THEN
            Ratio := INT_TO_REAL(g_trust_sync_active_ratio_numerator[SlaveIndex]) / UINT_TO_REAL(g_trust_sync_active_ratio_denominator[SlaveIndex]);
        END_IF;
        TargetPosition := g_trust_sync_active_command_start_slave_position[SlaveIndex] +
            ((SourcePosition - g_trust_sync_active_command_start_master_position[SlaveIndex]) * Ratio);
        TRUST_SYNC_ApplySlaveTarget(SlaveIndex := SlaveIndex, TargetPosition := TargetPosition);
        IF SourcePosition <> g_trust_sync_active_command_start_master_position[SlaveIndex] THEN
            g_trust_sync_active_in_gear[SlaveIndex] := TRUE;
            g_trust_sync_axis_state[SlaveIndex] := MC_AXIS_STATUS#mcSynchronizedMotion;
        ELSE
            g_trust_sync_axis_state[SlaveIndex] := MC_AXIS_STATUS#mcContinuousMotion;
        END_IF;
    TRUST_SYNC_COMMAND_KIND#trustSyncGearInPos:
        StartThreshold := g_trust_sync_active_master_sync_position[SlaveIndex] - g_trust_sync_active_master_start_distance[SlaveIndex];
        IF (NOT g_trust_sync_active_start_sync_seen[SlaveIndex]) AND (SourcePosition >= StartThreshold) THEN
            g_trust_sync_active_start_sync_seen[SlaveIndex] := TRUE;
            g_trust_sync_active_start_sync_pulse[SlaveIndex] := TRUE;
        END_IF;
        IF SourcePosition >= g_trust_sync_active_master_sync_position[SlaveIndex] THEN
            Ratio := REAL#0.0;
            IF g_trust_sync_active_ratio_denominator[SlaveIndex] <> UINT#0 THEN
                Ratio := INT_TO_REAL(g_trust_sync_active_ratio_numerator[SlaveIndex]) / UINT_TO_REAL(g_trust_sync_active_ratio_denominator[SlaveIndex]);
            END_IF;
            TargetPosition := g_trust_sync_active_slave_sync_position[SlaveIndex] +
                ((SourcePosition - g_trust_sync_active_master_sync_position[SlaveIndex]) * Ratio);
            TRUST_SYNC_ApplySlaveTarget(SlaveIndex := SlaveIndex, TargetPosition := TargetPosition);
            g_trust_sync_active_in_sync[SlaveIndex] := TRUE;
            g_trust_sync_axis_state[SlaveIndex] := MC_AXIS_STATUS#mcSynchronizedMotion;
        ELSE
            g_trust_sync_axis_state[SlaveIndex] := MC_AXIS_STATUS#mcContinuousMotion;
        END_IF;
END_CASE;
g_trust_sync_active_previous_source_position[SlaveIndex] := SourcePosition;
TRUST_SYNC_ProcessActiveCommand := TRUE;
END_IF;
END_FUNCTION

FUNCTION_BLOCK TRUST_SYNC_TestSeed
VAR_IN_OUT
    Master : AXIS_REF;
    Slave : AXIS_REF;
END_VAR
VAR_INPUT
    StartPosition : REAL;
    ExitPosition : REAL;
    RatioNumerator : REAL;
    RatioDenominator : REAL;
    PhaseOffset : REAL;
    LoadDefaultCam : BOOL;
END_VAR
VAR
    MasterIndex : UINT;
    SlaveIndex : UINT;
END_VAR
MasterIndex := TRUST_SYNC_AxisIndex(Axis := Master);
SlaveIndex := TRUST_SYNC_AxisIndex(Axis := Slave);
TRUST_SYNC_ResetAxis(AxisIndex := MasterIndex);
TRUST_SYNC_ResetSlaveKernel(AxisIndex := SlaveIndex);
g_trust_sync_seed_start_position[SlaveIndex] := StartPosition;
g_trust_sync_seed_exit_position[SlaveIndex] := ExitPosition;
g_trust_sync_seed_ratio_numerator[SlaveIndex] := RatioNumerator;
g_trust_sync_seed_ratio_denominator[SlaveIndex] := RatioDenominator;
g_trust_sync_seed_phase_offset[SlaveIndex] := PhaseOffset;
g_trust_sync_seed_break_requested[SlaveIndex] := FALSE;
IF LoadDefaultCam THEN
    TRUST_SYNC_LoadDefaultCamTable(SlaveIndex := SlaveIndex);
END_IF;
END_FUNCTION_BLOCK

FUNCTION_BLOCK TRUST_SYNC_AdvanceMasterAxis
VAR_IN_OUT
    Master : AXIS_REF;
    Slave : AXIS_REF;
END_VAR
VAR_INPUT
    NextCommandedMasterPosition : REAL;
    NextActualMasterPosition : REAL;
    UseActualMasterPosition : BOOL;
    BreakSync : BOOL;
END_VAR
VAR_OUTPUT
    InSync : BOOL;
    InGear : BOOL;
    GearSlavePosition : REAL;
    CamSlavePosition : REAL;
    StartSync : BOOL;
    EndOfProfile : BOOL;
END_VAR
VAR
    MasterIndex : UINT;
    SlaveIndex : UINT;
    Ratio : REAL;
END_VAR
MasterIndex := TRUST_SYNC_AxisIndex(Axis := Master);
SlaveIndex := TRUST_SYNC_AxisIndex(Axis := Slave);
IF NOT g_trust_sync_axis_initialized[MasterIndex] THEN
    TRUST_SYNC_ResetAxis(AxisIndex := MasterIndex);
END_IF;
IF NOT g_trust_sync_axis_initialized[SlaveIndex] THEN
    TRUST_SYNC_ResetSlaveKernel(AxisIndex := SlaveIndex);
END_IF;
g_trust_sync_axis_last_commanded_position[MasterIndex] := g_trust_sync_axis_commanded_position[MasterIndex];
g_trust_sync_axis_commanded_position[MasterIndex] := NextCommandedMasterPosition;
IF UseActualMasterPosition THEN
    g_trust_sync_axis_actual_position[MasterIndex] := NextActualMasterPosition;
ELSE
    g_trust_sync_axis_actual_position[MasterIndex] := NextCommandedMasterPosition;
END_IF;
g_trust_sync_axis_velocity[MasterIndex] :=
    g_trust_sync_axis_commanded_position[MasterIndex] - g_trust_sync_axis_last_commanded_position[MasterIndex];
g_trust_sync_seed_break_requested[SlaveIndex] := BreakSync;
IF BreakSync AND (g_trust_sync_active_command_id[SlaveIndex] <> UINT#0) THEN
    TRUST_SYNC_RecordAbortedCommand(AxisIndex := SlaveIndex, CommandID := g_trust_sync_active_command_id[SlaveIndex]);
    TRUST_SYNC_ClearActiveCommandSlot(AxisIndex := SlaveIndex);
    g_trust_sync_axis_state[SlaveIndex] := MC_AXIS_STATUS#mcContinuousMotion;
    TRUST_SYNC_PromoteQueuedCommand(SlaveIndex := SlaveIndex);
END_IF;
IF g_trust_sync_active_command_id[SlaveIndex] <> UINT#0 THEN
    TRUST_SYNC_ProcessActiveCommand(SlaveIndex := SlaveIndex);
    InSync := g_trust_sync_active_in_sync[SlaveIndex];
    InGear := g_trust_sync_active_in_gear[SlaveIndex];
    StartSync := g_trust_sync_active_start_sync_pulse[SlaveIndex];
    EndOfProfile := g_trust_sync_active_end_of_profile_pulse[SlaveIndex];
    GearSlavePosition := g_trust_sync_axis_commanded_position[SlaveIndex];
    CamSlavePosition := g_trust_sync_axis_commanded_position[SlaveIndex];
ELSE
    InSync :=
        (g_trust_sync_axis_commanded_position[MasterIndex] >= g_trust_sync_seed_start_position[SlaveIndex]) AND
        (g_trust_sync_axis_commanded_position[MasterIndex] < g_trust_sync_seed_exit_position[SlaveIndex]) AND
        (NOT BreakSync);
    Ratio := REAL#0.0;
    IF g_trust_sync_seed_ratio_denominator[SlaveIndex] <> REAL#0.0 THEN
        Ratio := g_trust_sync_seed_ratio_numerator[SlaveIndex] / g_trust_sync_seed_ratio_denominator[SlaveIndex];
    END_IF;
    GearSlavePosition := (g_trust_sync_axis_commanded_position[MasterIndex] * Ratio) + g_trust_sync_seed_phase_offset[SlaveIndex];
    CamSlavePosition := TRUST_SYNC_CamSlavePositionAt(
        SlaveIndex := SlaveIndex,
        SampleMaster := g_trust_sync_axis_commanded_position[MasterIndex]
    );
    InGear := InSync;
    StartSync := FALSE;
    EndOfProfile := FALSE;
    IF InSync THEN
        g_trust_sync_axis_state[SlaveIndex] := MC_AXIS_STATUS#mcSynchronizedMotion;
        TRUST_SYNC_ApplySlaveTarget(SlaveIndex := SlaveIndex, TargetPosition := GearSlavePosition);
    ELSE
        g_trust_sync_axis_state[SlaveIndex] := MC_AXIS_STATUS#mcContinuousMotion;
    END_IF;
END_IF;
END_FUNCTION_BLOCK

FUNCTION_BLOCK TRUST_SYNC_TestProbe
VAR_IN_OUT
    Axis : AXIS_REF;
END_VAR
VAR_OUTPUT
    State : MC_AXIS_STATUS;
    CommandedPosition : REAL;
    ActualPosition : REAL;
    Velocity : REAL;
    SelectedCamReady : BOOL;
    SelectedCamID : MC_CAM_ID;
    HasActiveCommand : BOOL;
    HasQueuedCommand : BOOL;
    InSync : BOOL;
    InGear : BOOL;
    StartSync : BOOL;
    EndOfProfile : BOOL;
END_VAR
VAR
    AxisIndex : UINT;
END_VAR
AxisIndex := TRUST_SYNC_AxisIndex(Axis := Axis);
State := g_trust_sync_axis_state[AxisIndex];
CommandedPosition := g_trust_sync_axis_commanded_position[AxisIndex];
ActualPosition := g_trust_sync_axis_actual_position[AxisIndex];
Velocity := g_trust_sync_axis_velocity[AxisIndex];
SelectedCamReady := g_trust_sync_selected_cam_ready[AxisIndex];
SelectedCamID := g_trust_sync_selected_cam_id[AxisIndex];
HasActiveCommand := g_trust_sync_active_command_id[AxisIndex] <> UINT#0;
HasQueuedCommand := g_trust_sync_queued_command_id[AxisIndex] <> UINT#0;
InSync := g_trust_sync_active_in_sync[AxisIndex];
InGear := g_trust_sync_active_in_gear[AxisIndex];
StartSync := g_trust_sync_active_start_sync_pulse[AxisIndex];
EndOfProfile := g_trust_sync_active_end_of_profile_pulse[AxisIndex];
END_FUNCTION_BLOCK
