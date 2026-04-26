TYPE MC_ERROR : WORD;
END_TYPE

TYPE MC_AXIS_DIRECTION : (
    mcDirectionPositive,
    mcDirectionNegative
);
END_TYPE

TYPE MC_MOTION_STATUS : (
    mcConstantVelocity,
    mcAccelerating,
    mcDecelerating
);
END_TYPE

TYPE MC_COMBINE_MODE : (
    mcCombineAdditive
);
END_TYPE

TYPE MC_TRIGGER_REF :
STRUCT
    Id : UDINT;
END_STRUCT
END_TYPE

TYPE MC_CAMSWITCH_REF :
STRUCT
    Id : UDINT;
END_STRUCT
END_TYPE

TYPE MC_OUTPUT_REF :
STRUCT
    Id : UDINT;
END_STRUCT
END_TYPE

TYPE MC_TRACK_REF :
STRUCT
    Id : UDINT;
END_STRUCT
END_TYPE

TYPE TRUST_OOP_ACTIVE_COMMAND_KIND : (
    trustOopCommandNone,
    trustOopCommandHome,
    trustOopCommandStop,
    trustOopCommandHalt,
    trustOopCommandMoveAbsolute,
    trustOopCommandMoveRelative,
    trustOopCommandMoveAdditive,
    trustOopCommandMoveVelocity,
    trustOopCommandMoveContinuousAbsolute,
    trustOopCommandMoveContinuousRelative
);
END_TYPE

INTERFACE itfCommand
    PROPERTY Done : BOOL
    GET
    END_GET
    END_PROPERTY

    PROPERTY Busy : BOOL
    GET
    END_GET
    END_PROPERTY

    PROPERTY Active : BOOL
    GET
    END_GET
    END_PROPERTY

    PROPERTY CommandAborted : BOOL
    GET
    END_GET
    END_PROPERTY

    PROPERTY Error : BOOL
    GET
    END_GET
    END_PROPERTY

    PROPERTY ErrorId : MC_ERROR
    GET
    END_GET
    END_PROPERTY

    METHOD Abort : MC_ERROR
    END_METHOD

    METHOD Wait : MC_ERROR
    VAR_INPUT
        Timeout : TIME;
        AbortOnTimeout : BOOL;
    END_VAR
    END_METHOD
END_INTERFACE

INTERFACE itfAxisCommand EXTENDS itfCommand
    METHOD Update : MC_ERROR
    VAR_INPUT
        Position : REAL;
        Velocity : REAL;
        EndVelocity : REAL;
        Acceleration : REAL;
        Deceleration : REAL;
        Jerk : REAL;
    END_VAR
    END_METHOD
END_INTERFACE

INTERFACE itfContinuousAxisCommand EXTENDS itfAxisCommand
    PROPERTY InVelocity : BOOL
    GET
    END_GET
    END_PROPERTY
END_INTERFACE

INTERFACE itfContinousAxisCommand EXTENDS itfContinuousAxisCommand
END_INTERFACE

INTERFACE itfSynchronizedAxisCommand EXTENDS itfAxisCommand
    PROPERTY InSync : BOOL
    GET
    END_GET
    END_PROPERTY
END_INTERFACE

INTERFACE itfSynchronizedCommand EXTENDS itfSynchronizedAxisCommand
END_INTERFACE

INTERFACE itfCamTable
    METHOD Select : itfCommand
    VAR_INPUT
        Periodic : BOOL;
        MasterAbsolute : BOOL;
        SlaveAbsolute : BOOL;
        CamTable : MC_CAM_REF;
    END_VAR
    END_METHOD
END_INTERFACE

