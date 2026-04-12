TYPE MC_HOME_DIRECTION : (
    mcPositiveDirection,
    mcNegativeDirection,
    mcSwitchPositive,
    mcSwitchNegative
);
END_TYPE

TYPE MC_SWITCH_MODE : (
    mcOn,
    mcOff,
    mcRisingEdge,
    mcFallingEdge,
    mcEdgeSwitchPositive,
    mcEdgeSwitchNegative
);
END_TYPE

TYPE MC_REF_SIGNAL_REF :
STRUCT
    Signal : BOOL;
    PositionStamp : REAL;
    MarkerCode : UINT;
END_STRUCT
END_TYPE

FUNCTION_BLOCK MC_StepAbsoluteSwitch
VAR_IN_OUT
    Axis : AXIS_REF;
END_VAR
VAR_INPUT
    Execute : BOOL;
    Direction : MC_HOME_DIRECTION;
    SwitchMode : MC_SWITCH_MODE;
    ReferenceSignal : MC_REF_SIGNAL_REF;
    Velocity : REAL;
    SetPosition : REAL;
    TorqueLimit : REAL;
    TimeLimit : TIME;
    DistanceLimit : REAL;
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
    AcceptedCommandID : UINT;
    AcceptedSetPosition : REAL;
    DirectionPositive : BOOL;
    WasActive : BOOL;
    LastSignal : BOOL;
    CurrentSignal : BOOL;
    ConditionMet : BOOL;
    Request : TRUST_MOTION_RequestBufferedCommand;
