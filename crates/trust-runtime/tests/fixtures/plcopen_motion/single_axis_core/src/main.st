VAR_GLOBAL
    g_trust_motion_axis_initialized : ARRAY[0..31] OF BOOL;
    g_trust_motion_state : ARRAY[0..31] OF MC_AXIS_STATUS;
    g_trust_motion_grouped : ARRAY[0..31] OF BOOL;
    g_trust_motion_power_command : ARRAY[0..31] OF BOOL;
    g_trust_motion_power_stage : ARRAY[0..31] OF BOOL;
    g_trust_motion_axis_error : ARRAY[0..31] OF WORD;
    g_trust_motion_actual_position : ARRAY[0..31] OF REAL;
    g_trust_motion_set_position : ARRAY[0..31] OF REAL;
    g_trust_motion_commanded_position : ARRAY[0..31] OF REAL;
    g_trust_motion_actual_velocity : ARRAY[0..31] OF REAL;
    g_trust_motion_set_velocity : ARRAY[0..31] OF REAL;
    g_trust_motion_commanded_velocity : ARRAY[0..31] OF REAL;
    g_trust_motion_actual_torque : ARRAY[0..31] OF REAL;
    g_trust_motion_home_abs_switch : ARRAY[0..31] OF BOOL;
    g_trust_motion_limit_switch_pos : ARRAY[0..31] OF BOOL;
    g_trust_motion_limit_switch_neg : ARRAY[0..31] OF BOOL;
    g_trust_motion_simulation : ARRAY[0..31] OF BOOL;
    g_trust_motion_communication_ready : ARRAY[0..31] OF BOOL;
    g_trust_motion_ready_for_power_on : ARRAY[0..31] OF BOOL;
    g_trust_motion_is_homed : ARRAY[0..31] OF BOOL;
    g_trust_motion_axis_warning : ARRAY[0..31] OF BOOL;
    g_trust_motion_constant_velocity : ARRAY[0..31] OF BOOL;
    g_trust_motion_accelerating : ARRAY[0..31] OF BOOL;
    g_trust_motion_decelerating : ARRAY[0..31] OF BOOL;
    g_trust_motion_vel_factor : ARRAY[0..31] OF REAL;
    g_trust_motion_acc_factor : ARRAY[0..31] OF REAL;
    g_trust_motion_jerk_factor : ARRAY[0..31] OF REAL;
    g_trust_motion_sw_limit_pos : ARRAY[0..31] OF REAL;
    g_trust_motion_sw_limit_neg : ARRAY[0..31] OF REAL;
    g_trust_motion_enable_limit_pos : ARRAY[0..31] OF BOOL;
    g_trust_motion_enable_limit_neg : ARRAY[0..31] OF BOOL;
    g_trust_motion_enable_pos_lag_monitoring : ARRAY[0..31] OF BOOL;
    g_trust_motion_max_position_lag : ARRAY[0..31] OF REAL;
    g_trust_motion_max_velocity_system : ARRAY[0..31] OF REAL;
    g_trust_motion_max_velocity_appl : ARRAY[0..31] OF REAL;
    g_trust_motion_max_acceleration_system : ARRAY[0..31] OF REAL;
    g_trust_motion_max_acceleration_appl : ARRAY[0..31] OF REAL;
    g_trust_motion_max_deceleration_system : ARRAY[0..31] OF REAL;
    g_trust_motion_max_deceleration_appl : ARRAY[0..31] OF REAL;
    g_trust_motion_max_jerk_system : ARRAY[0..31] OF REAL;
    g_trust_motion_max_jerk_appl : ARRAY[0..31] OF REAL;
    g_trust_motion_vendor_bool_1000 : ARRAY[0..31] OF BOOL;
END_VAR

PROGRAM Main
END_PROGRAM
