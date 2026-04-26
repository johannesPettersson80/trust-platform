FUNCTION_BLOCK MC_OopAxis IMPLEMENTS itfAxis
VAR
    AxisData : AXIS_REF;
    ActiveKind : TRUST_OOP_ACTIVE_COMMAND_KIND := TRUST_OOP_ACTIVE_COMMAND_KIND#trustOopCommandNone;

    Constants : MC_Constants;
    PowerFb : MC_Power;
    HomeFb : MC_Home;
    StopFb : MC_Stop;
    HaltFb : MC_Halt;
    MoveAbsoluteFb : MC_MoveAbsolute;
    MoveRelativeFb : MC_MoveRelative;
    MoveAdditiveFb : MC_MoveAdditive;
    MoveVelocityFb : MC_MoveVelocity;
    MoveContinuousAbsoluteFb : MC_MoveContinuousAbsolute;
    MoveContinuousRelativeFb : MC_MoveContinuousRelative;
    SetPositionFb : MC_SetPosition;
    SetOverrideFb : MC_SetOverride;
    ResetFb : MC_Reset;
    ReadActualPositionFb : MC_ReadActualPosition;
    ReadActualVelocityFb : MC_ReadActualVelocity;
    ReadActualTorqueFb : MC_ReadActualTorque;
    ReadStatusFb : MC_ReadStatus;
    ReadMotionStateFb : MC_ReadMotionState;
    ReadAxisInfoFb : MC_ReadAxisInfo;
    ReadAxisErrorFb : MC_ReadAxisError;
    ReadParameterFb : MC_ReadParameter;
    ReadBoolParameterFb : MC_ReadBoolParameter;
    WriteParameterFb : MC_WriteParameter;
    WriteBoolParameterFb : MC_WriteBoolParameter;

    PowerCommand : MC_OopCommand;
    SetPositionCommand : MC_OopCommand;
    UnsupportedCommand : MC_OopCommand;
    HomeCommand : MC_OopAxisCommand;
    StopCommand : MC_OopAxisCommand;
    HaltCommand : MC_OopAxisCommand;
    MoveAbsoluteCommand : MC_OopAxisCommand;
    MoveRelativeCommand : MC_OopAxisCommand;
    MoveAdditiveCommand : MC_OopAxisCommand;
    UnsupportedAxisCommand : MC_OopAxisCommand;
    MoveVelocityCommand : MC_OopContinuousAxisCommand;
    MoveContinuousAbsoluteCommand : MC_OopContinuousAxisCommand;
    MoveContinuousRelativeCommand : MC_OopContinuousAxisCommand;
    UnsupportedContinuousCommand : MC_OopContinuousAxisCommand;
    UnsupportedSynchronizedCommand : MC_OopSynchronizedAxisCommand;

    LastHomePosition : REAL;
    LastHomeBufferMode : MC_BUFFER_MODE;
    LastStopDeceleration : REAL;
    LastStopJerk : REAL;
    LastHaltDeceleration : REAL;
    LastHaltJerk : REAL;
    LastHaltBufferMode : MC_BUFFER_MODE;
    LastMovePosition : REAL;
    LastMoveDistance : REAL;
    LastEndVelocity : REAL;
    LastVelocity : REAL;
    LastAcceleration : REAL;
    LastDeceleration : REAL;
    LastJerk : REAL;
    LastDirection : MC_DIRECTION;
    LastBufferMode : MC_BUFFER_MODE;

    ActualPositionValue : REAL;
    ActualTorqueValue : REAL;
    ActualVelocityValue : REAL;
    AxisWarningValue : BOOL;
    CommunicationReadyValue : BOOL;
    DirectionValue : MC_AXIS_DIRECTION;
    ErrorIdValue : MC_ERROR;
    HomeAbsSwitchValue : BOOL;
    IsHomedValue : BOOL;
    LimitSwitchNegativeValue : BOOL;
    LimitSwitchPositiveValue : BOOL;
    MotionStatusValue : MC_MOTION_STATUS;
    PowerOnValue : BOOL;
    ReadyForPowerOnValue : BOOL;
    SimulationValue : BOOL;
    StatusValue : MC_AXIS_STATUS;
END_VAR

METHOD PUBLIC Bind : MC_ERROR
VAR_INPUT
    AxisId : UDINT;
    InternalIndex : UINT;
END_VAR
AxisData.AxisId := AxisId;
AxisData.InternalIndex := InternalIndex;
Bind := WORD#16#0000;
END_METHOD

