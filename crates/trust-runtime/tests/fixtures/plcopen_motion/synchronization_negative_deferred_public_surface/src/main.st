VAR_GLOBAL
    g_trust_sync_axis_initialized : ARRAY[0..31] OF BOOL;
    g_trust_sync_axis_state : ARRAY[0..31] OF MC_AXIS_STATUS;
    g_trust_sync_axis_commanded_position : ARRAY[0..31] OF REAL;
    g_trust_sync_axis_actual_position : ARRAY[0..31] OF REAL;
    g_trust_sync_axis_last_commanded_position : ARRAY[0..31] OF REAL;
    g_trust_sync_axis_velocity : ARRAY[0..31] OF REAL;

    g_trust_sync_selected_cam_ready : ARRAY[0..31] OF BOOL;
    g_trust_sync_selected_cam_id : ARRAY[0..31] OF MC_CAM_ID;
    g_trust_sync_selected_cam_periodic : ARRAY[0..31] OF BOOL;
    g_trust_sync_selected_cam_master_absolute : ARRAY[0..31] OF BOOL;
    g_trust_sync_selected_cam_slave_absolute : ARRAY[0..31] OF BOOL;
    g_trust_sync_selected_cam_point_count : ARRAY[0..31] OF UINT;
    g_trust_sync_selected_cam_master : ARRAY[0..31, 0..7] OF REAL;
    g_trust_sync_selected_cam_slave : ARRAY[0..31, 0..7] OF REAL;

    g_trust_sync_next_command_id : ARRAY[0..31] OF UINT;

    g_trust_sync_active_command_id : ARRAY[0..31] OF UINT;
    g_trust_sync_active_command_kind : ARRAY[0..31] OF TRUST_SYNC_COMMAND_KIND;
    g_trust_sync_active_buffer_mode : ARRAY[0..31] OF MC_BUFFER_MODE;
    g_trust_sync_active_master_axis_index : ARRAY[0..31] OF UINT;
    g_trust_sync_active_ratio_numerator : ARRAY[0..31] OF INT;
    g_trust_sync_active_ratio_denominator : ARRAY[0..31] OF UINT;
    g_trust_sync_active_master_offset : ARRAY[0..31] OF REAL;
    g_trust_sync_active_slave_offset : ARRAY[0..31] OF REAL;
    g_trust_sync_active_master_scaling : ARRAY[0..31] OF REAL;
    g_trust_sync_active_slave_scaling : ARRAY[0..31] OF REAL;
    g_trust_sync_active_master_start_distance : ARRAY[0..31] OF REAL;
    g_trust_sync_active_master_sync_position : ARRAY[0..31] OF REAL;
    g_trust_sync_active_slave_sync_position : ARRAY[0..31] OF REAL;
    g_trust_sync_active_velocity : ARRAY[0..31] OF REAL;
    g_trust_sync_active_acceleration : ARRAY[0..31] OF REAL;
    g_trust_sync_active_deceleration : ARRAY[0..31] OF REAL;
    g_trust_sync_active_jerk : ARRAY[0..31] OF REAL;
    g_trust_sync_active_start_mode : ARRAY[0..31] OF MC_START_MODE;
    g_trust_sync_active_sync_mode : ARRAY[0..31] OF MC_SYNC_MODE;
    g_trust_sync_active_master_value_source : ARRAY[0..31] OF MC_SOURCE;
    g_trust_sync_active_cam_table_id : ARRAY[0..31] OF MC_CAM_ID;
    g_trust_sync_active_command_start_master_position : ARRAY[0..31] OF REAL;
    g_trust_sync_active_command_start_slave_position : ARRAY[0..31] OF REAL;
    g_trust_sync_active_start_sync_seen : ARRAY[0..31] OF BOOL;
    g_trust_sync_active_start_sync_pulse : ARRAY[0..31] OF BOOL;
    g_trust_sync_active_end_of_profile_pulse : ARRAY[0..31] OF BOOL;
    g_trust_sync_active_in_sync : ARRAY[0..31] OF BOOL;
    g_trust_sync_active_in_gear : ARRAY[0..31] OF BOOL;
    g_trust_sync_active_previous_source_position : ARRAY[0..31] OF REAL;

    g_trust_sync_queued_command_id : ARRAY[0..31] OF UINT;
    g_trust_sync_queued_command_kind : ARRAY[0..31] OF TRUST_SYNC_COMMAND_KIND;
    g_trust_sync_queued_buffer_mode : ARRAY[0..31] OF MC_BUFFER_MODE;
    g_trust_sync_queued_master_axis_index : ARRAY[0..31] OF UINT;
    g_trust_sync_queued_ratio_numerator : ARRAY[0..31] OF INT;
    g_trust_sync_queued_ratio_denominator : ARRAY[0..31] OF UINT;
    g_trust_sync_queued_master_offset : ARRAY[0..31] OF REAL;
    g_trust_sync_queued_slave_offset : ARRAY[0..31] OF REAL;
    g_trust_sync_queued_master_scaling : ARRAY[0..31] OF REAL;
    g_trust_sync_queued_slave_scaling : ARRAY[0..31] OF REAL;
    g_trust_sync_queued_master_start_distance : ARRAY[0..31] OF REAL;
    g_trust_sync_queued_master_sync_position : ARRAY[0..31] OF REAL;
    g_trust_sync_queued_slave_sync_position : ARRAY[0..31] OF REAL;
    g_trust_sync_queued_velocity : ARRAY[0..31] OF REAL;
    g_trust_sync_queued_acceleration : ARRAY[0..31] OF REAL;
    g_trust_sync_queued_deceleration : ARRAY[0..31] OF REAL;
    g_trust_sync_queued_jerk : ARRAY[0..31] OF REAL;
    g_trust_sync_queued_start_mode : ARRAY[0..31] OF MC_START_MODE;
    g_trust_sync_queued_sync_mode : ARRAY[0..31] OF MC_SYNC_MODE;
    g_trust_sync_queued_master_value_source : ARRAY[0..31] OF MC_SOURCE;
    g_trust_sync_queued_cam_table_id : ARRAY[0..31] OF MC_CAM_ID;

    g_trust_sync_aborted0_command_id : ARRAY[0..31] OF UINT;
    g_trust_sync_aborted1_command_id : ARRAY[0..31] OF UINT;

    g_trust_sync_seed_start_position : ARRAY[0..31] OF REAL;
    g_trust_sync_seed_exit_position : ARRAY[0..31] OF REAL;
    g_trust_sync_seed_ratio_numerator : ARRAY[0..31] OF REAL;
    g_trust_sync_seed_ratio_denominator : ARRAY[0..31] OF REAL;
    g_trust_sync_seed_phase_offset : ARRAY[0..31] OF REAL;
    g_trust_sync_seed_break_requested : ARRAY[0..31] OF BOOL;
END_VAR

PROGRAM Main
END_PROGRAM
