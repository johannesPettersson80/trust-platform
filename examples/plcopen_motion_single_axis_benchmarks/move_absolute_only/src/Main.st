PROGRAM Main
VAR
    Axis : AXIS_REF;
    Power : MC_Power;
    MoveAbsolute : MC_MoveAbsolute;
    ReadStatus : MC_ReadStatus;
    ReadAxisError : MC_ReadAxisError;
    ReadActualPosition : MC_ReadActualPosition;
    Initialized : BOOL;
    MotionStep : UINT;
    TargetPosition : REAL;
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
    g_motion_bench_current_step := UINT#10;
    MotionStep := UINT#10;
    TargetPosition := REAL#80.0;
    Initialized := TRUE;
END_IF;

g_motion_bench_cycles := g_motion_bench_cycles + UDINT#1;
Power(Axis := Axis, Enable := TRUE);
MoveAbsolute(
    Axis := Axis,
    Execute := MotionStep = UINT#20,
    ContinuousUpdate := FALSE,
    Position := TargetPosition,
    Velocity := REAL#25.0,
    Acceleration := REAL#0.0,
    Deceleration := REAL#0.0,
    Jerk := REAL#0.0,
    Direction := MC_DIRECTION#mcShortestWay,
    BufferMode := MC_BUFFER_MODE#mcAborting
);
ReadStatus(Axis := Axis, Enable := TRUE);
ReadAxisError(Axis := Axis, Enable := TRUE);
ReadActualPosition(Axis := Axis, Enable := TRUE);

g_motion_bench_power_on := Power.Status;
g_motion_bench_is_homed := FALSE;
g_motion_bench_last_position := ReadActualPosition.Position;
g_motion_bench_last_velocity := REAL#0.0;
g_motion_bench_current_step := MotionStep;

IF Power.Error THEN
    g_motion_bench_last_error := Power.ErrorID;
ELSIF MoveAbsolute.Error THEN
    g_motion_bench_last_error := MoveAbsolute.ErrorID;
ELSIF ReadAxisError.AxisErrorID <> WORD#16#0000 THEN
    g_motion_bench_last_error := ReadAxisError.AxisErrorID;
ELSE
    g_motion_bench_last_error := WORD#16#0000;
END_IF;

CASE MotionStep OF
    UINT#10:
        IF Power.Status THEN
            MotionStep := UINT#20;
        END_IF;
    UINT#20:
        IF MoveAbsolute.Busy AND MoveAbsolute.Active THEN
            MotionStep := UINT#30;
        END_IF;
    UINT#30:
        IF MoveAbsolute.Done THEN
            g_motion_bench_completed_sequences := g_motion_bench_completed_sequences + UDINT#1;
            IF TargetPosition > REAL#0.1 THEN
                TargetPosition := REAL#0.0;
            ELSE
                TargetPosition := REAL#80.0;
            END_IF;
            MotionStep := UINT#20;
        END_IF;
    ELSE
        MotionStep := UINT#10;
END_CASE;
END_PROGRAM