INTERFACE itfAxis
    PROPERTY ActualPosition : REAL
    GET
    END_GET
    END_PROPERTY

    PROPERTY ActualTorque : REAL
    GET
    END_GET
    END_PROPERTY

    PROPERTY ActualVelocity : REAL
    GET
    END_GET
    END_PROPERTY

    PROPERTY AxisWarning : BOOL
    GET
    END_GET
    END_PROPERTY

    PROPERTY CommunicationReady : BOOL
    GET
    END_GET
    END_PROPERTY

    PROPERTY Direction : MC_AXIS_DIRECTION
    GET
    END_GET
    END_PROPERTY

    PROPERTY ErrorId : MC_ERROR
    GET
    END_GET
    END_PROPERTY

    PROPERTY HomeAbsSwitch : BOOL
    GET
    END_GET
    END_PROPERTY

    PROPERTY IsHomed : BOOL
    GET
    END_GET
    END_PROPERTY

    PROPERTY LimitSwitchNegative : BOOL
    GET
    END_GET
    END_PROPERTY

    PROPERTY LimitSwitchPositive : BOOL
    GET
    END_GET
    END_PROPERTY

    PROPERTY MotionStatus : MC_MOTION_STATUS
    GET
    END_GET
    END_PROPERTY

    PROPERTY PowerOn : BOOL
    GET
    END_GET
    END_PROPERTY

    PROPERTY ReadyForPowerOn : BOOL
    GET
    END_GET
    END_PROPERTY

    PROPERTY Simulation : BOOL
    GET
    END_GET
    END_PROPERTY

    PROPERTY Status : MC_AXIS_STATUS
    GET
    END_GET
    END_PROPERTY

    METHOD Power : itfCommand
    VAR_INPUT
        Enable : BOOL;
        EnablePositive : BOOL;
        EnableNegative : BOOL;
    END_VAR
    END_METHOD

    METHOD ReadBoolParameter : MC_ERROR
    VAR_INPUT
        ParameterNumber : INT;
    END_VAR
    VAR_OUTPUT
        Value : BOOL;
    END_VAR
    END_METHOD

    METHOD ReadParameter : MC_ERROR
    VAR_INPUT
        ParameterNumber : INT;
    END_VAR
    VAR_OUTPUT
        Value : REAL;
    END_VAR
    END_METHOD

    METHOD Release : MC_ERROR
    END_METHOD

    METHOD Reset : MC_ERROR
    END_METHOD

    METHOD SetOverride : MC_ERROR
    VAR_INPUT
        VelFactor : REAL;
        AccFactor : REAL;
        JerkFactor : REAL;
    END_VAR
    END_METHOD

    METHOD SetPosition : itfCommand
    VAR_INPUT
        Position : REAL;
        Relative : BOOL;
        ExecutionMode : MC_EXECUTION_MODE;
    END_VAR
    END_METHOD

    METHOD WriteBoolParameter : MC_ERROR
    VAR_INPUT
        ParameterNumber : INT;
        Value : BOOL;
    END_VAR
    END_METHOD

    METHOD WriteParameter : MC_ERROR
    VAR_INPUT
        ParameterNumber : INT;
        Value : REAL;
    END_VAR
    END_METHOD

    METHOD DigitalCamSwitch : itfCommand
    VAR_INPUT
        Switches : MC_CAMSWITCH_REF;
        Outputs : MC_OUTPUT_REF;
        TrackOptions : MC_TRACK_REF;
        Enable : BOOL;
        EnableMask : DWORD;
        ValueSource : MC_SOURCE;
    END_VAR
    END_METHOD

    METHOD TouchProbe : itfCommand
    VAR_INPUT
        TriggerInput : MC_TRIGGER_REF;
        WindowOnly : BOOL;
        FirstPosition : REAL;
        LastPosition : REAL;
    END_VAR
    END_METHOD

    METHOD AbortTrigger : itfCommand
    VAR_INPUT
        TriggerInput : MC_TRIGGER_REF;
    END_VAR
    END_METHOD

    METHOD Home : itfAxisCommand
    VAR_INPUT
        Position : REAL;
        BufferMode : MC_BUFFER_MODE;
    END_VAR
    END_METHOD

    METHOD Stop : itfAxisCommand
    VAR_INPUT
        Deceleration : REAL;
        Jerk : REAL;
    END_VAR
    END_METHOD

    METHOD Halt : itfAxisCommand
    VAR_INPUT
        Deceleration : REAL;
        Jerk : REAL;
        BufferMode : MC_BUFFER_MODE;
    END_VAR
    END_METHOD

    METHOD MoveAbsolute : itfAxisCommand
    VAR_INPUT
        Position : REAL;
        Velocity : REAL;
        Acceleration : REAL;
        Deceleration : REAL;
        Jerk : REAL;
        Direction : MC_DIRECTION;
        BufferMode : MC_BUFFER_MODE;
    END_VAR
    END_METHOD

    METHOD MoveRelative : itfAxisCommand
    VAR_INPUT
        Distance : REAL;
        Velocity : REAL;
        Acceleration : REAL;
        Deceleration : REAL;
        Jerk : REAL;
        BufferMode : MC_BUFFER_MODE;
    END_VAR
    END_METHOD

    METHOD MoveAdditive : itfAxisCommand
    VAR_INPUT
        Distance : REAL;
        Velocity : REAL;
        Acceleration : REAL;
        Deceleration : REAL;
        Jerk : REAL;
        BufferMode : MC_BUFFER_MODE;
    END_VAR
    END_METHOD

    METHOD MoveSuperimposed : itfAxisCommand
    VAR_INPUT
        Distance : REAL;
        VelocityDiff : REAL;
        Acceleration : REAL;
        Deceleration : REAL;
        Jerk : REAL;
    END_VAR
    END_METHOD

    METHOD HaltSuperimposed : itfAxisCommand
    VAR_INPUT
        Deceleration : REAL;
        Jerk : REAL;
    END_VAR
    END_METHOD

    METHOD MoveVelocity : itfContinuousAxisCommand
    VAR_INPUT
        Velocity : REAL;
        Acceleration : REAL;
        Deceleration : REAL;
        Jerk : REAL;
        BufferMode : MC_BUFFER_MODE;
    END_VAR
    END_METHOD

    METHOD MoveContinuousAbsolute : itfContinuousAxisCommand
    VAR_INPUT
        Position : REAL;
        EndVelocity : REAL;
        Velocity : REAL;
        Acceleration : REAL;
        Deceleration : REAL;
        Jerk : REAL;
        Direction : MC_DIRECTION;
        BufferMode : MC_BUFFER_MODE;
    END_VAR
    END_METHOD

    METHOD MoveContinuousRelative : itfContinuousAxisCommand
    VAR_INPUT
        Distance : REAL;
        EndVelocity : REAL;
        Velocity : REAL;
        Acceleration : REAL;
        Deceleration : REAL;
        Jerk : REAL;
        Direction : MC_DIRECTION;
        BufferMode : MC_BUFFER_MODE;
    END_VAR
    END_METHOD

    METHOD TorqueControl : itfAxisCommand
    VAR_INPUT
        Torque : REAL;
        TorqueRamp : REAL;
        Velocity : REAL;
        Acceleration : REAL;
        Deceleration : REAL;
        Jerk : REAL;
        Direction : MC_DIRECTION;
        BufferMode : MC_BUFFER_MODE;
    END_VAR
    END_METHOD

    METHOD PositionProfile : itfCommand
    VAR_INPUT
        TimeScale : REAL;
        PositionScale : REAL;
        Offset : REAL;
        BufferMode : MC_BUFFER_MODE;
    END_VAR
    END_METHOD

    METHOD VelocityProfile : itfCommand
    VAR_INPUT
        TimeScale : REAL;
        VelocityScale : REAL;
        Offset : REAL;
        BufferMode : MC_BUFFER_MODE;
    END_VAR
    END_METHOD

    METHOD AccelerationProfile : itfCommand
    VAR_INPUT
        TimeScale : REAL;
        AccelerationScale : REAL;
        Offset : REAL;
        BufferMode : MC_BUFFER_MODE;
    END_VAR
    END_METHOD

    METHOD CamIn : itfSynchronizedAxisCommand
    VAR_INPUT
        Master : itfAxis;
        MasterOffset : REAL;
        SlaveOffset : REAL;
        MasterScaling : REAL;
        SlaveScaling : REAL;
        MasterStartDistance : REAL;
        MasterSyncPosition : REAL;
        StartMode : MC_START_MODE;
        MasterValueSource : MC_SOURCE;
        CamTable : itfCamTable;
        BufferMode : MC_BUFFER_MODE;
    END_VAR
    END_METHOD

    METHOD GearIn : itfSynchronizedAxisCommand
    VAR_INPUT
        Master : itfAxis;
        RatioNumerator : REAL;
        RatioDenominator : REAL;
        MasterValueSource : MC_SOURCE;
        Acceleration : REAL;
        Deceleration : REAL;
        Jerk : REAL;
        BufferMode : MC_BUFFER_MODE;
    END_VAR
    END_METHOD

    METHOD GearInPos : itfSynchronizedAxisCommand
    VAR_INPUT
        Master : itfAxis;
        RatioNumerator : REAL;
        RatioDenominator : REAL;
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
    END_METHOD

    METHOD PhasingAbsolute : itfSynchronizedAxisCommand
    VAR_INPUT
        Master : itfAxis;
        PhaseShift : REAL;
        Velocity : REAL;
        Acceleration : REAL;
        Deceleration : REAL;
        Jerk : REAL;
        BufferMode : MC_BUFFER_MODE;
    END_VAR
    END_METHOD

    METHOD PhasingRelative : itfSynchronizedAxisCommand
    VAR_INPUT
        Master : itfAxis;
        PhaseShift : REAL;
        Velocity : REAL;
        Acceleration : REAL;
        Deceleration : REAL;
        Jerk : REAL;
        BufferMode : MC_BUFFER_MODE;
    END_VAR
    END_METHOD

    METHOD CombineAxes : itfSynchronizedAxisCommand
    VAR_INPUT
        Master1 : itfAxis;
        Master2 : itfAxis;
        CombineMode : MC_COMBINE_MODE;
        GearRatioNumeratorM1 : INT;
        GearRatioDenominatorM1 : INT;
        GearRatioNumeratorM2 : INT;
        GearRatioDenominatorM2 : INT;
        MasterValueSourceM1 : MC_SOURCE;
        MasterValueSourceM2 : MC_SOURCE;
        BufferMode : MC_BUFFER_MODE;
    END_VAR
    END_METHOD
END_INTERFACE
