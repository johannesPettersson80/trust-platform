TYPE AXIS_REF :
STRUCT
    AxisId : UDINT;
    InternalIndex : UINT;
END_STRUCT
END_TYPE

TYPE MC_BUFFER_MODE : (
    mcAborting,
    mcBuffered,
    mcBlendingLow,
    mcBlendingPrevious,
    mcBlendingNext,
    mcBlendingHigh
);
END_TYPE

TYPE MC_EXECUTION_MODE : (
    mcImmediately,
    mcDelayed,
    mcQueued
);
END_TYPE

TYPE MC_SOURCE : (
    mcCommandedValue,
    mcSetValue,
    mcActualValue
);
END_TYPE

TYPE AXES_GROUP_REF :
STRUCT
    GroupId : UDINT;
    InternalIndex : UINT;
END_STRUCT
END_TYPE

TYPE AXIS_ID : UDINT;
END_TYPE

TYPE AXES_GROUP_ID : UDINT;
END_TYPE

TYPE IDENT_IN_GROUP_REF : STRING[63];
END_TYPE

TYPE MC_COMMAND_ID : UINT;
END_TYPE

TYPE MC_GROUP_PARAMETER : (
    mcDynamicsMode,
    mcTransitionReferencePoint
);
END_TYPE

TYPE MC_TRANSITION_PARAMETER : REAL;
END_TYPE

TYPE MC_KIN_REF : UINT;
END_TYPE

TYPE MC_COORD_SYSTEM : (
    mcACS,
    mcMCS,
    mcWCS,
    mcPCS,
    mcFCS,
    mcTCS
);
END_TYPE

TYPE MC_DYNAMICS_MODE : (
    mcAbsolute,
    mcPercentage
);
END_TYPE

TYPE MC_TRANSITION_MODE : (
    mcTMNone,
    mcTMStartVelocity,
    mcTMConstantVelocity,
    mcTMCornerDistance,
    mcTMMaxCornerDeviation
);
END_TYPE

TYPE MC_TRANSITION_VELOCITY : (
    mcTVZero,
    mcTVLow,
    mcTVPrevious,
    mcTVNext,
    mcTVHigh
);
END_TYPE

TYPE MC_TRANSITION_REFERENCE : (
    mcStartPoint,
    mcEndPoint
);
END_TYPE

TYPE MC_ORIENTATION_MODE : (
    mcLinear,
    mcJointInterpolated,
    mcFixed,
    mcPathBased
);
END_TYPE

TYPE MC_COMMAND_STATE : (
    mcAccepted,
    mcActive
);
END_TYPE

TYPE MC_GROUP_STATUS : (
    mcGroupErrorStop,
    mcGroupDisabled,
    mcGroupStandby,
    mcGroupHoming,
    mcGroupStopping,
    mcGroupMoving
);
END_TYPE

TYPE MC_CART_REF :
STRUCT
    X : REAL;
    Y : REAL;
    Z : REAL;
    RX : REAL;
    RY : REAL;
    RZ : REAL;
END_STRUCT
END_TYPE

TYPE MC_COORD_REF :
STRUCT
    X : REAL;
    Y : REAL;
    Z : REAL;
    RX : REAL;
    RY : REAL;
    RZ : REAL;
END_STRUCT
END_TYPE

TYPE MC_CONFIG_DATA :
STRUCT
    ConfigValid : BOOL;
    Shoulder : BOOL;
    Elbow : BOOL;
    Wrist : BOOL;
END_STRUCT
END_TYPE

TYPE MC_TURN_INFO :
STRUCT
    ATurns : ARRAY[1..4] OF SINT;
END_STRUCT
END_TYPE

TYPE MC_CART_POS_REF :
STRUCT
    Tcp : MC_CART_REF;
    Cfg : MC_CONFIG_DATA;
    TurnInfo : MC_TURN_INFO;
    AuxiliaryAxes : ARRAY[1..4] OF REAL;
END_STRUCT
END_TYPE

TYPE MC_AXES_POS_REF :
STRUCT
    Axes : ARRAY[1..4] OF REAL;
END_STRUCT
END_TYPE

TYPE MC_POS_REF :
STRUCT
    C : MC_CART_POS_REF;
    A : MC_AXES_POS_REF;
END_STRUCT
END_TYPE

TYPE MC_DISTANCE_REF : MC_POS_REF;
END_TYPE

TYPE MC_SWLIMIT :
STRUCT
    SWLimitPos : REAL;
    SWLimitNeg : REAL;
END_STRUCT
END_TYPE

TYPE MC_GROUP_SWLIMITS : ARRAY[1..4] OF MC_SWLIMIT;
END_TYPE


FUNCTION_BLOCK MC_AddAxisToGroup
VAR_IN_OUT
    AxesGroup : AXES_GROUP_REF;
    Axis : AXIS_REF;
END_VAR
VAR_INPUT
    Execute : BOOL;
    IdentInGroup : IDENT_IN_GROUP_REF;
END_VAR
VAR_OUTPUT
    Done : BOOL;
    Busy : BOOL;
    Error : BOOL;
    ErrorID : WORD;
END_VAR
VAR
    LastExecute : BOOL;
END_VAR
IF Execute AND (NOT LastExecute) THEN
    Done := FALSE;
    Busy := FALSE;
    Error := FALSE;
    ErrorID := WORD#16#0000;
    IF NOT TRUST_GROUP_AddMember(Group := AxesGroup, Axis := Axis, IdentInGroup := IdentInGroup) THEN
        Error := TRUE;
        ErrorID := WORD#16#0001;
    ELSE
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

FUNCTION_BLOCK MC_RemoveAxisFromGroup
VAR_IN_OUT
    AxesGroup : AXES_GROUP_REF;
END_VAR
VAR_INPUT
    Execute : BOOL;
    IdentInGroup : IDENT_IN_GROUP_REF;
END_VAR
VAR_OUTPUT
    Done : BOOL;
    Busy : BOOL;
    Error : BOOL;
    ErrorID : WORD;
END_VAR
VAR
    LastExecute : BOOL;
END_VAR
IF Execute AND (NOT LastExecute) THEN
    Done := FALSE;
    Busy := FALSE;
    Error := FALSE;
    ErrorID := WORD#16#0000;
    IF NOT TRUST_GROUP_RemoveMemberByIdent(Group := AxesGroup, IdentInGroup := IdentInGroup) THEN
        Error := TRUE;
        ErrorID := WORD#16#0100;
    ELSE
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

FUNCTION_BLOCK MC_UngroupAllAxes
VAR_IN_OUT
    AxesGroup : AXES_GROUP_REF;
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
    LastExecute : BOOL;
END_VAR
IF Execute AND (NOT LastExecute) THEN
    Done := FALSE;
    Busy := FALSE;
    Error := FALSE;
    ErrorID := WORD#16#0000;
    IF NOT TRUST_GROUP_RemoveAllMembers(Group := AxesGroup) THEN
        Error := TRUE;
        ErrorID := WORD#16#0100;
    ELSE
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

FUNCTION_BLOCK MC_GroupReadConfiguration
VAR_IN_OUT
    AxesGroup : AXES_GROUP_REF;
END_VAR
VAR_INPUT
    Enable : BOOL;
    IdentInGroup : IDENT_IN_GROUP_REF;
    CoordSystem : MC_COORD_SYSTEM;
END_VAR
VAR_OUTPUT
    Valid : BOOL;
    Busy : BOOL;
    Error : BOOL;
    ErrorID : WORD;
    Axis : AXIS_REF;
    AxisID : AXIS_ID;
END_VAR
VAR
    Slot : INT;
    GroupIndex : UINT;
END_VAR
GroupIndex := TRUST_GROUP_GroupIndex(Group := AxesGroup);
IF Enable THEN
    Busy := FALSE;
    Slot := TRUST_GROUP_FindMemberSlotByIdent(Group := AxesGroup, IdentInGroup := IdentInGroup);
    IF Slot = INT#-1 THEN
        Valid := FALSE;
        Error := TRUE;
        ErrorID := WORD#16#0001;
        Axis.AxisId := UDINT#0;
        Axis.InternalIndex := UINT#0;
        AxisID := UDINT#0;
    ELSE
        Valid := TRUE;
        Error := FALSE;
        ErrorID := WORD#16#0000;
        Axis.AxisId := g_trust_group_member_axis_id[GroupIndex, Slot];
        Axis.InternalIndex := g_trust_group_member_internal_index[GroupIndex, Slot];
        AxisID := Axis.AxisId;
    END_IF;
ELSE
    Valid := FALSE;
    Busy := FALSE;
    Error := FALSE;
    ErrorID := WORD#16#0000;
    Axis.AxisId := UDINT#0;
    Axis.InternalIndex := UINT#0;
    AxisID := UDINT#0;
END_IF;
END_FUNCTION_BLOCK

FUNCTION_BLOCK MC_ReadAxisGroupInfo
VAR_IN_OUT
    Axis : AXIS_REF;
END_VAR
VAR_INPUT
    Enable : BOOL;
END_VAR
VAR_OUTPUT
    Valid : BOOL;
    Busy : BOOL;
    Error : BOOL;
    ErrorID : WORD;
    AxesGroup : AXES_GROUP_REF;
    AxesGroupID : AXES_GROUP_ID;
    IdentInGroup : IDENT_IN_GROUP_REF;
END_VAR
VAR
    Match : INT;
    GroupIndex : INT;
    Slot : INT;
