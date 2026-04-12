PROGRAM Main
VAR
    Axis : AXIS_REF;
    Power : MC_Power;
    Home : MC_Home;
    MoveAbsolute : MC_MoveAbsolute;
    MoveRelative : MC_MoveRelative;
    MoveVelocity : MC_MoveVelocity;
    Stop : MC_Stop;
    WriteLimitPos : MC_WriteParameter;
    WriteEnableLimitPos : MC_WriteBoolParameter;
    ReadStatus : MC_ReadStatus;
    ReadAxisInfo : MC_ReadAxisInfo;
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
WriteLimitPos(Axis := Axis, Execute := FALSE, ParameterNumber := INT#2, Value := REAL#90.0, ExecutionMode := MC_EXECUTION_MODE#mcImmediately);
WriteEnableLimitPos(Axis := Axis, Execute := FALSE, ParameterNumber := INT#4, Value := TRUE, ExecutionMode := MC_EXECUTION_MODE#mcImmediately);
Home(Axis := Axis, Execute := FALSE, Position := REAL#0.0, BufferMode := MC_BUFFER_MODE#mcAborting);
MoveAbsolute(Axis := Axis, Execute := FALSE, ContinuousUpdate := FALSE, Position := REAL#80.0, Velocity := REAL#25.0, Acceleration := REAL#0.0, Deceleration := REAL#0.0, Jerk := REAL#0.0, Direction := MC_DIRECTION#mcPositiveDirection, BufferMode := MC_BUFFER_MODE#mcAborting);
MoveRelative(Axis := Axis, Execute := FALSE, ContinuousUpdate := FALSE, Distance := REAL#15.0, Velocity := REAL#20.0, Acceleration := REAL#0.0, Deceleration := REAL#0.0, Jerk := REAL#0.0, BufferMode := MC_BUFFER_MODE#mcBuffered);
MoveVelocity(Axis := Axis, Execute := FALSE, ContinuousUpdate := FALSE, Velocity := REAL#10.0, Acceleration := REAL#0.0, Deceleration := REAL#0.0, Jerk := REAL#0.0, Direction := MC_DIRECTION#mcPositiveDirection, BufferMode := MC_BUFFER_MODE#mcAborting);
Stop(Axis := Axis, Execute := FALSE, Deceleration := REAL#0.0, Jerk := REAL#0.0);
ReadStatus(Axis := Axis, Enable := TRUE);
ReadAxisInfo(Axis := Axis, Enable := TRUE);
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
ELSIF WriteLimitPos.Error THEN
    g_motion_bench_last_error := WriteLimitPos.ErrorID;
ELSIF WriteEnableLimitPos.Error THEN
    g_motion_bench_last_error := WriteEnableLimitPos.ErrorID;
ELSIF Home.Error THEN
    g_motion_bench_last_error := Home.ErrorID;
ELSIF MoveAbsolute.Error THEN
    g_motion_bench_last_error := MoveAbsolute.ErrorID;
ELSIF MoveRelative.Error THEN
    g_motion_bench_last_error := MoveRelative.ErrorID;
ELSIF MoveVelocity.Error THEN
    g_motion_bench_last_error := MoveVelocity.ErrorID;
ELSIF Stop.Error THEN
    g_motion_bench_last_error := Stop.ErrorID;
ELSIF ReadAxisError.AxisErrorID <> WORD#16#0000 THEN
    g_motion_bench_last_error := ReadAxisError.AxisErrorID;
ELSE
    g_motion_bench_last_error := WORD#16#0000;
END_IF;
END_PROGRAM
