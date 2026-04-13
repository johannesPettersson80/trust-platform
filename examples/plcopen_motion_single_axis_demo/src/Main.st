PROGRAM Main
VAR
    Axis : AXIS_REF;
    Constants : MC_Constants;
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
    ConstantsLoaded : BOOL;
    PreviousStep : UINT;
    StepScanCounter : UDINT;
END_VAR

(* Load published parameter numbers and motion error constants once. *)
IF NOT ConstantsLoaded THEN
    Constants();
    ConstantsLoaded := TRUE;
END_IF;

(* One-time demo setup: shared axis handle plus visible status globals. *)
IF NOT Initialized THEN
    Axis.AxisId := UDINT#1;
    Axis.InternalIndex := UINT#1;
    g_motion_demo_completed_sequences := UDINT#0;
    g_motion_demo_last_error := Constants.mcERR_None;
    g_motion_demo_current_step := UINT#10;
    g_motion_demo_limit_clamp_verified := FALSE;
    g_motion_demo_last_position := REAL#0.0;
    g_motion_demo_last_velocity := REAL#0.0;
    g_motion_demo_power_on := FALSE;
    g_motion_demo_is_homed := FALSE;
    PreviousStep := g_motion_demo_current_step;
    StepScanCounter := UDINT#0;
    Initialized := TRUE;
END_IF;

IF g_motion_demo_current_step <> PreviousStep THEN
    PreviousStep := g_motion_demo_current_step;
    StepScanCounter := UDINT#0;
ELSE
    StepScanCounter := StepScanCounter + UDINT#1;
END_IF;

