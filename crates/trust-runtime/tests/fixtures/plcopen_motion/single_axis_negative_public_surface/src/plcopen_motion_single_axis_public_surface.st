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
END_FUNCTION_BLOCK

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
