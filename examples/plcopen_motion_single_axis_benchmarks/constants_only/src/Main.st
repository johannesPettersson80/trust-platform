PROGRAM Main
VAR
    Constants : MC_Constants;
END_VAR

g_motion_bench_cycles := g_motion_bench_cycles + UDINT#1;
g_motion_bench_current_step := UINT#1;
Constants();
g_motion_bench_completed_sequences := g_motion_bench_cycles;
g_motion_bench_last_error := Constants.mcERR_None;
g_motion_bench_power_on := FALSE;
g_motion_bench_is_homed := FALSE;
g_motion_bench_last_position := REAL#0.0;
g_motion_bench_last_velocity := REAL#0.0;
END_PROGRAM
