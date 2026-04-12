FUNCTION TRUST_HOME_DirectionPositive : BOOL
VAR_INPUT
    Direction : MC_HOME_DIRECTION;
    SignalActive : BOOL;
END_VAR
CASE Direction OF
    MC_HOME_DIRECTION#mcPositiveDirection:
        TRUST_HOME_DirectionPositive := TRUE;
    MC_HOME_DIRECTION#mcNegativeDirection:
        TRUST_HOME_DirectionPositive := FALSE;
    MC_HOME_DIRECTION#mcSwitchPositive:
        TRUST_HOME_DirectionPositive := NOT SignalActive;
    MC_HOME_DIRECTION#mcSwitchNegative:
        TRUST_HOME_DirectionPositive := SignalActive;
ELSE
    TRUST_HOME_DirectionPositive := TRUE;
END_CASE;
END_FUNCTION

FUNCTION TRUST_HOME_SwitchModeMet : BOOL
VAR_INPUT
    SwitchMode : MC_SWITCH_MODE;
    CurrentSignal : BOOL;
    PreviousSignal : BOOL;
    DirectionPositive : BOOL;
END_VAR
CASE SwitchMode OF
    MC_SWITCH_MODE#mcOn:
        TRUST_HOME_SwitchModeMet := CurrentSignal;
    MC_SWITCH_MODE#mcOff:
        TRUST_HOME_SwitchModeMet := NOT CurrentSignal;
    MC_SWITCH_MODE#mcRisingEdge:
        TRUST_HOME_SwitchModeMet := (NOT PreviousSignal) AND CurrentSignal;
    MC_SWITCH_MODE#mcFallingEdge:
        TRUST_HOME_SwitchModeMet := PreviousSignal AND (NOT CurrentSignal);
    MC_SWITCH_MODE#mcEdgeSwitchPositive:
        IF DirectionPositive THEN
            TRUST_HOME_SwitchModeMet := (NOT PreviousSignal) AND CurrentSignal;
        ELSE
            TRUST_HOME_SwitchModeMet := PreviousSignal AND (NOT CurrentSignal);
        END_IF;
    MC_SWITCH_MODE#mcEdgeSwitchNegative:
        IF DirectionPositive THEN
            TRUST_HOME_SwitchModeMet := PreviousSignal AND (NOT CurrentSignal);
        ELSE
            TRUST_HOME_SwitchModeMet := (NOT PreviousSignal) AND CurrentSignal;
        END_IF;
ELSE
    TRUST_HOME_SwitchModeMet := FALSE;
END_CASE;
END_FUNCTION

FUNCTION TRUST_HOME_LimitError : BOOL
VAR_INPUT
    ConditionMet : BOOL;
    TimeLimit : TIME;
    DistanceLimit : REAL;
