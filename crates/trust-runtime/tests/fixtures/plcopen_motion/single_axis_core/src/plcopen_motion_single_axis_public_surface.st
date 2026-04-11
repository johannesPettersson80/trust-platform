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

TYPE MC_DIRECTION : (
    mcPositiveDirection,
    mcShortestWay,
    mcNegativeDirection,
    mcCurrentDirection
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

TYPE MC_AXIS_STATUS : (
    mcErrorStop,
    mcDisabled,
    mcStandstill,
    mcHoming,
    mcStopping,
    mcDiscreteMotion,
    mcContinuousMotion,
    mcSynchronizedMotion
);
END_TYPE

FUNCTION_BLOCK MC_Power
VAR_IN_OUT
    Axis : AXIS_REF;
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
    AxisIndex : UINT;
    Context : TRUST_MOTION_AxisContext;
END_VAR
Context(Axis := Axis);
AxisIndex := Context.AxisIndex;
Error := FALSE;
ErrorID := WORD#16#0000;
IF Enable THEN
    g_trust_motion_power_command[AxisIndex] := TRUE;
    IF g_trust_motion_ready_for_power_on[AxisIndex] AND g_trust_motion_communication_ready[AxisIndex] THEN
        g_trust_motion_power_stage[AxisIndex] := TRUE;
        IF g_trust_motion_state[AxisIndex] = MC_AXIS_STATUS#mcDisabled THEN
            g_trust_motion_state[AxisIndex] := MC_AXIS_STATUS#mcStandstill;
        END_IF;
    END_IF;
    Valid := TRUE;
ELSE
    g_trust_motion_power_command[AxisIndex] := FALSE;
    Valid := FALSE;
    IF (NOT g_trust_motion_power_stage[AxisIndex]) AND (g_trust_motion_state[AxisIndex] <> MC_AXIS_STATUS#mcErrorStop) THEN
        g_trust_motion_state[AxisIndex] := MC_AXIS_STATUS#mcDisabled;
    END_IF;
END_IF;
Status := g_trust_motion_power_stage[AxisIndex];
END_FUNCTION_BLOCK

FUNCTION_BLOCK MC_Home
VAR_IN_OUT
    Axis : AXIS_REF;
END_VAR
VAR_INPUT
    Execute : BOOL;
    Position : REAL;
    BufferMode : MC_BUFFER_MODE;
END_VAR
VAR_OUTPUT
    Done : BOOL;
    Busy : BOOL;
    Active : BOOL;
    CommandAborted : BOOL;
    Error : BOOL;
    ErrorID : WORD;
END_VAR
VAR
    AxisIndex : UINT;
    Context : TRUST_MOTION_AxisContext;
    LastExecute : BOOL;
    RequestedPosition : REAL;
    Cleared : BOOL;
END_VAR
Context(Axis := Axis);
AxisIndex := Context.AxisIndex;
IF Execute AND (NOT LastExecute) THEN
    Done := FALSE;
    Busy := FALSE;
    Active := FALSE;
    CommandAborted := FALSE;
    Error := FALSE;
    ErrorID := WORD#16#0000;
    IF g_trust_motion_grouped[AxisIndex] THEN
        Error := TRUE;
        ErrorID := WORD#16#0101;
    ELSE
        RequestedPosition := Position;
        Busy := TRUE;
        Active := TRUE;
        g_trust_motion_state[AxisIndex] := MC_AXIS_STATUS#mcHoming;
        g_trust_motion_constant_velocity[AxisIndex] := FALSE;
        g_trust_motion_accelerating[AxisIndex] := FALSE;
        g_trust_motion_decelerating[AxisIndex] := FALSE;
    END_IF;
ELSIF Execute AND Active THEN
    IF g_trust_motion_axis_error[AxisIndex] <> WORD#16#0000 THEN
        Busy := FALSE;
        Active := FALSE;
        Error := TRUE;
        ErrorID := g_trust_motion_axis_error[AxisIndex];
        g_trust_motion_state[AxisIndex] := MC_AXIS_STATUS#mcErrorStop;
    END_IF;
ELSIF (NOT Execute) AND LastExecute THEN
    IF Active THEN
        Busy := FALSE;
        Active := FALSE;
        Done := TRUE;
        g_trust_motion_state[AxisIndex] := MC_AXIS_STATUS#mcStandstill;
        g_trust_motion_is_homed[AxisIndex] := TRUE;
        g_trust_motion_actual_position[AxisIndex] := RequestedPosition;
        g_trust_motion_set_position[AxisIndex] := RequestedPosition;
        g_trust_motion_commanded_position[AxisIndex] := RequestedPosition;
        g_trust_motion_constant_velocity[AxisIndex] := FALSE;
        g_trust_motion_accelerating[AxisIndex] := FALSE;
        g_trust_motion_decelerating[AxisIndex] := FALSE;
    END_IF;
ELSIF NOT Execute THEN
    Done := FALSE;
    Error := FALSE;
    ErrorID := WORD#16#0000;
    CommandAborted := FALSE;
END_IF;
LastExecute := Execute;
END_FUNCTION_BLOCK

FUNCTION_BLOCK MC_Stop
VAR_IN_OUT
    Axis : AXIS_REF;
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
    AxisIndex : UINT;
    Context : TRUST_MOTION_AxisContext;
    LastExecute : BOOL;
END_VAR
Context(Axis := Axis);
AxisIndex := Context.AxisIndex;
IF Execute THEN
    IF NOT LastExecute THEN
        Error := FALSE;
        ErrorID := WORD#16#0000;
        CommandAborted := FALSE;
        IF g_trust_motion_grouped[AxisIndex] THEN
            Busy := FALSE;
            Done := FALSE;
            Error := TRUE;
            ErrorID := WORD#16#0101;
        ELSE
            Busy := TRUE;
            Done := TRUE;
            g_trust_motion_state[AxisIndex] := MC_AXIS_STATUS#mcStopping;
            g_trust_motion_actual_velocity[AxisIndex] := REAL#0.0;
            g_trust_motion_set_velocity[AxisIndex] := REAL#0.0;
            g_trust_motion_commanded_velocity[AxisIndex] := REAL#0.0;
            g_trust_motion_constant_velocity[AxisIndex] := FALSE;
            g_trust_motion_accelerating[AxisIndex] := FALSE;
            g_trust_motion_decelerating[AxisIndex] := TRUE;
        END_IF;
    ELSE
        IF NOT Error THEN
            Busy := TRUE;
            Done := TRUE;
            g_trust_motion_state[AxisIndex] := MC_AXIS_STATUS#mcStopping;
        END_IF;
    END_IF;
ELSIF LastExecute THEN
    IF NOT Error THEN
        Busy := FALSE;
        Done := TRUE;
        g_trust_motion_state[AxisIndex] := MC_AXIS_STATUS#mcStandstill;
        g_trust_motion_decelerating[AxisIndex] := FALSE;
    END_IF;
ELSE
    Done := FALSE;
    Busy := FALSE;
    Error := FALSE;
    ErrorID := WORD#16#0000;
    CommandAborted := FALSE;
END_IF;
LastExecute := Execute;
END_FUNCTION_BLOCK

FUNCTION_BLOCK MC_Halt
VAR_IN_OUT
    Axis : AXIS_REF;
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
    CommandAborted : BOOL;
    Error : BOOL;
    ErrorID : WORD;
END_VAR
VAR
    AxisIndex : UINT;
    Context : TRUST_MOTION_AxisContext;
    LastExecute : BOOL;
END_VAR
Context(Axis := Axis);
AxisIndex := Context.AxisIndex;
IF Execute AND (NOT LastExecute) THEN
    Done := FALSE;
    Busy := FALSE;
    Active := FALSE;
    CommandAborted := FALSE;
    Error := FALSE;
    ErrorID := WORD#16#0000;
    IF g_trust_motion_grouped[AxisIndex] THEN
        Error := TRUE;
        ErrorID := WORD#16#0101;
    ELSE
        Busy := TRUE;
        Active := TRUE;
        g_trust_motion_state[AxisIndex] := MC_AXIS_STATUS#mcDiscreteMotion;
        g_trust_motion_actual_velocity[AxisIndex] := REAL#0.0;
        g_trust_motion_set_velocity[AxisIndex] := REAL#0.0;
        g_trust_motion_commanded_velocity[AxisIndex] := REAL#0.0;
        g_trust_motion_constant_velocity[AxisIndex] := FALSE;
        g_trust_motion_accelerating[AxisIndex] := FALSE;
        g_trust_motion_decelerating[AxisIndex] := TRUE;
    END_IF;
ELSIF (NOT Execute) AND LastExecute THEN
    IF Active THEN
        Busy := FALSE;
        Active := FALSE;
        Done := TRUE;
        g_trust_motion_state[AxisIndex] := MC_AXIS_STATUS#mcStandstill;
        g_trust_motion_decelerating[AxisIndex] := FALSE;
    END_IF;
ELSIF NOT Execute THEN
    Done := FALSE;
    Error := FALSE;
    ErrorID := WORD#16#0000;
    CommandAborted := FALSE;
END_IF;
LastExecute := Execute;
END_FUNCTION_BLOCK

FUNCTION_BLOCK MC_MoveAbsolute
VAR_IN_OUT
    Axis : AXIS_REF;
END_VAR
VAR_INPUT
    Execute : BOOL;
    ContinuousUpdate : BOOL;
    Position : REAL;
    Velocity : REAL;
    Acceleration : REAL;
    Deceleration : REAL;
    Jerk : REAL;
    Direction : MC_DIRECTION;
    BufferMode : MC_BUFFER_MODE;
END_VAR
VAR_OUTPUT
    Done : BOOL;
    Busy : BOOL;
    Active : BOOL;
    CommandAborted : BOOL;
    Error : BOOL;
    ErrorID : WORD;
END_VAR
VAR
    AxisIndex : UINT;
    Context : TRUST_MOTION_AxisContext;
    LastExecute : BOOL;
    TargetPosition : REAL;
END_VAR
Context(Axis := Axis);
AxisIndex := Context.AxisIndex;
IF Execute AND (NOT LastExecute) THEN
    Done := FALSE;
    Busy := FALSE;
    Active := FALSE;
    CommandAborted := FALSE;
    Error := FALSE;
    ErrorID := WORD#16#0000;
    IF g_trust_motion_grouped[AxisIndex] THEN
        Error := TRUE;
        ErrorID := WORD#16#0101;
    ELSE
        TargetPosition := Position;
        Busy := TRUE;
        Active := TRUE;
        g_trust_motion_state[AxisIndex] := MC_AXIS_STATUS#mcDiscreteMotion;
        g_trust_motion_commanded_position[AxisIndex] := Position;
        g_trust_motion_set_position[AxisIndex] := Position;
        g_trust_motion_commanded_velocity[AxisIndex] := Velocity;
        g_trust_motion_set_velocity[AxisIndex] := Velocity;
        g_trust_motion_actual_velocity[AxisIndex] := Velocity;
        g_trust_motion_constant_velocity[AxisIndex] := FALSE;
        g_trust_motion_accelerating[AxisIndex] := TRUE;
        g_trust_motion_decelerating[AxisIndex] := FALSE;
    END_IF;
ELSIF Execute AND Active THEN
    IF ContinuousUpdate THEN
        TargetPosition := Position;
        g_trust_motion_commanded_position[AxisIndex] := Position;
        g_trust_motion_set_position[AxisIndex] := Position;
    END_IF;
ELSIF (NOT Execute) AND LastExecute THEN
    IF Active THEN
        Busy := FALSE;
        Active := FALSE;
        Done := TRUE;
        g_trust_motion_state[AxisIndex] := MC_AXIS_STATUS#mcStandstill;
        g_trust_motion_actual_position[AxisIndex] := TargetPosition;
        g_trust_motion_set_position[AxisIndex] := TargetPosition;
        g_trust_motion_commanded_position[AxisIndex] := TargetPosition;
        g_trust_motion_actual_velocity[AxisIndex] := REAL#0.0;
        g_trust_motion_set_velocity[AxisIndex] := REAL#0.0;
        g_trust_motion_commanded_velocity[AxisIndex] := REAL#0.0;
        g_trust_motion_accelerating[AxisIndex] := FALSE;
    END_IF;
ELSIF NOT Execute THEN
    Done := FALSE;
    Error := FALSE;
    ErrorID := WORD#16#0000;
    CommandAborted := FALSE;
END_IF;
LastExecute := Execute;
END_FUNCTION_BLOCK

FUNCTION_BLOCK MC_MoveRelative
VAR_IN_OUT
    Axis : AXIS_REF;
END_VAR
VAR_INPUT
    Execute : BOOL;
    ContinuousUpdate : BOOL;
    Distance : REAL;
    Velocity : REAL;
    Acceleration : REAL;
    Deceleration : REAL;
    Jerk : REAL;
    BufferMode : MC_BUFFER_MODE;
END_VAR
VAR_OUTPUT
    Done : BOOL;
    Busy : BOOL;
    Active : BOOL;
    CommandAborted : BOOL;
    Error : BOOL;
    ErrorID : WORD;
END_VAR
VAR
    AxisIndex : UINT;
    Context : TRUST_MOTION_AxisContext;
    LastExecute : BOOL;
    TargetPosition : REAL;
END_VAR
Context(Axis := Axis);
AxisIndex := Context.AxisIndex;
IF Execute AND (NOT LastExecute) THEN
    Done := FALSE;
    Busy := FALSE;
    Active := FALSE;
    CommandAborted := FALSE;
    Error := FALSE;
    ErrorID := WORD#16#0000;
    IF g_trust_motion_grouped[AxisIndex] THEN
        Error := TRUE;
        ErrorID := WORD#16#0101;
    ELSE
        TargetPosition := g_trust_motion_actual_position[AxisIndex] + Distance;
        Busy := TRUE;
        Active := TRUE;
        g_trust_motion_state[AxisIndex] := MC_AXIS_STATUS#mcDiscreteMotion;
        g_trust_motion_commanded_position[AxisIndex] := TargetPosition;
        g_trust_motion_set_position[AxisIndex] := TargetPosition;
        g_trust_motion_commanded_velocity[AxisIndex] := Velocity;
        g_trust_motion_set_velocity[AxisIndex] := Velocity;
        g_trust_motion_actual_velocity[AxisIndex] := Velocity;
        g_trust_motion_accelerating[AxisIndex] := TRUE;
        g_trust_motion_decelerating[AxisIndex] := FALSE;
    END_IF;
ELSIF Execute AND Active THEN
    IF ContinuousUpdate THEN
        g_trust_motion_commanded_position[AxisIndex] := TargetPosition;
        g_trust_motion_set_position[AxisIndex] := TargetPosition;
    END_IF;
ELSIF (NOT Execute) AND LastExecute THEN
    IF Active THEN
        Busy := FALSE;
        Active := FALSE;
        Done := TRUE;
        g_trust_motion_state[AxisIndex] := MC_AXIS_STATUS#mcStandstill;
        g_trust_motion_actual_position[AxisIndex] := TargetPosition;
        g_trust_motion_set_position[AxisIndex] := TargetPosition;
        g_trust_motion_commanded_position[AxisIndex] := TargetPosition;
        g_trust_motion_actual_velocity[AxisIndex] := REAL#0.0;
        g_trust_motion_set_velocity[AxisIndex] := REAL#0.0;
        g_trust_motion_commanded_velocity[AxisIndex] := REAL#0.0;
        g_trust_motion_accelerating[AxisIndex] := FALSE;
    END_IF;
ELSIF NOT Execute THEN
    Done := FALSE;
    Error := FALSE;
    ErrorID := WORD#16#0000;
    CommandAborted := FALSE;
END_IF;
LastExecute := Execute;
END_FUNCTION_BLOCK

FUNCTION_BLOCK MC_MoveAdditive
VAR_IN_OUT
    Axis : AXIS_REF;
END_VAR
VAR_INPUT
    Execute : BOOL;
    ContinuousUpdate : BOOL;
    Distance : REAL;
    Velocity : REAL;
    Acceleration : REAL;
    Deceleration : REAL;
    Jerk : REAL;
    BufferMode : MC_BUFFER_MODE;
END_VAR
VAR_OUTPUT
    Done : BOOL;
    Busy : BOOL;
    Active : BOOL;
    CommandAborted : BOOL;
    Error : BOOL;
    ErrorID : WORD;
END_VAR
VAR
    AxisIndex : UINT;
    Context : TRUST_MOTION_AxisContext;
    LastExecute : BOOL;
    TargetPosition : REAL;
END_VAR
Context(Axis := Axis);
AxisIndex := Context.AxisIndex;
IF Execute AND (NOT LastExecute) THEN
    Done := FALSE;
    Busy := FALSE;
    Active := FALSE;
    CommandAborted := FALSE;
    Error := FALSE;
    ErrorID := WORD#16#0000;
    IF g_trust_motion_grouped[AxisIndex] THEN
        Error := TRUE;
        ErrorID := WORD#16#0101;
    ELSE
        TargetPosition := g_trust_motion_commanded_position[AxisIndex] + Distance;
        Busy := TRUE;
        Active := TRUE;
        g_trust_motion_state[AxisIndex] := MC_AXIS_STATUS#mcDiscreteMotion;
        g_trust_motion_commanded_position[AxisIndex] := TargetPosition;
        g_trust_motion_set_position[AxisIndex] := TargetPosition;
        g_trust_motion_commanded_velocity[AxisIndex] := Velocity;
        g_trust_motion_set_velocity[AxisIndex] := Velocity;
        g_trust_motion_actual_velocity[AxisIndex] := Velocity;
        g_trust_motion_accelerating[AxisIndex] := TRUE;
        g_trust_motion_decelerating[AxisIndex] := FALSE;
    END_IF;
ELSIF Execute AND Active THEN
    IF ContinuousUpdate THEN
        g_trust_motion_commanded_position[AxisIndex] := TargetPosition;
        g_trust_motion_set_position[AxisIndex] := TargetPosition;
    END_IF;
ELSIF (NOT Execute) AND LastExecute THEN
    IF Active THEN
        Busy := FALSE;
        Active := FALSE;
        Done := TRUE;
        g_trust_motion_state[AxisIndex] := MC_AXIS_STATUS#mcStandstill;
        g_trust_motion_actual_position[AxisIndex] := TargetPosition;
        g_trust_motion_set_position[AxisIndex] := TargetPosition;
        g_trust_motion_commanded_position[AxisIndex] := TargetPosition;
        g_trust_motion_actual_velocity[AxisIndex] := REAL#0.0;
        g_trust_motion_set_velocity[AxisIndex] := REAL#0.0;
        g_trust_motion_commanded_velocity[AxisIndex] := REAL#0.0;
        g_trust_motion_accelerating[AxisIndex] := FALSE;
    END_IF;
ELSIF NOT Execute THEN
    Done := FALSE;
    Error := FALSE;
    ErrorID := WORD#16#0000;
    CommandAborted := FALSE;
END_IF;
LastExecute := Execute;
END_FUNCTION_BLOCK

FUNCTION_BLOCK MC_MoveVelocity
VAR_IN_OUT
    Axis : AXIS_REF;
END_VAR
VAR_INPUT
    Execute : BOOL;
    ContinuousUpdate : BOOL;
    Velocity : REAL;
    Acceleration : REAL;
    Deceleration : REAL;
    Jerk : REAL;
    Direction : MC_DIRECTION;
    BufferMode : MC_BUFFER_MODE;
END_VAR
VAR_OUTPUT
    InVelocity : BOOL;
    Busy : BOOL;
    Active : BOOL;
    CommandAborted : BOOL;
    Error : BOOL;
    ErrorID : WORD;
END_VAR
VAR
    AxisIndex : UINT;
    Context : TRUST_MOTION_AxisContext;
    LastExecute : BOOL;
    RequestedVelocity : REAL;
END_VAR
Context(Axis := Axis);
AxisIndex := Context.AxisIndex;
IF Execute AND (NOT LastExecute) THEN
    Busy := FALSE;
    Active := FALSE;
    InVelocity := FALSE;
    CommandAborted := FALSE;
    Error := FALSE;
    ErrorID := WORD#16#0000;
    IF g_trust_motion_grouped[AxisIndex] THEN
        Error := TRUE;
        ErrorID := WORD#16#0101;
    ELSE
        RequestedVelocity := Velocity;
        IF Direction = MC_DIRECTION#mcNegativeDirection THEN
            RequestedVelocity := -Velocity;
        END_IF;
        Busy := TRUE;
        Active := TRUE;
        InVelocity := TRUE;
        g_trust_motion_state[AxisIndex] := MC_AXIS_STATUS#mcContinuousMotion;
        g_trust_motion_actual_velocity[AxisIndex] := RequestedVelocity;
        g_trust_motion_set_velocity[AxisIndex] := RequestedVelocity;
        g_trust_motion_commanded_velocity[AxisIndex] := RequestedVelocity;
        g_trust_motion_constant_velocity[AxisIndex] := TRUE;
        g_trust_motion_accelerating[AxisIndex] := FALSE;
        g_trust_motion_decelerating[AxisIndex] := FALSE;
    END_IF;
ELSIF Execute AND Active THEN
    IF ContinuousUpdate THEN
        g_trust_motion_set_velocity[AxisIndex] := RequestedVelocity;
        g_trust_motion_commanded_velocity[AxisIndex] := RequestedVelocity;
        g_trust_motion_actual_velocity[AxisIndex] := RequestedVelocity;
        InVelocity := TRUE;
    END_IF;
ELSIF (NOT Execute) AND LastExecute THEN
    IF Active THEN
        Busy := FALSE;
        Active := FALSE;
        InVelocity := FALSE;
        g_trust_motion_state[AxisIndex] := MC_AXIS_STATUS#mcStandstill;
        g_trust_motion_actual_velocity[AxisIndex] := REAL#0.0;
        g_trust_motion_set_velocity[AxisIndex] := REAL#0.0;
        g_trust_motion_commanded_velocity[AxisIndex] := REAL#0.0;
        g_trust_motion_constant_velocity[AxisIndex] := FALSE;
    END_IF;
ELSIF NOT Execute THEN
    InVelocity := FALSE;
    Error := FALSE;
    ErrorID := WORD#16#0000;
    CommandAborted := FALSE;
END_IF;
LastExecute := Execute;
END_FUNCTION_BLOCK

FUNCTION_BLOCK MC_MoveContinuousAbsolute
VAR_IN_OUT
    Axis : AXIS_REF;
END_VAR
VAR_INPUT
    Execute : BOOL;
    ContinuousUpdate : BOOL;
    Position : REAL;
    EndVelocity : REAL;
    Velocity : REAL;
    Acceleration : REAL;
    Deceleration : REAL;
    Jerk : REAL;
    Direction : MC_DIRECTION;
    BufferMode : MC_BUFFER_MODE;
END_VAR
VAR_OUTPUT
    InEndVelocity : BOOL;
    Busy : BOOL;
    Active : BOOL;
    CommandAborted : BOOL;
    Error : BOOL;
    ErrorID : WORD;
END_VAR
VAR
    AxisIndex : UINT;
    Context : TRUST_MOTION_AxisContext;
    LastExecute : BOOL;
    RequestedEndVelocity : REAL;
END_VAR
Context(Axis := Axis);
AxisIndex := Context.AxisIndex;
IF Execute AND (NOT LastExecute) THEN
    Busy := FALSE;
    Active := FALSE;
    InEndVelocity := FALSE;
    CommandAborted := FALSE;
    Error := FALSE;
    ErrorID := WORD#16#0000;
    IF g_trust_motion_grouped[AxisIndex] THEN
        Error := TRUE;
        ErrorID := WORD#16#0101;
    ELSE
        RequestedEndVelocity := EndVelocity;
        Busy := TRUE;
        Active := TRUE;
        InEndVelocity := TRUE;
        g_trust_motion_state[AxisIndex] := MC_AXIS_STATUS#mcContinuousMotion;
        g_trust_motion_commanded_position[AxisIndex] := Position;
        g_trust_motion_set_position[AxisIndex] := Position;
        g_trust_motion_actual_velocity[AxisIndex] := RequestedEndVelocity;
        g_trust_motion_set_velocity[AxisIndex] := RequestedEndVelocity;
        g_trust_motion_commanded_velocity[AxisIndex] := RequestedEndVelocity;
        g_trust_motion_constant_velocity[AxisIndex] := TRUE;
    END_IF;
ELSIF (NOT Execute) AND LastExecute THEN
    IF Active THEN
        Busy := FALSE;
        Active := FALSE;
        InEndVelocity := FALSE;
        g_trust_motion_state[AxisIndex] := MC_AXIS_STATUS#mcStandstill;
        g_trust_motion_actual_position[AxisIndex] := Position;
        g_trust_motion_set_position[AxisIndex] := Position;
        g_trust_motion_commanded_position[AxisIndex] := Position;
        g_trust_motion_actual_velocity[AxisIndex] := REAL#0.0;
        g_trust_motion_set_velocity[AxisIndex] := REAL#0.0;
        g_trust_motion_commanded_velocity[AxisIndex] := REAL#0.0;
        g_trust_motion_constant_velocity[AxisIndex] := FALSE;
    END_IF;
ELSIF NOT Execute THEN
    InEndVelocity := FALSE;
    Error := FALSE;
    ErrorID := WORD#16#0000;
    CommandAborted := FALSE;
END_IF;
LastExecute := Execute;
END_FUNCTION_BLOCK

FUNCTION_BLOCK MC_MoveContinuousRelative
VAR_IN_OUT
    Axis : AXIS_REF;
END_VAR
VAR_INPUT
    Execute : BOOL;
    ContinuousUpdate : BOOL;
    Distance : REAL;
    EndVelocity : REAL;
    Velocity : REAL;
    Acceleration : REAL;
    Deceleration : REAL;
    Jerk : REAL;
    BufferMode : MC_BUFFER_MODE;
END_VAR
VAR_OUTPUT
    InEndVelocity : BOOL;
    Busy : BOOL;
    Active : BOOL;
    CommandAborted : BOOL;
    Error : BOOL;
    ErrorID : WORD;
END_VAR
VAR
    AxisIndex : UINT;
    Context : TRUST_MOTION_AxisContext;
    LastExecute : BOOL;
    TargetPosition : REAL;
    RequestedEndVelocity : REAL;
END_VAR
Context(Axis := Axis);
AxisIndex := Context.AxisIndex;
IF Execute AND (NOT LastExecute) THEN
    Busy := FALSE;
    Active := FALSE;
    InEndVelocity := FALSE;
    CommandAborted := FALSE;
    Error := FALSE;
    ErrorID := WORD#16#0000;
    IF g_trust_motion_grouped[AxisIndex] THEN
        Error := TRUE;
        ErrorID := WORD#16#0101;
    ELSE
        TargetPosition := g_trust_motion_actual_position[AxisIndex] + Distance;
        RequestedEndVelocity := EndVelocity;
        Busy := TRUE;
        Active := TRUE;
        InEndVelocity := TRUE;
        g_trust_motion_state[AxisIndex] := MC_AXIS_STATUS#mcContinuousMotion;
        g_trust_motion_commanded_position[AxisIndex] := TargetPosition;
        g_trust_motion_set_position[AxisIndex] := TargetPosition;
        g_trust_motion_actual_velocity[AxisIndex] := RequestedEndVelocity;
        g_trust_motion_set_velocity[AxisIndex] := RequestedEndVelocity;
        g_trust_motion_commanded_velocity[AxisIndex] := RequestedEndVelocity;
        g_trust_motion_constant_velocity[AxisIndex] := TRUE;
    END_IF;
ELSIF (NOT Execute) AND LastExecute THEN
    IF Active THEN
        Busy := FALSE;
        Active := FALSE;
        InEndVelocity := FALSE;
        g_trust_motion_state[AxisIndex] := MC_AXIS_STATUS#mcStandstill;
        g_trust_motion_actual_position[AxisIndex] := TargetPosition;
        g_trust_motion_set_position[AxisIndex] := TargetPosition;
        g_trust_motion_commanded_position[AxisIndex] := TargetPosition;
        g_trust_motion_actual_velocity[AxisIndex] := REAL#0.0;
        g_trust_motion_set_velocity[AxisIndex] := REAL#0.0;
        g_trust_motion_commanded_velocity[AxisIndex] := REAL#0.0;
        g_trust_motion_constant_velocity[AxisIndex] := FALSE;
    END_IF;
ELSIF NOT Execute THEN
    InEndVelocity := FALSE;
    Error := FALSE;
    ErrorID := WORD#16#0000;
    CommandAborted := FALSE;
END_IF;
LastExecute := Execute;
END_FUNCTION_BLOCK

FUNCTION_BLOCK MC_SetPosition
VAR_IN_OUT
    Axis : AXIS_REF;
END_VAR
VAR_INPUT
    Execute : BOOL;
    Position : REAL;
    Relative : BOOL;
    ExecutionMode : MC_EXECUTION_MODE;
END_VAR
VAR_OUTPUT
    Done : BOOL;
    Busy : BOOL;
    Error : BOOL;
    ErrorID : WORD;
END_VAR
VAR
    AxisIndex : UINT;
    Context : TRUST_MOTION_AxisContext;
    LastExecute : BOOL;
END_VAR
Context(Axis := Axis);
AxisIndex := Context.AxisIndex;
IF Execute AND (NOT LastExecute) THEN
    Busy := FALSE;
    Done := FALSE;
    Error := FALSE;
    ErrorID := WORD#16#0000;
    IF ExecutionMode = MC_EXECUTION_MODE#mcDelayed THEN
        Error := TRUE;
        ErrorID := WORD#16#0500;
    ELSE
        IF Relative THEN
            g_trust_motion_actual_position[AxisIndex] := g_trust_motion_actual_position[AxisIndex] + Position;
            g_trust_motion_set_position[AxisIndex] := g_trust_motion_set_position[AxisIndex] + Position;
            g_trust_motion_commanded_position[AxisIndex] := g_trust_motion_commanded_position[AxisIndex] + Position;
        ELSE
            g_trust_motion_actual_position[AxisIndex] := Position;
            g_trust_motion_set_position[AxisIndex] := Position;
            g_trust_motion_commanded_position[AxisIndex] := Position;
        END_IF;
        Done := TRUE;
    END_IF;
ELSIF NOT Execute THEN
    Busy := FALSE;
    IF NOT LastExecute THEN
        Done := FALSE;
        Error := FALSE;
        ErrorID := WORD#16#0000;
    END_IF;
END_IF;
LastExecute := Execute;
END_FUNCTION_BLOCK

FUNCTION_BLOCK MC_SetOverride
VAR_IN_OUT
    Axis : AXIS_REF;
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
    AxisIndex : UINT;
    Context : TRUST_MOTION_AxisContext;
    Cleared : BOOL;
END_VAR
Context(Axis := Axis);
AxisIndex := Context.AxisIndex;
Busy := FALSE;
IF NOT Enable THEN
    Enabled := FALSE;
    Error := FALSE;
    ErrorID := WORD#16#0000;
ELSE
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
        Enabled := TRUE;
        g_trust_motion_vel_factor[AxisIndex] := VelFactor;
        g_trust_motion_acc_factor[AxisIndex] := AccFactor;
        g_trust_motion_jerk_factor[AxisIndex] := JerkFactor;
        IF VelFactor = REAL#0.0 THEN
            g_trust_motion_state[AxisIndex] := MC_AXIS_STATUS#mcStopping;
            g_trust_motion_actual_velocity[AxisIndex] := REAL#0.0;
            g_trust_motion_set_velocity[AxisIndex] := REAL#0.0;
            g_trust_motion_commanded_velocity[AxisIndex] := REAL#0.0;
            g_trust_motion_constant_velocity[AxisIndex] := FALSE;
        g_trust_motion_accelerating[AxisIndex] := FALSE;
        g_trust_motion_decelerating[AxisIndex] := FALSE;
            g_trust_motion_decelerating[AxisIndex] := TRUE;
        END_IF;
    END_IF;
END_IF;
END_FUNCTION_BLOCK

FUNCTION_BLOCK MC_ReadActualPosition
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
    Position : REAL;
END_VAR
VAR
    AxisIndex : UINT;
    Context : TRUST_MOTION_AxisContext;
END_VAR
Context(Axis := Axis);
AxisIndex := Context.AxisIndex;
Busy := FALSE;
Error := FALSE;
ErrorID := WORD#16#0000;
IF Enable THEN
    Valid := TRUE;
    Position := g_trust_motion_actual_position[AxisIndex];
ELSE
    Valid := FALSE;
END_IF;
END_FUNCTION_BLOCK

FUNCTION_BLOCK MC_ReadActualVelocity
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
    Velocity : REAL;
END_VAR
VAR
    AxisIndex : UINT;
    Context : TRUST_MOTION_AxisContext;
END_VAR
Context(Axis := Axis);
AxisIndex := Context.AxisIndex;
Busy := FALSE;
Error := FALSE;
ErrorID := WORD#16#0000;
IF Enable THEN
    Valid := TRUE;
    Velocity := g_trust_motion_actual_velocity[AxisIndex];
ELSE
    Valid := FALSE;
END_IF;
END_FUNCTION_BLOCK

FUNCTION_BLOCK MC_ReadActualTorque
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
    Torque : REAL;
END_VAR
VAR
    AxisIndex : UINT;
    Context : TRUST_MOTION_AxisContext;
END_VAR
Context(Axis := Axis);
AxisIndex := Context.AxisIndex;
Busy := FALSE;
Error := FALSE;
ErrorID := WORD#16#0000;
IF Enable THEN
    Valid := TRUE;
    Torque := g_trust_motion_actual_torque[AxisIndex];
ELSE
    Valid := FALSE;
END_IF;
END_FUNCTION_BLOCK

FUNCTION_BLOCK MC_ReadStatus
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
    ErrorStop : BOOL;
    Disabled : BOOL;
    Stopping : BOOL;
    Homing : BOOL;
    Standstill : BOOL;
    DiscreteMotion : BOOL;
    ContinuousMotion : BOOL;
    SynchronizedMotion : BOOL;
END_VAR
VAR
    AxisIndex : UINT;
    Context : TRUST_MOTION_AxisContext;
END_VAR
Context(Axis := Axis);
AxisIndex := Context.AxisIndex;
Busy := FALSE;
Error := FALSE;
ErrorID := WORD#16#0000;
ErrorStop := FALSE;
Disabled := FALSE;
Stopping := FALSE;
Homing := FALSE;
Standstill := FALSE;
DiscreteMotion := FALSE;
ContinuousMotion := FALSE;
SynchronizedMotion := FALSE;
IF Enable THEN
    Valid := TRUE;
    ErrorStop := g_trust_motion_state[AxisIndex] = MC_AXIS_STATUS#mcErrorStop;
    Disabled := g_trust_motion_state[AxisIndex] = MC_AXIS_STATUS#mcDisabled;
    Stopping := g_trust_motion_state[AxisIndex] = MC_AXIS_STATUS#mcStopping;
    Homing := g_trust_motion_state[AxisIndex] = MC_AXIS_STATUS#mcHoming;
    Standstill := g_trust_motion_state[AxisIndex] = MC_AXIS_STATUS#mcStandstill;
    DiscreteMotion := g_trust_motion_state[AxisIndex] = MC_AXIS_STATUS#mcDiscreteMotion;
    ContinuousMotion := g_trust_motion_state[AxisIndex] = MC_AXIS_STATUS#mcContinuousMotion;
    SynchronizedMotion := g_trust_motion_state[AxisIndex] = MC_AXIS_STATUS#mcSynchronizedMotion;
ELSE
    Valid := FALSE;
END_IF;
END_FUNCTION_BLOCK

FUNCTION_BLOCK MC_ReadMotionState
VAR_IN_OUT
    Axis : AXIS_REF;
END_VAR
VAR_INPUT
    Enable : BOOL;
    Source : MC_SOURCE;
END_VAR
VAR_OUTPUT
    Valid : BOOL;
    Busy : BOOL;
    Error : BOOL;
    ErrorID : WORD;
    ConstantVelocity : BOOL;
    Accelerating : BOOL;
    Decelerating : BOOL;
    DirectionPositive : BOOL;
    DirectionNegative : BOOL;
END_VAR
VAR
    AxisIndex : UINT;
    Context : TRUST_MOTION_AxisContext;
    SelectedVelocity : REAL;
END_VAR
Context(Axis := Axis);
AxisIndex := Context.AxisIndex;
Busy := FALSE;
Error := FALSE;
ErrorID := WORD#16#0000;
ConstantVelocity := FALSE;
Accelerating := FALSE;
Decelerating := FALSE;
DirectionPositive := FALSE;
DirectionNegative := FALSE;
IF Enable THEN
    Valid := TRUE;
    SelectedVelocity := g_trust_motion_actual_velocity[AxisIndex];
    IF Source = MC_SOURCE#mcCommandedValue THEN
        SelectedVelocity := g_trust_motion_commanded_velocity[AxisIndex];
    ELSIF Source = MC_SOURCE#mcSetValue THEN
        SelectedVelocity := g_trust_motion_set_velocity[AxisIndex];
    END_IF;
    ConstantVelocity := g_trust_motion_constant_velocity[AxisIndex];
    Accelerating := g_trust_motion_accelerating[AxisIndex];
    Decelerating := g_trust_motion_decelerating[AxisIndex];
    DirectionPositive := SelectedVelocity > REAL#0.0;
    DirectionNegative := SelectedVelocity < REAL#0.0;
ELSE
    Valid := FALSE;
END_IF;
END_FUNCTION_BLOCK

FUNCTION_BLOCK MC_ReadAxisInfo
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
    HomeAbsSwitch : BOOL;
    LimitSwitchPos : BOOL;
    LimitSwitchNeg : BOOL;
    Simulation : BOOL;
    CommunicationReady : BOOL;
    ReadyForPowerOn : BOOL;
    PowerOn : BOOL;
    IsHomed : BOOL;
    AxisWarning : BOOL;
END_VAR
VAR
    AxisIndex : UINT;
    Context : TRUST_MOTION_AxisContext;
END_VAR
Context(Axis := Axis);
AxisIndex := Context.AxisIndex;
Busy := FALSE;
Error := FALSE;
ErrorID := WORD#16#0000;
HomeAbsSwitch := FALSE;
LimitSwitchPos := FALSE;
LimitSwitchNeg := FALSE;
Simulation := FALSE;
CommunicationReady := FALSE;
ReadyForPowerOn := FALSE;
PowerOn := FALSE;
IsHomed := FALSE;
AxisWarning := FALSE;
IF Enable THEN
    Valid := TRUE;
    HomeAbsSwitch := g_trust_motion_home_abs_switch[AxisIndex];
    LimitSwitchPos := g_trust_motion_limit_switch_pos[AxisIndex];
    LimitSwitchNeg := g_trust_motion_limit_switch_neg[AxisIndex];
    Simulation := g_trust_motion_simulation[AxisIndex];
    CommunicationReady := g_trust_motion_communication_ready[AxisIndex];
    ReadyForPowerOn := g_trust_motion_ready_for_power_on[AxisIndex];
    PowerOn := g_trust_motion_power_stage[AxisIndex];
    IsHomed := g_trust_motion_is_homed[AxisIndex];
    AxisWarning := g_trust_motion_axis_warning[AxisIndex];
ELSE
    Valid := FALSE;
END_IF;
END_FUNCTION_BLOCK

FUNCTION_BLOCK MC_ReadAxisError
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
    AxisErrorID : WORD;
END_VAR
VAR
    AxisIndex : UINT;
    Context : TRUST_MOTION_AxisContext;
END_VAR
Context(Axis := Axis);
AxisIndex := Context.AxisIndex;
Busy := FALSE;
Error := FALSE;
ErrorID := WORD#16#0000;
AxisErrorID := WORD#16#0000;
IF Enable THEN
    Valid := TRUE;
    AxisErrorID := g_trust_motion_axis_error[AxisIndex];
ELSE
    Valid := FALSE;
END_IF;
END_FUNCTION_BLOCK

FUNCTION_BLOCK MC_Reset
VAR_IN_OUT
    Axis : AXIS_REF;
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
    AxisIndex : UINT;
    Context : TRUST_MOTION_AxisContext;
    LastExecute : BOOL;
END_VAR
Context(Axis := Axis);
AxisIndex := Context.AxisIndex;
IF Execute AND (NOT LastExecute) THEN
    Busy := FALSE;
    Done := FALSE;
    Error := FALSE;
    ErrorID := WORD#16#0000;
    IF g_trust_motion_state[AxisIndex] = MC_AXIS_STATUS#mcErrorStop THEN
        g_trust_motion_axis_error[AxisIndex] := WORD#16#0000;
        IF g_trust_motion_power_stage[AxisIndex] THEN
            g_trust_motion_state[AxisIndex] := MC_AXIS_STATUS#mcStandstill;
        ELSE
            g_trust_motion_state[AxisIndex] := MC_AXIS_STATUS#mcDisabled;
        END_IF;
    END_IF;
    Done := TRUE;
ELSIF NOT Execute THEN
    Busy := FALSE;
    IF NOT LastExecute THEN
        Done := FALSE;
        Error := FALSE;
        ErrorID := WORD#16#0000;
    END_IF;
END_IF;
LastExecute := Execute;
END_FUNCTION_BLOCK

FUNCTION_BLOCK MC_ReadParameter
VAR_IN_OUT
    Axis : AXIS_REF;
END_VAR
VAR_INPUT
    Enable : BOOL;
    ParameterNumber : INT;
END_VAR
VAR_OUTPUT
    Valid : BOOL;
    Busy : BOOL;
    Error : BOOL;
    ErrorID : WORD;
    Value : REAL;
END_VAR
VAR
    Context : TRUST_MOTION_AxisContext;
END_VAR
Context(Axis := Axis);
Busy := FALSE;
Error := FALSE;
ErrorID := WORD#16#0000;
Value := REAL#0.0;
IF Enable THEN
    IF TRUST_MOTION_IsNumericParameter(ParameterNumber := ParameterNumber) THEN
        Valid := TRUE;
        IF ParameterNumber = INT#1 THEN
            Value := g_trust_motion_commanded_position[Context.AxisIndex];
        ELSIF ParameterNumber = INT#2 THEN
            Value := g_trust_motion_sw_limit_pos[Context.AxisIndex];
        ELSIF ParameterNumber = INT#3 THEN
            Value := g_trust_motion_sw_limit_neg[Context.AxisIndex];
        ELSIF ParameterNumber = INT#7 THEN
            Value := g_trust_motion_max_position_lag[Context.AxisIndex];
        ELSIF ParameterNumber = INT#8 THEN
            Value := g_trust_motion_max_velocity_system[Context.AxisIndex];
        ELSIF ParameterNumber = INT#9 THEN
            Value := g_trust_motion_max_velocity_appl[Context.AxisIndex];
        ELSIF ParameterNumber = INT#10 THEN
            Value := g_trust_motion_actual_velocity[Context.AxisIndex];
        ELSIF ParameterNumber = INT#11 THEN
            Value := g_trust_motion_commanded_velocity[Context.AxisIndex];
        ELSIF ParameterNumber = INT#12 THEN
            Value := g_trust_motion_max_acceleration_system[Context.AxisIndex];
        ELSIF ParameterNumber = INT#13 THEN
            Value := g_trust_motion_max_acceleration_appl[Context.AxisIndex];
        ELSIF ParameterNumber = INT#14 THEN
            Value := g_trust_motion_max_deceleration_system[Context.AxisIndex];
        ELSIF ParameterNumber = INT#15 THEN
            Value := g_trust_motion_max_deceleration_appl[Context.AxisIndex];
        ELSIF ParameterNumber = INT#16 THEN
            Value := g_trust_motion_max_jerk_system[Context.AxisIndex];
        ELSIF ParameterNumber = INT#17 THEN
            Value := g_trust_motion_max_jerk_appl[Context.AxisIndex];
        END_IF;
    ELSE
        Valid := FALSE;
        Error := TRUE;
        ErrorID := WORD#16#0001;
    END_IF;
ELSE
    Valid := FALSE;
END_IF;
END_FUNCTION_BLOCK

FUNCTION_BLOCK MC_ReadBoolParameter
VAR_IN_OUT
    Axis : AXIS_REF;
END_VAR
VAR_INPUT
    Enable : BOOL;
    ParameterNumber : INT;
END_VAR
VAR_OUTPUT
    Valid : BOOL;
    Busy : BOOL;
    Error : BOOL;
    ErrorID : WORD;
    Value : BOOL;
END_VAR
VAR
    Context : TRUST_MOTION_AxisContext;
END_VAR
Context(Axis := Axis);
Busy := FALSE;
Error := FALSE;
ErrorID := WORD#16#0000;
Value := FALSE;
IF Enable THEN
    IF TRUST_MOTION_IsBoolParameter(ParameterNumber := ParameterNumber) THEN
        Valid := TRUE;
        IF ParameterNumber = INT#4 THEN
            Value := g_trust_motion_enable_limit_pos[Context.AxisIndex];
        ELSIF ParameterNumber = INT#5 THEN
            Value := g_trust_motion_enable_limit_neg[Context.AxisIndex];
        ELSIF ParameterNumber = INT#6 THEN
            Value := g_trust_motion_enable_pos_lag_monitoring[Context.AxisIndex];
        ELSIF ParameterNumber = INT#1000 THEN
            Value := g_trust_motion_vendor_bool_1000[Context.AxisIndex];
        END_IF;
    ELSE
        Valid := FALSE;
        Error := TRUE;
        ErrorID := WORD#16#0001;
    END_IF;
ELSE
    Valid := FALSE;
END_IF;
END_FUNCTION_BLOCK

FUNCTION_BLOCK MC_WriteParameter
VAR_IN_OUT
    Axis : AXIS_REF;
END_VAR
VAR_INPUT
    Execute : BOOL;
    ParameterNumber : INT;
    Value : REAL;
    ExecutionMode : MC_EXECUTION_MODE;
END_VAR
VAR_OUTPUT
    Done : BOOL;
    Busy : BOOL;
    Error : BOOL;
    ErrorID : WORD;
END_VAR
VAR
    Context : TRUST_MOTION_AxisContext;
    LastExecute : BOOL;
    Accepted : BOOL;
END_VAR
Context(Axis := Axis);
IF Execute AND (NOT LastExecute) THEN
    Busy := FALSE;
    Done := FALSE;
    Error := FALSE;
    ErrorID := WORD#16#0000;
    Accepted := FALSE;
    IF ExecutionMode = MC_EXECUTION_MODE#mcDelayed THEN
        Error := TRUE;
        ErrorID := WORD#16#0500;
    ELSIF NOT TRUST_MOTION_IsNumericParameter(ParameterNumber := ParameterNumber) THEN
        Error := TRUE;
        ErrorID := WORD#16#0001;
    ELSIF ((ParameterNumber = INT#9) OR (ParameterNumber = INT#13) OR (ParameterNumber = INT#15) OR (ParameterNumber = INT#17)) AND (Value <= REAL#0.0) THEN
        Error := TRUE;
        ErrorID := WORD#16#0001;
    ELSE
        Accepted := TRUE;
        IF ParameterNumber = INT#1 THEN
            g_trust_motion_commanded_position[Context.AxisIndex] := Value;
        ELSIF ParameterNumber = INT#2 THEN
            g_trust_motion_sw_limit_pos[Context.AxisIndex] := Value;
        ELSIF ParameterNumber = INT#3 THEN
            g_trust_motion_sw_limit_neg[Context.AxisIndex] := Value;
        ELSIF ParameterNumber = INT#7 THEN
            g_trust_motion_max_position_lag[Context.AxisIndex] := Value;
        ELSIF ParameterNumber = INT#8 THEN
            g_trust_motion_max_velocity_system[Context.AxisIndex] := Value;
        ELSIF ParameterNumber = INT#9 THEN
            g_trust_motion_max_velocity_appl[Context.AxisIndex] := Value;
        ELSIF ParameterNumber = INT#10 THEN
            g_trust_motion_actual_velocity[Context.AxisIndex] := Value;
        ELSIF ParameterNumber = INT#11 THEN
            g_trust_motion_commanded_velocity[Context.AxisIndex] := Value;
        ELSIF ParameterNumber = INT#12 THEN
            g_trust_motion_max_acceleration_system[Context.AxisIndex] := Value;
        ELSIF ParameterNumber = INT#13 THEN
            g_trust_motion_max_acceleration_appl[Context.AxisIndex] := Value;
        ELSIF ParameterNumber = INT#14 THEN
            g_trust_motion_max_deceleration_system[Context.AxisIndex] := Value;
        ELSIF ParameterNumber = INT#15 THEN
            g_trust_motion_max_deceleration_appl[Context.AxisIndex] := Value;
        ELSIF ParameterNumber = INT#16 THEN
            g_trust_motion_max_jerk_system[Context.AxisIndex] := Value;
        ELSIF ParameterNumber = INT#17 THEN
            g_trust_motion_max_jerk_appl[Context.AxisIndex] := Value;
        ELSE
            Accepted := FALSE;
        END_IF;
        IF Accepted THEN
            Done := TRUE;
        ELSE
            Error := TRUE;
            ErrorID := WORD#16#0001;
        END_IF;
    END_IF;
ELSIF NOT Execute THEN
    Busy := FALSE;
    IF NOT LastExecute THEN
        Done := FALSE;
        Error := FALSE;
        ErrorID := WORD#16#0000;
    END_IF;
END_IF;
LastExecute := Execute;
END_FUNCTION_BLOCK

FUNCTION_BLOCK MC_WriteBoolParameter
VAR_IN_OUT
    Axis : AXIS_REF;
END_VAR
VAR_INPUT
    Execute : BOOL;
    ParameterNumber : INT;
    Value : BOOL;
    ExecutionMode : MC_EXECUTION_MODE;
END_VAR
VAR_OUTPUT
    Done : BOOL;
    Busy : BOOL;
    Error : BOOL;
    ErrorID : WORD;
END_VAR
VAR
    Context : TRUST_MOTION_AxisContext;
    LastExecute : BOOL;
    Accepted : BOOL;
END_VAR
Context(Axis := Axis);
IF Execute AND (NOT LastExecute) THEN
    Busy := FALSE;
    Done := FALSE;
    Error := FALSE;
    ErrorID := WORD#16#0000;
    Accepted := FALSE;
    IF ExecutionMode = MC_EXECUTION_MODE#mcDelayed THEN
        Error := TRUE;
        ErrorID := WORD#16#0500;
    ELSIF NOT TRUST_MOTION_IsBoolParameter(ParameterNumber := ParameterNumber) THEN
        Error := TRUE;
        ErrorID := WORD#16#0001;
    ELSE
        Accepted := TRUE;
        IF ParameterNumber = INT#4 THEN
            g_trust_motion_enable_limit_pos[Context.AxisIndex] := Value;
        ELSIF ParameterNumber = INT#5 THEN
            g_trust_motion_enable_limit_neg[Context.AxisIndex] := Value;
        ELSIF ParameterNumber = INT#6 THEN
            g_trust_motion_enable_pos_lag_monitoring[Context.AxisIndex] := Value;
        ELSIF ParameterNumber = INT#1000 THEN
            g_trust_motion_vendor_bool_1000[Context.AxisIndex] := Value;
        ELSE
            Accepted := FALSE;
        END_IF;
        IF Accepted THEN
            Done := TRUE;
        ELSE
            Error := TRUE;
            ErrorID := WORD#16#0001;
        END_IF;
    END_IF;
ELSIF NOT Execute THEN
    Busy := FALSE;
    IF NOT LastExecute THEN
        Done := FALSE;
        Error := FALSE;
        ErrorID := WORD#16#0000;
    END_IF;
END_IF;
LastExecute := Execute;
END_FUNCTION_BLOCK

(* Call the FB once per instance before reading members; the public constants are assigned in the FB body. *)
FUNCTION_BLOCK MC_Constants
VAR_OUTPUT
    PN_CommandedPosition : INT;
    PN_SWLimitPos : INT;
    PN_SWLimitNeg : INT;
    PN_EnableLimitPos : INT;
    PN_EnableLimitNeg : INT;
    PN_EnablePosLagMonitoring : INT;
    PN_MaxPositionLag : INT;
    PN_MaxVelocitySystem : INT;
    PN_MaxVelocityAppl : INT;
    PN_ActualVelocity : INT;
    PN_CommandedVelocity : INT;
    PN_MaxAccelerationSystem : INT;
    PN_MaxAccelerationAppl : INT;
    PN_MaxDecelerationSystem : INT;
    PN_MaxDecelerationAppl : INT;
    PN_MaxJerkSystem : INT;
    PN_MaxJerkAppl : INT;

    mcERR_None : WORD;
    mcERR_InvalidParameter : WORD;
    mcERR_InvalidState : WORD;
    mcERR_AxisGrouped : WORD;
    mcERR_GroupDisabled : WORD;
    mcERR_GroupNotReady : WORD;
    mcERR_NotHomed : WORD;
    mcERR_NotPowered : WORD;
    mcERR_BackendFault : WORD;
    mcERR_KinematicNoSolution : WORD;
    mcERR_KinematicSingularity : WORD;
    mcERR_QueueFull : WORD;
    mcERR_NotSupported : WORD;
END_VAR
PN_CommandedPosition := INT#1;
PN_SWLimitPos := INT#2;
PN_SWLimitNeg := INT#3;
PN_EnableLimitPos := INT#4;
PN_EnableLimitNeg := INT#5;
PN_EnablePosLagMonitoring := INT#6;
PN_MaxPositionLag := INT#7;
PN_MaxVelocitySystem := INT#8;
PN_MaxVelocityAppl := INT#9;
PN_ActualVelocity := INT#10;
PN_CommandedVelocity := INT#11;
PN_MaxAccelerationSystem := INT#12;
PN_MaxAccelerationAppl := INT#13;
PN_MaxDecelerationSystem := INT#14;
PN_MaxDecelerationAppl := INT#15;
PN_MaxJerkSystem := INT#16;
PN_MaxJerkAppl := INT#17;

mcERR_None := WORD#16#0000;
mcERR_InvalidParameter := WORD#16#0001;
mcERR_InvalidState := WORD#16#0100;
mcERR_AxisGrouped := WORD#16#0101;
mcERR_GroupDisabled := WORD#16#0102;
mcERR_GroupNotReady := WORD#16#0103;
mcERR_NotHomed := WORD#16#0104;
mcERR_NotPowered := WORD#16#0105;
mcERR_BackendFault := WORD#16#0200;
mcERR_KinematicNoSolution := WORD#16#0300;
mcERR_KinematicSingularity := WORD#16#0301;
mcERR_QueueFull := WORD#16#0400;
mcERR_NotSupported := WORD#16#0500;
END_FUNCTION_BLOCK
