TYPE TRUST_MOTION_COMMAND_KIND : (
    trustMotionNone,
    trustMotionHome,
    trustMotionStepAbsoluteSwitch,
    trustMotionStepLimitSwitch,
    trustMotionStepBlock,
    trustMotionStepReferencePulse,
    trustMotionStepDistanceCoded,
    trustMotionHomeDirect,
    trustMotionHomeAbsolute,
    trustMotionFinishHoming,
    trustMotionHalt,
    trustMotionMoveAbsolute,
    trustMotionMoveRelative,
    trustMotionMoveAdditive,
    trustMotionMoveVelocity,
    trustMotionMoveContinuousAbsolute,
    trustMotionMoveContinuousRelative,
    trustMotionSetPosition,
    trustMotionWriteParameter,
    trustMotionWriteBoolParameter
);
END_TYPE

FUNCTION TRUST_MOTION_AxisIndex : UINT
VAR_INPUT
    Axis : AXIS_REF;
END_VAR
TRUST_MOTION_AxisIndex := Axis.InternalIndex;
END_FUNCTION

FUNCTION TRUST_MOTION_IsNumericParameter : BOOL
VAR_INPUT
    ParameterNumber : INT;
END_VAR
TRUST_MOTION_IsNumericParameter :=
    (ParameterNumber = INT#1) OR
    (ParameterNumber = INT#2) OR
    (ParameterNumber = INT#3) OR
    (ParameterNumber = INT#7) OR
    (ParameterNumber = INT#8) OR
    (ParameterNumber = INT#9) OR
    (ParameterNumber = INT#10) OR
    (ParameterNumber = INT#11) OR
    (ParameterNumber = INT#12) OR
    (ParameterNumber = INT#13) OR
    (ParameterNumber = INT#14) OR
    (ParameterNumber = INT#15) OR
    (ParameterNumber = INT#16) OR
    (ParameterNumber = INT#17);
END_FUNCTION

FUNCTION TRUST_MOTION_IsBoolParameter : BOOL
VAR_INPUT
    ParameterNumber : INT;
END_VAR
TRUST_MOTION_IsBoolParameter :=
    (ParameterNumber = INT#4) OR
    (ParameterNumber = INT#5) OR
    (ParameterNumber = INT#6) OR
    (ParameterNumber = INT#1000);
END_FUNCTION

FUNCTION TRUST_MOTION_NextCommandId : UINT
VAR_INPUT
    AxisIndex : UINT;
END_VAR
g_trust_motion_next_command_id[AxisIndex] := g_trust_motion_next_command_id[AxisIndex] + UINT#1;
IF g_trust_motion_next_command_id[AxisIndex] = UINT#0 THEN
    g_trust_motion_next_command_id[AxisIndex] := UINT#1;
END_IF;
TRUST_MOTION_NextCommandId := g_trust_motion_next_command_id[AxisIndex];
END_FUNCTION

FUNCTION TRUST_MOTION_CommandIsQueued : BOOL
VAR_INPUT
    AxisIndex : UINT;
    CommandID : UINT;
END_VAR
TRUST_MOTION_CommandIsQueued :=
    ((g_trust_motion_queue_depth[AxisIndex] >= UINT#1) AND (g_trust_motion_queue0_command_id[AxisIndex] = CommandID)) OR
    ((g_trust_motion_queue_depth[AxisIndex] >= UINT#2) AND (g_trust_motion_queue1_command_id[AxisIndex] = CommandID));
END_FUNCTION

FUNCTION TRUST_MOTION_CommandWasAborted : BOOL
VAR_INPUT
    AxisIndex : UINT;
    CommandID : UINT;
END_VAR
TRUST_MOTION_CommandWasAborted :=
    (g_trust_motion_aborted0_command_id[AxisIndex] = CommandID) OR
    (g_trust_motion_aborted1_command_id[AxisIndex] = CommandID) OR
    (g_trust_motion_aborted2_command_id[AxisIndex] = CommandID);
END_FUNCTION

FUNCTION TRUST_MOTION_ClearAbortedCommand : BOOL
VAR_INPUT
    AxisIndex : UINT;
    CommandID : UINT;
END_VAR
TRUST_MOTION_ClearAbortedCommand := FALSE;
IF g_trust_motion_aborted0_command_id[AxisIndex] = CommandID THEN
    g_trust_motion_aborted0_command_id[AxisIndex] := UINT#0;
    TRUST_MOTION_ClearAbortedCommand := TRUE;
END_IF;
IF g_trust_motion_aborted1_command_id[AxisIndex] = CommandID THEN
    g_trust_motion_aborted1_command_id[AxisIndex] := UINT#0;
    TRUST_MOTION_ClearAbortedCommand := TRUE;
END_IF;
IF g_trust_motion_aborted2_command_id[AxisIndex] = CommandID THEN
    g_trust_motion_aborted2_command_id[AxisIndex] := UINT#0;
    TRUST_MOTION_ClearAbortedCommand := TRUE;
END_IF;
END_FUNCTION

FUNCTION TRUST_MOTION_RecordAbortedCommand : BOOL
VAR_INPUT
    AxisIndex : UINT;
    CommandID : UINT;
END_VAR
TRUST_MOTION_RecordAbortedCommand := FALSE;
IF CommandID <> UINT#0 THEN
    IF TRUST_MOTION_CommandWasAborted(AxisIndex := AxisIndex, CommandID := CommandID) THEN
        TRUST_MOTION_RecordAbortedCommand := TRUE;
    ELSIF g_trust_motion_aborted0_command_id[AxisIndex] = UINT#0 THEN
        g_trust_motion_aborted0_command_id[AxisIndex] := CommandID;
        TRUST_MOTION_RecordAbortedCommand := TRUE;
    ELSIF g_trust_motion_aborted1_command_id[AxisIndex] = UINT#0 THEN
        g_trust_motion_aborted1_command_id[AxisIndex] := CommandID;
        TRUST_MOTION_RecordAbortedCommand := TRUE;
    ELSIF g_trust_motion_aborted2_command_id[AxisIndex] = UINT#0 THEN
        g_trust_motion_aborted2_command_id[AxisIndex] := CommandID;
        TRUST_MOTION_RecordAbortedCommand := TRUE;
    END_IF;
END_IF;
END_FUNCTION

FUNCTION TRUST_MOTION_PromoteQueuedCommand : BOOL
VAR_INPUT
    AxisIndex : UINT;
END_VAR
TRUST_MOTION_PromoteQueuedCommand := FALSE;
IF (g_trust_motion_active_command_id[AxisIndex] = UINT#0) AND (g_trust_motion_queue_depth[AxisIndex] >= UINT#1) THEN
    g_trust_motion_active_command_id[AxisIndex] := g_trust_motion_queue0_command_id[AxisIndex];
    g_trust_motion_active_command_kind[AxisIndex] := g_trust_motion_queue0_command_kind[AxisIndex];
    g_trust_motion_active_buffer_mode[AxisIndex] := g_trust_motion_queue0_buffer_mode[AxisIndex];
    g_trust_motion_queue0_command_id[AxisIndex] := g_trust_motion_queue1_command_id[AxisIndex];
    g_trust_motion_queue0_command_kind[AxisIndex] := g_trust_motion_queue1_command_kind[AxisIndex];
    g_trust_motion_queue0_buffer_mode[AxisIndex] := g_trust_motion_queue1_buffer_mode[AxisIndex];
    g_trust_motion_queue1_command_id[AxisIndex] := UINT#0;
    g_trust_motion_queue1_command_kind[AxisIndex] := TRUST_MOTION_COMMAND_KIND#trustMotionNone;
    g_trust_motion_queue1_buffer_mode[AxisIndex] := MC_BUFFER_MODE#mcAborting;
    g_trust_motion_queue_depth[AxisIndex] := g_trust_motion_queue_depth[AxisIndex] - UINT#1;
    TRUST_MOTION_PromoteQueuedCommand := TRUE;
END_IF;
END_FUNCTION

FUNCTION TRUST_MOTION_FlushQueuedCommands : BOOL
VAR_INPUT
    AxisIndex : UINT;
END_VAR
TRUST_MOTION_FlushQueuedCommands := FALSE;
IF g_trust_motion_queue_depth[AxisIndex] >= UINT#1 THEN
    TRUST_MOTION_RecordAbortedCommand(AxisIndex := AxisIndex, CommandID := g_trust_motion_queue0_command_id[AxisIndex]);
    g_trust_motion_queue0_command_id[AxisIndex] := UINT#0;
    g_trust_motion_queue0_command_kind[AxisIndex] := TRUST_MOTION_COMMAND_KIND#trustMotionNone;
    g_trust_motion_queue0_buffer_mode[AxisIndex] := MC_BUFFER_MODE#mcAborting;
    TRUST_MOTION_FlushQueuedCommands := TRUE;
END_IF;
IF g_trust_motion_queue_depth[AxisIndex] >= UINT#2 THEN
    TRUST_MOTION_RecordAbortedCommand(AxisIndex := AxisIndex, CommandID := g_trust_motion_queue1_command_id[AxisIndex]);
    g_trust_motion_queue1_command_id[AxisIndex] := UINT#0;
    g_trust_motion_queue1_command_kind[AxisIndex] := TRUST_MOTION_COMMAND_KIND#trustMotionNone;
    g_trust_motion_queue1_buffer_mode[AxisIndex] := MC_BUFFER_MODE#mcAborting;
    TRUST_MOTION_FlushQueuedCommands := TRUE;
END_IF;
g_trust_motion_queue_depth[AxisIndex] := UINT#0;
END_FUNCTION

FUNCTION TRUST_MOTION_ClearActiveCommandSlot : BOOL
VAR_INPUT
    AxisIndex : UINT;
END_VAR
TRUST_MOTION_ClearActiveCommandSlot := TRUE;
g_trust_motion_active_command_id[AxisIndex] := UINT#0;
g_trust_motion_active_command_kind[AxisIndex] := TRUST_MOTION_COMMAND_KIND#trustMotionNone;
g_trust_motion_active_buffer_mode[AxisIndex] := MC_BUFFER_MODE#mcAborting;
END_FUNCTION

FUNCTION TRUST_MOTION_EnterErrorStopAndFlushQueue : BOOL
VAR_INPUT
    AxisIndex : UINT;
END_VAR
g_trust_motion_state[AxisIndex] := MC_AXIS_STATUS#mcErrorStop;
TRUST_MOTION_ClearActiveCommandSlot(AxisIndex := AxisIndex);
TRUST_MOTION_FlushQueuedCommands(AxisIndex := AxisIndex);
TRUST_MOTION_EnterErrorStopAndFlushQueue := TRUE;
END_FUNCTION

FUNCTION TRUST_MOTION_SupportsSingleAxisBufferMode : BOOL
VAR_INPUT
    BufferMode : MC_BUFFER_MODE;
END_VAR
TRUST_MOTION_SupportsSingleAxisBufferMode :=
    (BufferMode = MC_BUFFER_MODE#mcAborting) OR
    (BufferMode = MC_BUFFER_MODE#mcBuffered);
END_FUNCTION

FUNCTION_BLOCK TRUST_MOTION_RequestBufferedCommand
VAR_INPUT
    AxisIndex : UINT;
    Kind : TRUST_MOTION_COMMAND_KIND;
    BufferMode : MC_BUFFER_MODE;
END_VAR
VAR_OUTPUT
    Accepted : BOOL;
    ActiveNow : BOOL;
    Error : BOOL;
    ErrorID : WORD;
    CommandID : UINT;
END_VAR
IF NOT TRUST_MOTION_SupportsSingleAxisBufferMode(BufferMode := BufferMode) THEN
    Error := TRUE;
    ErrorID := WORD#16#0500;
ELSIF BufferMode = MC_BUFFER_MODE#mcAborting THEN
    IF g_trust_motion_active_command_id[AxisIndex] <> UINT#0 THEN
        TRUST_MOTION_RecordAbortedCommand(AxisIndex := AxisIndex, CommandID := g_trust_motion_active_command_id[AxisIndex]);
    END_IF;
    TRUST_MOTION_FlushQueuedCommands(AxisIndex := AxisIndex);
    g_trust_motion_active_command_id[AxisIndex] := UINT#0;
    g_trust_motion_active_command_kind[AxisIndex] := TRUST_MOTION_COMMAND_KIND#trustMotionNone;
    g_trust_motion_active_buffer_mode[AxisIndex] := MC_BUFFER_MODE#mcAborting;
    CommandID := TRUST_MOTION_NextCommandId(AxisIndex := AxisIndex);
    g_trust_motion_active_command_id[AxisIndex] := CommandID;
    g_trust_motion_active_command_kind[AxisIndex] := Kind;
    g_trust_motion_active_buffer_mode[AxisIndex] := BufferMode;
    Accepted := TRUE;
    ActiveNow := TRUE;
ELSIF (g_trust_motion_active_command_id[AxisIndex] = UINT#0) AND (g_trust_motion_queue_depth[AxisIndex] = UINT#0) THEN
    CommandID := TRUST_MOTION_NextCommandId(AxisIndex := AxisIndex);
    g_trust_motion_active_command_id[AxisIndex] := CommandID;
    g_trust_motion_active_command_kind[AxisIndex] := Kind;
    g_trust_motion_active_buffer_mode[AxisIndex] := BufferMode;
    Accepted := TRUE;
    ActiveNow := TRUE;
ELSIF g_trust_motion_queue_depth[AxisIndex] >= UINT#2 THEN
    Error := TRUE;
    ErrorID := WORD#16#0400;
ELSE
    CommandID := TRUST_MOTION_NextCommandId(AxisIndex := AxisIndex);
    IF g_trust_motion_queue_depth[AxisIndex] = UINT#0 THEN
        g_trust_motion_queue0_command_id[AxisIndex] := CommandID;
        g_trust_motion_queue0_command_kind[AxisIndex] := Kind;
        g_trust_motion_queue0_buffer_mode[AxisIndex] := BufferMode;
    ELSE
        g_trust_motion_queue1_command_id[AxisIndex] := CommandID;
        g_trust_motion_queue1_command_kind[AxisIndex] := Kind;
        g_trust_motion_queue1_buffer_mode[AxisIndex] := BufferMode;
    END_IF;
    g_trust_motion_queue_depth[AxisIndex] := g_trust_motion_queue_depth[AxisIndex] + UINT#1;
    Accepted := TRUE;
    ActiveNow := FALSE;
END_IF;
END_FUNCTION_BLOCK

FUNCTION TRUST_MOTION_ApplyNumericParameterWrite : BOOL
VAR_INPUT
    AxisIndex : UINT;
    ParameterNumber : INT;
    Value : REAL;
END_VAR
TRUST_MOTION_ApplyNumericParameterWrite := TRUE;
IF ParameterNumber = INT#1 THEN
    g_trust_motion_commanded_position[AxisIndex] := Value;
ELSIF ParameterNumber = INT#2 THEN
    g_trust_motion_sw_limit_pos[AxisIndex] := Value;
ELSIF ParameterNumber = INT#3 THEN
    g_trust_motion_sw_limit_neg[AxisIndex] := Value;
ELSIF ParameterNumber = INT#7 THEN
    g_trust_motion_max_position_lag[AxisIndex] := Value;
ELSIF ParameterNumber = INT#8 THEN
    g_trust_motion_max_velocity_system[AxisIndex] := Value;
ELSIF ParameterNumber = INT#9 THEN
    g_trust_motion_max_velocity_appl[AxisIndex] := Value;
ELSIF ParameterNumber = INT#10 THEN
    g_trust_motion_actual_velocity[AxisIndex] := Value;
ELSIF ParameterNumber = INT#11 THEN
    g_trust_motion_commanded_velocity[AxisIndex] := Value;
ELSIF ParameterNumber = INT#12 THEN
    g_trust_motion_max_acceleration_system[AxisIndex] := Value;
ELSIF ParameterNumber = INT#13 THEN
    g_trust_motion_max_acceleration_appl[AxisIndex] := Value;
ELSIF ParameterNumber = INT#14 THEN
    g_trust_motion_max_deceleration_system[AxisIndex] := Value;
ELSIF ParameterNumber = INT#15 THEN
    g_trust_motion_max_deceleration_appl[AxisIndex] := Value;
ELSIF ParameterNumber = INT#16 THEN
    g_trust_motion_max_jerk_system[AxisIndex] := Value;
ELSIF ParameterNumber = INT#17 THEN
    g_trust_motion_max_jerk_appl[AxisIndex] := Value;
ELSE
    TRUST_MOTION_ApplyNumericParameterWrite := FALSE;
END_IF;
END_FUNCTION

FUNCTION TRUST_MOTION_ApplyBoolParameterWrite : BOOL
VAR_INPUT
    AxisIndex : UINT;
    ParameterNumber : INT;
    Value : BOOL;
END_VAR
TRUST_MOTION_ApplyBoolParameterWrite := TRUE;
IF ParameterNumber = INT#4 THEN
    g_trust_motion_enable_limit_pos[AxisIndex] := Value;
ELSIF ParameterNumber = INT#5 THEN
    g_trust_motion_enable_limit_neg[AxisIndex] := Value;
ELSIF ParameterNumber = INT#6 THEN
    g_trust_motion_enable_pos_lag_monitoring[AxisIndex] := Value;
ELSIF ParameterNumber = INT#1000 THEN
    g_trust_motion_vendor_bool_1000[AxisIndex] := Value;
ELSE
    TRUST_MOTION_ApplyBoolParameterWrite := FALSE;
END_IF;
END_FUNCTION

FUNCTION TRUST_MOTION_ResolveDynamicInput : REAL
VAR_INPUT
    RequestedValue : REAL;
    MaximumValue : REAL;
END_VAR
IF RequestedValue = REAL#0.0 THEN
    TRUST_MOTION_ResolveDynamicInput := MaximumValue;
ELSE
    TRUST_MOTION_ResolveDynamicInput := RequestedValue;
END_IF;
END_FUNCTION

FUNCTION TRUST_MOTION_ClampPositionToSoftwareLimits : REAL
VAR_INPUT
    AxisIndex : UINT;
    RequestedPosition : REAL;
END_VAR
TRUST_MOTION_ClampPositionToSoftwareLimits := RequestedPosition;
IF g_trust_motion_enable_limit_pos[AxisIndex] AND (TRUST_MOTION_ClampPositionToSoftwareLimits > g_trust_motion_sw_limit_pos[AxisIndex]) THEN
    TRUST_MOTION_ClampPositionToSoftwareLimits := g_trust_motion_sw_limit_pos[AxisIndex];
END_IF;
IF g_trust_motion_enable_limit_neg[AxisIndex] AND (TRUST_MOTION_ClampPositionToSoftwareLimits < g_trust_motion_sw_limit_neg[AxisIndex]) THEN
    TRUST_MOTION_ClampPositionToSoftwareLimits := g_trust_motion_sw_limit_neg[AxisIndex];
END_IF;
END_FUNCTION


FUNCTION_BLOCK TRUST_MOTION_AxisContext
VAR_IN_OUT
    Axis : AXIS_REF;
END_VAR
VAR_OUTPUT
    AxisIndex : UINT;
END_VAR
AxisIndex := TRUST_MOTION_AxisIndex(Axis := Axis);
IF NOT g_trust_motion_axis_initialized[AxisIndex] THEN
    g_trust_motion_axis_initialized[AxisIndex] := TRUE;
    g_trust_motion_state[AxisIndex] := MC_AXIS_STATUS#mcDisabled;
    g_trust_motion_grouped[AxisIndex] := FALSE;
    g_trust_motion_power_command[AxisIndex] := FALSE;
    g_trust_motion_power_stage[AxisIndex] := FALSE;
    g_trust_motion_axis_error[AxisIndex] := WORD#16#0000;
    g_trust_motion_actual_position[AxisIndex] := REAL#0.0;
    g_trust_motion_set_position[AxisIndex] := REAL#0.0;
    g_trust_motion_commanded_position[AxisIndex] := REAL#0.0;
    g_trust_motion_actual_velocity[AxisIndex] := REAL#0.0;
    g_trust_motion_set_velocity[AxisIndex] := REAL#0.0;
    g_trust_motion_commanded_velocity[AxisIndex] := REAL#0.0;
    g_trust_motion_actual_torque[AxisIndex] := REAL#0.0;
    g_trust_motion_home_abs_switch[AxisIndex] := FALSE;
    g_trust_motion_limit_switch_pos[AxisIndex] := FALSE;
    g_trust_motion_limit_switch_neg[AxisIndex] := FALSE;
    g_trust_motion_simulation[AxisIndex] := FALSE;
    g_trust_motion_communication_ready[AxisIndex] := TRUE;
    g_trust_motion_ready_for_power_on[AxisIndex] := TRUE;
    g_trust_motion_is_homed[AxisIndex] := FALSE;
    g_trust_motion_axis_warning[AxisIndex] := FALSE;
    g_trust_motion_constant_velocity[AxisIndex] := FALSE;
    g_trust_motion_accelerating[AxisIndex] := FALSE;
    g_trust_motion_decelerating[AxisIndex] := FALSE;
    g_trust_motion_resolved_acceleration[AxisIndex] := REAL#0.0;
    g_trust_motion_resolved_deceleration[AxisIndex] := REAL#0.0;
    g_trust_motion_resolved_jerk[AxisIndex] := REAL#0.0;
    g_trust_motion_next_command_id[AxisIndex] := UINT#0;
    g_trust_motion_active_command_id[AxisIndex] := UINT#0;
    g_trust_motion_active_command_kind[AxisIndex] := TRUST_MOTION_COMMAND_KIND#trustMotionNone;
    g_trust_motion_active_buffer_mode[AxisIndex] := MC_BUFFER_MODE#mcAborting;
    g_trust_motion_queue_depth[AxisIndex] := UINT#0;
    g_trust_motion_queue0_command_id[AxisIndex] := UINT#0;
    g_trust_motion_queue0_command_kind[AxisIndex] := TRUST_MOTION_COMMAND_KIND#trustMotionNone;
    g_trust_motion_queue0_buffer_mode[AxisIndex] := MC_BUFFER_MODE#mcAborting;
    g_trust_motion_queue1_command_id[AxisIndex] := UINT#0;
    g_trust_motion_queue1_command_kind[AxisIndex] := TRUST_MOTION_COMMAND_KIND#trustMotionNone;
    g_trust_motion_queue1_buffer_mode[AxisIndex] := MC_BUFFER_MODE#mcAborting;
    g_trust_motion_aborted0_command_id[AxisIndex] := UINT#0;
    g_trust_motion_aborted1_command_id[AxisIndex] := UINT#0;
    g_trust_motion_aborted2_command_id[AxisIndex] := UINT#0;
    g_trust_motion_vel_factor[AxisIndex] := REAL#1.0;
    g_trust_motion_acc_factor[AxisIndex] := REAL#1.0;
    g_trust_motion_jerk_factor[AxisIndex] := REAL#1.0;
    g_trust_motion_sw_limit_pos[AxisIndex] := REAL#1000.0;
    g_trust_motion_sw_limit_neg[AxisIndex] := REAL#0.0 - REAL#1000.0;
    g_trust_motion_enable_limit_pos[AxisIndex] := FALSE;
    g_trust_motion_enable_limit_neg[AxisIndex] := FALSE;
    g_trust_motion_enable_pos_lag_monitoring[AxisIndex] := FALSE;
    g_trust_motion_max_position_lag[AxisIndex] := REAL#100.0;
    g_trust_motion_max_velocity_system[AxisIndex] := REAL#100.0;
    g_trust_motion_max_velocity_appl[AxisIndex] := REAL#100.0;
    g_trust_motion_max_acceleration_system[AxisIndex] := REAL#50.0;
    g_trust_motion_max_acceleration_appl[AxisIndex] := REAL#50.0;
    g_trust_motion_max_deceleration_system[AxisIndex] := REAL#50.0;
    g_trust_motion_max_deceleration_appl[AxisIndex] := REAL#50.0;
    g_trust_motion_max_jerk_system[AxisIndex] := REAL#25.0;
    g_trust_motion_max_jerk_appl[AxisIndex] := REAL#25.0;
    g_trust_motion_vendor_bool_1000[AxisIndex] := FALSE;
END_IF;
END_FUNCTION_BLOCK

FUNCTION_BLOCK TRUST_MOTION_TestAxisSeed
VAR_IN_OUT
    Axis : AXIS_REF;
END_VAR
VAR_INPUT
    ResetDefaults : BOOL;
    ApplyState : BOOL;
    State : MC_AXIS_STATUS;
    ApplyGrouped : BOOL;
    Grouped : BOOL;
    ApplyPowerStage : BOOL;
    PowerStage : BOOL;
    ApplyAxisError : BOOL;
    AxisErrorID : WORD;
    ApplyActuals : BOOL;
    ActualPosition : REAL;
    SetPosition : REAL;
    CommandedPosition : REAL;
    ActualVelocity : REAL;
    SetVelocity : REAL;
    CommandedVelocity : REAL;
    ActualTorque : REAL;
    ApplyAxisInfo : BOOL;
    HomeAbsSwitch : BOOL;
    LimitSwitchPos : BOOL;
    LimitSwitchNeg : BOOL;
    Simulation : BOOL;
    CommunicationReady : BOOL;
    ReadyForPowerOn : BOOL;
    IsHomed : BOOL;
    AxisWarning : BOOL;
    ApplyMotionFlags : BOOL;
    ConstantVelocity : BOOL;
    Accelerating : BOOL;
    Decelerating : BOOL;
END_VAR
VAR
    Context : TRUST_MOTION_AxisContext;
    AxisIndex : UINT;
END_VAR
Context(Axis := Axis);
AxisIndex := Context.AxisIndex;
IF ResetDefaults THEN
    g_trust_motion_axis_initialized[AxisIndex] := FALSE;
    Context(Axis := Axis);
    AxisIndex := Context.AxisIndex;
END_IF;
IF ApplyState THEN
    g_trust_motion_state[AxisIndex] := State;
END_IF;
IF ApplyGrouped THEN
    g_trust_motion_grouped[AxisIndex] := Grouped;
END_IF;
IF ApplyPowerStage THEN
    g_trust_motion_power_stage[AxisIndex] := PowerStage;
END_IF;
IF ApplyAxisError THEN
    g_trust_motion_axis_error[AxisIndex] := AxisErrorID;
END_IF;
IF ApplyActuals THEN
    g_trust_motion_actual_position[AxisIndex] := ActualPosition;
    g_trust_motion_set_position[AxisIndex] := SetPosition;
    g_trust_motion_commanded_position[AxisIndex] := CommandedPosition;
    g_trust_motion_actual_velocity[AxisIndex] := ActualVelocity;
    g_trust_motion_set_velocity[AxisIndex] := SetVelocity;
    g_trust_motion_commanded_velocity[AxisIndex] := CommandedVelocity;
    g_trust_motion_actual_torque[AxisIndex] := ActualTorque;
END_IF;
IF ApplyAxisInfo THEN
    g_trust_motion_home_abs_switch[AxisIndex] := HomeAbsSwitch;
    g_trust_motion_limit_switch_pos[AxisIndex] := LimitSwitchPos;
    g_trust_motion_limit_switch_neg[AxisIndex] := LimitSwitchNeg;
    g_trust_motion_simulation[AxisIndex] := Simulation;
    g_trust_motion_communication_ready[AxisIndex] := CommunicationReady;
    g_trust_motion_ready_for_power_on[AxisIndex] := ReadyForPowerOn;
    g_trust_motion_is_homed[AxisIndex] := IsHomed;
    g_trust_motion_axis_warning[AxisIndex] := AxisWarning;
END_IF;
IF ApplyMotionFlags THEN
    g_trust_motion_constant_velocity[AxisIndex] := ConstantVelocity;
    g_trust_motion_accelerating[AxisIndex] := Accelerating;
    g_trust_motion_decelerating[AxisIndex] := Decelerating;
END_IF;
END_FUNCTION_BLOCK

FUNCTION_BLOCK TRUST_MOTION_TestAxisProbe
VAR_IN_OUT
    Axis : AXIS_REF;
END_VAR
VAR_OUTPUT
    ResolvedAcceleration : REAL;
    ResolvedDeceleration : REAL;
    ResolvedJerk : REAL;
END_VAR
VAR
    Context : TRUST_MOTION_AxisContext;
END_VAR
Context(Axis := Axis);
ResolvedAcceleration := g_trust_motion_resolved_acceleration[Context.AxisIndex];
ResolvedDeceleration := g_trust_motion_resolved_deceleration[Context.AxisIndex];
ResolvedJerk := g_trust_motion_resolved_jerk[Context.AxisIndex];
END_FUNCTION_BLOCK

FUNCTION_BLOCK TRUST_MOTION_TestAxisQueueProbe
VAR_IN_OUT
    Axis : AXIS_REF;
END_VAR
VAR_OUTPUT
    ActiveCommandID : UINT;
    QueueDepth : UINT;
    Queue0CommandID : UINT;
    Queue1CommandID : UINT;
    ActiveBufferMode : MC_BUFFER_MODE;
    Queue0BufferMode : MC_BUFFER_MODE;
    Queue1BufferMode : MC_BUFFER_MODE;
END_VAR
VAR
    Context : TRUST_MOTION_AxisContext;
END_VAR
Context(Axis := Axis);
ActiveCommandID := g_trust_motion_active_command_id[Context.AxisIndex];
QueueDepth := g_trust_motion_queue_depth[Context.AxisIndex];
Queue0CommandID := g_trust_motion_queue0_command_id[Context.AxisIndex];
Queue1CommandID := g_trust_motion_queue1_command_id[Context.AxisIndex];
ActiveBufferMode := g_trust_motion_active_buffer_mode[Context.AxisIndex];
Queue0BufferMode := g_trust_motion_queue0_buffer_mode[Context.AxisIndex];
Queue1BufferMode := g_trust_motion_queue1_buffer_mode[Context.AxisIndex];
END_FUNCTION_BLOCK
