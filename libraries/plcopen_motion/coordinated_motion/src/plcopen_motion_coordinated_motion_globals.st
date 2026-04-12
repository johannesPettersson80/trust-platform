VAR_GLOBAL
    g_trust_group_initialized : ARRAY[0..7] OF BOOL;
    g_trust_group_status : ARRAY[0..7] OF MC_GROUP_STATUS;
    g_trust_group_member_count : ARRAY[0..7] OF UINT;
    g_trust_group_member_axis_id : ARRAY[0..7, 0..3] OF UDINT;
    g_trust_group_member_internal_index : ARRAY[0..7, 0..3] OF UINT;
    g_trust_group_member_ident : ARRAY[0..7, 0..3] OF STRING[63];

    g_trust_group_commanded_x : ARRAY[0..7] OF REAL;
    g_trust_group_commanded_y : ARRAY[0..7] OF REAL;
    g_trust_group_commanded_z : ARRAY[0..7] OF REAL;
    g_trust_group_commanded_rx : ARRAY[0..7] OF REAL;
    g_trust_group_commanded_ry : ARRAY[0..7] OF REAL;
    g_trust_group_commanded_rz : ARRAY[0..7] OF REAL;

    g_trust_group_actual_x : ARRAY[0..7] OF REAL;
    g_trust_group_actual_y : ARRAY[0..7] OF REAL;
    g_trust_group_actual_z : ARRAY[0..7] OF REAL;
    g_trust_group_actual_rx : ARRAY[0..7] OF REAL;
    g_trust_group_actual_ry : ARRAY[0..7] OF REAL;
    g_trust_group_actual_rz : ARRAY[0..7] OF REAL;

    g_trust_group_commanded_path_velocity : ARRAY[0..7] OF REAL;
    g_trust_group_actual_path_velocity : ARRAY[0..7] OF REAL;
    g_trust_group_commanded_path_acceleration : ARRAY[0..7] OF REAL;
    g_trust_group_actual_path_acceleration : ARRAY[0..7] OF REAL;

    g_trust_group_active_coord_system : ARRAY[0..7] OF MC_COORD_SYSTEM;
    g_trust_group_next_command_id : ARRAY[0..7] OF UINT;
    g_trust_group_last_command_id : ARRAY[0..7] OF UINT;
    g_trust_group_last_command_state : ARRAY[0..7] OF MC_COMMAND_STATE;
    g_trust_group_last_command_coord_system : ARRAY[0..7] OF MC_COORD_SYSTEM;
    g_trust_group_transform_identity : ARRAY[0..7] OF BOOL;
    g_trust_group_power_stage : ARRAY[0..7] OF BOOL;
    g_trust_group_error_id : ARRAY[0..7] OF WORD;
    g_trust_group_tracking : ARRAY[0..7] OF BOOL;
    g_trust_group_in_sync : ARRAY[0..7] OF BOOL;
    g_trust_group_in_position : ARRAY[0..7] OF BOOL;
    g_trust_group_constant_velocity : ARRAY[0..7] OF BOOL;
    g_trust_group_accelerating : ARRAY[0..7] OF BOOL;
    g_trust_group_decelerating : ARRAY[0..7] OF BOOL;
    g_trust_group_dynamics_mode : ARRAY[0..7] OF MC_DYNAMICS_MODE;
    g_trust_group_transition_reference : ARRAY[0..7] OF MC_TRANSITION_REFERENCE;
    g_trust_group_swlimits : ARRAY[0..7, 1..4] OF MC_SWLIMIT;

    g_trust_group_kin_transform : ARRAY[0..7] OF MC_KIN_REF;
    g_trust_group_cartesian_trans_x : ARRAY[0..7, 0..1] OF REAL;
    g_trust_group_cartesian_trans_y : ARRAY[0..7, 0..1] OF REAL;
    g_trust_group_cartesian_trans_z : ARRAY[0..7, 0..1] OF REAL;
    g_trust_group_cartesian_rot_1 : ARRAY[0..7, 0..1] OF REAL;
    g_trust_group_cartesian_rot_2 : ARRAY[0..7, 0..1] OF REAL;
    g_trust_group_cartesian_rot_3 : ARRAY[0..7, 0..1] OF REAL;
    g_trust_group_coord_trans_x : ARRAY[0..7, 0..1] OF REAL;
    g_trust_group_coord_trans_y : ARRAY[0..7, 0..1] OF REAL;
    g_trust_group_coord_trans_z : ARRAY[0..7, 0..1] OF REAL;
    g_trust_group_coord_rot_x : ARRAY[0..7, 0..1] OF REAL;
    g_trust_group_coord_rot_y : ARRAY[0..7, 0..1] OF REAL;
    g_trust_group_coord_rot_z : ARRAY[0..7, 0..1] OF REAL;

    g_trust_group_reference_velocity : ARRAY[0..7] OF REAL;
    g_trust_group_reference_acceleration : ARRAY[0..7] OF REAL;
    g_trust_group_reference_deceleration : ARRAY[0..7] OF REAL;
    g_trust_group_reference_jerk : ARRAY[0..7] OF REAL;
    g_trust_group_default_velocity : ARRAY[0..7] OF REAL;
    g_trust_group_default_acceleration : ARRAY[0..7] OF REAL;
    g_trust_group_default_deceleration : ARRAY[0..7] OF REAL;
    g_trust_group_default_jerk : ARRAY[0..7] OF REAL;

    g_trust_group_override_vel_factor : ARRAY[0..7] OF REAL;
    g_trust_group_override_acc_factor : ARRAY[0..7] OF REAL;
    g_trust_group_override_jerk_factor : ARRAY[0..7] OF REAL;
    g_trust_group_stop_hold : ARRAY[0..7] OF BOOL;

    g_trust_group_command_elapsed_duration : ARRAY[0..7] OF TIME;
    g_trust_group_command_remaining_duration : ARRAY[0..7] OF TIME;
    g_trust_group_command_remaining_distance : ARRAY[0..7] OF REAL;
    g_trust_group_command_progress : ARRAY[0..7] OF REAL;
    g_trust_group_command_info_id : ARRAY[0..7] OF WORD;
    g_trust_group_command_warning_id : ARRAY[0..7] OF WORD;
END_VAR