END_VAR
TRUST_HOME_LimitError := (NOT ConditionMet) AND ((TimeLimit > TIME#0ms) OR (DistanceLimit > REAL#0.0));
END_FUNCTION

FUNCTION TRUST_HOME_BlockConditionMet : BOOL
VAR_INPUT
    AxisIndex : UINT;
    DetectionVelocityLimit : REAL;
    TorqueLimit : REAL;
END_VAR
VAR
    VelocityInWindow : BOOL;
    ActualVelocity : REAL;
END_VAR
ActualVelocity := g_trust_motion_actual_velocity[AxisIndex];
VelocityInWindow := (ActualVelocity <= DetectionVelocityLimit) AND (ActualVelocity >= (REAL#0.0 - DetectionVelocityLimit));
TRUST_HOME_BlockConditionMet := (TorqueLimit > REAL#0.0) AND (g_trust_motion_actual_torque[AxisIndex] >= TorqueLimit) AND VelocityInWindow;
END_FUNCTION

FUNCTION TRUST_HOME_ReferenceSignalCurrent : BOOL
VAR_INPUT
    AxisIndex : UINT;
    ReferenceSignal : MC_REF_SIGNAL_REF;
END_VAR
TRUST_HOME_ReferenceSignalCurrent := ReferenceSignal.Signal OR g_trust_motion_ref_signal_active[AxisIndex];
END_FUNCTION

FUNCTION TRUST_HOME_StartHoming : BOOL
VAR_INPUT
    AxisIndex : UINT;
    DirectionPositive : BOOL;
    Velocity : REAL;
END_VAR
VAR
    SignedVelocity : REAL;
END_VAR
SignedVelocity := Velocity;
IF NOT DirectionPositive THEN
    SignedVelocity := REAL#0.0 - Velocity;
END_IF;
g_trust_motion_state[AxisIndex] := MC_AXIS_STATUS#mcHoming;
g_trust_motion_commanded_velocity[AxisIndex] := SignedVelocity;
g_trust_motion_set_velocity[AxisIndex] := SignedVelocity;
g_trust_motion_actual_velocity[AxisIndex] := SignedVelocity;
g_trust_motion_constant_velocity[AxisIndex] := FALSE;
g_trust_motion_accelerating[AxisIndex] := TRUE;
g_trust_motion_decelerating[AxisIndex] := FALSE;
TRUST_HOME_StartHoming := TRUE;
END_FUNCTION

FUNCTION TRUST_HOME_ApplyAxisPosition : BOOL
VAR_INPUT
    AxisIndex : UINT;
    Position : REAL;
END_VAR
g_trust_motion_actual_position[AxisIndex] := Position;
g_trust_motion_set_position[AxisIndex] := Position;
g_trust_motion_commanded_position[AxisIndex] := Position;
TRUST_HOME_ApplyAxisPosition := TRUE;
END_FUNCTION

FUNCTION TRUST_HOME_CompleteStep : BOOL
VAR_INPUT
    AxisIndex : UINT;
    UpdatePosition : BOOL;
    Position : REAL;
END_VAR
IF UpdatePosition THEN
    TRUST_HOME_ApplyAxisPosition(AxisIndex := AxisIndex, Position := Position);
END_IF;
g_trust_motion_state[AxisIndex] := MC_AXIS_STATUS#mcHoming;
g_trust_motion_commanded_velocity[AxisIndex] := REAL#0.0;
g_trust_motion_set_velocity[AxisIndex] := REAL#0.0;
g_trust_motion_actual_velocity[AxisIndex] := REAL#0.0;
g_trust_motion_constant_velocity[AxisIndex] := FALSE;
g_trust_motion_accelerating[AxisIndex] := FALSE;
g_trust_motion_decelerating[AxisIndex] := FALSE;
g_trust_motion_is_homed[AxisIndex] := FALSE;
TRUST_HOME_CompleteStep := TRUE;
END_FUNCTION

FUNCTION TRUST_HOME_FinalizeHoming : BOOL
VAR_INPUT
    AxisIndex : UINT;
    Position : REAL;
END_VAR
TRUST_HOME_ApplyAxisPosition(AxisIndex := AxisIndex, Position := Position);
g_trust_motion_state[AxisIndex] := MC_AXIS_STATUS#mcStandstill;
g_trust_motion_commanded_velocity[AxisIndex] := REAL#0.0;
g_trust_motion_set_velocity[AxisIndex] := REAL#0.0;
g_trust_motion_actual_velocity[AxisIndex] := REAL#0.0;
g_trust_motion_constant_velocity[AxisIndex] := FALSE;
g_trust_motion_accelerating[AxisIndex] := FALSE;
g_trust_motion_decelerating[AxisIndex] := FALSE;
g_trust_motion_is_homed[AxisIndex] := TRUE;
TRUST_HOME_FinalizeHoming := TRUE;
END_FUNCTION

FUNCTION_BLOCK TRUST_HOME_TestSignalSeed
VAR_IN_OUT
    Axis : AXIS_REF;
END_VAR
VAR_INPUT
    ApplyReferenceSignal : BOOL;
    ReferenceSignalActive : BOOL;
    ReferenceSignalPosition : REAL;
    ReferenceSignalCode : UINT;
    ApplyAbsoluteEncoder : BOOL;
    AbsoluteEncoderPosition : REAL;
    ApplyDistanceCoded : BOOL;
    DistanceCodedReady : BOOL;
    DistanceCodedPosition : REAL;
END_VAR
VAR
    Context : TRUST_MOTION_AxisContext;
    AxisIndex : UINT;
END_VAR
Context(Axis := Axis);
AxisIndex := Context.AxisIndex;
IF ApplyReferenceSignal THEN
    g_trust_motion_ref_signal_active[AxisIndex] := ReferenceSignalActive;
    g_trust_motion_ref_signal_position[AxisIndex] := ReferenceSignalPosition;
    g_trust_motion_ref_signal_code[AxisIndex] := ReferenceSignalCode;
END_IF;
IF ApplyAbsoluteEncoder THEN
    g_trust_motion_abs_encoder_position[AxisIndex] := AbsoluteEncoderPosition;
END_IF;
IF ApplyDistanceCoded THEN
    g_trust_motion_distance_coded_ready[AxisIndex] := DistanceCodedReady;
    g_trust_motion_distance_coded_position[AxisIndex] := DistanceCodedPosition;
END_IF;
END_FUNCTION_BLOCK

FUNCTION_BLOCK TRUST_HOME_TestSignalProbe
VAR_IN_OUT
    Axis : AXIS_REF;
END_VAR
VAR_OUTPUT
    ReferenceSignalActive : BOOL;
    ReferenceSignalPosition : REAL;
    ReferenceSignalCode : UINT;
    AbsoluteEncoderPosition : REAL;
    DistanceCodedReady : BOOL;
    DistanceCodedPosition : REAL;
END_VAR
VAR
    Context : TRUST_MOTION_AxisContext;
    AxisIndex : UINT;
END_VAR
Context(Axis := Axis);
AxisIndex := Context.AxisIndex;
ReferenceSignalActive := g_trust_motion_ref_signal_active[AxisIndex];
ReferenceSignalPosition := g_trust_motion_ref_signal_position[AxisIndex];
ReferenceSignalCode := g_trust_motion_ref_signal_code[AxisIndex];
AbsoluteEncoderPosition := g_trust_motion_abs_encoder_position[AxisIndex];
DistanceCodedReady := g_trust_motion_distance_coded_ready[AxisIndex];
DistanceCodedPosition := g_trust_motion_distance_coded_position[AxisIndex];
END_FUNCTION_BLOCK


FUNCTION_BLOCK TRUST_HOME_TestStateProbe
VAR_IN_OUT
    Axis : AXIS_REF;
END_VAR
VAR_OUTPUT
    Standstill : BOOL;
    Homing : BOOL;
    ErrorStop : BOOL;
    IsHomed : BOOL;
    Position : REAL;
    AxisErrorID : WORD;
END_VAR
VAR
    ReadStatus : MC_ReadStatus;
    ReadInfo : MC_ReadAxisInfo;
    ReadPosition : MC_ReadActualPosition;
    ReadAxisError : MC_ReadAxisError;
END_VAR
ReadStatus(Axis := Axis, Enable := TRUE);
ReadInfo(Axis := Axis, Enable := TRUE);
ReadPosition(Axis := Axis, Enable := TRUE);
ReadAxisError(Axis := Axis, Enable := TRUE);
Standstill := ReadStatus.Standstill;
Homing := ReadStatus.Homing;
ErrorStop := ReadStatus.ErrorStop;
IsHomed := ReadInfo.IsHomed;
Position := ReadPosition.Position;
AxisErrorID := ReadAxisError.AxisErrorID;
END_FUNCTION_BLOCK