END_VAR
Context(Axis := Axis);
AxisIndex := Context.AxisIndex;
TRUST_MOTION_PromoteQueuedCommand(AxisIndex := AxisIndex);
CurrentSignal := TRUST_HOME_ReferenceSignalCurrent(AxisIndex := AxisIndex, ReferenceSignal := ReferenceSignal);
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
    ELSIF (Velocity < REAL#0.0) OR (TorqueLimit < REAL#0.0) OR (DistanceLimit < REAL#0.0) THEN
        Error := TRUE;
        ErrorID := WORD#16#0001;
    ELSIF NOT TRUST_MOTION_SupportsSingleAxisBufferMode(BufferMode := BufferMode) THEN
        Error := TRUE;
        ErrorID := WORD#16#0500;
    ELSE
        AcceptedSetPosition := SetPosition;
        DirectionPositive := TRUST_HOME_DirectionPositive(Direction := Direction, SignalActive := CurrentSignal);
        LastSignal := CurrentSignal;
        WasActive := FALSE;
        Request(AxisIndex := AxisIndex, Kind := TRUST_MOTION_COMMAND_KIND#trustMotionStepAbsoluteSwitch, BufferMode := BufferMode);
        IF Request.Error THEN
            Error := TRUE;
            ErrorID := Request.ErrorID;
            AcceptedCommandID := UINT#0;
        ELSE
            AcceptedCommandID := Request.CommandID;
            Busy := TRUE;
            Active := Request.ActiveNow;
            IF Request.ActiveNow THEN
                WasActive := TRUE;
                TRUST_HOME_StartHoming(AxisIndex := AxisIndex, DirectionPositive := DirectionPositive, Velocity := Velocity);
            END_IF;
        END_IF;
    END_IF;
ELSIF AcceptedCommandID <> UINT#0 THEN
    IF TRUST_MOTION_CommandWasAborted(AxisIndex := AxisIndex, CommandID := AcceptedCommandID) THEN
        Busy := FALSE;
        Active := FALSE;
        Done := FALSE;
        CommandAborted := TRUE;
        Error := FALSE;
        ErrorID := WORD#16#0000;
        TRUST_MOTION_ClearAbortedCommand(AxisIndex := AxisIndex, CommandID := AcceptedCommandID);
        AcceptedCommandID := UINT#0;
        WasActive := FALSE;
    ELSIF g_trust_motion_active_command_id[AxisIndex] = AcceptedCommandID THEN
        IF NOT WasActive THEN
            WasActive := TRUE;
            Busy := TRUE;
            Active := TRUE;
            TRUST_HOME_StartHoming(AxisIndex := AxisIndex, DirectionPositive := DirectionPositive, Velocity := Velocity);
        END_IF;
        ConditionMet := TRUST_HOME_SwitchModeMet(SwitchMode := SwitchMode, CurrentSignal := CurrentSignal, PreviousSignal := LastSignal, DirectionPositive := DirectionPositive);
        IF ConditionMet THEN
            Busy := FALSE;
            Active := FALSE;
            Done := TRUE;
            CommandAborted := FALSE;
            Error := FALSE;
            ErrorID := WORD#16#0000;
            TRUST_HOME_CompleteStep(AxisIndex := AxisIndex, UpdatePosition := TRUE, Position := AcceptedSetPosition);
            TRUST_MOTION_ClearActiveCommandSlot(AxisIndex := AxisIndex);
            AcceptedCommandID := UINT#0;
            WasActive := FALSE;
        ELSIF TRUST_HOME_LimitError(ConditionMet := FALSE, TimeLimit := TimeLimit, DistanceLimit := DistanceLimit) THEN
            Busy := FALSE;
            Active := FALSE;
            Done := FALSE;
            CommandAborted := FALSE;
            Error := TRUE;
            ErrorID := WORD#16#0600;
            g_trust_motion_axis_error[AxisIndex] := ErrorID;
            TRUST_MOTION_EnterErrorStopAndFlushQueue(AxisIndex := AxisIndex);
            AcceptedCommandID := UINT#0;
            WasActive := FALSE;
        ELSE
            Busy := TRUE;
            Active := TRUE;
        END_IF;
    ELSE
        Busy := TRUE;
        Active := FALSE;
    END_IF;
END_IF;
LastExecute := Execute;
LastSignal := CurrentSignal;
END_FUNCTION_BLOCK

FUNCTION_BLOCK MC_StepLimitSwitch
VAR_IN_OUT
    Axis : AXIS_REF;
END_VAR
VAR_INPUT
    Execute : BOOL;
    Direction : MC_HOME_DIRECTION;
    LimitSwitchMode : MC_SWITCH_MODE;
    Velocity : REAL;
    SetPosition : REAL;
    TorqueLimit : REAL;
    TimeLimit : TIME;
    DistanceLimit : REAL;
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
    AcceptedCommandID : UINT;
    AcceptedSetPosition : REAL;
    DirectionPositive : BOOL;
    WasActive : BOOL;
    LastSignal : BOOL;
    CurrentSignal : BOOL;
    ConditionMet : BOOL;
    Request : TRUST_MOTION_RequestBufferedCommand;
END_VAR
Context(Axis := Axis);
AxisIndex := Context.AxisIndex;
TRUST_MOTION_PromoteQueuedCommand(AxisIndex := AxisIndex);
DirectionPositive := TRUST_HOME_DirectionPositive(Direction := Direction, SignalActive := FALSE);
IF DirectionPositive THEN
    CurrentSignal := g_trust_motion_limit_switch_pos[AxisIndex];
ELSE
    CurrentSignal := g_trust_motion_limit_switch_neg[AxisIndex];
END_IF;
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
    ELSIF (Velocity < REAL#0.0) OR (TorqueLimit < REAL#0.0) OR (DistanceLimit < REAL#0.0) THEN
        Error := TRUE;
        ErrorID := WORD#16#0001;
    ELSIF NOT TRUST_MOTION_SupportsSingleAxisBufferMode(BufferMode := BufferMode) THEN
        Error := TRUE;
        ErrorID := WORD#16#0500;
    ELSE
        AcceptedSetPosition := SetPosition;
        LastSignal := CurrentSignal;
        WasActive := FALSE;
        Request(AxisIndex := AxisIndex, Kind := TRUST_MOTION_COMMAND_KIND#trustMotionStepLimitSwitch, BufferMode := BufferMode);
        IF Request.Error THEN
            Error := TRUE;
            ErrorID := Request.ErrorID;
            AcceptedCommandID := UINT#0;
        ELSE
            AcceptedCommandID := Request.CommandID;
            Busy := TRUE;
            Active := Request.ActiveNow;
            IF Request.ActiveNow THEN
                WasActive := TRUE;
                TRUST_HOME_StartHoming(AxisIndex := AxisIndex, DirectionPositive := DirectionPositive, Velocity := Velocity);
            END_IF;
        END_IF;
    END_IF;
ELSIF AcceptedCommandID <> UINT#0 THEN
    IF TRUST_MOTION_CommandWasAborted(AxisIndex := AxisIndex, CommandID := AcceptedCommandID) THEN
        Busy := FALSE;
        Active := FALSE;
        Done := FALSE;
        CommandAborted := TRUE;
        Error := FALSE;
        ErrorID := WORD#16#0000;
        TRUST_MOTION_ClearAbortedCommand(AxisIndex := AxisIndex, CommandID := AcceptedCommandID);
        AcceptedCommandID := UINT#0;
        WasActive := FALSE;
    ELSIF g_trust_motion_active_command_id[AxisIndex] = AcceptedCommandID THEN
        IF NOT WasActive THEN
            WasActive := TRUE;
            Busy := TRUE;
            Active := TRUE;
            TRUST_HOME_StartHoming(AxisIndex := AxisIndex, DirectionPositive := DirectionPositive, Velocity := Velocity);
        END_IF;
        IF DirectionPositive THEN
            CurrentSignal := g_trust_motion_limit_switch_pos[AxisIndex];
        ELSE
            CurrentSignal := g_trust_motion_limit_switch_neg[AxisIndex];
        END_IF;
        ConditionMet := TRUST_HOME_SwitchModeMet(SwitchMode := LimitSwitchMode, CurrentSignal := CurrentSignal, PreviousSignal := LastSignal, DirectionPositive := DirectionPositive);
        IF ConditionMet THEN
            Busy := FALSE;
            Active := FALSE;
            Done := TRUE;
            CommandAborted := FALSE;
            Error := FALSE;
            ErrorID := WORD#16#0000;
            TRUST_HOME_CompleteStep(AxisIndex := AxisIndex, UpdatePosition := TRUE, Position := AcceptedSetPosition);
            TRUST_MOTION_ClearActiveCommandSlot(AxisIndex := AxisIndex);
            AcceptedCommandID := UINT#0;
            WasActive := FALSE;
        ELSIF TRUST_HOME_LimitError(ConditionMet := FALSE, TimeLimit := TimeLimit, DistanceLimit := DistanceLimit) THEN
            Busy := FALSE;
            Active := FALSE;
            Done := FALSE;
            CommandAborted := FALSE;
            Error := TRUE;
            ErrorID := WORD#16#0600;
            g_trust_motion_axis_error[AxisIndex] := ErrorID;
            TRUST_MOTION_EnterErrorStopAndFlushQueue(AxisIndex := AxisIndex);
            AcceptedCommandID := UINT#0;
            WasActive := FALSE;
        ELSE
            Busy := TRUE;
            Active := TRUE;
        END_IF;
    ELSE
        Busy := TRUE;
        Active := FALSE;
    END_IF;
END_IF;
LastExecute := Execute;
LastSignal := CurrentSignal;
END_FUNCTION_BLOCK

FUNCTION_BLOCK MC_StepBlock
VAR_IN_OUT
    Axis : AXIS_REF;
END_VAR
VAR_INPUT
    Execute : BOOL;
    Direction : MC_HOME_DIRECTION;
    Velocity : REAL;
    SetPosition : REAL;
    DetectionVelocityLimit : REAL;
    DetectionVelocityTime : TIME;
    TorqueLimit : REAL;
    TimeLimit : TIME;
    DistanceLimit : REAL;
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
    AcceptedCommandID : UINT;
    AcceptedSetPosition : REAL;
    DirectionPositive : BOOL;
    WasActive : BOOL;
    ConditionMet : BOOL;
    DetectionWindowStable : BOOL;
    Request : TRUST_MOTION_RequestBufferedCommand;
END_VAR
Context(Axis := Axis);
AxisIndex := Context.AxisIndex;
TRUST_MOTION_PromoteQueuedCommand(AxisIndex := AxisIndex);
DirectionPositive := TRUST_HOME_DirectionPositive(Direction := Direction, SignalActive := FALSE);
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
    ELSIF (Velocity < REAL#0.0) OR (DetectionVelocityLimit < REAL#0.0) OR (TorqueLimit < REAL#0.0) OR (DistanceLimit < REAL#0.0) THEN
        Error := TRUE;
        ErrorID := WORD#16#0001;
    ELSIF NOT TRUST_MOTION_SupportsSingleAxisBufferMode(BufferMode := BufferMode) THEN
        Error := TRUE;
        ErrorID := WORD#16#0500;
    ELSE
        AcceptedSetPosition := SetPosition;
        WasActive := FALSE;
        DetectionWindowStable := FALSE;
        Request(AxisIndex := AxisIndex, Kind := TRUST_MOTION_COMMAND_KIND#trustMotionStepBlock, BufferMode := BufferMode);
        IF Request.Error THEN
            Error := TRUE;
            ErrorID := Request.ErrorID;
            AcceptedCommandID := UINT#0;
        ELSE
            AcceptedCommandID := Request.CommandID;
            Busy := TRUE;
            Active := Request.ActiveNow;
            IF Request.ActiveNow THEN
                WasActive := TRUE;
                TRUST_HOME_StartHoming(AxisIndex := AxisIndex, DirectionPositive := DirectionPositive, Velocity := Velocity);
            END_IF;
        END_IF;
    END_IF;
ELSIF AcceptedCommandID <> UINT#0 THEN
    IF TRUST_MOTION_CommandWasAborted(AxisIndex := AxisIndex, CommandID := AcceptedCommandID) THEN
        Busy := FALSE;
        Active := FALSE;
        Done := FALSE;
        CommandAborted := TRUE;
        Error := FALSE;
        ErrorID := WORD#16#0000;
        TRUST_MOTION_ClearAbortedCommand(AxisIndex := AxisIndex, CommandID := AcceptedCommandID);
        AcceptedCommandID := UINT#0;
        WasActive := FALSE;
        DetectionWindowStable := FALSE;
    ELSIF g_trust_motion_active_command_id[AxisIndex] = AcceptedCommandID THEN
        IF NOT WasActive THEN
            WasActive := TRUE;
            Busy := TRUE;
            Active := TRUE;
            TRUST_HOME_StartHoming(AxisIndex := AxisIndex, DirectionPositive := DirectionPositive, Velocity := Velocity);
        END_IF;
        ConditionMet := TRUST_HOME_BlockConditionMet(AxisIndex := AxisIndex, DetectionVelocityLimit := DetectionVelocityLimit, TorqueLimit := TorqueLimit);
        IF DetectionVelocityTime > TIME#0ms THEN
            IF ConditionMet THEN
                IF DetectionWindowStable THEN
                    ConditionMet := TRUE;
                ELSE
                    DetectionWindowStable := TRUE;
                    ConditionMet := FALSE;
                END_IF;
            ELSE
                DetectionWindowStable := FALSE;
                ConditionMet := FALSE;
            END_IF;
        END_IF;
        IF ConditionMet THEN
            Busy := FALSE;
            Active := FALSE;
            Done := TRUE;
            CommandAborted := FALSE;
            Error := FALSE;
            ErrorID := WORD#16#0000;
            TRUST_HOME_CompleteStep(AxisIndex := AxisIndex, UpdatePosition := TRUE, Position := AcceptedSetPosition);
            TRUST_MOTION_ClearActiveCommandSlot(AxisIndex := AxisIndex);
            AcceptedCommandID := UINT#0;
            WasActive := FALSE;
            DetectionWindowStable := FALSE;
        ELSIF TRUST_HOME_LimitError(ConditionMet := FALSE, TimeLimit := TimeLimit, DistanceLimit := DistanceLimit) THEN
            Busy := FALSE;
            Active := FALSE;
            Done := FALSE;
            CommandAborted := FALSE;
            Error := TRUE;
            ErrorID := WORD#16#0600;
            g_trust_motion_axis_error[AxisIndex] := ErrorID;
            TRUST_MOTION_EnterErrorStopAndFlushQueue(AxisIndex := AxisIndex);
            AcceptedCommandID := UINT#0;
            WasActive := FALSE;
            DetectionWindowStable := FALSE;
        ELSE
            Busy := TRUE;
            Active := TRUE;
        END_IF;
    ELSE
        Busy := TRUE;
        Active := FALSE;
    END_IF;
END_IF;
LastExecute := Execute;
END_FUNCTION_BLOCK

FUNCTION_BLOCK MC_StepReferencePulse
VAR_IN_OUT
    Axis : AXIS_REF;
END_VAR
VAR_INPUT
    Execute : BOOL;
    Direction : MC_HOME_DIRECTION;
    ReferenceSignal : MC_REF_SIGNAL_REF;
    Velocity : REAL;
    SetPosition : REAL;
    TorqueLimit : REAL;
    TimeLimit : TIME;
    DistanceLimit : REAL;
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
    AcceptedCommandID : UINT;
    AcceptedSetPosition : REAL;
    DirectionPositive : BOOL;
    WasActive : BOOL;
    CurrentSignal : BOOL;
    LastSignal : BOOL;
    Request : TRUST_MOTION_RequestBufferedCommand;
END_VAR
Context(Axis := Axis);
AxisIndex := Context.AxisIndex;
TRUST_MOTION_PromoteQueuedCommand(AxisIndex := AxisIndex);
CurrentSignal := TRUST_HOME_ReferenceSignalCurrent(AxisIndex := AxisIndex, ReferenceSignal := ReferenceSignal);
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
    ELSIF (Velocity < REAL#0.0) OR (TorqueLimit < REAL#0.0) OR (DistanceLimit < REAL#0.0) THEN
        Error := TRUE;
        ErrorID := WORD#16#0001;
    ELSIF NOT TRUST_MOTION_SupportsSingleAxisBufferMode(BufferMode := BufferMode) THEN
        Error := TRUE;
        ErrorID := WORD#16#0500;
    ELSE
        AcceptedSetPosition := SetPosition;
        DirectionPositive := TRUST_HOME_DirectionPositive(Direction := Direction, SignalActive := CurrentSignal);
        LastSignal := CurrentSignal;
        WasActive := FALSE;
        Request(AxisIndex := AxisIndex, Kind := TRUST_MOTION_COMMAND_KIND#trustMotionStepReferencePulse, BufferMode := BufferMode);
        IF Request.Error THEN
            Error := TRUE;
            ErrorID := Request.ErrorID;
            AcceptedCommandID := UINT#0;
        ELSE
            AcceptedCommandID := Request.CommandID;
            Busy := TRUE;
            Active := Request.ActiveNow;
            IF Request.ActiveNow THEN
                WasActive := TRUE;
                TRUST_HOME_StartHoming(AxisIndex := AxisIndex, DirectionPositive := DirectionPositive, Velocity := Velocity);
            END_IF;
        END_IF;
    END_IF;
ELSIF AcceptedCommandID <> UINT#0 THEN
    IF TRUST_MOTION_CommandWasAborted(AxisIndex := AxisIndex, CommandID := AcceptedCommandID) THEN
        Busy := FALSE;
        Active := FALSE;
        Done := FALSE;
        CommandAborted := TRUE;
        Error := FALSE;
        ErrorID := WORD#16#0000;
        TRUST_MOTION_ClearAbortedCommand(AxisIndex := AxisIndex, CommandID := AcceptedCommandID);
        AcceptedCommandID := UINT#0;
        WasActive := FALSE;
    ELSIF g_trust_motion_active_command_id[AxisIndex] = AcceptedCommandID THEN
        IF NOT WasActive THEN
            WasActive := TRUE;
            Busy := TRUE;
            Active := TRUE;
            TRUST_HOME_StartHoming(AxisIndex := AxisIndex, DirectionPositive := DirectionPositive, Velocity := Velocity);
        END_IF;
        CurrentSignal := TRUST_HOME_ReferenceSignalCurrent(AxisIndex := AxisIndex, ReferenceSignal := ReferenceSignal);
        IF (NOT LastSignal) AND CurrentSignal THEN
            Busy := FALSE;
            Active := FALSE;
            Done := TRUE;
            CommandAborted := FALSE;
            Error := FALSE;
            ErrorID := WORD#16#0000;
            TRUST_HOME_CompleteStep(AxisIndex := AxisIndex, UpdatePosition := TRUE, Position := AcceptedSetPosition);
            TRUST_MOTION_ClearActiveCommandSlot(AxisIndex := AxisIndex);
            AcceptedCommandID := UINT#0;
            WasActive := FALSE;
        ELSIF TRUST_HOME_LimitError(ConditionMet := FALSE, TimeLimit := TimeLimit, DistanceLimit := DistanceLimit) THEN
            Busy := FALSE;
            Active := FALSE;
            Done := FALSE;
            CommandAborted := FALSE;
            Error := TRUE;
            ErrorID := WORD#16#0600;
            g_trust_motion_axis_error[AxisIndex] := ErrorID;
            TRUST_MOTION_EnterErrorStopAndFlushQueue(AxisIndex := AxisIndex);
            AcceptedCommandID := UINT#0;
            WasActive := FALSE;
        ELSE
            Busy := TRUE;
            Active := TRUE;
        END_IF;
    ELSE
        Busy := TRUE;
        Active := FALSE;
    END_IF;
END_IF;
LastExecute := Execute;
LastSignal := CurrentSignal;
END_FUNCTION_BLOCK

FUNCTION_BLOCK MC_StepDistanceCoded
VAR_IN_OUT
    Axis : AXIS_REF;
END_VAR
VAR_INPUT
    Execute : BOOL;
    Direction : MC_HOME_DIRECTION;
    Velocity : REAL;
    TorqueLimit : REAL;
    TimeLimit : TIME;
    DistanceLimit : REAL;
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
    AcceptedCommandID : UINT;
    DirectionPositive : BOOL;
    WasActive : BOOL;
    Request : TRUST_MOTION_RequestBufferedCommand;
END_VAR
Context(Axis := Axis);
AxisIndex := Context.AxisIndex;
TRUST_MOTION_PromoteQueuedCommand(AxisIndex := AxisIndex);
DirectionPositive := TRUST_HOME_DirectionPositive(Direction := Direction, SignalActive := FALSE);
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
    ELSIF (Velocity < REAL#0.0) OR (TorqueLimit < REAL#0.0) OR (DistanceLimit < REAL#0.0) THEN
        Error := TRUE;
        ErrorID := WORD#16#0001;
    ELSIF NOT TRUST_MOTION_SupportsSingleAxisBufferMode(BufferMode := BufferMode) THEN
        Error := TRUE;
        ErrorID := WORD#16#0500;
    ELSE
        WasActive := FALSE;
        Request(AxisIndex := AxisIndex, Kind := TRUST_MOTION_COMMAND_KIND#trustMotionStepDistanceCoded, BufferMode := BufferMode);
        IF Request.Error THEN
            Error := TRUE;
            ErrorID := Request.ErrorID;
            AcceptedCommandID := UINT#0;
        ELSE
            AcceptedCommandID := Request.CommandID;
            Busy := TRUE;
            Active := Request.ActiveNow;
            IF Request.ActiveNow THEN
                WasActive := TRUE;
                TRUST_HOME_StartHoming(AxisIndex := AxisIndex, DirectionPositive := DirectionPositive, Velocity := Velocity);
            END_IF;
        END_IF;
    END_IF;
ELSIF AcceptedCommandID <> UINT#0 THEN
    IF TRUST_MOTION_CommandWasAborted(AxisIndex := AxisIndex, CommandID := AcceptedCommandID) THEN
        Busy := FALSE;
        Active := FALSE;
        Done := FALSE;
        CommandAborted := TRUE;
        Error := FALSE;
        ErrorID := WORD#16#0000;
        TRUST_MOTION_ClearAbortedCommand(AxisIndex := AxisIndex, CommandID := AcceptedCommandID);
        AcceptedCommandID := UINT#0;
        WasActive := FALSE;
    ELSIF g_trust_motion_active_command_id[AxisIndex] = AcceptedCommandID THEN
        IF NOT WasActive THEN
            WasActive := TRUE;
            Busy := TRUE;
            Active := TRUE;
            TRUST_HOME_StartHoming(AxisIndex := AxisIndex, DirectionPositive := DirectionPositive, Velocity := Velocity);
        END_IF;
        IF g_trust_motion_distance_coded_ready[AxisIndex] THEN
            Busy := FALSE;
            Active := FALSE;
            Done := TRUE;
            CommandAborted := FALSE;
            Error := FALSE;
            ErrorID := WORD#16#0000;
            TRUST_HOME_CompleteStep(AxisIndex := AxisIndex, UpdatePosition := TRUE, Position := g_trust_motion_distance_coded_position[AxisIndex]);
            TRUST_MOTION_ClearActiveCommandSlot(AxisIndex := AxisIndex);
            AcceptedCommandID := UINT#0;
            WasActive := FALSE;
        ELSIF TRUST_HOME_LimitError(ConditionMet := FALSE, TimeLimit := TimeLimit, DistanceLimit := DistanceLimit) THEN
            Busy := FALSE;
            Active := FALSE;
            Done := FALSE;
            CommandAborted := FALSE;
            Error := TRUE;
            ErrorID := WORD#16#0600;
            g_trust_motion_axis_error[AxisIndex] := ErrorID;
            TRUST_MOTION_EnterErrorStopAndFlushQueue(AxisIndex := AxisIndex);
            AcceptedCommandID := UINT#0;
            WasActive := FALSE;
        ELSE
            Busy := TRUE;
            Active := TRUE;
        END_IF;
    ELSE
        Busy := TRUE;
        Active := FALSE;
    END_IF;
END_IF;
LastExecute := Execute;
END_FUNCTION_BLOCK

FUNCTION_BLOCK MC_HomeDirect
VAR_IN_OUT
    Axis : AXIS_REF;
END_VAR
VAR_INPUT
    Execute : BOOL;
    SetPosition : REAL;
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
    AcceptedCommandID : UINT;
    AcceptedSetPosition : REAL;
    WasActive : BOOL;
    Request : TRUST_MOTION_RequestBufferedCommand;
END_VAR
Context(Axis := Axis);
AxisIndex := Context.AxisIndex;
TRUST_MOTION_PromoteQueuedCommand(AxisIndex := AxisIndex);
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
    ELSIF NOT TRUST_MOTION_SupportsSingleAxisBufferMode(BufferMode := BufferMode) THEN
        Error := TRUE;
        ErrorID := WORD#16#0500;
    ELSE
        AcceptedSetPosition := SetPosition;
        WasActive := FALSE;
        Request(AxisIndex := AxisIndex, Kind := TRUST_MOTION_COMMAND_KIND#trustMotionHomeDirect, BufferMode := BufferMode);
        IF Request.Error THEN
            Error := TRUE;
            ErrorID := Request.ErrorID;
            AcceptedCommandID := UINT#0;
        ELSE
            AcceptedCommandID := Request.CommandID;
            Busy := TRUE;
            Active := Request.ActiveNow;
            IF Request.ActiveNow THEN
                WasActive := TRUE;
                g_trust_motion_state[AxisIndex] := MC_AXIS_STATUS#mcHoming;
            END_IF;
        END_IF;
    END_IF;
ELSIF AcceptedCommandID <> UINT#0 THEN
    IF TRUST_MOTION_CommandWasAborted(AxisIndex := AxisIndex, CommandID := AcceptedCommandID) THEN
        Busy := FALSE;
        Active := FALSE;
        Done := FALSE;
        CommandAborted := TRUE;
        Error := FALSE;
        ErrorID := WORD#16#0000;
        TRUST_MOTION_ClearAbortedCommand(AxisIndex := AxisIndex, CommandID := AcceptedCommandID);
        AcceptedCommandID := UINT#0;
        WasActive := FALSE;
    ELSIF g_trust_motion_active_command_id[AxisIndex] = AcceptedCommandID THEN
        IF NOT WasActive THEN
            WasActive := TRUE;
            Busy := TRUE;
            Active := TRUE;
            g_trust_motion_state[AxisIndex] := MC_AXIS_STATUS#mcHoming;
        END_IF;
        Busy := FALSE;
        Active := FALSE;
        Done := TRUE;
        CommandAborted := FALSE;
        Error := FALSE;
        ErrorID := WORD#16#0000;
        TRUST_HOME_FinalizeHoming(AxisIndex := AxisIndex, Position := AcceptedSetPosition);
        TRUST_MOTION_ClearActiveCommandSlot(AxisIndex := AxisIndex);
        AcceptedCommandID := UINT#0;
        WasActive := FALSE;
    ELSE
        Busy := TRUE;
        Active := FALSE;
    END_IF;
END_IF;
LastExecute := Execute;
END_FUNCTION_BLOCK

FUNCTION_BLOCK MC_HomeAbsolute
VAR_IN_OUT
    Axis : AXIS_REF;
END_VAR
VAR_INPUT
    Execute : BOOL;
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
    AcceptedCommandID : UINT;
    WasActive : BOOL;
    Request : TRUST_MOTION_RequestBufferedCommand;
END_VAR
Context(Axis := Axis);
AxisIndex := Context.AxisIndex;
TRUST_MOTION_PromoteQueuedCommand(AxisIndex := AxisIndex);
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
    ELSIF NOT TRUST_MOTION_SupportsSingleAxisBufferMode(BufferMode := BufferMode) THEN
        Error := TRUE;
        ErrorID := WORD#16#0500;
    ELSE
        WasActive := FALSE;
        Request(AxisIndex := AxisIndex, Kind := TRUST_MOTION_COMMAND_KIND#trustMotionHomeAbsolute, BufferMode := BufferMode);
        IF Request.Error THEN
            Error := TRUE;
            ErrorID := Request.ErrorID;
            AcceptedCommandID := UINT#0;
        ELSE
            AcceptedCommandID := Request.CommandID;
            Busy := TRUE;
            Active := Request.ActiveNow;
            IF Request.ActiveNow THEN
                WasActive := TRUE;
                g_trust_motion_state[AxisIndex] := MC_AXIS_STATUS#mcHoming;
            END_IF;
        END_IF;
    END_IF;
ELSIF AcceptedCommandID <> UINT#0 THEN
    IF TRUST_MOTION_CommandWasAborted(AxisIndex := AxisIndex, CommandID := AcceptedCommandID) THEN
        Busy := FALSE;
        Active := FALSE;
        Done := FALSE;
        CommandAborted := TRUE;
        Error := FALSE;
        ErrorID := WORD#16#0000;
        TRUST_MOTION_ClearAbortedCommand(AxisIndex := AxisIndex, CommandID := AcceptedCommandID);
        AcceptedCommandID := UINT#0;
        WasActive := FALSE;
    ELSIF g_trust_motion_active_command_id[AxisIndex] = AcceptedCommandID THEN
        IF NOT WasActive THEN
            WasActive := TRUE;
            Busy := TRUE;
            Active := TRUE;
            g_trust_motion_state[AxisIndex] := MC_AXIS_STATUS#mcHoming;
        END_IF;
        Busy := FALSE;
        Active := FALSE;
        Done := TRUE;
        CommandAborted := FALSE;
        Error := FALSE;
        ErrorID := WORD#16#0000;
        TRUST_HOME_FinalizeHoming(AxisIndex := AxisIndex, Position := g_trust_motion_abs_encoder_position[AxisIndex]);
        TRUST_MOTION_ClearActiveCommandSlot(AxisIndex := AxisIndex);
        AcceptedCommandID := UINT#0;
        WasActive := FALSE;
    ELSE
        Busy := TRUE;
        Active := FALSE;
    END_IF;
END_IF;
LastExecute := Execute;
END_FUNCTION_BLOCK

FUNCTION_BLOCK MC_FinishHoming
VAR_IN_OUT
    Axis : AXIS_REF;
END_VAR
VAR_INPUT
    Execute : BOOL;
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
    AcceptedCommandID : UINT;
    AcceptedDistance : REAL;
    WasActive : BOOL;
    Request : TRUST_MOTION_RequestBufferedCommand;
END_VAR
Context(Axis := Axis);
AxisIndex := Context.AxisIndex;
TRUST_MOTION_PromoteQueuedCommand(AxisIndex := AxisIndex);
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
    ELSIF (Velocity < REAL#0.0) OR (Acceleration < REAL#0.0) OR (Deceleration < REAL#0.0) OR (Jerk < REAL#0.0) THEN
        Error := TRUE;
        ErrorID := WORD#16#0001;
    ELSIF NOT TRUST_MOTION_SupportsSingleAxisBufferMode(BufferMode := BufferMode) THEN
        Error := TRUE;
        ErrorID := WORD#16#0500;
    ELSE
        AcceptedDistance := Distance;
        WasActive := FALSE;
        Request(AxisIndex := AxisIndex, Kind := TRUST_MOTION_COMMAND_KIND#trustMotionFinishHoming, BufferMode := BufferMode);
        IF Request.Error THEN
            Error := TRUE;
            ErrorID := Request.ErrorID;
            AcceptedCommandID := UINT#0;
        ELSE
            AcceptedCommandID := Request.CommandID;
            Busy := TRUE;
            Active := Request.ActiveNow;
            IF Request.ActiveNow THEN
                WasActive := TRUE;
                g_trust_motion_state[AxisIndex] := MC_AXIS_STATUS#mcHoming;
                g_trust_motion_resolved_acceleration[AxisIndex] := TRUST_MOTION_ResolveDynamicInput(RequestedValue := Acceleration, MaximumValue := g_trust_motion_max_acceleration_appl[AxisIndex]);
                g_trust_motion_resolved_deceleration[AxisIndex] := TRUST_MOTION_ResolveDynamicInput(RequestedValue := Deceleration, MaximumValue := g_trust_motion_max_deceleration_appl[AxisIndex]);
                g_trust_motion_resolved_jerk[AxisIndex] := TRUST_MOTION_ResolveDynamicInput(RequestedValue := Jerk, MaximumValue := g_trust_motion_max_jerk_appl[AxisIndex]);
            END_IF;
        END_IF;
    END_IF;
ELSIF AcceptedCommandID <> UINT#0 THEN
    IF TRUST_MOTION_CommandWasAborted(AxisIndex := AxisIndex, CommandID := AcceptedCommandID) THEN
        Busy := FALSE;
        Active := FALSE;
        Done := FALSE;
        CommandAborted := TRUE;
        Error := FALSE;
        ErrorID := WORD#16#0000;
        TRUST_MOTION_ClearAbortedCommand(AxisIndex := AxisIndex, CommandID := AcceptedCommandID);
        AcceptedCommandID := UINT#0;
        WasActive := FALSE;
    ELSIF g_trust_motion_active_command_id[AxisIndex] = AcceptedCommandID THEN
        IF NOT WasActive THEN
            WasActive := TRUE;
            Busy := TRUE;
            Active := TRUE;
            g_trust_motion_state[AxisIndex] := MC_AXIS_STATUS#mcHoming;
        END_IF;
        Busy := FALSE;
        Active := FALSE;
        Done := TRUE;
        CommandAborted := FALSE;
        Error := FALSE;
        ErrorID := WORD#16#0000;
        TRUST_HOME_FinalizeHoming(AxisIndex := AxisIndex, Position := g_trust_motion_actual_position[AxisIndex] + AcceptedDistance);
        TRUST_MOTION_ClearActiveCommandSlot(AxisIndex := AxisIndex);
        AcceptedCommandID := UINT#0;
        WasActive := FALSE;
    ELSE
        Busy := TRUE;
        Active := FALSE;
    END_IF;
END_IF;
LastExecute := Execute;
END_FUNCTION_BLOCK
