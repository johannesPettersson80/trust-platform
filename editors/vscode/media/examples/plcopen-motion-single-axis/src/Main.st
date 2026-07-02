PROGRAM Main
VAR
    Axis : AXIS_REF;
    Power : MC_Power;
    MoveAbsolute : MC_MoveAbsolute;
    ReadActualPosition : MC_ReadActualPosition;
    ReadStatus : MC_ReadStatus;
    Initialized : BOOL;
    Step : INT;
    MoveExecute : BOOL;
    motion_demo_cycles     : INT;   (* %MW0 - completed motion moves *)
    motion_demo_error      : INT;   (* %MW2 - 0 healthy, 1 faulted *)
    motion_demo_power_on   : BOOL;  (* %QX0.0 *)
    motion_demo_target_ok  : BOOL;  (* %QX0.1 *)
    motion_demo_standstill : BOOL;  (* %QX0.2 *)
END_VAR

IF NOT Initialized THEN
    Axis.AxisId := UDINT#1;
    Axis.InternalIndex := UINT#1;
    Step := 0;
    motion_demo_cycles := 0;
    motion_demo_error := 0;
    motion_demo_power_on := FALSE;
    motion_demo_target_ok := FALSE;
    motion_demo_standstill := FALSE;
    Initialized := TRUE;
END_IF;

MoveExecute := Step = 10;

Power(Axis := Axis, Enable := TRUE);
MoveAbsolute(
    Axis := Axis,
    Execute := MoveExecute,
    ContinuousUpdate := FALSE,
    Position := REAL#25.0,
    Velocity := REAL#10.0,
    Acceleration := REAL#0.0,
    Deceleration := REAL#0.0,
    Jerk := REAL#0.0,
    Direction := MC_DIRECTION#mcPositiveDirection,
    BufferMode := MC_BUFFER_MODE#mcAborting
);
ReadActualPosition(Axis := Axis, Enable := TRUE);
ReadStatus(Axis := Axis, Enable := TRUE);

motion_demo_power_on := Power.Status;
motion_demo_standstill := ReadStatus.Standstill;
motion_demo_target_ok :=
    (ReadActualPosition.Position >= REAL#24.9) AND
    (ReadActualPosition.Position <= REAL#25.1);

IF Power.Error OR MoveAbsolute.Error OR ReadActualPosition.Error OR ReadStatus.Error THEN
    motion_demo_error := 1;
    Step := 900;
END_IF;

CASE Step OF
    0:
        IF Power.Status THEN
            Step := 10;
        END_IF;
    10:
        Step := 20;
    20:
        IF MoveAbsolute.Done THEN
            motion_demo_cycles := motion_demo_cycles + 1;
            Step := 30;
        END_IF;
    30:
        (* Keep readback visible in Live Values. *)
    900:
    ELSE
        motion_demo_error := 1;
        Step := 900;
END_CASE;
END_PROGRAM