(* PLCopen FBs are called every scan; the current step drives Execute inputs. *)
Power(Axis := Axis, Enable := g_motion_demo_current_step <> UINT#900);
WriteLimitPos(
    Axis := Axis,
    Execute := g_motion_demo_current_step = UINT#10,
    ParameterNumber := Constants.PN_SWLimitPos,
    Value := REAL#90.0,
    ExecutionMode := MC_EXECUTION_MODE#mcImmediately
);
WriteEnableLimitPos(
    Axis := Axis,
    Execute := g_motion_demo_current_step = UINT#20,
    ParameterNumber := Constants.PN_EnableLimitPos,
    Value := TRUE,
    ExecutionMode := MC_EXECUTION_MODE#mcImmediately
);
Home(
    Axis := Axis,
    Execute := g_motion_demo_current_step = UINT#40,
    Position := REAL#0.0,
    BufferMode := MC_BUFFER_MODE#mcAborting
);
MoveAbsolute(
    Axis := Axis,
    Execute :=
        (g_motion_demo_current_step = UINT#50) OR
        (g_motion_demo_current_step = UINT#51),
    ContinuousUpdate := FALSE,
    Position := REAL#80.0,
    Velocity := REAL#25.0,
    Acceleration := REAL#0.0,
    Deceleration := REAL#0.0,
    Jerk := REAL#0.0,
    Direction := MC_DIRECTION#mcPositiveDirection,
    BufferMode := MC_BUFFER_MODE#mcAborting
);
MoveRelative(
    Axis := Axis,
    Execute :=
        (g_motion_demo_current_step = UINT#51) OR
        (g_motion_demo_current_step = UINT#52),
    ContinuousUpdate := FALSE,
    Distance := REAL#15.0,
    Velocity := REAL#20.0,
    Acceleration := REAL#0.0,
    Deceleration := REAL#0.0,
    Jerk := REAL#0.0,
    BufferMode := MC_BUFFER_MODE#mcBuffered
);
MoveVelocity(
    Axis := Axis,
    Execute :=
        (g_motion_demo_current_step = UINT#60) OR
        (g_motion_demo_current_step = UINT#61),
    ContinuousUpdate := FALSE,
    Velocity := REAL#10.0,
    Acceleration := REAL#0.0,
    Deceleration := REAL#0.0,
    Jerk := REAL#0.0,
    Direction := MC_DIRECTION#mcPositiveDirection,
    BufferMode := MC_BUFFER_MODE#mcAborting
);
Stop(
    Axis := Axis,
    Execute := g_motion_demo_current_step = UINT#61,
    Deceleration := REAL#0.0,
    Jerk := REAL#0.0
);

ReadStatus(Axis := Axis, Enable := TRUE);
ReadAxisInfo(Axis := Axis, Enable := TRUE);
ReadAxisError(Axis := Axis, Enable := TRUE);
ReadActualPosition(Axis := Axis, Enable := TRUE);
ReadActualVelocity(Axis := Axis, Enable := TRUE);

g_motion_demo_power_on := Power.Status;
g_motion_demo_is_homed := ReadAxisInfo.IsHomed;
g_motion_demo_last_position := ReadActualPosition.Position;
g_motion_demo_last_velocity := ReadActualVelocity.Velocity;

IF g_motion_demo_last_error = Constants.mcERR_None THEN
    IF Power.Error THEN
        g_motion_demo_last_error := Power.ErrorID;
    ELSIF WriteLimitPos.Error THEN
        g_motion_demo_last_error := WriteLimitPos.ErrorID;
    ELSIF WriteEnableLimitPos.Error THEN
        g_motion_demo_last_error := WriteEnableLimitPos.ErrorID;
    ELSIF Home.Error THEN
        g_motion_demo_last_error := Home.ErrorID;
    ELSIF MoveAbsolute.Error THEN
        g_motion_demo_last_error := MoveAbsolute.ErrorID;
    ELSIF MoveRelative.Error THEN
        g_motion_demo_last_error := MoveRelative.ErrorID;
    ELSIF MoveVelocity.Error THEN
        g_motion_demo_last_error := MoveVelocity.ErrorID;
    ELSIF Stop.Error THEN
        g_motion_demo_last_error := Stop.ErrorID;
    ELSIF ReadAxisError.AxisErrorID <> Constants.mcERR_None THEN
        g_motion_demo_last_error := ReadAxisError.AxisErrorID;
    END_IF;
END_IF;

(* Sequence: configure limits, home, chain buffered moves, then stop and repeat. *)
CASE g_motion_demo_current_step OF
    UINT#10:
        IF Power.Status AND WriteLimitPos.Done THEN
            g_motion_demo_current_step := UINT#20;
        END_IF;
    UINT#20:
        IF WriteEnableLimitPos.Done THEN
            g_motion_demo_current_step := UINT#30;
        END_IF;
    UINT#30:
        IF Power.Status THEN
            g_motion_demo_current_step := UINT#40;
        END_IF;
    UINT#40:
        IF Home.Busy OR Home.Active THEN
            g_motion_demo_current_step := UINT#41;
        END_IF;
    UINT#41:
        IF Home.Done THEN
            g_motion_demo_current_step := UINT#50;
        END_IF;
    UINT#50:
        IF MoveAbsolute.Busy AND MoveAbsolute.Active THEN
            g_motion_demo_current_step := UINT#51;
        END_IF;
    UINT#51:
        IF MoveRelative.Busy AND (NOT MoveRelative.Active) THEN
            g_motion_demo_current_step := UINT#52;
        END_IF;
    UINT#52:
        IF MoveRelative.Active THEN
            g_motion_demo_current_step := UINT#53;
        END_IF;
    UINT#53:
        IF MoveRelative.Done THEN
            g_motion_demo_limit_clamp_verified :=
                (ReadActualPosition.Position >= REAL#89.9) AND
                (ReadActualPosition.Position <= REAL#90.1);
            g_motion_demo_current_step := UINT#60;
        END_IF;
    UINT#60:
        IF MoveVelocity.Busy AND MoveVelocity.Active AND MoveVelocity.InVelocity THEN
            g_motion_demo_current_step := UINT#61;
        END_IF;
    UINT#61:
        IF Stop.Done THEN
            g_motion_demo_current_step := UINT#62;
        END_IF;
    UINT#62:
        IF (NOT MoveVelocity.Busy) AND ReadStatus.Standstill THEN
            g_motion_demo_completed_sequences :=
                g_motion_demo_completed_sequences + UDINT#1;
            g_motion_demo_current_step := UINT#50;
        END_IF;
    UINT#900:
    ELSE
        g_motion_demo_last_error := Constants.mcERR_BackendFault;
        g_motion_demo_current_step := UINT#900;
END_CASE;

(* Fail closed if a step stalls or any FB reports an error. *)
IF (g_motion_demo_current_step <> UINT#900) AND
   (g_motion_demo_last_error = Constants.mcERR_None) AND
   (StepScanCounter > UDINT#32) THEN
    g_motion_demo_last_error := Constants.mcERR_BackendFault;
    g_motion_demo_current_step := UINT#900;
END_IF;

IF g_motion_demo_last_error <> Constants.mcERR_None THEN
    g_motion_demo_current_step := UINT#900;
END_IF;
END_PROGRAM
