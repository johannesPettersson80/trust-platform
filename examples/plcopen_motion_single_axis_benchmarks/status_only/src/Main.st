PROGRAM Main
VAR
    Axis : AXIS_REF;
    Power : MC_Power;
    ReadStatus : MC_ReadStatus;
    ReadAxisInfo : MC_ReadAxisInfo;
    ReadMotionState : MC_ReadMotionState;
    ReadAxisError : MC_ReadAxisError;
    ReadActualPosition : MC_ReadActualPosition;
    ReadActualVelocity : MC_ReadActualVelocity;
    Initialized : BOOL;
END_VAR

IF NOT Initialized THEN
    Axis.AxisId := UDINT#1;
    Axis.InternalIndex := UINT#1;
    g_motion_bench_cycles := UDINT#0;
    g_motion_bench_completed_sequences := UDINT#0;
    g_motion_bench_last_error := WORD#16#0000;
    g_motion_bench_power_on := FALSE;
    g_motion_bench_is_homed := FALSE;
    g_motion_bench_last_position := REAL#0.0;
    g_motion_bench_last_velocity := REAL#0.0;
    g_motion_bench_current_step := UINT#1;
    Initialized := TRUE;
END_IF;

g_motion_bench_cycles := g_motion_bench_cycles + UDINT#1;
Power(Axis := Axis, Enable := TRUE);
ReadStatus(Axis := Axis, Enable := TRUE);
ReadAxisInfo(Axis := Axis, Enable := TRUE);
ReadMotionState(Axis := Axis, Enable := TRUE, Source := MC_SOURCE#mcActualValue);
ReadAxisError(Axis := Axis, Enable := TRUE);
ReadActualPosition(Axis := Axis, Enable := TRUE);
ReadActualVelocity(Axis := Axis, Enable := TRUE);

g_motion_bench_completed_sequences := g_motion_bench_cycles;
g_motion_bench_power_on := Power.Status;
g_motion_bench_is_homed := ReadAxisInfo.IsHomed;
g_motion_bench_last_position := ReadActualPosition.Position;
g_motion_bench_last_velocity := ReadActualVelocity.Velocity;
g_motion_bench_current_step := UINT#1;

IF Power.Error THEN
    g_motion_bench_last_error := Power.ErrorID;
ELSIF ReadStatus.Error THEN
    g_motion_bench_last_error := ReadStatus.ErrorID;
ELSIF ReadAxisInfo.Error THEN
    g_motion_bench_last_error := ReadAxisInfo.ErrorID;
ELSIF ReadMotionState.Error THEN
    g_motion_bench_last_error := ReadMotionState.ErrorID;
ELSIF ReadAxisError.AxisErrorID <> WORD#16#0000 THEN
    g_motion_bench_last_error := ReadAxisError.AxisErrorID;
ELSIF ReadActualPosition.Error THEN
    g_motion_bench_last_error := ReadActualPosition.ErrorID;
ELSIF ReadActualVelocity.Error THEN
    g_motion_bench_last_error := ReadActualVelocity.ErrorID;
ELSE
    g_motion_bench_last_error := WORD#16#0000;
END_IF;
END_PROGRAM
