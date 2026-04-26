PROGRAM Main
VAR
    Conveyor : MC_OopAxis;
    AxisObject : itfAxis;
    PowerCommand : itfCommand;
    VelocityCommand : itfContinuousAxisCommand;
    HaltCommand : itfAxisCommand;
    Initialized : BOOL;
    CurrentStep : UINT;
    LabelsApplied : UDINT;
    DwellScans : UINT;
    LastError : MC_ERROR;
    Result : MC_ERROR;
END_VAR

IF NOT Initialized THEN
    Result := Conveyor.Bind(AxisId := UDINT#3201, InternalIndex := UINT#2);
    AxisObject := Conveyor;
    CurrentStep := UINT#10;
    LastError := Result;
    Initialized := TRUE;
END_IF;

Result := Conveyor.Refresh();

CASE CurrentStep OF
    UINT#10:
        PowerCommand := AxisObject.Power(Enable := TRUE, EnablePositive := TRUE, EnableNegative := TRUE);
        IF PowerCommand.Done THEN
            CurrentStep := UINT#20;
        ELSIF PowerCommand.Error THEN
            LastError := PowerCommand.ErrorId;
            CurrentStep := UINT#900;
        END_IF;
    UINT#20:
        VelocityCommand := AxisObject.MoveVelocity(
            Velocity := REAL#18.0,
            Acceleration := REAL#5.0,
            Deceleration := REAL#5.0,
            Jerk := REAL#1.0,
            BufferMode := MC_BUFFER_MODE#mcAborting
        );
        IF VelocityCommand.InVelocity THEN
            DwellScans := UINT#0;
            CurrentStep := UINT#30;
        END_IF;
    UINT#30:
        DwellScans := DwellScans + UINT#1;
        IF DwellScans >= UINT#3 THEN
            CurrentStep := UINT#40;
        END_IF;
    UINT#40:
        HaltCommand := AxisObject.Halt(
            Deceleration := REAL#8.0,
            Jerk := REAL#1.0,
            BufferMode := MC_BUFFER_MODE#mcAborting
        );
        IF HaltCommand.Busy THEN
            CurrentStep := UINT#41;
        END_IF;
    UINT#41:
        Result := AxisObject.Release();
        IF HaltCommand.Done THEN
            LabelsApplied := LabelsApplied + UDINT#1;
            CurrentStep := UINT#20;
        END_IF;
    UINT#900:
    ELSE
        LastError := WORD#16#0500;
        CurrentStep := UINT#900;
END_CASE;
END_PROGRAM
