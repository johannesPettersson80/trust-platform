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
