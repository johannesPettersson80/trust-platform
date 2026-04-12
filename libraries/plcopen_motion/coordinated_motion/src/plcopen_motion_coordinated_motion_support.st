FUNCTION TRUST_GROUP_GroupIndex : UINT
VAR_INPUT
    Group : AXES_GROUP_REF;
END_VAR
TRUST_GROUP_GroupIndex := Group.InternalIndex;
END_FUNCTION

FUNCTION TRUST_GROUP_CoordOffset : REAL
VAR_INPUT
    CoordSystem : MC_COORD_SYSTEM;
END_VAR
CASE CoordSystem OF
    MC_COORD_SYSTEM#mcACS:
        TRUST_GROUP_CoordOffset := REAL#0.0;
    MC_COORD_SYSTEM#mcMCS:
        TRUST_GROUP_CoordOffset := REAL#100.0;
    MC_COORD_SYSTEM#mcWCS:
        TRUST_GROUP_CoordOffset := REAL#200.0;
    MC_COORD_SYSTEM#mcPCS:
        TRUST_GROUP_CoordOffset := REAL#300.0;
    MC_COORD_SYSTEM#mcFCS:
        TRUST_GROUP_CoordOffset := REAL#400.0;
    MC_COORD_SYSTEM#mcTCS:
        TRUST_GROUP_CoordOffset := REAL#500.0;
ELSE
    TRUST_GROUP_CoordOffset := REAL#0.0;
END_CASE;
END_FUNCTION

FUNCTION TRUST_GROUP_IdentMatches : BOOL
VAR_INPUT
    Left : IDENT_IN_GROUP_REF;
    Right : IDENT_IN_GROUP_REF;
END_VAR
VAR
    Forward : INT;
    Reverse : INT;
END_VAR
Forward := FIND(IN1 := Left, IN2 := Right);
Reverse := FIND(IN1 := Right, IN2 := Left);
TRUST_GROUP_IdentMatches := (Forward = INT#1) AND (Reverse = INT#1);
END_FUNCTION

FUNCTION TRUST_GROUP_ClearSWLimit : BOOL
VAR_IN_OUT
    Limit : MC_SWLIMIT;
END_VAR
Limit.SWLimitPos := REAL#0.0;
Limit.SWLimitNeg := REAL#0.0;
TRUST_GROUP_ClearSWLimit := TRUE;
END_FUNCTION

FUNCTION TRUST_GROUP_ResetGroup : BOOL
VAR_INPUT
    GroupIndex : UINT;
END_VAR
VAR
    Slot : INT;
END_VAR
g_trust_group_initialized[GroupIndex] := TRUE;
g_trust_group_status[GroupIndex] := MC_GROUP_STATUS#mcGroupDisabled;
g_trust_group_member_count[GroupIndex] := UINT#0;
FOR Slot := INT#0 TO INT#3 DO
    g_trust_group_member_axis_id[GroupIndex, Slot] := UDINT#0;
    g_trust_group_member_internal_index[GroupIndex, Slot] := UINT#0;
    g_trust_group_member_ident[GroupIndex, Slot] := '';
    TRUST_GROUP_ClearSWLimit(Limit := g_trust_group_swlimits[GroupIndex, Slot + INT#1]);
    IF Slot <= INT#1 THEN
        g_trust_group_cartesian_trans_x[GroupIndex, Slot] := REAL#0.0;
        g_trust_group_cartesian_trans_y[GroupIndex, Slot] := REAL#0.0;
        g_trust_group_cartesian_trans_z[GroupIndex, Slot] := REAL#0.0;
        g_trust_group_cartesian_rot_1[GroupIndex, Slot] := REAL#0.0;
        g_trust_group_cartesian_rot_2[GroupIndex, Slot] := REAL#0.0;
        g_trust_group_cartesian_rot_3[GroupIndex, Slot] := REAL#0.0;
        g_trust_group_coord_trans_x[GroupIndex, Slot] := REAL#0.0;
        g_trust_group_coord_trans_y[GroupIndex, Slot] := REAL#0.0;
        g_trust_group_coord_trans_z[GroupIndex, Slot] := REAL#0.0;
        g_trust_group_coord_rot_x[GroupIndex, Slot] := REAL#0.0;
        g_trust_group_coord_rot_y[GroupIndex, Slot] := REAL#0.0;
        g_trust_group_coord_rot_z[GroupIndex, Slot] := REAL#0.0;
    END_IF;
END_FOR;
g_trust_group_commanded_x[GroupIndex] := REAL#0.0;
g_trust_group_commanded_y[GroupIndex] := REAL#0.0;
g_trust_group_commanded_z[GroupIndex] := REAL#0.0;
g_trust_group_commanded_rx[GroupIndex] := REAL#0.0;
g_trust_group_commanded_ry[GroupIndex] := REAL#0.0;
g_trust_group_commanded_rz[GroupIndex] := REAL#0.0;
g_trust_group_actual_x[GroupIndex] := REAL#0.0;
g_trust_group_actual_y[GroupIndex] := REAL#0.0;
g_trust_group_actual_z[GroupIndex] := REAL#0.0;
g_trust_group_actual_rx[GroupIndex] := REAL#0.0;
g_trust_group_actual_ry[GroupIndex] := REAL#0.0;
g_trust_group_actual_rz[GroupIndex] := REAL#0.0;
g_trust_group_commanded_path_velocity[GroupIndex] := REAL#0.0;
g_trust_group_actual_path_velocity[GroupIndex] := REAL#0.0;
g_trust_group_commanded_path_acceleration[GroupIndex] := REAL#0.0;
g_trust_group_actual_path_acceleration[GroupIndex] := REAL#0.0;
g_trust_group_active_coord_system[GroupIndex] := MC_COORD_SYSTEM#mcMCS;
g_trust_group_next_command_id[GroupIndex] := UINT#0;
g_trust_group_last_command_id[GroupIndex] := UINT#0;
g_trust_group_last_command_state[GroupIndex] := MC_COMMAND_STATE#mcAccepted;
g_trust_group_last_command_coord_system[GroupIndex] := MC_COORD_SYSTEM#mcMCS;
g_trust_group_transform_identity[GroupIndex] := TRUE;
g_trust_group_power_stage[GroupIndex] := FALSE;
g_trust_group_error_id[GroupIndex] := WORD#16#0000;
g_trust_group_tracking[GroupIndex] := FALSE;
g_trust_group_in_sync[GroupIndex] := TRUE;
g_trust_group_in_position[GroupIndex] := TRUE;
g_trust_group_constant_velocity[GroupIndex] := TRUE;
g_trust_group_accelerating[GroupIndex] := FALSE;
g_trust_group_decelerating[GroupIndex] := FALSE;
g_trust_group_dynamics_mode[GroupIndex] := MC_DYNAMICS_MODE#mcAbsolute;
g_trust_group_transition_reference[GroupIndex] := MC_TRANSITION_REFERENCE#mcStartPoint;
g_trust_group_kin_transform[GroupIndex] := UINT#0;
g_trust_group_reference_velocity[GroupIndex] := REAL#100.0;
g_trust_group_reference_acceleration[GroupIndex] := REAL#50.0;
g_trust_group_reference_deceleration[GroupIndex] := REAL#50.0;
g_trust_group_reference_jerk[GroupIndex] := REAL#25.0;
g_trust_group_default_velocity[GroupIndex] := REAL#10.0;
g_trust_group_default_acceleration[GroupIndex] := REAL#5.0;
g_trust_group_default_deceleration[GroupIndex] := REAL#5.0;
g_trust_group_default_jerk[GroupIndex] := REAL#2.5;
g_trust_group_override_vel_factor[GroupIndex] := REAL#1.0;
g_trust_group_override_acc_factor[GroupIndex] := REAL#1.0;
g_trust_group_override_jerk_factor[GroupIndex] := REAL#1.0;
g_trust_group_stop_hold[GroupIndex] := FALSE;
g_trust_group_command_elapsed_duration[GroupIndex] := TIME#0ms;
g_trust_group_command_remaining_duration[GroupIndex] := TIME#0ms;
g_trust_group_command_remaining_distance[GroupIndex] := REAL#0.0;
g_trust_group_command_progress[GroupIndex] := REAL#0.0;
g_trust_group_command_info_id[GroupIndex] := WORD#16#0000;
g_trust_group_command_warning_id[GroupIndex] := WORD#16#0000;
TRUST_GROUP_ResetGroup := TRUE;
END_FUNCTION

FUNCTION TRUST_GROUP_GroupAllowsMembershipEdit : BOOL
VAR_INPUT
    GroupIndex : UINT;
END_VAR
TRUST_GROUP_GroupAllowsMembershipEdit :=
    (g_trust_group_status[GroupIndex] = MC_GROUP_STATUS#mcGroupDisabled) OR
    (g_trust_group_status[GroupIndex] = MC_GROUP_STATUS#mcGroupStandby) OR
    (g_trust_group_status[GroupIndex] = MC_GROUP_STATUS#mcGroupErrorStop);
END_FUNCTION

FUNCTION TRUST_GROUP_FindMemberSlotByIdent : INT
VAR_INPUT
    Group : AXES_GROUP_REF;
    IdentInGroup : IDENT_IN_GROUP_REF;
END_VAR
VAR
    GroupIndex : UINT;
    Slot : INT;
END_VAR
GroupIndex := TRUST_GROUP_GroupIndex(Group := Group);
TRUST_GROUP_FindMemberSlotByIdent := INT#-1;
IF g_trust_group_member_count[GroupIndex] > UINT#0 THEN
    FOR Slot := INT#0 TO TO_INT(g_trust_group_member_count[GroupIndex]) - INT#1 DO
        IF TRUST_GROUP_IdentMatches(Left := g_trust_group_member_ident[GroupIndex, Slot], Right := IdentInGroup) THEN
            TRUST_GROUP_FindMemberSlotByIdent := Slot;
        END_IF;
    END_FOR;
END_IF;
END_FUNCTION

FUNCTION TRUST_GROUP_FindMemberSlotByAxis : INT
VAR_INPUT
    Axis : AXIS_REF;
END_VAR
VAR
    GroupIndex : INT;
    Slot : INT;
END_VAR
TRUST_GROUP_FindMemberSlotByAxis := INT#-1;
FOR GroupIndex := INT#0 TO INT#7 DO
    IF g_trust_group_member_count[TO_UINT(GroupIndex)] > UINT#0 THEN
        FOR Slot := INT#0 TO TO_INT(g_trust_group_member_count[TO_UINT(GroupIndex)]) - INT#1 DO
            IF g_trust_group_member_internal_index[GroupIndex, Slot] = Axis.InternalIndex THEN
                TRUST_GROUP_FindMemberSlotByAxis := GroupIndex * INT#10 + Slot;
            END_IF;
        END_FOR;
    END_IF;
END_FOR;
END_FUNCTION

FUNCTION TRUST_GROUP_AddMember : BOOL
VAR_INPUT
    Group : AXES_GROUP_REF;
    Axis : AXIS_REF;
    IdentInGroup : IDENT_IN_GROUP_REF;
END_VAR
VAR
    GroupIndex : UINT;
    Slot : UINT;
END_VAR
GroupIndex := TRUST_GROUP_GroupIndex(Group := Group);
IF NOT g_trust_group_initialized[GroupIndex] THEN
    TRUST_GROUP_ResetGroup(GroupIndex := GroupIndex);
END_IF;
IF NOT TRUST_GROUP_GroupAllowsMembershipEdit(GroupIndex := GroupIndex) THEN
    TRUST_GROUP_AddMember := FALSE;
ELSIF TRUST_GROUP_FindMemberSlotByIdent(Group := Group, IdentInGroup := IdentInGroup) <> INT#-1 THEN
    TRUST_GROUP_AddMember := FALSE;
ELSIF TRUST_GROUP_FindMemberSlotByAxis(Axis := Axis) <> INT#-1 THEN
    TRUST_GROUP_AddMember := FALSE;
ELSE
    Slot := g_trust_group_member_count[GroupIndex];
    IF Slot > UINT#3 THEN
        TRUST_GROUP_AddMember := FALSE;
    ELSE
        g_trust_group_member_axis_id[GroupIndex, Slot] := Axis.AxisId;
        g_trust_group_member_internal_index[GroupIndex, Slot] := Axis.InternalIndex;
        g_trust_group_member_ident[GroupIndex, Slot] := IdentInGroup;
        g_trust_group_member_count[GroupIndex] := Slot + UINT#1;
        TRUST_GROUP_AddMember := TRUE;
    END_IF;
END_IF;
END_FUNCTION

FUNCTION TRUST_GROUP_RemoveMemberByIdent : BOOL
VAR_INPUT
    Group : AXES_GROUP_REF;
    IdentInGroup : IDENT_IN_GROUP_REF;
END_VAR
VAR
    GroupIndex : UINT;
    Slot : INT;
    ShiftSlot : INT;
END_VAR
GroupIndex := TRUST_GROUP_GroupIndex(Group := Group);
TRUST_GROUP_RemoveMemberByIdent := FALSE;
IF NOT g_trust_group_initialized[GroupIndex] THEN
    TRUST_GROUP_ResetGroup(GroupIndex := GroupIndex);
END_IF;
IF TRUST_GROUP_GroupAllowsMembershipEdit(GroupIndex := GroupIndex) THEN
    Slot := TRUST_GROUP_FindMemberSlotByIdent(Group := Group, IdentInGroup := IdentInGroup);
    IF Slot <> INT#-1 THEN
        FOR ShiftSlot := Slot TO INT#2 DO
            g_trust_group_member_axis_id[GroupIndex, ShiftSlot] := g_trust_group_member_axis_id[GroupIndex, ShiftSlot + INT#1];
            g_trust_group_member_internal_index[GroupIndex, ShiftSlot] := g_trust_group_member_internal_index[GroupIndex, ShiftSlot + INT#1];
            g_trust_group_member_ident[GroupIndex, ShiftSlot] := g_trust_group_member_ident[GroupIndex, ShiftSlot + INT#1];
        END_FOR;
        g_trust_group_member_axis_id[GroupIndex, 3] := UDINT#0;
        g_trust_group_member_internal_index[GroupIndex, 3] := UINT#0;
        g_trust_group_member_ident[GroupIndex, 3] := '';
        g_trust_group_member_count[GroupIndex] := g_trust_group_member_count[GroupIndex] - UINT#1;
        IF g_trust_group_member_count[GroupIndex] = UINT#0 THEN
            g_trust_group_status[GroupIndex] := MC_GROUP_STATUS#mcGroupDisabled;
        END_IF;
        TRUST_GROUP_RemoveMemberByIdent := TRUE;
    END_IF;
END_IF;
END_FUNCTION

FUNCTION TRUST_GROUP_RemoveAllMembers : BOOL
VAR_INPUT
    Group : AXES_GROUP_REF;
END_VAR
VAR
    GroupIndex : UINT;
    Slot : INT;
END_VAR
GroupIndex := TRUST_GROUP_GroupIndex(Group := Group);
IF NOT g_trust_group_initialized[GroupIndex] THEN
    TRUST_GROUP_ResetGroup(GroupIndex := GroupIndex);
END_IF;
IF NOT TRUST_GROUP_GroupAllowsMembershipEdit(GroupIndex := GroupIndex) THEN
    TRUST_GROUP_RemoveAllMembers := FALSE;
ELSE
    FOR Slot := INT#0 TO INT#3 DO
        g_trust_group_member_axis_id[GroupIndex, Slot] := UDINT#0;
        g_trust_group_member_internal_index[GroupIndex, Slot] := UINT#0;
        g_trust_group_member_ident[GroupIndex, Slot] := '';
    END_FOR;
    g_trust_group_member_count[GroupIndex] := UINT#0;
    g_trust_group_status[GroupIndex] := MC_GROUP_STATUS#mcGroupDisabled;
    TRUST_GROUP_RemoveAllMembers := TRUE;
END_IF;
END_FUNCTION

FUNCTION TRUST_GROUP_RequestCommand : MC_COMMAND_ID
VAR_INPUT
    Group : AXES_GROUP_REF;
    CoordSystem : MC_COORD_SYSTEM;
END_VAR
VAR
    GroupIndex : UINT;
END_VAR
GroupIndex := TRUST_GROUP_GroupIndex(Group := Group);
IF NOT g_trust_group_initialized[GroupIndex] THEN
    TRUST_GROUP_ResetGroup(GroupIndex := GroupIndex);
END_IF;
IF g_trust_group_member_count[GroupIndex] = UINT#0 THEN
    TRUST_GROUP_RequestCommand := UINT#0;
ELSE
    g_trust_group_next_command_id[GroupIndex] := g_trust_group_next_command_id[GroupIndex] + UINT#1;
    IF g_trust_group_next_command_id[GroupIndex] = UINT#0 THEN
        g_trust_group_next_command_id[GroupIndex] := UINT#1;
    END_IF;
    g_trust_group_last_command_id[GroupIndex] := g_trust_group_next_command_id[GroupIndex];
    g_trust_group_last_command_state[GroupIndex] := MC_COMMAND_STATE#mcAccepted;
    g_trust_group_last_command_coord_system[GroupIndex] := CoordSystem;
    g_trust_group_active_coord_system[GroupIndex] := CoordSystem;
    TRUST_GROUP_RequestCommand := g_trust_group_last_command_id[GroupIndex];
END_IF;
END_FUNCTION

FUNCTION TRUST_GROUP_GroupParameterToReal : REAL
VAR_INPUT
    Group : AXES_GROUP_REF;
    ParameterNumber : MC_GROUP_PARAMETER;
END_VAR
VAR
    GroupIndex : UINT;
END_VAR
GroupIndex := TRUST_GROUP_GroupIndex(Group := Group);
IF ParameterNumber = MC_GROUP_PARAMETER#mcDynamicsMode THEN
    IF g_trust_group_dynamics_mode[GroupIndex] = MC_DYNAMICS_MODE#mcAbsolute THEN
        TRUST_GROUP_GroupParameterToReal := REAL#0.0;
    ELSE
        TRUST_GROUP_GroupParameterToReal := REAL#1.0;
    END_IF;
ELSE
    IF g_trust_group_transition_reference[GroupIndex] = MC_TRANSITION_REFERENCE#mcStartPoint THEN
        TRUST_GROUP_GroupParameterToReal := REAL#0.0;
    ELSE
        TRUST_GROUP_GroupParameterToReal := REAL#1.0;
    END_IF;
END_IF;
END_FUNCTION

FUNCTION TRUST_GROUP_SetPowerAndError : BOOL
VAR_INPUT
    Group : AXES_GROUP_REF;
    PowerOn : BOOL;
    GroupErrorID : WORD;
END_VAR
VAR
    GroupIndex : UINT;
END_VAR
GroupIndex := TRUST_GROUP_GroupIndex(Group := Group);
g_trust_group_power_stage[GroupIndex] := PowerOn;
g_trust_group_error_id[GroupIndex] := GroupErrorID;
TRUST_GROUP_SetPowerAndError := TRUE;
END_FUNCTION

FUNCTION TRUST_GROUP_SetMotionFlags : BOOL
VAR_INPUT
    Group : AXES_GROUP_REF;
    Tracking : BOOL;
    InSync : BOOL;
    InPosition : BOOL;
    ConstantVelocity : BOOL;
    Accelerating : BOOL;
    Decelerating : BOOL;
END_VAR
VAR
    GroupIndex : UINT;
END_VAR
GroupIndex := TRUST_GROUP_GroupIndex(Group := Group);
g_trust_group_tracking[GroupIndex] := Tracking;
g_trust_group_in_sync[GroupIndex] := InSync;
g_trust_group_in_position[GroupIndex] := InPosition;
g_trust_group_constant_velocity[GroupIndex] := ConstantVelocity;
g_trust_group_accelerating[GroupIndex] := Accelerating;
g_trust_group_decelerating[GroupIndex] := Decelerating;
TRUST_GROUP_SetMotionFlags := TRUE;
END_FUNCTION

FUNCTION TRUST_GROUP_SetPathKinematics : BOOL
VAR_INPUT
    Group : AXES_GROUP_REF;
    CommandedPathVelocity : REAL;
    ActualPathVelocity : REAL;
    CommandedPathAcceleration : REAL;
    ActualPathAcceleration : REAL;
END_VAR
VAR
    GroupIndex : UINT;
END_VAR
GroupIndex := TRUST_GROUP_GroupIndex(Group := Group);
g_trust_group_commanded_path_velocity[GroupIndex] := CommandedPathVelocity;
g_trust_group_actual_path_velocity[GroupIndex] := ActualPathVelocity;
g_trust_group_commanded_path_acceleration[GroupIndex] := CommandedPathAcceleration;
g_trust_group_actual_path_acceleration[GroupIndex] := ActualPathAcceleration;
TRUST_GROUP_SetPathKinematics := TRUE;
END_FUNCTION

FUNCTION TRUST_GROUP_TransformSlot : INT
VAR_INPUT
    CoordSystem : MC_COORD_SYSTEM;
END_VAR
IF CoordSystem = MC_COORD_SYSTEM#mcMCS THEN
    TRUST_GROUP_TransformSlot := INT#0;
ELSIF CoordSystem = MC_COORD_SYSTEM#mcPCS THEN
    TRUST_GROUP_TransformSlot := INT#1;
ELSE
    TRUST_GROUP_TransformSlot := INT#-1;
END_IF;
END_FUNCTION

FUNCTION TRUST_GROUP_BufferModeSupported : BOOL
VAR_INPUT
    BufferMode : MC_BUFFER_MODE;
END_VAR
TRUST_GROUP_BufferModeSupported :=
    (BufferMode = MC_BUFFER_MODE#mcAborting) OR
    (BufferMode = MC_BUFFER_MODE#mcBuffered);
END_FUNCTION

FUNCTION TRUST_GROUP_TransitionVelocitySupported : BOOL
VAR_INPUT
    TransitionVelocity : MC_TRANSITION_VELOCITY;
END_VAR
TRUST_GROUP_TransitionVelocitySupported :=
    (TransitionVelocity = MC_TRANSITION_VELOCITY#mcTVZero) OR
    (TransitionVelocity = MC_TRANSITION_VELOCITY#mcTVNext);
END_FUNCTION

FUNCTION TRUST_GROUP_TransitionModeSupported : BOOL
VAR_INPUT
    TransitionMode : MC_TRANSITION_MODE;
END_VAR
TRUST_GROUP_TransitionModeSupported :=
    (TransitionMode = MC_TRANSITION_MODE#mcTMNone) OR
    (TransitionMode = MC_TRANSITION_MODE#mcTMCornerDistance);
END_FUNCTION

FUNCTION TRUST_GROUP_GroupReadyForMotion : BOOL
VAR_INPUT
    Group : AXES_GROUP_REF;
END_VAR
VAR
    GroupIndex : UINT;
END_VAR
GroupIndex := TRUST_GROUP_GroupIndex(Group := Group);
TRUST_GROUP_GroupReadyForMotion :=
    (g_trust_group_status[GroupIndex] <> MC_GROUP_STATUS#mcGroupDisabled) AND
    (g_trust_group_status[GroupIndex] <> MC_GROUP_STATUS#mcGroupErrorStop) AND
    (NOT g_trust_group_stop_hold[GroupIndex]);
END_FUNCTION

FUNCTION TRUST_GROUP_SetCommandInfo : BOOL
VAR_INPUT
    Group : AXES_GROUP_REF;
    CommandState : MC_COMMAND_STATE;
    ElapsedDuration : TIME;
    RemainingDuration : TIME;
    RemainingDistance : REAL;
    Progress : REAL;
    InfoID : WORD;
    WarningID : WORD;
END_VAR
VAR
    GroupIndex : UINT;
END_VAR
GroupIndex := TRUST_GROUP_GroupIndex(Group := Group);
g_trust_group_last_command_state[GroupIndex] := CommandState;
g_trust_group_command_elapsed_duration[GroupIndex] := ElapsedDuration;
g_trust_group_command_remaining_duration[GroupIndex] := RemainingDuration;
g_trust_group_command_remaining_distance[GroupIndex] := RemainingDistance;
g_trust_group_command_progress[GroupIndex] := Progress;
g_trust_group_command_info_id[GroupIndex] := InfoID;
g_trust_group_command_warning_id[GroupIndex] := WarningID;
TRUST_GROUP_SetCommandInfo := TRUE;
END_FUNCTION

FUNCTION TRUST_GROUP_StoreMotionPosition : BOOL
VAR_INPUT
    Group : AXES_GROUP_REF;
    CoordSystem : MC_COORD_SYSTEM;
    Coord : MC_COORD_REF;
    Relative : BOOL;
END_VAR
VAR
    GroupIndex : UINT;
    Offset : REAL;
END_VAR
GroupIndex := TRUST_GROUP_GroupIndex(Group := Group);
Offset := TRUST_GROUP_CoordOffset(CoordSystem := CoordSystem) - TRUST_GROUP_CoordOffset(CoordSystem := MC_COORD_SYSTEM#mcMCS);
IF Relative THEN
    g_trust_group_commanded_x[GroupIndex] := g_trust_group_commanded_x[GroupIndex] + Coord.X;
    g_trust_group_commanded_y[GroupIndex] := g_trust_group_commanded_y[GroupIndex] + Coord.Y;
    g_trust_group_commanded_z[GroupIndex] := g_trust_group_commanded_z[GroupIndex] + Coord.Z;
    g_trust_group_commanded_rx[GroupIndex] := g_trust_group_commanded_rx[GroupIndex] + Coord.RX;
    g_trust_group_commanded_ry[GroupIndex] := g_trust_group_commanded_ry[GroupIndex] + Coord.RY;
    g_trust_group_commanded_rz[GroupIndex] := g_trust_group_commanded_rz[GroupIndex] + Coord.RZ;
ELSE
    g_trust_group_commanded_x[GroupIndex] := Coord.X - Offset;
    g_trust_group_commanded_y[GroupIndex] := Coord.Y - Offset;
    g_trust_group_commanded_z[GroupIndex] := Coord.Z - Offset;
    g_trust_group_commanded_rx[GroupIndex] := Coord.RX - Offset;
    g_trust_group_commanded_ry[GroupIndex] := Coord.RY - Offset;
    g_trust_group_commanded_rz[GroupIndex] := Coord.RZ - Offset;
END_IF;
g_trust_group_actual_x[GroupIndex] := g_trust_group_commanded_x[GroupIndex];
g_trust_group_actual_y[GroupIndex] := g_trust_group_commanded_y[GroupIndex];
g_trust_group_actual_z[GroupIndex] := g_trust_group_commanded_z[GroupIndex];
g_trust_group_actual_rx[GroupIndex] := g_trust_group_commanded_rx[GroupIndex];
g_trust_group_actual_ry[GroupIndex] := g_trust_group_commanded_ry[GroupIndex];
g_trust_group_actual_rz[GroupIndex] := g_trust_group_commanded_rz[GroupIndex];
TRUST_GROUP_StoreMotionPosition := TRUE;
END_FUNCTION

FUNCTION TRUST_GROUP_StoreDynamics : BOOL
VAR_INPUT
    Group : AXES_GROUP_REF;
    Velocity : REAL;
    Acceleration : REAL;
    Deceleration : REAL;
    Jerk : REAL;
END_VAR
VAR
    GroupIndex : UINT;
END_VAR
GroupIndex := TRUST_GROUP_GroupIndex(Group := Group);
g_trust_group_commanded_path_velocity[GroupIndex] := Velocity;
g_trust_group_actual_path_velocity[GroupIndex] := Velocity * g_trust_group_override_vel_factor[GroupIndex];
g_trust_group_commanded_path_acceleration[GroupIndex] := Acceleration;
g_trust_group_actual_path_acceleration[GroupIndex] := Acceleration * g_trust_group_override_acc_factor[GroupIndex];
TRUST_GROUP_StoreDynamics := TRUE;
END_FUNCTION

FUNCTION TRUST_GROUP_MotionCoordSystemSupported : BOOL
VAR_INPUT
    CoordSystem : MC_COORD_SYSTEM;
END_VAR
TRUST_GROUP_MotionCoordSystemSupported :=
    (CoordSystem = MC_COORD_SYSTEM#mcACS) OR
    (CoordSystem = MC_COORD_SYSTEM#mcMCS) OR
    (CoordSystem = MC_COORD_SYSTEM#mcPCS);
END_FUNCTION

FUNCTION TRUST_GROUP_TransitionInfoID : WORD
VAR_INPUT
    Group : AXES_GROUP_REF;
END_VAR
VAR
    GroupIndex : UINT;
END_VAR
GroupIndex := TRUST_GROUP_GroupIndex(Group := Group);
IF g_trust_group_transition_reference[GroupIndex] = MC_TRANSITION_REFERENCE#mcEndPoint THEN
    TRUST_GROUP_TransitionInfoID := WORD#16#1001;
ELSE
    TRUST_GROUP_TransitionInfoID := WORD#16#1000;
END_IF;
END_FUNCTION

FUNCTION_BLOCK TRUST_GROUP_TestSeed
VAR_IN_OUT
    Group : AXES_GROUP_REF;
END_VAR
VAR_INPUT
    Status : MC_GROUP_STATUS;
    ActiveCoordSystem : MC_COORD_SYSTEM;
    IdentityTransform : BOOL;
    CommandedX : REAL;
    CommandedY : REAL;
    CommandedZ : REAL;
    CommandedRX : REAL;
    CommandedRY : REAL;
    CommandedRZ : REAL;
    ActualX : REAL;
    ActualY : REAL;
    ActualZ : REAL;
    ActualRX : REAL;
    ActualRY : REAL;
    ActualRZ : REAL;
END_VAR
VAR
    GroupIndex : UINT;
END_VAR
GroupIndex := TRUST_GROUP_GroupIndex(Group := Group);
TRUST_GROUP_ResetGroup(GroupIndex := GroupIndex);
g_trust_group_status[GroupIndex] := Status;
g_trust_group_active_coord_system[GroupIndex] := ActiveCoordSystem;
g_trust_group_transform_identity[GroupIndex] := IdentityTransform;
g_trust_group_commanded_x[GroupIndex] := CommandedX;
g_trust_group_commanded_y[GroupIndex] := CommandedY;
g_trust_group_commanded_z[GroupIndex] := CommandedZ;
g_trust_group_commanded_rx[GroupIndex] := CommandedRX;
g_trust_group_commanded_ry[GroupIndex] := CommandedRY;
g_trust_group_commanded_rz[GroupIndex] := CommandedRZ;
g_trust_group_actual_x[GroupIndex] := ActualX;
g_trust_group_actual_y[GroupIndex] := ActualY;
g_trust_group_actual_z[GroupIndex] := ActualZ;
g_trust_group_actual_rx[GroupIndex] := ActualRX;
g_trust_group_actual_ry[GroupIndex] := ActualRY;
g_trust_group_actual_rz[GroupIndex] := ActualRZ;
END_FUNCTION_BLOCK

FUNCTION_BLOCK TRUST_GROUP_TestProbe
VAR_INPUT
    Group : AXES_GROUP_REF;
END_VAR
VAR_OUTPUT
    MemberCount : UINT;
    Status : MC_GROUP_STATUS;
    ActiveCoordSystem : MC_COORD_SYSTEM;
    CommandedX : REAL;
    CommandedY : REAL;
    CommandedZ : REAL;
    ActualX : REAL;
    ActualY : REAL;
    ActualZ : REAL;
    LastAcceptedCommandID : MC_COMMAND_ID;
    LastAcceptedCommandState : MC_COMMAND_STATE;
    LastAcceptedCoordSystem : MC_COORD_SYSTEM;
    TransformIdentity : BOOL;
    PowerOn : BOOL;
    GroupErrorID : WORD;
    Tracking : BOOL;
    InSync : BOOL;
    InPosition : BOOL;
    ConstantVelocity : BOOL;
    Accelerating : BOOL;
    Decelerating : BOOL;
    DynamicsMode : MC_DYNAMICS_MODE;
    TransitionReference : MC_TRANSITION_REFERENCE;
    PathVelocity : REAL;
    PathAcceleration : REAL;
END_VAR
VAR
    GroupIndex : UINT;
END_VAR
GroupIndex := TRUST_GROUP_GroupIndex(Group := Group);
MemberCount := g_trust_group_member_count[GroupIndex];
Status := g_trust_group_status[GroupIndex];
ActiveCoordSystem := g_trust_group_active_coord_system[GroupIndex];
CommandedX := g_trust_group_commanded_x[GroupIndex];
CommandedY := g_trust_group_commanded_y[GroupIndex];
CommandedZ := g_trust_group_commanded_z[GroupIndex];
ActualX := g_trust_group_actual_x[GroupIndex];
ActualY := g_trust_group_actual_y[GroupIndex];
ActualZ := g_trust_group_actual_z[GroupIndex];
LastAcceptedCommandID := g_trust_group_last_command_id[GroupIndex];
LastAcceptedCommandState := g_trust_group_last_command_state[GroupIndex];
LastAcceptedCoordSystem := g_trust_group_last_command_coord_system[GroupIndex];
TransformIdentity := g_trust_group_transform_identity[GroupIndex];
PowerOn := g_trust_group_power_stage[GroupIndex];
GroupErrorID := g_trust_group_error_id[GroupIndex];
Tracking := g_trust_group_tracking[GroupIndex];
InSync := g_trust_group_in_sync[GroupIndex];
InPosition := g_trust_group_in_position[GroupIndex];
ConstantVelocity := g_trust_group_constant_velocity[GroupIndex];
Accelerating := g_trust_group_accelerating[GroupIndex];
Decelerating := g_trust_group_decelerating[GroupIndex];
DynamicsMode := g_trust_group_dynamics_mode[GroupIndex];
TransitionReference := g_trust_group_transition_reference[GroupIndex];
PathVelocity := g_trust_group_commanded_path_velocity[GroupIndex];
PathAcceleration := g_trust_group_commanded_path_acceleration[GroupIndex];
END_FUNCTION_BLOCK

FUNCTION_BLOCK TRUST_GROUP_TransformRoundTripProbe
VAR_INPUT
    Group : AXES_GROUP_REF;
    InputPosition : MC_COORD_REF;
END_VAR
VAR_OUTPUT
    IdentityApplied : BOOL;
    X : REAL;
    Y : REAL;
    Z : REAL;
    RX : REAL;
    RY : REAL;
    RZ : REAL;
END_VAR
VAR
    GroupIndex : UINT;
END_VAR
GroupIndex := TRUST_GROUP_GroupIndex(Group := Group);
IdentityApplied := g_trust_group_transform_identity[GroupIndex];
IF IdentityApplied THEN
    X := InputPosition.X;
    Y := InputPosition.Y;
    Z := InputPosition.Z;
    RX := InputPosition.RX;
    RY := InputPosition.RY;
    RZ := InputPosition.RZ;
ELSE
    X := REAL#0.0;
    Y := REAL#0.0;
    Z := REAL#0.0;
    RX := REAL#0.0;
    RY := REAL#0.0;
    RZ := REAL#0.0;
END_IF;
END_FUNCTION_BLOCK