END_VAR
IF Enable THEN
    Busy := FALSE;
    Match := TRUST_GROUP_FindMemberSlotByAxis(Axis := Axis);
    IF Match = INT#-1 THEN
        Valid := FALSE;
        Error := TRUE;
        ErrorID := WORD#16#0001;
        AxesGroup.GroupId := UDINT#0;
        AxesGroup.InternalIndex := UINT#0;
        AxesGroupID := UDINT#0;
        IdentInGroup := '';
    ELSE
        GroupIndex := Match / INT#10;
        Slot := Match - (GroupIndex * INT#10);
        Valid := TRUE;
        Error := FALSE;
        ErrorID := WORD#16#0000;
        AxesGroup.GroupId := UDINT#100 + TO_UDINT(GroupIndex);
        AxesGroup.InternalIndex := TO_UINT(GroupIndex);
        AxesGroupID := AxesGroup.GroupId;
        IdentInGroup := g_trust_group_member_ident[TO_UINT(GroupIndex), Slot];
    END_IF;
ELSE
    Valid := FALSE;
    Busy := FALSE;
    Error := FALSE;
    ErrorID := WORD#16#0000;
    AxesGroup.GroupId := UDINT#0;
    AxesGroup.InternalIndex := UINT#0;
    AxesGroupID := UDINT#0;
    IdentInGroup := '';
END_IF;
END_FUNCTION_BLOCK

FUNCTION_BLOCK MC_GroupEnable
VAR_IN_OUT
    AxesGroup : AXES_GROUP_REF;
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
    LastExecute : BOOL;
    GroupIndex : UINT;
END_VAR
GroupIndex := TRUST_GROUP_GroupIndex(Group := AxesGroup);
IF Execute AND (NOT LastExecute) THEN
    Done := FALSE;
    Busy := FALSE;
    Error := FALSE;
    ErrorID := WORD#16#0000;
    IF g_trust_group_status[GroupIndex] = MC_GROUP_STATUS#mcGroupDisabled THEN
        g_trust_group_status[GroupIndex] := MC_GROUP_STATUS#mcGroupStandby;
        Done := TRUE;
    ELSE
        Error := TRUE;
        ErrorID := WORD#16#0100;
    END_IF;
ELSIF NOT Execute THEN
    Done := FALSE;
    Busy := FALSE;
    Error := FALSE;
    ErrorID := WORD#16#0000;
END_IF;
LastExecute := Execute;
END_FUNCTION_BLOCK

FUNCTION_BLOCK MC_GroupDisable
VAR_IN_OUT
    AxesGroup : AXES_GROUP_REF;
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
    LastExecute : BOOL;
    GroupIndex : UINT;
END_VAR
GroupIndex := TRUST_GROUP_GroupIndex(Group := AxesGroup);
IF Execute AND (NOT LastExecute) THEN
    g_trust_group_status[GroupIndex] := MC_GROUP_STATUS#mcGroupDisabled;
    Done := TRUE;
    Busy := FALSE;
    Error := FALSE;
    ErrorID := WORD#16#0000;
ELSIF NOT Execute THEN
    Done := FALSE;
    Busy := FALSE;
    Error := FALSE;
    ErrorID := WORD#16#0000;
END_IF;
LastExecute := Execute;
END_FUNCTION_BLOCK

FUNCTION_BLOCK MC_GroupPower
VAR_IN_OUT
    AxesGroup : AXES_GROUP_REF;
END_VAR
VAR_INPUT
    Enable : BOOL;
END_VAR
VAR_OUTPUT
    Status : BOOL;
    Valid : BOOL;
    Error : BOOL;
    ErrorID : WORD;
END_VAR
VAR
    GroupIndex : UINT;
END_VAR
GroupIndex := TRUST_GROUP_GroupIndex(Group := AxesGroup);
Valid := TRUE;
Error := FALSE;
ErrorID := WORD#16#0000;
IF Enable THEN
    g_trust_group_power_stage[GroupIndex] := TRUE;
ELSE
    IF g_trust_group_power_stage[GroupIndex] AND ((g_trust_group_status[GroupIndex] = MC_GROUP_STATUS#mcGroupMoving) OR (g_trust_group_status[GroupIndex] = MC_GROUP_STATUS#mcGroupHoming) OR (g_trust_group_status[GroupIndex] = MC_GROUP_STATUS#mcGroupStopping)) THEN
        g_trust_group_status[GroupIndex] := MC_GROUP_STATUS#mcGroupErrorStop;
        g_trust_group_error_id[GroupIndex] := WORD#16#0105;
    END_IF;
    g_trust_group_power_stage[GroupIndex] := FALSE;
END_IF;
Status := g_trust_group_power_stage[GroupIndex];
END_FUNCTION_BLOCK

FUNCTION_BLOCK MC_GroupReadStatus
VAR_IN_OUT
    AxesGroup : AXES_GROUP_REF;
END_VAR
VAR_INPUT
    Enable : BOOL;
END_VAR
VAR_OUTPUT
    Valid : BOOL;
    Busy : BOOL;
    Error : BOOL;
    ErrorID : WORD;
    GroupMoving : BOOL;
    GroupHoming : BOOL;
    GroupErrorStop : BOOL;
    GroupStandby : BOOL;
    GroupStopping : BOOL;
    GroupDisabled : BOOL;
END_VAR
VAR
    GroupIndex : UINT;
END_VAR
GroupIndex := TRUST_GROUP_GroupIndex(Group := AxesGroup);
IF Enable THEN
    Valid := TRUE;
    Busy := FALSE;
    Error := FALSE;
    ErrorID := WORD#16#0000;
    GroupMoving := g_trust_group_status[GroupIndex] = MC_GROUP_STATUS#mcGroupMoving;
    GroupHoming := g_trust_group_status[GroupIndex] = MC_GROUP_STATUS#mcGroupHoming;
    GroupErrorStop := g_trust_group_status[GroupIndex] = MC_GROUP_STATUS#mcGroupErrorStop;
    GroupStandby := g_trust_group_status[GroupIndex] = MC_GROUP_STATUS#mcGroupStandby;
    GroupStopping := g_trust_group_status[GroupIndex] = MC_GROUP_STATUS#mcGroupStopping;
    GroupDisabled := g_trust_group_status[GroupIndex] = MC_GROUP_STATUS#mcGroupDisabled;
ELSE
    Valid := FALSE;
    Busy := FALSE;
    Error := FALSE;
    ErrorID := WORD#16#0000;
    GroupMoving := FALSE;
    GroupHoming := FALSE;
    GroupErrorStop := FALSE;
    GroupStandby := FALSE;
    GroupStopping := FALSE;
    GroupDisabled := FALSE;
END_IF;
END_FUNCTION_BLOCK

FUNCTION_BLOCK MC_GroupReadError
VAR_IN_OUT
    AxesGroup : AXES_GROUP_REF;
END_VAR
VAR_INPUT
    Enable : BOOL;
END_VAR
VAR_OUTPUT
    Valid : BOOL;
    Busy : BOOL;
    Error : BOOL;
    ErrorID : WORD;
    GroupErrorID : WORD;
END_VAR
VAR
    GroupIndex : UINT;
END_VAR
GroupIndex := TRUST_GROUP_GroupIndex(Group := AxesGroup);
IF Enable THEN
    Valid := TRUE;
    Busy := FALSE;
    Error := FALSE;
    ErrorID := WORD#16#0000;
    GroupErrorID := g_trust_group_error_id[GroupIndex];
ELSE
    Valid := FALSE;
    Busy := FALSE;
    Error := FALSE;
    ErrorID := WORD#16#0000;
    GroupErrorID := WORD#16#0000;
END_IF;
END_FUNCTION_BLOCK

FUNCTION_BLOCK MC_GroupReset
VAR_IN_OUT
    AxesGroup : AXES_GROUP_REF;
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
    LastExecute : BOOL;
    GroupIndex : UINT;
END_VAR
GroupIndex := TRUST_GROUP_GroupIndex(Group := AxesGroup);
IF Execute AND (NOT LastExecute) THEN
    Done := FALSE;
    Busy := FALSE;
    Error := FALSE;
    ErrorID := WORD#16#0000;
    IF g_trust_group_status[GroupIndex] = MC_GROUP_STATUS#mcGroupErrorStop THEN
        g_trust_group_status[GroupIndex] := MC_GROUP_STATUS#mcGroupStandby;
        g_trust_group_error_id[GroupIndex] := WORD#16#0000;
    END_IF;
    Done := TRUE;
ELSIF NOT Execute THEN
    Done := FALSE;
    Busy := FALSE;
    Error := FALSE;
    ErrorID := WORD#16#0000;
END_IF;
LastExecute := Execute;
END_FUNCTION_BLOCK

FUNCTION_BLOCK MC_GroupReadPosition
VAR_IN_OUT
    AxesGroup : AXES_GROUP_REF;
END_VAR
VAR_INPUT
    Enable : BOOL;
    CoordSystem : MC_COORD_SYSTEM;
    Source : MC_SOURCE;
END_VAR
VAR_OUTPUT
    Valid : BOOL;
    Busy : BOOL;
    Error : BOOL;
    ErrorID : WORD;
    Position : MC_POS_REF;
END_VAR
VAR
    GroupIndex : UINT;
    Offset : REAL;
    Cart : MC_CART_REF;
    CartPos : MC_CART_POS_REF;
    Pos : MC_POS_REF;
END_VAR
GroupIndex := TRUST_GROUP_GroupIndex(Group := AxesGroup);
IF Enable THEN
    Valid := TRUE;
    Busy := FALSE;
    Error := FALSE;
    ErrorID := WORD#16#0000;
    Offset := TRUST_GROUP_CoordOffset(CoordSystem := CoordSystem) - TRUST_GROUP_CoordOffset(CoordSystem := MC_COORD_SYSTEM#mcMCS);
    IF Source = MC_SOURCE#mcActualValue THEN
        Cart.X := g_trust_group_actual_x[GroupIndex] + Offset;
        Cart.Y := g_trust_group_actual_y[GroupIndex] + Offset;
        Cart.Z := g_trust_group_actual_z[GroupIndex] + Offset;
        Cart.RX := g_trust_group_actual_rx[GroupIndex] + Offset;
        Cart.RY := g_trust_group_actual_ry[GroupIndex] + Offset;
        Cart.RZ := g_trust_group_actual_rz[GroupIndex] + Offset;
    ELSE
        Cart.X := g_trust_group_commanded_x[GroupIndex] + Offset;
        Cart.Y := g_trust_group_commanded_y[GroupIndex] + Offset;
        Cart.Z := g_trust_group_commanded_z[GroupIndex] + Offset;
        Cart.RX := g_trust_group_commanded_rx[GroupIndex] + Offset;
        Cart.RY := g_trust_group_commanded_ry[GroupIndex] + Offset;
        Cart.RZ := g_trust_group_commanded_rz[GroupIndex] + Offset;
    END_IF;
    CartPos.Tcp := Cart;
    Pos.C := CartPos;
    Position := Pos;
ELSE
    Valid := FALSE;
    Busy := FALSE;
    Error := FALSE;
    ErrorID := WORD#16#0000;
END_IF;
END_FUNCTION_BLOCK

FUNCTION_BLOCK MC_GroupReadVelocity
VAR_IN_OUT
    AxesGroup : AXES_GROUP_REF;
END_VAR
VAR_INPUT
    Enable : BOOL;
    CoordSystem : MC_COORD_SYSTEM;
    Source : MC_SOURCE;
END_VAR
VAR_OUTPUT
    Valid : BOOL;
    Busy : BOOL;
    Error : BOOL;
    ErrorID : WORD;
    Velocity : ARRAY[1..4] OF REAL;
    PathVelocity : REAL;
END_VAR
VAR
    GroupIndex : UINT;
    Slot : INT;
    BaseValue : REAL;
END_VAR
GroupIndex := TRUST_GROUP_GroupIndex(Group := AxesGroup);
IF Enable THEN
    Valid := TRUE;
    Busy := FALSE;
    Error := FALSE;
    ErrorID := WORD#16#0000;
    IF Source = MC_SOURCE#mcActualValue THEN
        BaseValue := g_trust_group_actual_path_velocity[GroupIndex];
    ELSE
        BaseValue := g_trust_group_commanded_path_velocity[GroupIndex];
    END_IF;
    PathVelocity := BaseValue;
    FOR Slot := INT#1 TO INT#4 DO
        Velocity[Slot] := BaseValue;
    END_FOR;
ELSE
    Valid := FALSE;
    Busy := FALSE;
    Error := FALSE;
    ErrorID := WORD#16#0000;
    PathVelocity := REAL#0.0;
END_IF;
END_FUNCTION_BLOCK

FUNCTION_BLOCK MC_GroupReadAcceleration
VAR_IN_OUT
    AxesGroup : AXES_GROUP_REF;
END_VAR
VAR_INPUT
    Enable : BOOL;
    CoordSystem : MC_COORD_SYSTEM;
    Source : MC_SOURCE;
END_VAR
VAR_OUTPUT
    Valid : BOOL;
    Busy : BOOL;
    Error : BOOL;
    ErrorID : WORD;
    Acceleration : ARRAY[1..4] OF REAL;
    PathAcceleration : REAL;
END_VAR
VAR
    GroupIndex : UINT;
    Slot : INT;
    BaseValue : REAL;
END_VAR
GroupIndex := TRUST_GROUP_GroupIndex(Group := AxesGroup);
IF Enable THEN
    Valid := TRUE;
    Busy := FALSE;
    Error := FALSE;
    ErrorID := WORD#16#0000;
    IF Source = MC_SOURCE#mcActualValue THEN
        BaseValue := g_trust_group_actual_path_acceleration[GroupIndex];
    ELSE
        BaseValue := g_trust_group_commanded_path_acceleration[GroupIndex];
    END_IF;
    PathAcceleration := BaseValue;
    FOR Slot := INT#1 TO INT#4 DO
        Acceleration[Slot] := BaseValue;
    END_FOR;
ELSE
    Valid := FALSE;
    Busy := FALSE;
    Error := FALSE;
    ErrorID := WORD#16#0000;
    PathAcceleration := REAL#0.0;
END_IF;
END_FUNCTION_BLOCK

FUNCTION_BLOCK MC_GroupReadMotionState
VAR_IN_OUT
    AxesGroup : AXES_GROUP_REF;
END_VAR
VAR_INPUT
    Enable : BOOL;
END_VAR
VAR_OUTPUT
    Valid : BOOL;
    Busy : BOOL;
    Error : BOOL;
    ErrorID : WORD;
    Tracking : BOOL;
    InSync : BOOL;
    InPosition : BOOL;
    Standstill : BOOL;
    ConstantVelocity : BOOL;
    Accelerating : BOOL;
    Decelerating : BOOL;
    ActiveCommandID : MC_COMMAND_ID;
END_VAR
VAR
    GroupIndex : UINT;
END_VAR
GroupIndex := TRUST_GROUP_GroupIndex(Group := AxesGroup);
IF Enable THEN
    Busy := FALSE;
    Error := FALSE;
    ErrorID := WORD#16#0000;
    Valid := (g_trust_group_status[GroupIndex] <> MC_GROUP_STATUS#mcGroupErrorStop) AND (g_trust_group_status[GroupIndex] <> MC_GROUP_STATUS#mcGroupDisabled);
    Tracking := g_trust_group_tracking[GroupIndex];
    IF Tracking THEN
        InSync := g_trust_group_in_sync[GroupIndex];
    ELSE
        InSync := TRUE;
    END_IF;
    InPosition := g_trust_group_in_position[GroupIndex];
    Standstill := g_trust_group_status[GroupIndex] = MC_GROUP_STATUS#mcGroupStandby;
    ConstantVelocity := g_trust_group_constant_velocity[GroupIndex];
    Accelerating := g_trust_group_accelerating[GroupIndex];
    Decelerating := g_trust_group_decelerating[GroupIndex];
    IF g_trust_group_status[GroupIndex] = MC_GROUP_STATUS#mcGroupMoving THEN
        ActiveCommandID := g_trust_group_last_command_id[GroupIndex];
    ELSE
        ActiveCommandID := UINT#0;
    END_IF;
ELSE
    Valid := FALSE;
    Busy := FALSE;
    Error := FALSE;
    ErrorID := WORD#16#0000;
    Tracking := FALSE;
    InSync := FALSE;
    InPosition := FALSE;
    Standstill := FALSE;
    ConstantVelocity := FALSE;
    Accelerating := FALSE;
    Decelerating := FALSE;
    ActiveCommandID := UINT#0;
END_IF;
END_FUNCTION_BLOCK

FUNCTION_BLOCK MC_GroupReadParameter
VAR_IN_OUT
    AxesGroup : AXES_GROUP_REF;
END_VAR
VAR_INPUT
    Enable : BOOL;
    ParameterNumber : MC_GROUP_PARAMETER;
END_VAR
VAR_OUTPUT
    Valid : BOOL;
    Busy : BOOL;
    Error : BOOL;
    ErrorID : WORD;
    Value : REAL;
END_VAR
IF Enable THEN
    Valid := TRUE;
    Busy := FALSE;
    Error := FALSE;
    ErrorID := WORD#16#0000;
    Value := TRUST_GROUP_GroupParameterToReal(Group := AxesGroup, ParameterNumber := ParameterNumber);
ELSE
    Valid := FALSE;
    Busy := FALSE;
    Error := FALSE;
    ErrorID := WORD#16#0000;
    Value := REAL#0.0;
END_IF;
END_FUNCTION_BLOCK

FUNCTION_BLOCK MC_GroupWriteParameter
VAR_IN_OUT
    AxesGroup : AXES_GROUP_REF;
END_VAR
VAR_INPUT
    Execute : BOOL;
    ParameterNumber : MC_GROUP_PARAMETER;
    Value : REAL;
    ExecutionMode : MC_EXECUTION_MODE;
END_VAR
VAR_OUTPUT
    Done : BOOL;
    Busy : BOOL;
    CommandAccepted : BOOL;
    CommandAborted : BOOL;
    CommandID : MC_COMMAND_ID;
    Error : BOOL;
    ErrorID : WORD;
END_VAR
VAR
    LastExecute : BOOL;
    GroupIndex : UINT;
END_VAR
GroupIndex := TRUST_GROUP_GroupIndex(Group := AxesGroup);
IF Execute AND (NOT LastExecute) THEN
    Done := FALSE;
    Busy := FALSE;
    CommandAccepted := FALSE;
    CommandAborted := FALSE;
    CommandID := UINT#0;
    Error := FALSE;
    ErrorID := WORD#16#0000;
    IF ExecutionMode = MC_EXECUTION_MODE#mcDelayed THEN
        Error := TRUE;
        ErrorID := WORD#16#0500;
    ELSIF ParameterNumber = MC_GROUP_PARAMETER#mcDynamicsMode THEN
        IF Value = REAL#0.0 THEN
            g_trust_group_dynamics_mode[GroupIndex] := MC_DYNAMICS_MODE#mcAbsolute;
        ELSIF Value = REAL#1.0 THEN
            g_trust_group_dynamics_mode[GroupIndex] := MC_DYNAMICS_MODE#mcPercentage;
        ELSE
            Error := TRUE;
            ErrorID := WORD#16#0001;
        END_IF;
    ELSE
        IF (g_trust_group_status[GroupIndex] <> MC_GROUP_STATUS#mcGroupDisabled) AND (g_trust_group_status[GroupIndex] <> MC_GROUP_STATUS#mcGroupStandby) THEN
            Error := TRUE;
            ErrorID := WORD#16#0100;
        ELSIF Value = REAL#0.0 THEN
            g_trust_group_transition_reference[GroupIndex] := MC_TRANSITION_REFERENCE#mcStartPoint;
        ELSIF Value = REAL#1.0 THEN
            g_trust_group_transition_reference[GroupIndex] := MC_TRANSITION_REFERENCE#mcEndPoint;
        ELSE
            Error := TRUE;
            ErrorID := WORD#16#0001;
        END_IF;
    END_IF;
    IF NOT Error THEN
        CommandID := TRUST_GROUP_RequestCommand(Group := AxesGroup, CoordSystem := g_trust_group_active_coord_system[GroupIndex]);
        CommandAccepted := TRUE;
        Done := TRUE;
    END_IF;
ELSIF NOT Execute THEN
    Done := FALSE;
    Busy := FALSE;
    CommandAccepted := FALSE;
    CommandAborted := FALSE;
    CommandID := UINT#0;
    Error := FALSE;
    ErrorID := WORD#16#0000;
END_IF;
LastExecute := Execute;
END_FUNCTION_BLOCK

FUNCTION_BLOCK MC_GroupReadSWLimits
VAR_IN_OUT
    AxesGroup : AXES_GROUP_REF;
END_VAR
VAR_INPUT
    Enable : BOOL;
END_VAR
VAR_OUTPUT
    Valid : BOOL;
    Busy : BOOL;
    Error : BOOL;
    ErrorID : WORD;
    LimitValues : MC_GROUP_SWLIMITS;
END_VAR
VAR
    GroupIndex : UINT;
    Slot : INT;
END_VAR
GroupIndex := TRUST_GROUP_GroupIndex(Group := AxesGroup);
IF Enable THEN
    Valid := TRUE;
    Busy := FALSE;
    Error := FALSE;
    ErrorID := WORD#16#0000;
    FOR Slot := INT#1 TO INT#4 DO
        LimitValues[Slot] := g_trust_group_swlimits[GroupIndex, Slot];
    END_FOR;
ELSE
    Valid := FALSE;
    Busy := FALSE;
    Error := FALSE;
    ErrorID := WORD#16#0000;
END_IF;
END_FUNCTION_BLOCK

FUNCTION_BLOCK MC_GroupWriteSWLimits
VAR_IN_OUT
    AxesGroup : AXES_GROUP_REF;
END_VAR
VAR_INPUT
    Execute : BOOL;
    LimitValues : MC_GROUP_SWLIMITS;
    ExecutionMode : MC_EXECUTION_MODE;
END_VAR
VAR_OUTPUT
    Done : BOOL;
    Busy : BOOL;
    Error : BOOL;
    ErrorID : WORD;
END_VAR
VAR
    LastExecute : BOOL;
    GroupIndex : UINT;
    Slot : INT;
END_VAR
GroupIndex := TRUST_GROUP_GroupIndex(Group := AxesGroup);
IF Execute AND (NOT LastExecute) THEN
    Done := FALSE;
    Busy := FALSE;
    Error := FALSE;
    ErrorID := WORD#16#0000;
    IF ExecutionMode = MC_EXECUTION_MODE#mcDelayed THEN
        Error := TRUE;
        ErrorID := WORD#16#0500;
    ELSE
        FOR Slot := INT#1 TO INT#4 DO
            g_trust_group_swlimits[GroupIndex, Slot] := LimitValues[Slot];
        END_FOR;
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

FUNCTION_BLOCK MC_SetKinTransform
VAR_IN_OUT
    AxesGroup : AXES_GROUP_REF;
END_VAR
VAR_INPUT
    Execute : BOOL;
    KinTransform : MC_KIN_REF;
    ExecutionMode : MC_EXECUTION_MODE;
END_VAR
VAR_OUTPUT
    Done : BOOL;
    Busy : BOOL;
    Active : BOOL;
    CommandAccepted : BOOL;
    CommandAborted : BOOL;
    CommandID : MC_COMMAND_ID;
    Error : BOOL;
    ErrorID : WORD;
END_VAR
VAR
    LastExecute : BOOL;
    GroupIndex : UINT;
END_VAR
GroupIndex := TRUST_GROUP_GroupIndex(Group := AxesGroup);
IF Execute AND (NOT LastExecute) THEN
    Done := FALSE;
    Busy := FALSE;
    Active := FALSE;
    CommandAccepted := FALSE;
    CommandAborted := FALSE;
    CommandID := UINT#0;
    Error := FALSE;
    ErrorID := WORD#16#0000;
    IF ExecutionMode = MC_EXECUTION_MODE#mcDelayed THEN
        Error := TRUE;
        ErrorID := WORD#16#0500;
    ELSE
        CommandID := TRUST_GROUP_RequestCommand(Group := AxesGroup, CoordSystem := g_trust_group_active_coord_system[GroupIndex]);
        IF CommandID = UINT#0 THEN
            Error := TRUE;
            ErrorID := WORD#16#0103;
        ELSE
            g_trust_group_kin_transform[GroupIndex] := KinTransform;
            TRUST_GROUP_SetCommandInfo(
                Group := AxesGroup,
                CommandState := MC_COMMAND_STATE#mcAccepted,
                ElapsedDuration := TIME#0ms,
                RemainingDuration := TIME#0ms,
                RemainingDistance := REAL#0.0,
                Progress := REAL#1.0,
                InfoID := TRUST_GROUP_TransitionInfoID(Group := AxesGroup),
                WarningID := WORD#16#2007
            );
            Done := TRUE;
            CommandAccepted := TRUE;
        END_IF;
    END_IF;
ELSIF NOT Execute THEN
    Done := FALSE;
    Busy := FALSE;
    Active := FALSE;
    CommandAccepted := FALSE;
    CommandAborted := FALSE;
    CommandID := UINT#0;
    Error := FALSE;
    ErrorID := WORD#16#0000;
END_IF;
LastExecute := Execute;
END_FUNCTION_BLOCK

FUNCTION_BLOCK MC_SetCartesianTransform
VAR_IN_OUT
    AxesGroup : AXES_GROUP_REF;
END_VAR
VAR_INPUT
    Execute : BOOL;
    TransX : REAL;
    TransY : REAL;
    TransZ : REAL;
    RotAngle1 : REAL;
    RotAngle2 : REAL;
    RotAngle3 : REAL;
    CoordSystem : MC_COORD_SYSTEM;
    ExecutionMode : MC_EXECUTION_MODE;
END_VAR
VAR_OUTPUT
    Done : BOOL;
    Busy : BOOL;
    Active : BOOL;
    CommandAccepted : BOOL;
    CommandAborted : BOOL;
    CommandID : MC_COMMAND_ID;
    Error : BOOL;
    ErrorID : WORD;
END_VAR
VAR
    LastExecute : BOOL;
    GroupIndex : UINT;
    Slot : INT;
END_VAR
GroupIndex := TRUST_GROUP_GroupIndex(Group := AxesGroup);
IF Execute AND (NOT LastExecute) THEN
    Done := FALSE;
    Busy := FALSE;
    Active := FALSE;
    CommandAccepted := FALSE;
    CommandAborted := FALSE;
    CommandID := UINT#0;
    Error := FALSE;
    ErrorID := WORD#16#0000;
    Slot := TRUST_GROUP_TransformSlot(CoordSystem := CoordSystem);
    IF ExecutionMode = MC_EXECUTION_MODE#mcDelayed THEN
        Error := TRUE;
        ErrorID := WORD#16#0500;
    ELSIF Slot = INT#-1 THEN
        Error := TRUE;
        ErrorID := WORD#16#0500;
    ELSE
        CommandID := TRUST_GROUP_RequestCommand(Group := AxesGroup, CoordSystem := CoordSystem);
        IF CommandID = UINT#0 THEN
            Error := TRUE;
            ErrorID := WORD#16#0103;
        ELSE
            g_trust_group_cartesian_trans_x[GroupIndex, Slot] := TransX;
            g_trust_group_cartesian_trans_y[GroupIndex, Slot] := TransY;
            g_trust_group_cartesian_trans_z[GroupIndex, Slot] := TransZ;
            g_trust_group_cartesian_rot_1[GroupIndex, Slot] := RotAngle1;
            g_trust_group_cartesian_rot_2[GroupIndex, Slot] := RotAngle2;
            g_trust_group_cartesian_rot_3[GroupIndex, Slot] := RotAngle3;
            TRUST_GROUP_SetCommandInfo(
                Group := AxesGroup,
                CommandState := MC_COMMAND_STATE#mcAccepted,
                ElapsedDuration := TIME#0ms,
                RemainingDuration := TIME#0ms,
                RemainingDistance := REAL#0.0,
                Progress := REAL#1.0,
                InfoID := TRUST_GROUP_TransitionInfoID(Group := AxesGroup),
                WarningID := WORD#16#2008
            );
            Done := TRUE;
            CommandAccepted := TRUE;
        END_IF;
    END_IF;
ELSIF NOT Execute THEN
    Done := FALSE;
    Busy := FALSE;
    Active := FALSE;
    CommandAccepted := FALSE;
    CommandAborted := FALSE;
    CommandID := UINT#0;
    Error := FALSE;
    ErrorID := WORD#16#0000;
END_IF;
LastExecute := Execute;
END_FUNCTION_BLOCK

FUNCTION_BLOCK MC_SetCoordinateTransform
VAR_IN_OUT
    AxesGroup : AXES_GROUP_REF;
END_VAR
VAR_INPUT
    Execute : BOOL;
    CoordTransform : MC_COORD_REF;
    CoordSystem : MC_COORD_SYSTEM;
    ExecutionMode : MC_EXECUTION_MODE;
END_VAR
VAR_OUTPUT
    Done : BOOL;
    Busy : BOOL;
    Active : BOOL;
    CommandAccepted : BOOL;
    CommandAborted : BOOL;
    CommandID : MC_COMMAND_ID;
    Error : BOOL;
    ErrorID : WORD;
END_VAR
VAR
    LastExecute : BOOL;
    GroupIndex : UINT;
    Slot : INT;
END_VAR
GroupIndex := TRUST_GROUP_GroupIndex(Group := AxesGroup);
IF Execute AND (NOT LastExecute) THEN
    Done := FALSE;
    Busy := FALSE;
    Active := FALSE;
    CommandAccepted := FALSE;
    CommandAborted := FALSE;
    CommandID := UINT#0;
    Error := FALSE;
    ErrorID := WORD#16#0000;
    Slot := TRUST_GROUP_TransformSlot(CoordSystem := CoordSystem);
    IF ExecutionMode = MC_EXECUTION_MODE#mcDelayed THEN
        Error := TRUE;
        ErrorID := WORD#16#0500;
    ELSIF Slot = INT#-1 THEN
        Error := TRUE;
        ErrorID := WORD#16#0500;
    ELSE
        CommandID := TRUST_GROUP_RequestCommand(Group := AxesGroup, CoordSystem := CoordSystem);
        IF CommandID = UINT#0 THEN
            Error := TRUE;
            ErrorID := WORD#16#0103;
        ELSE
            g_trust_group_coord_trans_x[GroupIndex, Slot] := CoordTransform.X;
            g_trust_group_coord_trans_y[GroupIndex, Slot] := CoordTransform.Y;
            g_trust_group_coord_trans_z[GroupIndex, Slot] := CoordTransform.Z;
            g_trust_group_coord_rot_x[GroupIndex, Slot] := CoordTransform.RX;
            g_trust_group_coord_rot_y[GroupIndex, Slot] := CoordTransform.RY;
            g_trust_group_coord_rot_z[GroupIndex, Slot] := CoordTransform.RZ;
            TRUST_GROUP_SetCommandInfo(
                Group := AxesGroup,
                CommandState := MC_COMMAND_STATE#mcAccepted,
                ElapsedDuration := TIME#0ms,
                RemainingDuration := TIME#0ms,
                RemainingDistance := REAL#0.0,
                Progress := REAL#1.0,
                InfoID := TRUST_GROUP_TransitionInfoID(Group := AxesGroup),
                WarningID := WORD#16#2009
            );
            Done := TRUE;
            CommandAccepted := TRUE;
        END_IF;
    END_IF;
ELSIF NOT Execute THEN
    Done := FALSE;
    Busy := FALSE;
    Active := FALSE;
    CommandAccepted := FALSE;
    CommandAborted := FALSE;
    CommandID := UINT#0;
    Error := FALSE;
    ErrorID := WORD#16#0000;
END_IF;
LastExecute := Execute;
END_FUNCTION_BLOCK

FUNCTION_BLOCK MC_ReadKinTransform
VAR_IN_OUT
    AxesGroup : AXES_GROUP_REF;
END_VAR
VAR_INPUT
    Enable : BOOL;
END_VAR
VAR_OUTPUT
    Valid : BOOL;
    Busy : BOOL;
    Error : BOOL;
    ErrorID : WORD;
    KinTransform : MC_KIN_REF;
END_VAR
VAR
    GroupIndex : UINT;
END_VAR
GroupIndex := TRUST_GROUP_GroupIndex(Group := AxesGroup);
IF Enable THEN
    Valid := TRUE;
    Busy := FALSE;
    Error := FALSE;
    ErrorID := WORD#16#0000;
    KinTransform := g_trust_group_kin_transform[GroupIndex];
ELSE
    Valid := FALSE;
    Busy := FALSE;
    Error := FALSE;
    ErrorID := WORD#16#0000;
    KinTransform := UINT#0;
END_IF;
END_FUNCTION_BLOCK

FUNCTION_BLOCK MC_ReadCartesianTransform
VAR_IN_OUT
    AxesGroup : AXES_GROUP_REF;
END_VAR
VAR_INPUT
    Enable : BOOL;
    CoordSystem : MC_COORD_SYSTEM;
END_VAR
VAR_OUTPUT
    Valid : BOOL;
    Busy : BOOL;
    Error : BOOL;
    ErrorID : WORD;
    TransX : REAL;
    TransY : REAL;
    TransZ : REAL;
    RotAngle1 : REAL;
    RotAngle2 : REAL;
    RotAngle3 : REAL;
END_VAR
VAR
    GroupIndex : UINT;
    Slot : INT;
END_VAR
GroupIndex := TRUST_GROUP_GroupIndex(Group := AxesGroup);
Slot := TRUST_GROUP_TransformSlot(CoordSystem := CoordSystem);
IF Enable THEN
    Busy := FALSE;
    IF Slot = INT#-1 THEN
        Valid := FALSE;
        Error := TRUE;
        ErrorID := WORD#16#0500;
        TransX := REAL#0.0;
        TransY := REAL#0.0;
        TransZ := REAL#0.0;
        RotAngle1 := REAL#0.0;
        RotAngle2 := REAL#0.0;
        RotAngle3 := REAL#0.0;
    ELSE
        Valid := TRUE;
        Error := FALSE;
        ErrorID := WORD#16#0000;
        TransX := g_trust_group_cartesian_trans_x[GroupIndex, Slot];
        TransY := g_trust_group_cartesian_trans_y[GroupIndex, Slot];
        TransZ := g_trust_group_cartesian_trans_z[GroupIndex, Slot];
        RotAngle1 := g_trust_group_cartesian_rot_1[GroupIndex, Slot];
        RotAngle2 := g_trust_group_cartesian_rot_2[GroupIndex, Slot];
        RotAngle3 := g_trust_group_cartesian_rot_3[GroupIndex, Slot];
    END_IF;
ELSE
    Valid := FALSE;
    Busy := FALSE;
    Error := FALSE;
    ErrorID := WORD#16#0000;
    TransX := REAL#0.0;
    TransY := REAL#0.0;
    TransZ := REAL#0.0;
    RotAngle1 := REAL#0.0;
    RotAngle2 := REAL#0.0;
    RotAngle3 := REAL#0.0;
END_IF;
END_FUNCTION_BLOCK

FUNCTION_BLOCK MC_ReadCoordinateTransform
VAR_IN_OUT
    AxesGroup : AXES_GROUP_REF;
END_VAR
VAR_INPUT
    Enable : BOOL;
    CoordSystem : MC_COORD_SYSTEM;
END_VAR
VAR_OUTPUT
    Valid : BOOL;
    Busy : BOOL;
    Error : BOOL;
    ErrorID : WORD;
    CoordTransform : MC_COORD_REF;
END_VAR
VAR
    GroupIndex : UINT;
    Slot : INT;
    CoordOut : MC_COORD_REF;
END_VAR
GroupIndex := TRUST_GROUP_GroupIndex(Group := AxesGroup);
Slot := TRUST_GROUP_TransformSlot(CoordSystem := CoordSystem);
IF Enable THEN
    Busy := FALSE;
    IF Slot = INT#-1 THEN
        Valid := FALSE;
        Error := TRUE;
        ErrorID := WORD#16#0500;
        CoordTransform := CoordOut;
    ELSE
        Valid := TRUE;
        Error := FALSE;
        ErrorID := WORD#16#0000;
        CoordOut.X := g_trust_group_coord_trans_x[GroupIndex, Slot];
        CoordOut.Y := g_trust_group_coord_trans_y[GroupIndex, Slot];
        CoordOut.Z := g_trust_group_coord_trans_z[GroupIndex, Slot];
        CoordOut.RX := g_trust_group_coord_rot_x[GroupIndex, Slot];
        CoordOut.RY := g_trust_group_coord_rot_y[GroupIndex, Slot];
        CoordOut.RZ := g_trust_group_coord_rot_z[GroupIndex, Slot];
        CoordTransform := CoordOut;
    END_IF;
ELSE
    Valid := FALSE;
    Busy := FALSE;
    Error := FALSE;
    ErrorID := WORD#16#0000;
    CoordTransform := CoordOut;
END_IF;
END_FUNCTION_BLOCK

FUNCTION_BLOCK MC_GroupSetPosition
VAR_IN_OUT
    AxesGroup : AXES_GROUP_REF;
END_VAR
VAR_INPUT
    Execute : BOOL;
    Position : MC_POS_REF;
    Relative : BOOL;
    CoordSystem : MC_COORD_SYSTEM;
    ExecutionMode : MC_EXECUTION_MODE;
END_VAR
VAR_OUTPUT
    Done : BOOL;
    Busy : BOOL;
    CommandAccepted : BOOL;
    CommandAborted : BOOL;
    CommandID : MC_COMMAND_ID;
    Error : BOOL;
    ErrorID : WORD;
END_VAR
VAR
    LastExecute : BOOL;
    GroupIndex : UINT;
    TargetCartPos : MC_CART_POS_REF;
    TargetCart : MC_CART_REF;
END_VAR
GroupIndex := TRUST_GROUP_GroupIndex(Group := AxesGroup);
IF Execute AND (NOT LastExecute) THEN
    Done := FALSE;
    Busy := FALSE;
    CommandAccepted := FALSE;
    CommandAborted := FALSE;
    CommandID := UINT#0;
    Error := FALSE;
    ErrorID := WORD#16#0000;
    IF ExecutionMode = MC_EXECUTION_MODE#mcDelayed THEN
        Error := TRUE;
        ErrorID := WORD#16#0500;
    ELSIF NOT TRUST_GROUP_GroupReadyForMotion(Group := AxesGroup) THEN
        Error := TRUE;
        ErrorID := WORD#16#0103;
    ELSIF NOT TRUST_GROUP_MotionCoordSystemSupported(CoordSystem := CoordSystem) THEN
        Error := TRUE;
        ErrorID := WORD#16#0500;
    ELSE
        CommandID := TRUST_GROUP_RequestCommand(Group := AxesGroup, CoordSystem := CoordSystem);
        IF CommandID = UINT#0 THEN
            Error := TRUE;
            ErrorID := WORD#16#0103;
        ELSE
            TargetCartPos := Position.C;
            TargetCart := TargetCartPos.Tcp;
            IF Relative THEN
                g_trust_group_commanded_x[GroupIndex] := g_trust_group_commanded_x[GroupIndex] + TargetCart.X;
                g_trust_group_commanded_y[GroupIndex] := g_trust_group_commanded_y[GroupIndex] + TargetCart.Y;
                g_trust_group_commanded_z[GroupIndex] := g_trust_group_commanded_z[GroupIndex] + TargetCart.Z;
                g_trust_group_commanded_rx[GroupIndex] := g_trust_group_commanded_rx[GroupIndex] + TargetCart.RX;
                g_trust_group_commanded_ry[GroupIndex] := g_trust_group_commanded_ry[GroupIndex] + TargetCart.RY;
                g_trust_group_commanded_rz[GroupIndex] := g_trust_group_commanded_rz[GroupIndex] + TargetCart.RZ;
            ELSE
                g_trust_group_commanded_x[GroupIndex] := TargetCart.X - (TRUST_GROUP_CoordOffset(CoordSystem := CoordSystem) - TRUST_GROUP_CoordOffset(CoordSystem := MC_COORD_SYSTEM#mcMCS));
                g_trust_group_commanded_y[GroupIndex] := TargetCart.Y - (TRUST_GROUP_CoordOffset(CoordSystem := CoordSystem) - TRUST_GROUP_CoordOffset(CoordSystem := MC_COORD_SYSTEM#mcMCS));
                g_trust_group_commanded_z[GroupIndex] := TargetCart.Z - (TRUST_GROUP_CoordOffset(CoordSystem := CoordSystem) - TRUST_GROUP_CoordOffset(CoordSystem := MC_COORD_SYSTEM#mcMCS));
                g_trust_group_commanded_rx[GroupIndex] := TargetCart.RX - (TRUST_GROUP_CoordOffset(CoordSystem := CoordSystem) - TRUST_GROUP_CoordOffset(CoordSystem := MC_COORD_SYSTEM#mcMCS));
                g_trust_group_commanded_ry[GroupIndex] := TargetCart.RY - (TRUST_GROUP_CoordOffset(CoordSystem := CoordSystem) - TRUST_GROUP_CoordOffset(CoordSystem := MC_COORD_SYSTEM#mcMCS));
                g_trust_group_commanded_rz[GroupIndex] := TargetCart.RZ - (TRUST_GROUP_CoordOffset(CoordSystem := CoordSystem) - TRUST_GROUP_CoordOffset(CoordSystem := MC_COORD_SYSTEM#mcMCS));
            END_IF;
            g_trust_group_actual_x[GroupIndex] := g_trust_group_commanded_x[GroupIndex];
            g_trust_group_actual_y[GroupIndex] := g_trust_group_commanded_y[GroupIndex];
            g_trust_group_actual_z[GroupIndex] := g_trust_group_commanded_z[GroupIndex];
            g_trust_group_actual_rx[GroupIndex] := g_trust_group_commanded_rx[GroupIndex];
            g_trust_group_actual_ry[GroupIndex] := g_trust_group_commanded_ry[GroupIndex];
            g_trust_group_actual_rz[GroupIndex] := g_trust_group_commanded_rz[GroupIndex];
            TRUST_GROUP_SetCommandInfo(
                Group := AxesGroup,
                CommandState := MC_COMMAND_STATE#mcAccepted,
                ElapsedDuration := TIME#0ms,
                RemainingDuration := TIME#0ms,
                RemainingDistance := REAL#0.0,
                Progress := REAL#1.0,
                InfoID := TRUST_GROUP_TransitionInfoID(Group := AxesGroup),
                WarningID := WORD#16#2006
            );
            Done := TRUE;
            CommandAccepted := TRUE;
        END_IF;
    END_IF;
ELSIF NOT Execute THEN
    Done := FALSE;
    Busy := FALSE;
    CommandAccepted := FALSE;
    CommandAborted := FALSE;
    CommandID := UINT#0;
    Error := FALSE;
    ErrorID := WORD#16#0000;
END_IF;
LastExecute := Execute;
END_FUNCTION_BLOCK

FUNCTION_BLOCK MC_MoveLinearAbsolute
VAR_IN_OUT
    AxesGroup : AXES_GROUP_REF;
END_VAR
VAR_INPUT
    Execute : BOOL;
    Position : MC_POS_REF;
    Velocity : REAL;
    Acceleration : REAL;
    Deceleration : REAL;
    Jerk : REAL;
    CoordSystem : MC_COORD_SYSTEM;
    BufferMode : MC_BUFFER_MODE;
    TransitionVelocity : MC_TRANSITION_VELOCITY;
    TransitionMode : MC_TRANSITION_MODE;
    TransitionParameter : MC_TRANSITION_PARAMETER;
    OrientationMode : MC_ORIENTATION_MODE;
END_VAR
VAR_OUTPUT
    Done : BOOL;
    Busy : BOOL;
    Active : BOOL;
    CommandAccepted : BOOL;
    CommandAborted : BOOL;
    Error : BOOL;
    ErrorID : WORD;
    CommandID : MC_COMMAND_ID;
END_VAR
VAR
    LastExecute : BOOL;
    PendingComplete : BOOL;
    AcceptedCommandID : MC_COMMAND_ID;
    TargetCartPos : MC_CART_POS_REF;
    TargetCart : MC_CART_REF;
END_VAR
IF Execute AND (NOT LastExecute) THEN
    Done := FALSE;
    Busy := FALSE;
    Active := FALSE;
    CommandAccepted := FALSE;
    CommandAborted := FALSE;
    Error := FALSE;
    ErrorID := WORD#16#0000;
    CommandID := UINT#0;
    IF NOT TRUST_GROUP_GroupReadyForMotion(Group := AxesGroup) THEN
        Error := TRUE;
        ErrorID := WORD#16#0103;
    ELSIF NOT TRUST_GROUP_MotionCoordSystemSupported(CoordSystem := CoordSystem) THEN
        Error := TRUE;
        ErrorID := WORD#16#0500;
    ELSIF NOT TRUST_GROUP_BufferModeSupported(BufferMode := BufferMode) THEN
        Error := TRUE;
        ErrorID := WORD#16#0500;
    ELSIF NOT TRUST_GROUP_TransitionModeSupported(TransitionMode := TransitionMode) THEN
        Error := TRUE;
        ErrorID := WORD#16#0500;
    ELSIF NOT TRUST_GROUP_TransitionVelocitySupported(TransitionVelocity := TransitionVelocity) THEN
        Error := TRUE;
        ErrorID := WORD#16#0500;
    ELSIF (Velocity < REAL#0.0) OR (Acceleration <= REAL#0.0) OR (Deceleration <= REAL#0.0) OR (Jerk <= REAL#0.0) THEN
        Error := TRUE;
        ErrorID := WORD#16#0001;
    ELSE
        AcceptedCommandID := TRUST_GROUP_RequestCommand(Group := AxesGroup, CoordSystem := CoordSystem);
        IF AcceptedCommandID = UINT#0 THEN
            Error := TRUE;
            ErrorID := WORD#16#0103;
        ELSE
            TargetCartPos := Position.C;
            TargetCart := TargetCartPos.Tcp;
            g_trust_group_commanded_x[TRUST_GROUP_GroupIndex(Group := AxesGroup)] := TargetCart.X - (TRUST_GROUP_CoordOffset(CoordSystem := CoordSystem) - TRUST_GROUP_CoordOffset(CoordSystem := MC_COORD_SYSTEM#mcMCS));
            g_trust_group_commanded_y[TRUST_GROUP_GroupIndex(Group := AxesGroup)] := TargetCart.Y - (TRUST_GROUP_CoordOffset(CoordSystem := CoordSystem) - TRUST_GROUP_CoordOffset(CoordSystem := MC_COORD_SYSTEM#mcMCS));
            g_trust_group_commanded_z[TRUST_GROUP_GroupIndex(Group := AxesGroup)] := TargetCart.Z - (TRUST_GROUP_CoordOffset(CoordSystem := CoordSystem) - TRUST_GROUP_CoordOffset(CoordSystem := MC_COORD_SYSTEM#mcMCS));
            g_trust_group_commanded_rx[TRUST_GROUP_GroupIndex(Group := AxesGroup)] := TargetCart.RX - (TRUST_GROUP_CoordOffset(CoordSystem := CoordSystem) - TRUST_GROUP_CoordOffset(CoordSystem := MC_COORD_SYSTEM#mcMCS));
            g_trust_group_commanded_ry[TRUST_GROUP_GroupIndex(Group := AxesGroup)] := TargetCart.RY - (TRUST_GROUP_CoordOffset(CoordSystem := CoordSystem) - TRUST_GROUP_CoordOffset(CoordSystem := MC_COORD_SYSTEM#mcMCS));
            g_trust_group_commanded_rz[TRUST_GROUP_GroupIndex(Group := AxesGroup)] := TargetCart.RZ - (TRUST_GROUP_CoordOffset(CoordSystem := CoordSystem) - TRUST_GROUP_CoordOffset(CoordSystem := MC_COORD_SYSTEM#mcMCS));
            g_trust_group_actual_x[TRUST_GROUP_GroupIndex(Group := AxesGroup)] := g_trust_group_commanded_x[TRUST_GROUP_GroupIndex(Group := AxesGroup)];
            g_trust_group_actual_y[TRUST_GROUP_GroupIndex(Group := AxesGroup)] := g_trust_group_commanded_y[TRUST_GROUP_GroupIndex(Group := AxesGroup)];
            g_trust_group_actual_z[TRUST_GROUP_GroupIndex(Group := AxesGroup)] := g_trust_group_commanded_z[TRUST_GROUP_GroupIndex(Group := AxesGroup)];
            g_trust_group_actual_rx[TRUST_GROUP_GroupIndex(Group := AxesGroup)] := g_trust_group_commanded_rx[TRUST_GROUP_GroupIndex(Group := AxesGroup)];
            g_trust_group_actual_ry[TRUST_GROUP_GroupIndex(Group := AxesGroup)] := g_trust_group_commanded_ry[TRUST_GROUP_GroupIndex(Group := AxesGroup)];
            g_trust_group_actual_rz[TRUST_GROUP_GroupIndex(Group := AxesGroup)] := g_trust_group_commanded_rz[TRUST_GROUP_GroupIndex(Group := AxesGroup)];
            TRUST_GROUP_StoreDynamics(Group := AxesGroup, Velocity := Velocity, Acceleration := Acceleration, Deceleration := Deceleration, Jerk := Jerk);
            g_trust_group_status[TRUST_GROUP_GroupIndex(Group := AxesGroup)] := MC_GROUP_STATUS#mcGroupMoving;
            TRUST_GROUP_SetMotionFlags(Group := AxesGroup, Tracking := FALSE, InSync := TRUE, InPosition := FALSE, ConstantVelocity := FALSE, Accelerating := TRUE, Decelerating := FALSE);
            TRUST_GROUP_SetCommandInfo(
                Group := AxesGroup,
                CommandState := MC_COMMAND_STATE#mcAccepted,
                ElapsedDuration := TIME#0ms,
                RemainingDuration := TIME#500ms,
                RemainingDistance := TargetCart.X + TargetCart.Y + TargetCart.Z,
                Progress := REAL#0.0,
                InfoID := TRUST_GROUP_TransitionInfoID(Group := AxesGroup),
                WarningID := WORD#16#2101
            );
            Busy := TRUE;
            Active := TRUE;
            CommandAccepted := TRUE;
            CommandID := AcceptedCommandID;
            PendingComplete := TRUE;
        END_IF;
    END_IF;
ELSIF Execute AND PendingComplete THEN
    Done := TRUE;
    Busy := FALSE;
    Active := FALSE;
    CommandAccepted := FALSE;
    CommandAborted := FALSE;
    Error := FALSE;
    ErrorID := WORD#16#0000;
    CommandID := AcceptedCommandID;
    g_trust_group_status[TRUST_GROUP_GroupIndex(Group := AxesGroup)] := MC_GROUP_STATUS#mcGroupStandby;
    TRUST_GROUP_SetMotionFlags(Group := AxesGroup, Tracking := FALSE, InSync := TRUE, InPosition := TRUE, ConstantVelocity := TRUE, Accelerating := FALSE, Decelerating := FALSE);
    TRUST_GROUP_SetCommandInfo(
        Group := AxesGroup,
        CommandState := MC_COMMAND_STATE#mcActive,
        ElapsedDuration := TIME#500ms,
        RemainingDuration := TIME#0ms,
        RemainingDistance := REAL#0.0,
        Progress := REAL#1.0,
        InfoID := TRUST_GROUP_TransitionInfoID(Group := AxesGroup),
        WarningID := WORD#16#2101
    );
    PendingComplete := FALSE;
ELSIF NOT Execute THEN
    Done := FALSE;
    Busy := FALSE;
    Active := FALSE;
    CommandAccepted := FALSE;
    CommandAborted := FALSE;
    Error := FALSE;
    ErrorID := WORD#16#0000;
    CommandID := UINT#0;
END_IF;
LastExecute := Execute;
END_FUNCTION_BLOCK

FUNCTION_BLOCK MC_MoveLinearRelative
VAR_IN_OUT
    AxesGroup : AXES_GROUP_REF;
END_VAR
VAR_INPUT
    Execute : BOOL;
    Distance : MC_DISTANCE_REF;
    Velocity : REAL;
    Acceleration : REAL;
    Deceleration : REAL;
    Jerk : REAL;
    CoordSystem : MC_COORD_SYSTEM;
    BufferMode : MC_BUFFER_MODE;
    TransitionVelocity : MC_TRANSITION_VELOCITY;
    TransitionMode : MC_TRANSITION_MODE;
    TransitionParameter : MC_TRANSITION_PARAMETER;
    OrientationMode : MC_ORIENTATION_MODE;
END_VAR
VAR_OUTPUT
    Done : BOOL;
    Busy : BOOL;
    Active : BOOL;
    CommandAccepted : BOOL;
    CommandAborted : BOOL;
    Error : BOOL;
    ErrorID : WORD;
    CommandID : MC_COMMAND_ID;
END_VAR
VAR
    LastExecute : BOOL;
    PendingComplete : BOOL;
    AcceptedCommandID : MC_COMMAND_ID;
    TargetCartPos : MC_CART_POS_REF;
    TargetCart : MC_CART_REF;
END_VAR
IF Execute AND (NOT LastExecute) THEN
    Done := FALSE;
    Busy := FALSE;
    Active := FALSE;
    CommandAccepted := FALSE;
    CommandAborted := FALSE;
    Error := FALSE;
    ErrorID := WORD#16#0000;
    CommandID := UINT#0;
    IF NOT TRUST_GROUP_GroupReadyForMotion(Group := AxesGroup) THEN
        Error := TRUE;
        ErrorID := WORD#16#0103;
    ELSIF NOT TRUST_GROUP_MotionCoordSystemSupported(CoordSystem := CoordSystem) THEN
        Error := TRUE;
        ErrorID := WORD#16#0500;
    ELSIF NOT TRUST_GROUP_BufferModeSupported(BufferMode := BufferMode) THEN
        Error := TRUE;
        ErrorID := WORD#16#0500;
    ELSIF NOT TRUST_GROUP_TransitionModeSupported(TransitionMode := TransitionMode) THEN
        Error := TRUE;
        ErrorID := WORD#16#0500;
    ELSIF NOT TRUST_GROUP_TransitionVelocitySupported(TransitionVelocity := TransitionVelocity) THEN
        Error := TRUE;
        ErrorID := WORD#16#0500;
    ELSIF (Velocity < REAL#0.0) OR (Acceleration <= REAL#0.0) OR (Deceleration <= REAL#0.0) OR (Jerk <= REAL#0.0) THEN
        Error := TRUE;
        ErrorID := WORD#16#0001;
    ELSE
        AcceptedCommandID := TRUST_GROUP_RequestCommand(Group := AxesGroup, CoordSystem := CoordSystem);
        IF AcceptedCommandID = UINT#0 THEN
            Error := TRUE;
            ErrorID := WORD#16#0103;
        ELSE
            TargetCartPos := Distance.C;
            TargetCart := TargetCartPos.Tcp;
            g_trust_group_commanded_x[TRUST_GROUP_GroupIndex(Group := AxesGroup)] := g_trust_group_commanded_x[TRUST_GROUP_GroupIndex(Group := AxesGroup)] + TargetCart.X;
            g_trust_group_commanded_y[TRUST_GROUP_GroupIndex(Group := AxesGroup)] := g_trust_group_commanded_y[TRUST_GROUP_GroupIndex(Group := AxesGroup)] + TargetCart.Y;
            g_trust_group_commanded_z[TRUST_GROUP_GroupIndex(Group := AxesGroup)] := g_trust_group_commanded_z[TRUST_GROUP_GroupIndex(Group := AxesGroup)] + TargetCart.Z;
            g_trust_group_commanded_rx[TRUST_GROUP_GroupIndex(Group := AxesGroup)] := g_trust_group_commanded_rx[TRUST_GROUP_GroupIndex(Group := AxesGroup)] + TargetCart.RX;
            g_trust_group_commanded_ry[TRUST_GROUP_GroupIndex(Group := AxesGroup)] := g_trust_group_commanded_ry[TRUST_GROUP_GroupIndex(Group := AxesGroup)] + TargetCart.RY;
            g_trust_group_commanded_rz[TRUST_GROUP_GroupIndex(Group := AxesGroup)] := g_trust_group_commanded_rz[TRUST_GROUP_GroupIndex(Group := AxesGroup)] + TargetCart.RZ;
            g_trust_group_actual_x[TRUST_GROUP_GroupIndex(Group := AxesGroup)] := g_trust_group_commanded_x[TRUST_GROUP_GroupIndex(Group := AxesGroup)];
            g_trust_group_actual_y[TRUST_GROUP_GroupIndex(Group := AxesGroup)] := g_trust_group_commanded_y[TRUST_GROUP_GroupIndex(Group := AxesGroup)];
            g_trust_group_actual_z[TRUST_GROUP_GroupIndex(Group := AxesGroup)] := g_trust_group_commanded_z[TRUST_GROUP_GroupIndex(Group := AxesGroup)];
            g_trust_group_actual_rx[TRUST_GROUP_GroupIndex(Group := AxesGroup)] := g_trust_group_commanded_rx[TRUST_GROUP_GroupIndex(Group := AxesGroup)];
            g_trust_group_actual_ry[TRUST_GROUP_GroupIndex(Group := AxesGroup)] := g_trust_group_commanded_ry[TRUST_GROUP_GroupIndex(Group := AxesGroup)];
            g_trust_group_actual_rz[TRUST_GROUP_GroupIndex(Group := AxesGroup)] := g_trust_group_commanded_rz[TRUST_GROUP_GroupIndex(Group := AxesGroup)];
            TRUST_GROUP_StoreDynamics(Group := AxesGroup, Velocity := Velocity, Acceleration := Acceleration, Deceleration := Deceleration, Jerk := Jerk);
            g_trust_group_status[TRUST_GROUP_GroupIndex(Group := AxesGroup)] := MC_GROUP_STATUS#mcGroupMoving;
            TRUST_GROUP_SetMotionFlags(Group := AxesGroup, Tracking := FALSE, InSync := TRUE, InPosition := FALSE, ConstantVelocity := FALSE, Accelerating := TRUE, Decelerating := FALSE);
            TRUST_GROUP_SetCommandInfo(
                Group := AxesGroup,
                CommandState := MC_COMMAND_STATE#mcAccepted,
                ElapsedDuration := TIME#0ms,
                RemainingDuration := TIME#400ms,
                RemainingDistance := TargetCart.X + TargetCart.Y + TargetCart.Z,
                Progress := REAL#0.0,
                InfoID := TRUST_GROUP_TransitionInfoID(Group := AxesGroup),
                WarningID := WORD#16#2102
            );
            Busy := TRUE;
            Active := TRUE;
            CommandAccepted := TRUE;
            CommandID := AcceptedCommandID;
            PendingComplete := TRUE;
        END_IF;
    END_IF;
ELSIF Execute AND PendingComplete THEN
    Done := TRUE;
    Busy := FALSE;
    Active := FALSE;
    CommandAccepted := FALSE;
    CommandAborted := FALSE;
    Error := FALSE;
    ErrorID := WORD#16#0000;
    CommandID := AcceptedCommandID;
    g_trust_group_status[TRUST_GROUP_GroupIndex(Group := AxesGroup)] := MC_GROUP_STATUS#mcGroupStandby;
    TRUST_GROUP_SetMotionFlags(Group := AxesGroup, Tracking := FALSE, InSync := TRUE, InPosition := TRUE, ConstantVelocity := TRUE, Accelerating := FALSE, Decelerating := FALSE);
    TRUST_GROUP_SetCommandInfo(
        Group := AxesGroup,
        CommandState := MC_COMMAND_STATE#mcActive,
        ElapsedDuration := TIME#400ms,
        RemainingDuration := TIME#0ms,
        RemainingDistance := REAL#0.0,
        Progress := REAL#1.0,
        InfoID := TRUST_GROUP_TransitionInfoID(Group := AxesGroup),
        WarningID := WORD#16#2102
    );
    PendingComplete := FALSE;
ELSIF NOT Execute THEN
    Done := FALSE;
    Busy := FALSE;
    Active := FALSE;
    CommandAccepted := FALSE;
    CommandAborted := FALSE;
    Error := FALSE;
    ErrorID := WORD#16#0000;
    CommandID := UINT#0;
END_IF;
LastExecute := Execute;
END_FUNCTION_BLOCK

FUNCTION_BLOCK MC_MoveDirectAbsolute
VAR_IN_OUT
    AxesGroup : AXES_GROUP_REF;
END_VAR
VAR_INPUT
    Execute : BOOL;
    Position : MC_POS_REF;
    Velocity : REAL;
    Acceleration : REAL;
    Deceleration : REAL;
    Jerk : REAL;
    CoordSystem : MC_COORD_SYSTEM;
    BufferMode : MC_BUFFER_MODE;
    TransitionVelocity : MC_TRANSITION_VELOCITY;
    TransitionMode : MC_TRANSITION_MODE;
    TransitionParameter : MC_TRANSITION_PARAMETER;
    OrientationMode : MC_ORIENTATION_MODE;
END_VAR
VAR_OUTPUT
    Done : BOOL;
    Busy : BOOL;
    Active : BOOL;
    CommandAccepted : BOOL;
    CommandAborted : BOOL;
    Error : BOOL;
    ErrorID : WORD;
    CommandID : MC_COMMAND_ID;
END_VAR
VAR
    LastExecute : BOOL;
    PendingComplete : BOOL;
    AcceptedCommandID : MC_COMMAND_ID;
    TargetCartPos : MC_CART_POS_REF;
    TargetCart : MC_CART_REF;
END_VAR
IF Execute AND (NOT LastExecute) THEN
    Done := FALSE;
    Busy := FALSE;
    Active := FALSE;
    CommandAccepted := FALSE;
    CommandAborted := FALSE;
    Error := FALSE;
    ErrorID := WORD#16#0000;
    CommandID := UINT#0;
    IF NOT TRUST_GROUP_GroupReadyForMotion(Group := AxesGroup) THEN
        Error := TRUE;
        ErrorID := WORD#16#0103;
    ELSIF NOT TRUST_GROUP_MotionCoordSystemSupported(CoordSystem := CoordSystem) THEN
        Error := TRUE;
        ErrorID := WORD#16#0500;
    ELSIF NOT TRUST_GROUP_BufferModeSupported(BufferMode := BufferMode) THEN
        Error := TRUE;
        ErrorID := WORD#16#0500;
    ELSIF NOT TRUST_GROUP_TransitionModeSupported(TransitionMode := TransitionMode) THEN
        Error := TRUE;
        ErrorID := WORD#16#0500;
    ELSIF NOT TRUST_GROUP_TransitionVelocitySupported(TransitionVelocity := TransitionVelocity) THEN
        Error := TRUE;
        ErrorID := WORD#16#0500;
    ELSIF (Velocity < REAL#0.0) OR (Acceleration <= REAL#0.0) OR (Deceleration <= REAL#0.0) OR (Jerk <= REAL#0.0) THEN
        Error := TRUE;
        ErrorID := WORD#16#0001;
    ELSE
        AcceptedCommandID := TRUST_GROUP_RequestCommand(Group := AxesGroup, CoordSystem := CoordSystem);
        IF AcceptedCommandID = UINT#0 THEN
            Error := TRUE;
            ErrorID := WORD#16#0103;
        ELSE
            TargetCartPos := Position.C;
            TargetCart := TargetCartPos.Tcp;
            g_trust_group_commanded_x[TRUST_GROUP_GroupIndex(Group := AxesGroup)] := TargetCart.X - (TRUST_GROUP_CoordOffset(CoordSystem := CoordSystem) - TRUST_GROUP_CoordOffset(CoordSystem := MC_COORD_SYSTEM#mcMCS));
            g_trust_group_commanded_y[TRUST_GROUP_GroupIndex(Group := AxesGroup)] := TargetCart.Y - (TRUST_GROUP_CoordOffset(CoordSystem := CoordSystem) - TRUST_GROUP_CoordOffset(CoordSystem := MC_COORD_SYSTEM#mcMCS));
            g_trust_group_commanded_z[TRUST_GROUP_GroupIndex(Group := AxesGroup)] := TargetCart.Z - (TRUST_GROUP_CoordOffset(CoordSystem := CoordSystem) - TRUST_GROUP_CoordOffset(CoordSystem := MC_COORD_SYSTEM#mcMCS));
            g_trust_group_commanded_rx[TRUST_GROUP_GroupIndex(Group := AxesGroup)] := TargetCart.RX - (TRUST_GROUP_CoordOffset(CoordSystem := CoordSystem) - TRUST_GROUP_CoordOffset(CoordSystem := MC_COORD_SYSTEM#mcMCS));
            g_trust_group_commanded_ry[TRUST_GROUP_GroupIndex(Group := AxesGroup)] := TargetCart.RY - (TRUST_GROUP_CoordOffset(CoordSystem := CoordSystem) - TRUST_GROUP_CoordOffset(CoordSystem := MC_COORD_SYSTEM#mcMCS));
            g_trust_group_commanded_rz[TRUST_GROUP_GroupIndex(Group := AxesGroup)] := TargetCart.RZ - (TRUST_GROUP_CoordOffset(CoordSystem := CoordSystem) - TRUST_GROUP_CoordOffset(CoordSystem := MC_COORD_SYSTEM#mcMCS));
            g_trust_group_actual_x[TRUST_GROUP_GroupIndex(Group := AxesGroup)] := g_trust_group_commanded_x[TRUST_GROUP_GroupIndex(Group := AxesGroup)];
            g_trust_group_actual_y[TRUST_GROUP_GroupIndex(Group := AxesGroup)] := g_trust_group_commanded_y[TRUST_GROUP_GroupIndex(Group := AxesGroup)];
            g_trust_group_actual_z[TRUST_GROUP_GroupIndex(Group := AxesGroup)] := g_trust_group_commanded_z[TRUST_GROUP_GroupIndex(Group := AxesGroup)];
            g_trust_group_actual_rx[TRUST_GROUP_GroupIndex(Group := AxesGroup)] := g_trust_group_commanded_rx[TRUST_GROUP_GroupIndex(Group := AxesGroup)];
            g_trust_group_actual_ry[TRUST_GROUP_GroupIndex(Group := AxesGroup)] := g_trust_group_commanded_ry[TRUST_GROUP_GroupIndex(Group := AxesGroup)];
            g_trust_group_actual_rz[TRUST_GROUP_GroupIndex(Group := AxesGroup)] := g_trust_group_commanded_rz[TRUST_GROUP_GroupIndex(Group := AxesGroup)];
            TRUST_GROUP_StoreDynamics(Group := AxesGroup, Velocity := Velocity, Acceleration := Acceleration, Deceleration := Deceleration, Jerk := Jerk);
            g_trust_group_status[TRUST_GROUP_GroupIndex(Group := AxesGroup)] := MC_GROUP_STATUS#mcGroupMoving;
            TRUST_GROUP_SetMotionFlags(Group := AxesGroup, Tracking := FALSE, InSync := TRUE, InPosition := FALSE, ConstantVelocity := FALSE, Accelerating := TRUE, Decelerating := FALSE);
            TRUST_GROUP_SetCommandInfo(
                Group := AxesGroup,
                CommandState := MC_COMMAND_STATE#mcAccepted,
                ElapsedDuration := TIME#0ms,
                RemainingDuration := TIME#350ms,
                RemainingDistance := TargetCart.X + TargetCart.Y + TargetCart.Z,
                Progress := REAL#0.0,
                InfoID := TRUST_GROUP_TransitionInfoID(Group := AxesGroup),
                WarningID := WORD#16#2201
            );
            Busy := TRUE;
            Active := TRUE;
            CommandAccepted := TRUE;
            CommandID := AcceptedCommandID;
            PendingComplete := TRUE;
        END_IF;
    END_IF;
ELSIF Execute AND PendingComplete THEN
    Done := TRUE;
    Busy := FALSE;
    Active := FALSE;
    CommandAccepted := FALSE;
    CommandAborted := FALSE;
    Error := FALSE;
    ErrorID := WORD#16#0000;
    CommandID := AcceptedCommandID;
    g_trust_group_status[TRUST_GROUP_GroupIndex(Group := AxesGroup)] := MC_GROUP_STATUS#mcGroupStandby;
    TRUST_GROUP_SetMotionFlags(Group := AxesGroup, Tracking := FALSE, InSync := TRUE, InPosition := TRUE, ConstantVelocity := TRUE, Accelerating := FALSE, Decelerating := FALSE);
    TRUST_GROUP_SetCommandInfo(
        Group := AxesGroup,
        CommandState := MC_COMMAND_STATE#mcActive,
        ElapsedDuration := TIME#350ms,
        RemainingDuration := TIME#0ms,
        RemainingDistance := REAL#0.0,
        Progress := REAL#1.0,
        InfoID := TRUST_GROUP_TransitionInfoID(Group := AxesGroup),
        WarningID := WORD#16#2201
    );
    PendingComplete := FALSE;
ELSIF NOT Execute THEN
    Done := FALSE;
    Busy := FALSE;
    Active := FALSE;
    CommandAccepted := FALSE;
    CommandAborted := FALSE;
    Error := FALSE;
    ErrorID := WORD#16#0000;
    CommandID := UINT#0;
END_IF;
LastExecute := Execute;
END_FUNCTION_BLOCK

FUNCTION_BLOCK MC_MoveDirectRelative
VAR_IN_OUT
    AxesGroup : AXES_GROUP_REF;
END_VAR
VAR_INPUT
    Execute : BOOL;
    Distance : MC_DISTANCE_REF;
    Velocity : REAL;
    Acceleration : REAL;
    Deceleration : REAL;
    Jerk : REAL;
    CoordSystem : MC_COORD_SYSTEM;
    BufferMode : MC_BUFFER_MODE;
    TransitionVelocity : MC_TRANSITION_VELOCITY;
    TransitionMode : MC_TRANSITION_MODE;
    TransitionParameter : MC_TRANSITION_PARAMETER;
    OrientationMode : MC_ORIENTATION_MODE;
END_VAR
VAR_OUTPUT
    Done : BOOL;
    Busy : BOOL;
    Active : BOOL;
    CommandAccepted : BOOL;
    CommandAborted : BOOL;
    Error : BOOL;
    ErrorID : WORD;
    CommandID : MC_COMMAND_ID;
END_VAR
VAR
    LastExecute : BOOL;
    PendingComplete : BOOL;
    AcceptedCommandID : MC_COMMAND_ID;
    TargetCartPos : MC_CART_POS_REF;
    TargetCart : MC_CART_REF;
END_VAR
IF Execute AND (NOT LastExecute) THEN
    Done := FALSE;
    Busy := FALSE;
    Active := FALSE;
    CommandAccepted := FALSE;
    CommandAborted := FALSE;
    Error := FALSE;
    ErrorID := WORD#16#0000;
    CommandID := UINT#0;
    IF NOT TRUST_GROUP_GroupReadyForMotion(Group := AxesGroup) THEN
        Error := TRUE;
        ErrorID := WORD#16#0103;
    ELSIF NOT TRUST_GROUP_MotionCoordSystemSupported(CoordSystem := CoordSystem) THEN
        Error := TRUE;
        ErrorID := WORD#16#0500;
    ELSIF NOT TRUST_GROUP_BufferModeSupported(BufferMode := BufferMode) THEN
        Error := TRUE;
        ErrorID := WORD#16#0500;
    ELSIF NOT TRUST_GROUP_TransitionModeSupported(TransitionMode := TransitionMode) THEN
        Error := TRUE;
        ErrorID := WORD#16#0500;
    ELSIF NOT TRUST_GROUP_TransitionVelocitySupported(TransitionVelocity := TransitionVelocity) THEN
        Error := TRUE;
        ErrorID := WORD#16#0500;
    ELSIF (Velocity < REAL#0.0) OR (Acceleration <= REAL#0.0) OR (Deceleration <= REAL#0.0) OR (Jerk <= REAL#0.0) THEN
        Error := TRUE;
        ErrorID := WORD#16#0001;
    ELSE
        AcceptedCommandID := TRUST_GROUP_RequestCommand(Group := AxesGroup, CoordSystem := CoordSystem);
        IF AcceptedCommandID = UINT#0 THEN
            Error := TRUE;
            ErrorID := WORD#16#0103;
        ELSE
            TargetCartPos := Distance.C;
            TargetCart := TargetCartPos.Tcp;
            g_trust_group_commanded_x[TRUST_GROUP_GroupIndex(Group := AxesGroup)] := g_trust_group_commanded_x[TRUST_GROUP_GroupIndex(Group := AxesGroup)] + TargetCart.X;
            g_trust_group_commanded_y[TRUST_GROUP_GroupIndex(Group := AxesGroup)] := g_trust_group_commanded_y[TRUST_GROUP_GroupIndex(Group := AxesGroup)] + TargetCart.Y;
            g_trust_group_commanded_z[TRUST_GROUP_GroupIndex(Group := AxesGroup)] := g_trust_group_commanded_z[TRUST_GROUP_GroupIndex(Group := AxesGroup)] + TargetCart.Z;
            g_trust_group_commanded_rx[TRUST_GROUP_GroupIndex(Group := AxesGroup)] := g_trust_group_commanded_rx[TRUST_GROUP_GroupIndex(Group := AxesGroup)] + TargetCart.RX;
            g_trust_group_commanded_ry[TRUST_GROUP_GroupIndex(Group := AxesGroup)] := g_trust_group_commanded_ry[TRUST_GROUP_GroupIndex(Group := AxesGroup)] + TargetCart.RY;
            g_trust_group_commanded_rz[TRUST_GROUP_GroupIndex(Group := AxesGroup)] := g_trust_group_commanded_rz[TRUST_GROUP_GroupIndex(Group := AxesGroup)] + TargetCart.RZ;
            g_trust_group_actual_x[TRUST_GROUP_GroupIndex(Group := AxesGroup)] := g_trust_group_commanded_x[TRUST_GROUP_GroupIndex(Group := AxesGroup)];
            g_trust_group_actual_y[TRUST_GROUP_GroupIndex(Group := AxesGroup)] := g_trust_group_commanded_y[TRUST_GROUP_GroupIndex(Group := AxesGroup)];
            g_trust_group_actual_z[TRUST_GROUP_GroupIndex(Group := AxesGroup)] := g_trust_group_commanded_z[TRUST_GROUP_GroupIndex(Group := AxesGroup)];
            g_trust_group_actual_rx[TRUST_GROUP_GroupIndex(Group := AxesGroup)] := g_trust_group_commanded_rx[TRUST_GROUP_GroupIndex(Group := AxesGroup)];
            g_trust_group_actual_ry[TRUST_GROUP_GroupIndex(Group := AxesGroup)] := g_trust_group_commanded_ry[TRUST_GROUP_GroupIndex(Group := AxesGroup)];
            g_trust_group_actual_rz[TRUST_GROUP_GroupIndex(Group := AxesGroup)] := g_trust_group_commanded_rz[TRUST_GROUP_GroupIndex(Group := AxesGroup)];
            TRUST_GROUP_StoreDynamics(Group := AxesGroup, Velocity := Velocity, Acceleration := Acceleration, Deceleration := Deceleration, Jerk := Jerk);
            g_trust_group_status[TRUST_GROUP_GroupIndex(Group := AxesGroup)] := MC_GROUP_STATUS#mcGroupMoving;
            TRUST_GROUP_SetMotionFlags(Group := AxesGroup, Tracking := FALSE, InSync := TRUE, InPosition := FALSE, ConstantVelocity := FALSE, Accelerating := TRUE, Decelerating := FALSE);
            TRUST_GROUP_SetCommandInfo(
                Group := AxesGroup,
                CommandState := MC_COMMAND_STATE#mcAccepted,
                ElapsedDuration := TIME#0ms,
                RemainingDuration := TIME#300ms,
                RemainingDistance := TargetCart.X + TargetCart.Y + TargetCart.Z,
                Progress := REAL#0.0,
                InfoID := TRUST_GROUP_TransitionInfoID(Group := AxesGroup),
                WarningID := WORD#16#2202
            );
            Busy := TRUE;
            Active := TRUE;
            CommandAccepted := TRUE;
            CommandID := AcceptedCommandID;
            PendingComplete := TRUE;
        END_IF;
    END_IF;
ELSIF Execute AND PendingComplete THEN
    Done := TRUE;
    Busy := FALSE;
    Active := FALSE;
    CommandAccepted := FALSE;
    CommandAborted := FALSE;
    Error := FALSE;
    ErrorID := WORD#16#0000;
    CommandID := AcceptedCommandID;
    g_trust_group_status[TRUST_GROUP_GroupIndex(Group := AxesGroup)] := MC_GROUP_STATUS#mcGroupStandby;
    TRUST_GROUP_SetMotionFlags(Group := AxesGroup, Tracking := FALSE, InSync := TRUE, InPosition := TRUE, ConstantVelocity := TRUE, Accelerating := FALSE, Decelerating := FALSE);
    TRUST_GROUP_SetCommandInfo(
        Group := AxesGroup,
        CommandState := MC_COMMAND_STATE#mcActive,
        ElapsedDuration := TIME#300ms,
        RemainingDuration := TIME#0ms,
        RemainingDistance := REAL#0.0,
        Progress := REAL#1.0,
        InfoID := TRUST_GROUP_TransitionInfoID(Group := AxesGroup),
        WarningID := WORD#16#2202
    );
    PendingComplete := FALSE;
ELSIF NOT Execute THEN
    Done := FALSE;
    Busy := FALSE;
    Active := FALSE;
    CommandAccepted := FALSE;
    CommandAborted := FALSE;
    Error := FALSE;
    ErrorID := WORD#16#0000;
    CommandID := UINT#0;
END_IF;
LastExecute := Execute;
END_FUNCTION_BLOCK

FUNCTION_BLOCK MC_GroupHome
VAR_IN_OUT
    AxesGroup : AXES_GROUP_REF;
END_VAR
VAR_INPUT
    Execute : BOOL;
    Position : MC_POS_REF;
    CoordSystem : MC_COORD_SYSTEM;
    BufferMode : MC_BUFFER_MODE;
END_VAR
VAR_OUTPUT
    Done : BOOL;
    Busy : BOOL;
    Active : BOOL;
    CommandAccepted : BOOL;
    CommandAborted : BOOL;
    Error : BOOL;
    ErrorID : WORD;
END_VAR
VAR
    LastExecute : BOOL;
    PendingComplete : BOOL;
    TargetCartPos : MC_CART_POS_REF;
    TargetCart : MC_CART_REF;
END_VAR
IF Execute AND (NOT LastExecute) THEN
    Done := FALSE;
    Busy := FALSE;
    Active := FALSE;
    CommandAccepted := FALSE;
    CommandAborted := FALSE;
    Error := FALSE;
    ErrorID := WORD#16#0000;
    IF NOT TRUST_GROUP_GroupReadyForMotion(Group := AxesGroup) THEN
        Error := TRUE;
        ErrorID := WORD#16#0103;
    ELSIF NOT TRUST_GROUP_MotionCoordSystemSupported(CoordSystem := CoordSystem) THEN
        Error := TRUE;
        ErrorID := WORD#16#0500;
    ELSIF NOT TRUST_GROUP_BufferModeSupported(BufferMode := BufferMode) THEN
        Error := TRUE;
        ErrorID := WORD#16#0500;
    ELSE
        g_trust_group_status[TRUST_GROUP_GroupIndex(Group := AxesGroup)] := MC_GROUP_STATUS#mcGroupHoming;
        TargetCartPos := Position.C;
        TargetCart := TargetCartPos.Tcp;
        g_trust_group_commanded_x[TRUST_GROUP_GroupIndex(Group := AxesGroup)] := TargetCart.X - (TRUST_GROUP_CoordOffset(CoordSystem := CoordSystem) - TRUST_GROUP_CoordOffset(CoordSystem := MC_COORD_SYSTEM#mcMCS));
        g_trust_group_commanded_y[TRUST_GROUP_GroupIndex(Group := AxesGroup)] := TargetCart.Y - (TRUST_GROUP_CoordOffset(CoordSystem := CoordSystem) - TRUST_GROUP_CoordOffset(CoordSystem := MC_COORD_SYSTEM#mcMCS));
        g_trust_group_commanded_z[TRUST_GROUP_GroupIndex(Group := AxesGroup)] := TargetCart.Z - (TRUST_GROUP_CoordOffset(CoordSystem := CoordSystem) - TRUST_GROUP_CoordOffset(CoordSystem := MC_COORD_SYSTEM#mcMCS));
        g_trust_group_commanded_rx[TRUST_GROUP_GroupIndex(Group := AxesGroup)] := TargetCart.RX - (TRUST_GROUP_CoordOffset(CoordSystem := CoordSystem) - TRUST_GROUP_CoordOffset(CoordSystem := MC_COORD_SYSTEM#mcMCS));
        g_trust_group_commanded_ry[TRUST_GROUP_GroupIndex(Group := AxesGroup)] := TargetCart.RY - (TRUST_GROUP_CoordOffset(CoordSystem := CoordSystem) - TRUST_GROUP_CoordOffset(CoordSystem := MC_COORD_SYSTEM#mcMCS));
        g_trust_group_commanded_rz[TRUST_GROUP_GroupIndex(Group := AxesGroup)] := TargetCart.RZ - (TRUST_GROUP_CoordOffset(CoordSystem := CoordSystem) - TRUST_GROUP_CoordOffset(CoordSystem := MC_COORD_SYSTEM#mcMCS));
        g_trust_group_actual_x[TRUST_GROUP_GroupIndex(Group := AxesGroup)] := g_trust_group_commanded_x[TRUST_GROUP_GroupIndex(Group := AxesGroup)];
        g_trust_group_actual_y[TRUST_GROUP_GroupIndex(Group := AxesGroup)] := g_trust_group_commanded_y[TRUST_GROUP_GroupIndex(Group := AxesGroup)];
        g_trust_group_actual_z[TRUST_GROUP_GroupIndex(Group := AxesGroup)] := g_trust_group_commanded_z[TRUST_GROUP_GroupIndex(Group := AxesGroup)];
        g_trust_group_actual_rx[TRUST_GROUP_GroupIndex(Group := AxesGroup)] := g_trust_group_commanded_rx[TRUST_GROUP_GroupIndex(Group := AxesGroup)];
        g_trust_group_actual_ry[TRUST_GROUP_GroupIndex(Group := AxesGroup)] := g_trust_group_commanded_ry[TRUST_GROUP_GroupIndex(Group := AxesGroup)];
        g_trust_group_actual_rz[TRUST_GROUP_GroupIndex(Group := AxesGroup)] := g_trust_group_commanded_rz[TRUST_GROUP_GroupIndex(Group := AxesGroup)];
        TRUST_GROUP_SetCommandInfo(
            Group := AxesGroup,
            CommandState := MC_COMMAND_STATE#mcAccepted,
            ElapsedDuration := TIME#0ms,
            RemainingDuration := TIME#300ms,
            RemainingDistance := REAL#0.0,
            Progress := REAL#0.0,
            InfoID := TRUST_GROUP_TransitionInfoID(Group := AxesGroup),
            WarningID := WORD#16#2301
        );
        Busy := TRUE;
        Active := TRUE;
        CommandAccepted := TRUE;
        PendingComplete := TRUE;
    END_IF;
ELSIF Execute AND PendingComplete THEN
    Done := TRUE;
    Busy := FALSE;
    Active := FALSE;
    CommandAccepted := FALSE;
    CommandAborted := FALSE;
    Error := FALSE;
    ErrorID := WORD#16#0000;
    g_trust_group_status[TRUST_GROUP_GroupIndex(Group := AxesGroup)] := MC_GROUP_STATUS#mcGroupStandby;
    TRUST_GROUP_SetMotionFlags(Group := AxesGroup, Tracking := FALSE, InSync := TRUE, InPosition := TRUE, ConstantVelocity := TRUE, Accelerating := FALSE, Decelerating := FALSE);
    TRUST_GROUP_SetCommandInfo(
        Group := AxesGroup,
        CommandState := MC_COMMAND_STATE#mcActive,
        ElapsedDuration := TIME#300ms,
        RemainingDuration := TIME#0ms,
        RemainingDistance := REAL#0.0,
        Progress := REAL#1.0,
        InfoID := TRUST_GROUP_TransitionInfoID(Group := AxesGroup),
        WarningID := WORD#16#2301
    );
    PendingComplete := FALSE;
ELSIF NOT Execute THEN
    Done := FALSE;
    Busy := FALSE;
    Active := FALSE;
    CommandAccepted := FALSE;
    CommandAborted := FALSE;
    Error := FALSE;
    ErrorID := WORD#16#0000;
END_IF;
LastExecute := Execute;
END_FUNCTION_BLOCK

FUNCTION_BLOCK MC_GroupStop
VAR_IN_OUT
    AxesGroup : AXES_GROUP_REF;
END_VAR
VAR_INPUT
    Execute : BOOL;
    Deceleration : REAL;
    Jerk : REAL;
END_VAR
VAR_OUTPUT
    Done : BOOL;
    Busy : BOOL;
    CommandAborted : BOOL;
    Error : BOOL;
    ErrorID : WORD;
END_VAR
VAR
    LastExecute : BOOL;
    PendingComplete : BOOL;
    GroupIndex : UINT;
END_VAR
GroupIndex := TRUST_GROUP_GroupIndex(Group := AxesGroup);
IF Execute AND (NOT LastExecute) THEN
    Done := FALSE;
    Busy := FALSE;
    CommandAborted := FALSE;
    Error := FALSE;
    ErrorID := WORD#16#0000;
    IF g_trust_group_status[GroupIndex] = MC_GROUP_STATUS#mcGroupDisabled THEN
        Error := TRUE;
        ErrorID := WORD#16#0103;
    ELSIF (Deceleration < REAL#0.0) OR (Jerk < REAL#0.0) THEN
        Error := TRUE;
        ErrorID := WORD#16#0001;
    ELSE
        g_trust_group_status[GroupIndex] := MC_GROUP_STATUS#mcGroupStopping;
        g_trust_group_stop_hold[GroupIndex] := TRUE;
        g_trust_group_commanded_path_velocity[GroupIndex] := REAL#0.0;
        g_trust_group_actual_path_velocity[GroupIndex] := REAL#0.0;
        g_trust_group_tracking[GroupIndex] := FALSE;
        g_trust_group_in_sync[GroupIndex] := FALSE;
        Busy := TRUE;
        PendingComplete := TRUE;
    END_IF;
ELSIF Execute AND PendingComplete THEN
    Done := TRUE;
    Busy := FALSE;
    CommandAborted := FALSE;
    Error := FALSE;
    ErrorID := WORD#16#0000;
    PendingComplete := FALSE;
ELSIF NOT Execute THEN
    IF Done THEN
        g_trust_group_status[GroupIndex] := MC_GROUP_STATUS#mcGroupStandby;
        g_trust_group_stop_hold[GroupIndex] := FALSE;
        g_trust_group_in_position[GroupIndex] := TRUE;
        g_trust_group_constant_velocity[GroupIndex] := TRUE;
        g_trust_group_accelerating[GroupIndex] := FALSE;
        g_trust_group_decelerating[GroupIndex] := FALSE;
    END_IF;
    Done := FALSE;
    Busy := FALSE;
    CommandAborted := FALSE;
    Error := FALSE;
    ErrorID := WORD#16#0000;
END_IF;
LastExecute := Execute;
END_FUNCTION_BLOCK

FUNCTION_BLOCK MC_GroupHalt
VAR_IN_OUT
    AxesGroup : AXES_GROUP_REF;
END_VAR
VAR_INPUT
    Execute : BOOL;
    Deceleration : REAL;
    Jerk : REAL;
    BufferMode : MC_BUFFER_MODE;
END_VAR
VAR_OUTPUT
    Done : BOOL;
    Busy : BOOL;
    Active : BOOL;
    CommandAccepted : BOOL;
    CommandID : MC_COMMAND_ID;
    CommandAborted : BOOL;
    Error : BOOL;
    ErrorID : WORD;
END_VAR
VAR
    LastExecute : BOOL;
    PendingComplete : BOOL;
    AcceptedCommandID : MC_COMMAND_ID;
    GroupIndex : UINT;
END_VAR
GroupIndex := TRUST_GROUP_GroupIndex(Group := AxesGroup);
IF Execute AND (NOT LastExecute) THEN
    Done := FALSE;
    Busy := FALSE;
    Active := FALSE;
    CommandAccepted := FALSE;
    CommandID := UINT#0;
    CommandAborted := FALSE;
    Error := FALSE;
    ErrorID := WORD#16#0000;
    IF NOT TRUST_GROUP_GroupReadyForMotion(Group := AxesGroup) THEN
        Error := TRUE;
        ErrorID := WORD#16#0103;
    ELSIF NOT TRUST_GROUP_BufferModeSupported(BufferMode := BufferMode) THEN
        Error := TRUE;
        ErrorID := WORD#16#0500;
    ELSIF (Deceleration < REAL#0.0) OR (Jerk < REAL#0.0) THEN
        Error := TRUE;
        ErrorID := WORD#16#0001;
    ELSE
        AcceptedCommandID := TRUST_GROUP_RequestCommand(Group := AxesGroup, CoordSystem := g_trust_group_active_coord_system[GroupIndex]);
        IF AcceptedCommandID = UINT#0 THEN
            Error := TRUE;
            ErrorID := WORD#16#0103;
        ELSE
            Busy := TRUE;
            Active := TRUE;
            CommandAccepted := TRUE;
            CommandID := AcceptedCommandID;
            g_trust_group_commanded_path_velocity[GroupIndex] := REAL#0.0;
            g_trust_group_actual_path_velocity[GroupIndex] := REAL#0.0;
            TRUST_GROUP_SetCommandInfo(
                Group := AxesGroup,
                CommandState := MC_COMMAND_STATE#mcAccepted,
                ElapsedDuration := TIME#0ms,
                RemainingDuration := TIME#200ms,
                RemainingDistance := REAL#0.0,
                Progress := REAL#0.0,
                InfoID := TRUST_GROUP_TransitionInfoID(Group := AxesGroup),
                WarningID := WORD#16#2302
            );
            PendingComplete := TRUE;
        END_IF;
    END_IF;
ELSIF Execute AND PendingComplete THEN
    Done := TRUE;
    Busy := FALSE;
    Active := FALSE;
    CommandAccepted := FALSE;
    CommandID := AcceptedCommandID;
    CommandAborted := FALSE;
    Error := FALSE;
    ErrorID := WORD#16#0000;
    IF (g_trust_group_active_coord_system[GroupIndex] = MC_COORD_SYSTEM#mcPCS) AND g_trust_group_tracking[GroupIndex] THEN
        g_trust_group_status[GroupIndex] := MC_GROUP_STATUS#mcGroupMoving;
    ELSE
        g_trust_group_status[GroupIndex] := MC_GROUP_STATUS#mcGroupStandby;
    END_IF;
    TRUST_GROUP_SetCommandInfo(
        Group := AxesGroup,
        CommandState := MC_COMMAND_STATE#mcActive,
        ElapsedDuration := TIME#200ms,
        RemainingDuration := TIME#0ms,
        RemainingDistance := REAL#0.0,
        Progress := REAL#1.0,
        InfoID := TRUST_GROUP_TransitionInfoID(Group := AxesGroup),
        WarningID := WORD#16#2302
    );
    PendingComplete := FALSE;
ELSIF NOT Execute THEN
    Done := FALSE;
    Busy := FALSE;
    Active := FALSE;
    CommandAccepted := FALSE;
    CommandID := UINT#0;
    CommandAborted := FALSE;
    Error := FALSE;
    ErrorID := WORD#16#0000;
END_IF;
LastExecute := Execute;
END_FUNCTION_BLOCK

FUNCTION_BLOCK MC_GroupWaitTime
VAR_IN_OUT
    AxesGroup : AXES_GROUP_REF;
END_VAR
VAR_INPUT
    Execute : BOOL;
    Duration : TIME;
    BufferMode : MC_BUFFER_MODE;
END_VAR
VAR_OUTPUT
    Done : BOOL;
    Busy : BOOL;
    Active : BOOL;
    CommandAborted : BOOL;
    Error : BOOL;
    ErrorID : WORD;
    CommandID : MC_COMMAND_ID;
END_VAR
VAR
    LastExecute : BOOL;
    PendingComplete : BOOL;
    AcceptedCommandID : MC_COMMAND_ID;
END_VAR
IF Execute AND (NOT LastExecute) THEN
    Done := FALSE;
    Busy := FALSE;
    Active := FALSE;
    CommandAborted := FALSE;
    Error := FALSE;
    ErrorID := WORD#16#0000;
    CommandID := UINT#0;
    IF NOT TRUST_GROUP_GroupReadyForMotion(Group := AxesGroup) THEN
        Error := TRUE;
        ErrorID := WORD#16#0103;
    ELSIF NOT TRUST_GROUP_BufferModeSupported(BufferMode := BufferMode) THEN
        Error := TRUE;
        ErrorID := WORD#16#0500;
    ELSIF Duration <= TIME#0ms THEN
        Error := TRUE;
        ErrorID := WORD#16#0001;
    ELSE
        AcceptedCommandID := TRUST_GROUP_RequestCommand(Group := AxesGroup, CoordSystem := g_trust_group_active_coord_system[TRUST_GROUP_GroupIndex(Group := AxesGroup)]);
        IF AcceptedCommandID = UINT#0 THEN
            Error := TRUE;
            ErrorID := WORD#16#0103;
        ELSE
            Busy := TRUE;
            Active := TRUE;
            CommandID := AcceptedCommandID;
            TRUST_GROUP_SetCommandInfo(
                Group := AxesGroup,
                CommandState := MC_COMMAND_STATE#mcAccepted,
                ElapsedDuration := TIME#0ms,
                RemainingDuration := Duration,
                RemainingDistance := REAL#0.0,
                Progress := REAL#0.0,
                InfoID := TRUST_GROUP_TransitionInfoID(Group := AxesGroup),
                WarningID := WORD#16#2303
            );
            PendingComplete := TRUE;
        END_IF;
    END_IF;
ELSIF Execute AND PendingComplete THEN
    Done := TRUE;
    Busy := FALSE;
    Active := FALSE;
    CommandAborted := FALSE;
    Error := FALSE;
    ErrorID := WORD#16#0000;
    CommandID := AcceptedCommandID;
    TRUST_GROUP_SetCommandInfo(
        Group := AxesGroup,
        CommandState := MC_COMMAND_STATE#mcActive,
        ElapsedDuration := TIME#250ms,
        RemainingDuration := TIME#0ms,
        RemainingDistance := REAL#0.0,
        Progress := REAL#1.0,
        InfoID := TRUST_GROUP_TransitionInfoID(Group := AxesGroup),
        WarningID := WORD#16#2303
    );
    PendingComplete := FALSE;
ELSIF NOT Execute THEN
    Done := FALSE;
    Busy := FALSE;
    Active := FALSE;
    CommandAborted := FALSE;
    Error := FALSE;
    ErrorID := WORD#16#0000;
    CommandID := UINT#0;
END_IF;
LastExecute := Execute;
END_FUNCTION_BLOCK

FUNCTION_BLOCK MC_GroupSetOverride
VAR_IN_OUT
    AxesGroup : AXES_GROUP_REF;
END_VAR
VAR_INPUT
    Enable : BOOL;
    VelFactor : REAL;
    AccFactor : REAL;
    JerkFactor : REAL;
END_VAR
VAR_OUTPUT
    Enabled : BOOL;
    Busy : BOOL;
    Error : BOOL;
    ErrorID : WORD;
END_VAR
VAR
    GroupIndex : UINT;
END_VAR
GroupIndex := TRUST_GROUP_GroupIndex(Group := AxesGroup);
IF Enable THEN
    Busy := TRUE;
    Enabled := FALSE;
    Error := FALSE;
    ErrorID := WORD#16#0000;
    IF (VelFactor < REAL#0.0) OR (AccFactor < REAL#0.0) OR (JerkFactor < REAL#0.0) THEN
        Error := TRUE;
        ErrorID := WORD#16#0001;
    ELSIF (AccFactor = REAL#0.0) OR (JerkFactor = REAL#0.0) THEN
        Error := TRUE;
        ErrorID := WORD#16#0001;
    ELSIF (VelFactor > REAL#1.0) OR (AccFactor > REAL#1.0) OR (JerkFactor > REAL#1.0) THEN
        Error := TRUE;
        ErrorID := WORD#16#0500;
    ELSE
        g_trust_group_override_vel_factor[GroupIndex] := VelFactor;
        g_trust_group_override_acc_factor[GroupIndex] := AccFactor;
        g_trust_group_override_jerk_factor[GroupIndex] := JerkFactor;
        g_trust_group_actual_path_velocity[GroupIndex] := g_trust_group_commanded_path_velocity[GroupIndex] * VelFactor;
        g_trust_group_actual_path_acceleration[GroupIndex] := g_trust_group_commanded_path_acceleration[GroupIndex] * AccFactor;
        Enabled := TRUE;
    END_IF;
ELSE
    Enabled := FALSE;
    Busy := FALSE;
    Error := FALSE;
    ErrorID := WORD#16#0000;
END_IF;
END_FUNCTION_BLOCK

FUNCTION_BLOCK MC_TransformPosition
VAR_IN_OUT
    AxesGroup : AXES_GROUP_REF;
END_VAR
VAR_INPUT
    Enable : BOOL;
    InputPosition : MC_POS_REF;
    InputCoordSystem : MC_COORD_SYSTEM;
    OutputCoordSystem : MC_COORD_SYSTEM;
END_VAR
VAR_OUTPUT
    Valid : BOOL;
    Busy : BOOL;
    Error : BOOL;
    ErrorID : WORD;
    CommandID : MC_COMMAND_ID;
    OutputPosition : MC_POS_REF;
    SingularPosition : BOOL;
END_VAR
VAR
    GroupIndex : UINT;
    InputSlot : INT;
    OutputSlot : INT;
    BaseX : REAL;
    BaseY : REAL;
    BaseZ : REAL;
    InputCartPos : MC_CART_POS_REF;
    InputCart : MC_CART_REF;
    OutputCart : MC_CART_REF;
    OutputCartPos : MC_CART_POS_REF;
    OutputPos : MC_POS_REF;
END_VAR
GroupIndex := TRUST_GROUP_GroupIndex(Group := AxesGroup);
IF Enable THEN
    Busy := FALSE;
    Error := FALSE;
    ErrorID := WORD#16#0000;
    Valid := FALSE;
    SingularPosition := FALSE;
    CommandID := g_trust_group_last_command_id[GroupIndex];
    IF (NOT TRUST_GROUP_MotionCoordSystemSupported(CoordSystem := InputCoordSystem)) OR (NOT TRUST_GROUP_MotionCoordSystemSupported(CoordSystem := OutputCoordSystem)) THEN
        Error := TRUE;
        ErrorID := WORD#16#0500;
        OutputPosition := OutputPos;
    ELSE
        InputSlot := TRUST_GROUP_TransformSlot(CoordSystem := InputCoordSystem);
        OutputSlot := TRUST_GROUP_TransformSlot(CoordSystem := OutputCoordSystem);
        InputCartPos := InputPosition.C;
        InputCart := InputCartPos.Tcp;
        BaseX := InputCart.X - (TRUST_GROUP_CoordOffset(CoordSystem := InputCoordSystem) - TRUST_GROUP_CoordOffset(CoordSystem := MC_COORD_SYSTEM#mcMCS));
        BaseY := InputCart.Y - (TRUST_GROUP_CoordOffset(CoordSystem := InputCoordSystem) - TRUST_GROUP_CoordOffset(CoordSystem := MC_COORD_SYSTEM#mcMCS));
        BaseZ := InputCart.Z - (TRUST_GROUP_CoordOffset(CoordSystem := InputCoordSystem) - TRUST_GROUP_CoordOffset(CoordSystem := MC_COORD_SYSTEM#mcMCS));
        IF InputSlot <> INT#-1 THEN
            BaseX := BaseX - g_trust_group_coord_trans_x[GroupIndex, InputSlot];
            BaseY := BaseY - g_trust_group_coord_trans_y[GroupIndex, InputSlot];
            BaseZ := BaseZ - g_trust_group_coord_trans_z[GroupIndex, InputSlot];
        END_IF;
        OutputCart.X := BaseX + (TRUST_GROUP_CoordOffset(CoordSystem := OutputCoordSystem) - TRUST_GROUP_CoordOffset(CoordSystem := MC_COORD_SYSTEM#mcMCS));
        OutputCart.Y := BaseY + (TRUST_GROUP_CoordOffset(CoordSystem := OutputCoordSystem) - TRUST_GROUP_CoordOffset(CoordSystem := MC_COORD_SYSTEM#mcMCS));
        OutputCart.Z := BaseZ + (TRUST_GROUP_CoordOffset(CoordSystem := OutputCoordSystem) - TRUST_GROUP_CoordOffset(CoordSystem := MC_COORD_SYSTEM#mcMCS));
        IF OutputSlot <> INT#-1 THEN
            OutputCart.X := OutputCart.X + g_trust_group_coord_trans_x[GroupIndex, OutputSlot];
            OutputCart.Y := OutputCart.Y + g_trust_group_coord_trans_y[GroupIndex, OutputSlot];
            OutputCart.Z := OutputCart.Z + g_trust_group_coord_trans_z[GroupIndex, OutputSlot];
        END_IF;
        OutputCartPos.Tcp := OutputCart;
        OutputPos.C := OutputCartPos;
        OutputPosition := OutputPos;
        Valid := TRUE;
    END_IF;
ELSE
    Valid := FALSE;
    Busy := FALSE;
    Error := FALSE;
    ErrorID := WORD#16#0000;
    CommandID := UINT#0;
    OutputPosition := OutputPos;
    SingularPosition := FALSE;
END_IF;
END_FUNCTION_BLOCK

FUNCTION_BLOCK MC_GroupReadCommandInfo
VAR_IN_OUT
    AxesGroup : AXES_GROUP_REF;
END_VAR
VAR_INPUT
    Enable : BOOL;
    CommandID : MC_COMMAND_ID;
END_VAR
VAR_OUTPUT
    Valid : BOOL;
    Busy : BOOL;
    Error : BOOL;
    ErrorID : WORD;
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
GroupIndex := TRUST_GROUP_GroupIndex(Group := AxesGroup);
IF Enable THEN
    Busy := FALSE;
    Error := FALSE;
    ErrorID := WORD#16#0000;
    IF (CommandID <> UINT#0) AND (CommandID = g_trust_group_last_command_id[GroupIndex]) THEN
        Valid := TRUE;
        CommandState := g_trust_group_last_command_state[GroupIndex];
        ElapsedDuration := g_trust_group_command_elapsed_duration[GroupIndex];
        RemainingDuration := g_trust_group_command_remaining_duration[GroupIndex];
        RemainingDistance := g_trust_group_command_remaining_distance[GroupIndex];
        Progress := g_trust_group_command_progress[GroupIndex];
        InfoID := g_trust_group_command_info_id[GroupIndex];
        WarningID := g_trust_group_command_warning_id[GroupIndex];
    ELSE
        Valid := FALSE;
        ElapsedDuration := TIME#0ms;
        RemainingDuration := TIME#0ms;
        RemainingDistance := REAL#0.0;
        Progress := REAL#0.0;
        InfoID := WORD#16#0000;
        WarningID := WORD#16#0000;
    END_IF;
ELSE
    Valid := FALSE;
    Busy := FALSE;
    Error := FALSE;
    ErrorID := WORD#16#0000;
    ElapsedDuration := TIME#0ms;
    RemainingDuration := TIME#0ms;
    RemainingDistance := REAL#0.0;
    Progress := REAL#0.0;
    InfoID := WORD#16#0000;
    WarningID := WORD#16#0000;
END_IF;
END_FUNCTION_BLOCK

FUNCTION_BLOCK MC_GroupWriteReferenceDynamics
VAR_IN_OUT
    AxesGroup : AXES_GROUP_REF;
END_VAR
VAR_INPUT
    Execute : BOOL;
    Velocity : REAL;
    Acceleration : REAL;
    Deceleration : REAL;
    Jerk : REAL;
END_VAR
VAR_OUTPUT
    Done : BOOL;
    Busy : BOOL;
    Error : BOOL;
    ErrorID : WORD;
END_VAR
VAR
    LastExecute : BOOL;
    GroupIndex : UINT;
END_VAR
GroupIndex := TRUST_GROUP_GroupIndex(Group := AxesGroup);
IF Execute AND (NOT LastExecute) THEN
    Busy := FALSE;
    Done := FALSE;
    Error := FALSE;
    ErrorID := WORD#16#0000;
    IF (Velocity <= REAL#0.0) OR (Acceleration <= REAL#0.0) OR (Deceleration <= REAL#0.0) OR (Jerk <= REAL#0.0) THEN
        Error := TRUE;
        ErrorID := WORD#16#0001;
    ELSE
        g_trust_group_reference_velocity[GroupIndex] := Velocity;
        g_trust_group_reference_acceleration[GroupIndex] := Acceleration;
        g_trust_group_reference_deceleration[GroupIndex] := Deceleration;
        g_trust_group_reference_jerk[GroupIndex] := Jerk;
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

FUNCTION_BLOCK MC_GroupReadReferenceDynamics
VAR_IN_OUT
    AxesGroup : AXES_GROUP_REF;
END_VAR
VAR_INPUT
    Enable : BOOL;
END_VAR
VAR_OUTPUT
    Valid : BOOL;
    Busy : BOOL;
    Error : BOOL;
    ErrorID : WORD;
    Velocity : REAL;
    Acceleration : REAL;
    Deceleration : REAL;
    Jerk : REAL;
END_VAR
VAR
    GroupIndex : UINT;
END_VAR
GroupIndex := TRUST_GROUP_GroupIndex(Group := AxesGroup);
IF Enable THEN
    Valid := TRUE;
    Busy := FALSE;
    Error := FALSE;
    ErrorID := WORD#16#0000;
    Velocity := g_trust_group_reference_velocity[GroupIndex];
    Acceleration := g_trust_group_reference_acceleration[GroupIndex];
    Deceleration := g_trust_group_reference_deceleration[GroupIndex];
    Jerk := g_trust_group_reference_jerk[GroupIndex];
ELSE
    Valid := FALSE;
    Busy := FALSE;
    Error := FALSE;
    ErrorID := WORD#16#0000;
    Velocity := REAL#0.0;
    Acceleration := REAL#0.0;
    Deceleration := REAL#0.0;
    Jerk := REAL#0.0;
END_IF;
END_FUNCTION_BLOCK

FUNCTION_BLOCK MC_GroupWriteDefaultDynamics
VAR_IN_OUT
    AxesGroup : AXES_GROUP_REF;
END_VAR
VAR_INPUT
    Execute : BOOL;
    Velocity : REAL;
    Acceleration : REAL;
    Deceleration : REAL;
    Jerk : REAL;
END_VAR
VAR_OUTPUT
    Done : BOOL;
    Busy : BOOL;
    Error : BOOL;
    ErrorID : WORD;
END_VAR
VAR
    LastExecute : BOOL;
    GroupIndex : UINT;
END_VAR
GroupIndex := TRUST_GROUP_GroupIndex(Group := AxesGroup);
IF Execute AND (NOT LastExecute) THEN
    Busy := FALSE;
    Done := FALSE;
    Error := FALSE;
    ErrorID := WORD#16#0000;
    IF (Velocity <= REAL#0.0) OR (Acceleration <= REAL#0.0) OR (Deceleration <= REAL#0.0) OR (Jerk <= REAL#0.0) THEN
        Error := TRUE;
        ErrorID := WORD#16#0001;
    ELSE
        g_trust_group_default_velocity[GroupIndex] := Velocity;
        g_trust_group_default_acceleration[GroupIndex] := Acceleration;
        g_trust_group_default_deceleration[GroupIndex] := Deceleration;
        g_trust_group_default_jerk[GroupIndex] := Jerk;
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

FUNCTION_BLOCK MC_GroupReadDefaultDynamics
VAR_IN_OUT
    AxesGroup : AXES_GROUP_REF;
END_VAR
VAR_INPUT
    Enable : BOOL;
END_VAR
VAR_OUTPUT
    Valid : BOOL;
    Busy : BOOL;
    Error : BOOL;
    ErrorID : WORD;
    Velocity : REAL;
    Acceleration : REAL;
    Deceleration : REAL;
    Jerk : REAL;
END_VAR
VAR
    GroupIndex : UINT;
END_VAR
GroupIndex := TRUST_GROUP_GroupIndex(Group := AxesGroup);
IF Enable THEN
    Valid := TRUE;
    Busy := FALSE;
    Error := FALSE;
    ErrorID := WORD#16#0000;
    Velocity := g_trust_group_default_velocity[GroupIndex];
    Acceleration := g_trust_group_default_acceleration[GroupIndex];
    Deceleration := g_trust_group_default_deceleration[GroupIndex];
    Jerk := g_trust_group_default_jerk[GroupIndex];
ELSE
    Valid := FALSE;
    Busy := FALSE;
    Error := FALSE;
    ErrorID := WORD#16#0000;
    Velocity := REAL#0.0;
    Acceleration := REAL#0.0;
    Deceleration := REAL#0.0;
    Jerk := REAL#0.0;
END_IF;
END_FUNCTION_BLOCK