METHOD PUBLIC Refresh : MC_ERROR
ReadActualPositionFb(Axis := AxisData, Enable := TRUE);
ReadActualVelocityFb(Axis := AxisData, Enable := TRUE);
ReadActualTorqueFb(Axis := AxisData, Enable := TRUE);
ReadStatusFb(Axis := AxisData, Enable := TRUE);
ReadMotionStateFb(Axis := AxisData, Enable := TRUE, Source := MC_SOURCE#mcActualValue);
ReadAxisInfoFb(Axis := AxisData, Enable := TRUE);
ReadAxisErrorFb(Axis := AxisData, Enable := TRUE);

IF ReadActualPositionFb.Valid THEN
    ActualPositionValue := ReadActualPositionFb.Position;
END_IF;
IF ReadActualVelocityFb.Valid THEN
    ActualVelocityValue := ReadActualVelocityFb.Velocity;
END_IF;
IF ReadActualTorqueFb.Valid THEN
    ActualTorqueValue := ReadActualTorqueFb.Torque;
END_IF;
IF ReadStatusFb.Valid THEN
    IF ReadStatusFb.ErrorStop THEN
        StatusValue := MC_AXIS_STATUS#mcErrorStop;
    ELSIF ReadStatusFb.Disabled THEN
        StatusValue := MC_AXIS_STATUS#mcDisabled;
    ELSIF ReadStatusFb.Stopping THEN
        StatusValue := MC_AXIS_STATUS#mcStopping;
    ELSIF ReadStatusFb.Homing THEN
        StatusValue := MC_AXIS_STATUS#mcHoming;
    ELSIF ReadStatusFb.DiscreteMotion THEN
        StatusValue := MC_AXIS_STATUS#mcDiscreteMotion;
    ELSIF ReadStatusFb.ContinuousMotion THEN
        StatusValue := MC_AXIS_STATUS#mcContinuousMotion;
    ELSIF ReadStatusFb.SynchronizedMotion THEN
        StatusValue := MC_AXIS_STATUS#mcSynchronizedMotion;
    ELSE
        StatusValue := MC_AXIS_STATUS#mcStandstill;
    END_IF;
END_IF;
IF ReadMotionStateFb.Valid THEN
    IF ReadMotionStateFb.DirectionNegative THEN
        DirectionValue := MC_AXIS_DIRECTION#mcDirectionNegative;
    ELSE
        DirectionValue := MC_AXIS_DIRECTION#mcDirectionPositive;
    END_IF;
    IF ReadMotionStateFb.Accelerating THEN
        MotionStatusValue := MC_MOTION_STATUS#mcAccelerating;
    ELSIF ReadMotionStateFb.Decelerating THEN
        MotionStatusValue := MC_MOTION_STATUS#mcDecelerating;
    ELSE
        MotionStatusValue := MC_MOTION_STATUS#mcConstantVelocity;
    END_IF;
END_IF;
IF ReadAxisInfoFb.Valid THEN
    AxisWarningValue := ReadAxisInfoFb.AxisWarning;
    CommunicationReadyValue := ReadAxisInfoFb.CommunicationReady;
    HomeAbsSwitchValue := ReadAxisInfoFb.HomeAbsSwitch;
    IsHomedValue := ReadAxisInfoFb.IsHomed;
    LimitSwitchNegativeValue := ReadAxisInfoFb.LimitSwitchNeg;
    LimitSwitchPositiveValue := ReadAxisInfoFb.LimitSwitchPos;
    PowerOnValue := ReadAxisInfoFb.PowerOn;
    ReadyForPowerOnValue := ReadAxisInfoFb.ReadyForPowerOn;
    SimulationValue := ReadAxisInfoFb.Simulation;
END_IF;
IF ReadAxisErrorFb.Valid THEN
    ErrorIdValue := ReadAxisErrorFb.AxisErrorID;
END_IF;
Refresh := WORD#16#0000;
END_METHOD

METHOD PRIVATE RefreshAndReturnOk : MC_ERROR
RefreshAndReturnOk := Refresh();
END_METHOD

METHOD PRIVATE UpdateHomeCommand : BOOL
UpdateHomeCommand := HomeCommand.SetStatus(
    Done := HomeFb.Done,
    Busy := HomeFb.Busy,
    Active := HomeFb.Active,
    CommandAborted := HomeFb.CommandAborted,
    Error := HomeFb.Error,
    ErrorId := HomeFb.ErrorID
);
END_METHOD

METHOD PRIVATE UpdateStopCommand : BOOL
UpdateStopCommand := StopCommand.SetStatus(
    Done := StopFb.Done,
    Busy := StopFb.Busy,
    Active := StopFb.Busy,
    CommandAborted := StopFb.CommandAborted,
    Error := StopFb.Error,
    ErrorId := StopFb.ErrorID
);
END_METHOD

METHOD PRIVATE UpdateHaltCommand : BOOL
UpdateHaltCommand := HaltCommand.SetStatus(
    Done := HaltFb.Done,
    Busy := HaltFb.Busy,
    Active := HaltFb.Active,
    CommandAborted := HaltFb.CommandAborted,
    Error := HaltFb.Error,
    ErrorId := HaltFb.ErrorID
);
END_METHOD

METHOD PRIVATE UpdateMoveAbsoluteCommand : BOOL
UpdateMoveAbsoluteCommand := MoveAbsoluteCommand.SetStatus(
    Done := MoveAbsoluteFb.Done,
    Busy := MoveAbsoluteFb.Busy,
    Active := MoveAbsoluteFb.Active,
    CommandAborted := MoveAbsoluteFb.CommandAborted,
    Error := MoveAbsoluteFb.Error,
    ErrorId := MoveAbsoluteFb.ErrorID
);
END_METHOD

METHOD PRIVATE UpdateMoveRelativeCommand : BOOL
UpdateMoveRelativeCommand := MoveRelativeCommand.SetStatus(
    Done := MoveRelativeFb.Done,
    Busy := MoveRelativeFb.Busy,
    Active := MoveRelativeFb.Active,
    CommandAborted := MoveRelativeFb.CommandAborted,
    Error := MoveRelativeFb.Error,
    ErrorId := MoveRelativeFb.ErrorID
);
END_METHOD

METHOD PRIVATE UpdateMoveAdditiveCommand : BOOL
UpdateMoveAdditiveCommand := MoveAdditiveCommand.SetStatus(
    Done := MoveAdditiveFb.Done,
    Busy := MoveAdditiveFb.Busy,
    Active := MoveAdditiveFb.Active,
    CommandAborted := MoveAdditiveFb.CommandAborted,
    Error := MoveAdditiveFb.Error,
    ErrorId := MoveAdditiveFb.ErrorID
);
END_METHOD

METHOD PRIVATE UpdateMoveVelocityCommand : BOOL
UpdateMoveVelocityCommand := MoveVelocityCommand.SetContinuousStatus(
    Done := MoveVelocityFb.InVelocity,
    Busy := MoveVelocityFb.Busy,
    Active := MoveVelocityFb.Active,
    CommandAborted := MoveVelocityFb.CommandAborted,
    Error := MoveVelocityFb.Error,
    ErrorId := MoveVelocityFb.ErrorID,
    InVelocity := MoveVelocityFb.InVelocity
);
END_METHOD

METHOD PRIVATE UpdateMoveContinuousAbsoluteCommand : BOOL
UpdateMoveContinuousAbsoluteCommand := MoveContinuousAbsoluteCommand.SetContinuousStatus(
    Done := MoveContinuousAbsoluteFb.InEndVelocity,
    Busy := MoveContinuousAbsoluteFb.Busy,
    Active := MoveContinuousAbsoluteFb.Active,
    CommandAborted := MoveContinuousAbsoluteFb.CommandAborted,
    Error := MoveContinuousAbsoluteFb.Error,
    ErrorId := MoveContinuousAbsoluteFb.ErrorID,
    InVelocity := MoveContinuousAbsoluteFb.InEndVelocity
);
END_METHOD

METHOD PRIVATE UpdateMoveContinuousRelativeCommand : BOOL
UpdateMoveContinuousRelativeCommand := MoveContinuousRelativeCommand.SetContinuousStatus(
    Done := MoveContinuousRelativeFb.InEndVelocity,
    Busy := MoveContinuousRelativeFb.Busy,
    Active := MoveContinuousRelativeFb.Active,
    CommandAborted := MoveContinuousRelativeFb.CommandAborted,
    Error := MoveContinuousRelativeFb.Error,
    ErrorId := MoveContinuousRelativeFb.ErrorID,
    InVelocity := MoveContinuousRelativeFb.InEndVelocity
);
END_METHOD

METHOD PUBLIC Power : itfCommand
VAR_INPUT
    Enable : BOOL;
    EnablePositive : BOOL;
    EnableNegative : BOOL;
END_VAR
VAR
    Updated : BOOL;
    Ignored : MC_ERROR;
END_VAR
PowerFb(Axis := AxisData, Enable := Enable);
Ignored := Refresh();
Updated := PowerCommand.SetStatus(
    Done := PowerFb.Status AND PowerFb.Valid AND NOT PowerFb.Error,
    Busy := FALSE,
    Active := Enable,
    CommandAborted := FALSE,
    Error := PowerFb.Error,
    ErrorId := PowerFb.ErrorID
);
Power := PowerCommand;
END_METHOD

METHOD PUBLIC ReadBoolParameter : MC_ERROR
VAR_INPUT
    ParameterNumber : INT;
END_VAR
VAR_OUTPUT
    Value : BOOL;
END_VAR
ReadBoolParameterFb(Axis := AxisData, Enable := TRUE, ParameterNumber := ParameterNumber);
Value := ReadBoolParameterFb.Value;
IF ReadBoolParameterFb.Error THEN
    ReadBoolParameter := ReadBoolParameterFb.ErrorID;
ELSE
    ReadBoolParameter := WORD#16#0000;
END_IF;
END_METHOD

METHOD PUBLIC ReadParameter : MC_ERROR
VAR_INPUT
    ParameterNumber : INT;
END_VAR
VAR_OUTPUT
    Value : REAL;
END_VAR
ReadParameterFb(Axis := AxisData, Enable := TRUE, ParameterNumber := ParameterNumber);
Value := ReadParameterFb.Value;
IF ReadParameterFb.Error THEN
    ReadParameter := ReadParameterFb.ErrorID;
ELSE
    ReadParameter := WORD#16#0000;
END_IF;
END_METHOD

METHOD PUBLIC Release : MC_ERROR
VAR
    Updated : BOOL;
    Ignored : MC_ERROR;
END_VAR
CASE ActiveKind OF
    TRUST_OOP_ACTIVE_COMMAND_KIND#trustOopCommandHome:
        HomeFb(Axis := AxisData, Execute := FALSE, Position := LastHomePosition, BufferMode := LastHomeBufferMode);
        Updated := UpdateHomeCommand();
    TRUST_OOP_ACTIVE_COMMAND_KIND#trustOopCommandStop:
        StopFb(Axis := AxisData, Execute := FALSE, Deceleration := LastStopDeceleration, Jerk := LastStopJerk);
        Updated := UpdateStopCommand();
    TRUST_OOP_ACTIVE_COMMAND_KIND#trustOopCommandHalt:
        HaltFb(Axis := AxisData, Execute := FALSE, Deceleration := LastHaltDeceleration, Jerk := LastHaltJerk, BufferMode := LastHaltBufferMode);
        Updated := UpdateHaltCommand();
    TRUST_OOP_ACTIVE_COMMAND_KIND#trustOopCommandMoveAbsolute:
        MoveAbsoluteFb(
            Axis := AxisData,
            Execute := FALSE,
            ContinuousUpdate := FALSE,
            Position := LastMovePosition,
            Velocity := LastVelocity,
            Acceleration := LastAcceleration,
            Deceleration := LastDeceleration,
            Jerk := LastJerk,
            Direction := LastDirection,
            BufferMode := LastBufferMode
        );
        Updated := UpdateMoveAbsoluteCommand();
    TRUST_OOP_ACTIVE_COMMAND_KIND#trustOopCommandMoveRelative:
        MoveRelativeFb(
            Axis := AxisData,
            Execute := FALSE,
            ContinuousUpdate := FALSE,
            Distance := LastMoveDistance,
            Velocity := LastVelocity,
            Acceleration := LastAcceleration,
            Deceleration := LastDeceleration,
            Jerk := LastJerk,
            BufferMode := LastBufferMode
        );
        Updated := UpdateMoveRelativeCommand();
    TRUST_OOP_ACTIVE_COMMAND_KIND#trustOopCommandMoveAdditive:
        MoveAdditiveFb(
            Axis := AxisData,
            Execute := FALSE,
            ContinuousUpdate := FALSE,
            Distance := LastMoveDistance,
            Velocity := LastVelocity,
            Acceleration := LastAcceleration,
            Deceleration := LastDeceleration,
            Jerk := LastJerk,
            BufferMode := LastBufferMode
        );
        Updated := UpdateMoveAdditiveCommand();
    TRUST_OOP_ACTIVE_COMMAND_KIND#trustOopCommandMoveVelocity:
        MoveVelocityFb(
            Axis := AxisData,
            Execute := FALSE,
            ContinuousUpdate := FALSE,
            Velocity := LastVelocity,
            Acceleration := LastAcceleration,
            Deceleration := LastDeceleration,
            Jerk := LastJerk,
            Direction := LastDirection,
            BufferMode := LastBufferMode
        );
        Updated := UpdateMoveVelocityCommand();
    TRUST_OOP_ACTIVE_COMMAND_KIND#trustOopCommandMoveContinuousAbsolute:
        MoveContinuousAbsoluteFb(
            Axis := AxisData,
            Execute := FALSE,
            ContinuousUpdate := FALSE,
            Position := LastMovePosition,
            EndVelocity := LastEndVelocity,
            Velocity := LastVelocity,
            Acceleration := LastAcceleration,
            Deceleration := LastDeceleration,
            Jerk := LastJerk,
            Direction := LastDirection,
            BufferMode := LastBufferMode
        );
        Updated := UpdateMoveContinuousAbsoluteCommand();
    TRUST_OOP_ACTIVE_COMMAND_KIND#trustOopCommandMoveContinuousRelative:
        MoveContinuousRelativeFb(
            Axis := AxisData,
            Execute := FALSE,
            ContinuousUpdate := FALSE,
            Distance := LastMoveDistance,
            EndVelocity := LastEndVelocity,
            Velocity := LastVelocity,
            Acceleration := LastAcceleration,
            Deceleration := LastDeceleration,
            Jerk := LastJerk,
            BufferMode := LastBufferMode
        );
        Updated := UpdateMoveContinuousRelativeCommand();
ELSE
    Updated := TRUE;
END_CASE;
ActiveKind := TRUST_OOP_ACTIVE_COMMAND_KIND#trustOopCommandNone;
Ignored := Refresh();
Release := WORD#16#0000;
END_METHOD

METHOD PUBLIC Reset : MC_ERROR
VAR
    Ignored : MC_ERROR;
END_VAR
ResetFb(Axis := AxisData, Execute := TRUE);
ResetFb(Axis := AxisData, Execute := FALSE);
Ignored := Refresh();
IF ResetFb.Error THEN
    Reset := ResetFb.ErrorID;
ELSE
    Reset := WORD#16#0000;
END_IF;
END_METHOD

METHOD PUBLIC SetOverride : MC_ERROR
VAR_INPUT
    VelFactor : REAL;
    AccFactor : REAL;
    JerkFactor : REAL;
END_VAR
VAR
    Ignored : MC_ERROR;
END_VAR
SetOverrideFb(Axis := AxisData, Enable := TRUE, VelFactor := VelFactor, AccFactor := AccFactor, JerkFactor := JerkFactor);
Ignored := Refresh();
IF SetOverrideFb.Error THEN
    SetOverride := SetOverrideFb.ErrorID;
ELSE
    SetOverride := WORD#16#0000;
END_IF;
END_METHOD

METHOD PUBLIC SetPosition : itfCommand
VAR_INPUT
    Position : REAL;
    Relative : BOOL;
    ExecutionMode : MC_EXECUTION_MODE;
END_VAR
VAR
    Updated : BOOL;
    Ignored : MC_ERROR;
END_VAR
SetPositionFb(Axis := AxisData, Execute := TRUE, Position := Position, Relative := Relative, ExecutionMode := ExecutionMode);
SetPositionFb(Axis := AxisData, Execute := FALSE, Position := Position, Relative := Relative, ExecutionMode := ExecutionMode);
Ignored := Refresh();
Updated := SetPositionCommand.SetStatus(
    Done := SetPositionFb.Done,
    Busy := SetPositionFb.Busy,
    Active := FALSE,
    CommandAborted := FALSE,
    Error := SetPositionFb.Error,
    ErrorId := SetPositionFb.ErrorID
);
SetPosition := SetPositionCommand;
END_METHOD

METHOD PUBLIC WriteBoolParameter : MC_ERROR
VAR_INPUT
    ParameterNumber : INT;
    Value : BOOL;
END_VAR
WriteBoolParameterFb(Axis := AxisData, Execute := TRUE, ParameterNumber := ParameterNumber, Value := Value, ExecutionMode := MC_EXECUTION_MODE#mcImmediately);
WriteBoolParameterFb(Axis := AxisData, Execute := FALSE, ParameterNumber := ParameterNumber, Value := Value, ExecutionMode := MC_EXECUTION_MODE#mcImmediately);
IF WriteBoolParameterFb.Error THEN
    WriteBoolParameter := WriteBoolParameterFb.ErrorID;
ELSE
    WriteBoolParameter := WORD#16#0000;
END_IF;
END_METHOD

METHOD PUBLIC WriteParameter : MC_ERROR
VAR_INPUT
    ParameterNumber : INT;
    Value : REAL;
END_VAR
WriteParameterFb(Axis := AxisData, Execute := TRUE, ParameterNumber := ParameterNumber, Value := Value, ExecutionMode := MC_EXECUTION_MODE#mcImmediately);
WriteParameterFb(Axis := AxisData, Execute := FALSE, ParameterNumber := ParameterNumber, Value := Value, ExecutionMode := MC_EXECUTION_MODE#mcImmediately);
IF WriteParameterFb.Error THEN
    WriteParameter := WriteParameterFb.ErrorID;
ELSE
    WriteParameter := WORD#16#0000;
END_IF;
END_METHOD

METHOD PUBLIC Home : itfAxisCommand
VAR_INPUT
    Position : REAL;
    BufferMode : MC_BUFFER_MODE;
END_VAR
VAR
    Updated : BOOL;
    Ignored : MC_ERROR;
END_VAR
LastHomePosition := Position;
LastHomeBufferMode := BufferMode;
HomeFb(Axis := AxisData, Execute := TRUE, Position := Position, BufferMode := BufferMode);
ActiveKind := TRUST_OOP_ACTIVE_COMMAND_KIND#trustOopCommandHome;
Ignored := Refresh();
Updated := UpdateHomeCommand();
Home := HomeCommand;
END_METHOD

METHOD PUBLIC Stop : itfAxisCommand
VAR_INPUT
    Deceleration : REAL;
    Jerk : REAL;
END_VAR
VAR
    Updated : BOOL;
    Ignored : MC_ERROR;
END_VAR
LastStopDeceleration := Deceleration;
LastStopJerk := Jerk;
StopFb(Axis := AxisData, Execute := TRUE, Deceleration := Deceleration, Jerk := Jerk);
ActiveKind := TRUST_OOP_ACTIVE_COMMAND_KIND#trustOopCommandStop;
Ignored := Refresh();
Updated := UpdateStopCommand();
Stop := StopCommand;
END_METHOD

METHOD PUBLIC Halt : itfAxisCommand
VAR_INPUT
    Deceleration : REAL;
    Jerk : REAL;
    BufferMode : MC_BUFFER_MODE;
END_VAR
VAR
    Updated : BOOL;
    Ignored : MC_ERROR;
END_VAR
LastHaltDeceleration := Deceleration;
LastHaltJerk := Jerk;
LastHaltBufferMode := BufferMode;
HaltFb(Axis := AxisData, Execute := TRUE, Deceleration := Deceleration, Jerk := Jerk, BufferMode := BufferMode);
ActiveKind := TRUST_OOP_ACTIVE_COMMAND_KIND#trustOopCommandHalt;
Ignored := Refresh();
Updated := UpdateHaltCommand();
Halt := HaltCommand;
END_METHOD

METHOD PUBLIC MoveAbsolute : itfAxisCommand
VAR_INPUT
    Position : REAL;
    Velocity : REAL;
    Acceleration : REAL;
    Deceleration : REAL;
    Jerk : REAL;
    Direction : MC_DIRECTION;
    BufferMode : MC_BUFFER_MODE;
END_VAR
VAR
    Updated : BOOL;
    Ignored : MC_ERROR;
END_VAR
LastMovePosition := Position;
LastVelocity := Velocity;
LastAcceleration := Acceleration;
LastDeceleration := Deceleration;
LastJerk := Jerk;
LastDirection := Direction;
LastBufferMode := BufferMode;
MoveAbsoluteFb(
    Axis := AxisData,
    Execute := TRUE,
    ContinuousUpdate := FALSE,
    Position := Position,
    Velocity := Velocity,
    Acceleration := Acceleration,
    Deceleration := Deceleration,
    Jerk := Jerk,
    Direction := Direction,
    BufferMode := BufferMode
);
ActiveKind := TRUST_OOP_ACTIVE_COMMAND_KIND#trustOopCommandMoveAbsolute;
Ignored := Refresh();
Updated := UpdateMoveAbsoluteCommand();
MoveAbsolute := MoveAbsoluteCommand;
END_METHOD

METHOD PUBLIC MoveRelative : itfAxisCommand
VAR_INPUT
    Distance : REAL;
    Velocity : REAL;
    Acceleration : REAL;
    Deceleration : REAL;
    Jerk : REAL;
    BufferMode : MC_BUFFER_MODE;
END_VAR
VAR
    Updated : BOOL;
    Ignored : MC_ERROR;
END_VAR
LastMoveDistance := Distance;
LastVelocity := Velocity;
LastAcceleration := Acceleration;
LastDeceleration := Deceleration;
LastJerk := Jerk;
LastBufferMode := BufferMode;
MoveRelativeFb(
    Axis := AxisData,
    Execute := TRUE,
    ContinuousUpdate := FALSE,
    Distance := Distance,
    Velocity := Velocity,
    Acceleration := Acceleration,
    Deceleration := Deceleration,
    Jerk := Jerk,
    BufferMode := BufferMode
);
ActiveKind := TRUST_OOP_ACTIVE_COMMAND_KIND#trustOopCommandMoveRelative;
Ignored := Refresh();
Updated := UpdateMoveRelativeCommand();
MoveRelative := MoveRelativeCommand;
END_METHOD

METHOD PUBLIC MoveAdditive : itfAxisCommand
VAR_INPUT
    Distance : REAL;
    Velocity : REAL;
    Acceleration : REAL;
    Deceleration : REAL;
    Jerk : REAL;
    BufferMode : MC_BUFFER_MODE;
END_VAR
VAR
    Updated : BOOL;
    Ignored : MC_ERROR;
END_VAR
LastMoveDistance := Distance;
LastVelocity := Velocity;
LastAcceleration := Acceleration;
LastDeceleration := Deceleration;
LastJerk := Jerk;
LastBufferMode := BufferMode;
MoveAdditiveFb(
    Axis := AxisData,
    Execute := TRUE,
    ContinuousUpdate := FALSE,
    Distance := Distance,
    Velocity := Velocity,
    Acceleration := Acceleration,
    Deceleration := Deceleration,
    Jerk := Jerk,
    BufferMode := BufferMode
);
ActiveKind := TRUST_OOP_ACTIVE_COMMAND_KIND#trustOopCommandMoveAdditive;
Ignored := Refresh();
Updated := UpdateMoveAdditiveCommand();
MoveAdditive := MoveAdditiveCommand;
END_METHOD

METHOD PUBLIC MoveVelocity : itfContinuousAxisCommand
VAR_INPUT
    Velocity : REAL;
    Acceleration : REAL;
    Deceleration : REAL;
    Jerk : REAL;
    BufferMode : MC_BUFFER_MODE;
END_VAR
VAR
    Updated : BOOL;
    Ignored : MC_ERROR;
END_VAR
LastVelocity := Velocity;
LastAcceleration := Acceleration;
LastDeceleration := Deceleration;
LastJerk := Jerk;
LastDirection := MC_DIRECTION#mcCurrentDirection;
LastBufferMode := BufferMode;
MoveVelocityFb(
    Axis := AxisData,
    Execute := TRUE,
    ContinuousUpdate := FALSE,
    Velocity := Velocity,
    Acceleration := Acceleration,
    Deceleration := Deceleration,
    Jerk := Jerk,
    Direction := MC_DIRECTION#mcCurrentDirection,
    BufferMode := BufferMode
);
ActiveKind := TRUST_OOP_ACTIVE_COMMAND_KIND#trustOopCommandMoveVelocity;
Ignored := Refresh();
Updated := UpdateMoveVelocityCommand();
MoveVelocity := MoveVelocityCommand;
END_METHOD

METHOD PUBLIC MoveContinuousAbsolute : itfContinuousAxisCommand
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
VAR
    Updated : BOOL;
    Ignored : MC_ERROR;
END_VAR
LastMovePosition := Position;
LastEndVelocity := EndVelocity;
LastVelocity := Velocity;
LastAcceleration := Acceleration;
LastDeceleration := Deceleration;
LastJerk := Jerk;
LastDirection := Direction;
LastBufferMode := BufferMode;
MoveContinuousAbsoluteFb(
    Axis := AxisData,
    Execute := TRUE,
    ContinuousUpdate := FALSE,
    Position := Position,
    EndVelocity := EndVelocity,
    Velocity := Velocity,
    Acceleration := Acceleration,
    Deceleration := Deceleration,
    Jerk := Jerk,
    Direction := Direction,
    BufferMode := BufferMode
);
ActiveKind := TRUST_OOP_ACTIVE_COMMAND_KIND#trustOopCommandMoveContinuousAbsolute;
Ignored := Refresh();
Updated := UpdateMoveContinuousAbsoluteCommand();
MoveContinuousAbsolute := MoveContinuousAbsoluteCommand;
END_METHOD

METHOD PUBLIC MoveContinuousRelative : itfContinuousAxisCommand
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
VAR
    Updated : BOOL;
    Ignored : MC_ERROR;
END_VAR
LastMoveDistance := Distance;
LastEndVelocity := EndVelocity;
LastVelocity := Velocity;
LastAcceleration := Acceleration;
LastDeceleration := Deceleration;
LastJerk := Jerk;
LastDirection := Direction;
LastBufferMode := BufferMode;
MoveContinuousRelativeFb(
    Axis := AxisData,
    Execute := TRUE,
    ContinuousUpdate := FALSE,
    Distance := Distance,
    EndVelocity := EndVelocity,
    Velocity := Velocity,
    Acceleration := Acceleration,
    Deceleration := Deceleration,
    Jerk := Jerk,
    BufferMode := BufferMode
);
ActiveKind := TRUST_OOP_ACTIVE_COMMAND_KIND#trustOopCommandMoveContinuousRelative;
Ignored := Refresh();
Updated := UpdateMoveContinuousRelativeCommand();
MoveContinuousRelative := MoveContinuousRelativeCommand;
END_METHOD

METHOD PRIVATE UnsupportedAsCommand : itfCommand
VAR
    Updated : BOOL;
END_VAR
Updated := UnsupportedCommand.SetError(ErrorId := WORD#16#0500);
UnsupportedAsCommand := UnsupportedCommand;
END_METHOD

METHOD PRIVATE UnsupportedAsAxisCommand : itfAxisCommand
VAR
    Updated : BOOL;
END_VAR
Updated := UnsupportedAxisCommand.SetError(ErrorId := WORD#16#0500);
UnsupportedAsAxisCommand := UnsupportedAxisCommand;
END_METHOD

METHOD PRIVATE UnsupportedAsContinuousCommand : itfContinuousAxisCommand
VAR
    Updated : BOOL;
END_VAR
Updated := UnsupportedContinuousCommand.SetContinuousError(ErrorId := WORD#16#0500);
UnsupportedAsContinuousCommand := UnsupportedContinuousCommand;
END_METHOD

METHOD PRIVATE UnsupportedAsSynchronizedCommand : itfSynchronizedAxisCommand
VAR
    Updated : BOOL;
END_VAR
Updated := UnsupportedSynchronizedCommand.SetSynchronizedError(ErrorId := WORD#16#0500);
UnsupportedAsSynchronizedCommand := UnsupportedSynchronizedCommand;
END_METHOD

METHOD PUBLIC DigitalCamSwitch : itfCommand
VAR_INPUT
    Switches : MC_CAMSWITCH_REF;
    Outputs : MC_OUTPUT_REF;
    TrackOptions : MC_TRACK_REF;
    Enable : BOOL;
    EnableMask : DWORD;
    ValueSource : MC_SOURCE;
END_VAR
DigitalCamSwitch := UnsupportedAsCommand();
END_METHOD

METHOD PUBLIC TouchProbe : itfCommand
VAR_INPUT
    TriggerInput : MC_TRIGGER_REF;
    WindowOnly : BOOL;
    FirstPosition : REAL;
    LastPosition : REAL;
END_VAR
TouchProbe := UnsupportedAsCommand();
END_METHOD

METHOD PUBLIC AbortTrigger : itfCommand
VAR_INPUT
    TriggerInput : MC_TRIGGER_REF;
END_VAR
AbortTrigger := UnsupportedAsCommand();
END_METHOD

METHOD PUBLIC MoveSuperimposed : itfAxisCommand
VAR_INPUT
    Distance : REAL;
    VelocityDiff : REAL;
    Acceleration : REAL;
    Deceleration : REAL;
    Jerk : REAL;
END_VAR
MoveSuperimposed := UnsupportedAsAxisCommand();
END_METHOD

METHOD PUBLIC HaltSuperimposed : itfAxisCommand
VAR_INPUT
    Deceleration : REAL;
    Jerk : REAL;
END_VAR
HaltSuperimposed := UnsupportedAsAxisCommand();
END_METHOD

METHOD PUBLIC TorqueControl : itfAxisCommand
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
TorqueControl := UnsupportedAsAxisCommand();
END_METHOD

METHOD PUBLIC PositionProfile : itfCommand
VAR_INPUT
    TimeScale : REAL;
    PositionScale : REAL;
    Offset : REAL;
    BufferMode : MC_BUFFER_MODE;
END_VAR
PositionProfile := UnsupportedAsCommand();
END_METHOD

METHOD PUBLIC VelocityProfile : itfCommand
VAR_INPUT
    TimeScale : REAL;
    VelocityScale : REAL;
    Offset : REAL;
    BufferMode : MC_BUFFER_MODE;
END_VAR
VelocityProfile := UnsupportedAsCommand();
END_METHOD

METHOD PUBLIC AccelerationProfile : itfCommand
VAR_INPUT
    TimeScale : REAL;
    AccelerationScale : REAL;
    Offset : REAL;
    BufferMode : MC_BUFFER_MODE;
END_VAR
AccelerationProfile := UnsupportedAsCommand();
END_METHOD

METHOD PUBLIC CamIn : itfSynchronizedAxisCommand
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
CamIn := UnsupportedAsSynchronizedCommand();
END_METHOD

METHOD PUBLIC GearIn : itfSynchronizedAxisCommand
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
GearIn := UnsupportedAsSynchronizedCommand();
END_METHOD

METHOD PUBLIC GearInPos : itfSynchronizedAxisCommand
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
GearInPos := UnsupportedAsSynchronizedCommand();
END_METHOD

METHOD PUBLIC PhasingAbsolute : itfSynchronizedAxisCommand
VAR_INPUT
    Master : itfAxis;
    PhaseShift : REAL;
    Velocity : REAL;
    Acceleration : REAL;
    Deceleration : REAL;
    Jerk : REAL;
    BufferMode : MC_BUFFER_MODE;
END_VAR
PhasingAbsolute := UnsupportedAsSynchronizedCommand();
END_METHOD

METHOD PUBLIC PhasingRelative : itfSynchronizedAxisCommand
VAR_INPUT
    Master : itfAxis;
    PhaseShift : REAL;
    Velocity : REAL;
    Acceleration : REAL;
    Deceleration : REAL;
    Jerk : REAL;
    BufferMode : MC_BUFFER_MODE;
END_VAR
PhasingRelative := UnsupportedAsSynchronizedCommand();
END_METHOD

METHOD PUBLIC CombineAxes : itfSynchronizedAxisCommand
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
CombineAxes := UnsupportedAsSynchronizedCommand();
END_METHOD

PUBLIC PROPERTY ActualPosition : REAL
GET
    ActualPosition := ActualPositionValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY ActualTorque : REAL
GET
    ActualTorque := ActualTorqueValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY ActualVelocity : REAL
GET
    ActualVelocity := ActualVelocityValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY AxisWarning : BOOL
GET
    AxisWarning := AxisWarningValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY CommunicationReady : BOOL
GET
    CommunicationReady := CommunicationReadyValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY Direction : MC_AXIS_DIRECTION
GET
    Direction := DirectionValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY ErrorId : MC_ERROR
GET
    ErrorId := ErrorIdValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY HomeAbsSwitch : BOOL
GET
    HomeAbsSwitch := HomeAbsSwitchValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY IsHomed : BOOL
GET
    IsHomed := IsHomedValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY LimitSwitchNegative : BOOL
GET
    LimitSwitchNegative := LimitSwitchNegativeValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY LimitSwitchPositive : BOOL
GET
    LimitSwitchPositive := LimitSwitchPositiveValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY MotionStatus : MC_MOTION_STATUS
GET
    MotionStatus := MotionStatusValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY PowerOn : BOOL
GET
    PowerOn := PowerOnValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY ReadyForPowerOn : BOOL
GET
    ReadyForPowerOn := ReadyForPowerOnValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY Simulation : BOOL
GET
    Simulation := SimulationValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY Status : MC_AXIS_STATUS
GET
    Status := StatusValue;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK
