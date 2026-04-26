PROGRAM Main
VAR
    Lift : MC_OopAxis;
    AxisObject : itfAxis;
    PowerCommand : itfCommand;
    HomeCommand : itfAxisCommand;
    MoveCommand : itfAxisCommand;
    SetPositionCommand : itfCommand;
    Initialized : BOOL;
    CurrentStep : UINT;
    PartsPlaced : UDINT;
    LastError : MC_ERROR;
    Result : MC_ERROR;
END_VAR

IF NOT Initialized THEN
    Result := Lift.Bind(AxisId := UDINT#3301, InternalIndex := UINT#3);
    AxisObject := Lift;
    CurrentStep := UINT#10;
    LastError := Result;
    Initialized := TRUE;
END_IF;

Result := Lift.Refresh();

CASE CurrentStep OF
    UINT#10:
        PowerCommand := AxisObject.Power(Enable := TRUE, EnablePositive := TRUE, EnableNegative := TRUE);
        IF PowerCommand.Done THEN
            CurrentStep := UINT#20;
        END_IF;
    UINT#20:
        HomeCommand := AxisObject.Home(Position := REAL#0.0, BufferMode := MC_BUFFER_MODE#mcAborting);
        IF HomeCommand.Busy THEN
            CurrentStep := UINT#21;
        END_IF;
    UINT#21:
        Result := AxisObject.Release();
        IF HomeCommand.Done THEN
            CurrentStep := UINT#30;
        END_IF;
    UINT#30:
        MoveCommand := AxisObject.MoveAbsolute(
            Position := REAL#25.0,
            Velocity := REAL#12.0,
            Acceleration := REAL#6.0,
            Deceleration := REAL#6.0,
            Jerk := REAL#1.0,
            Direction := MC_DIRECTION#mcPositiveDirection,
            BufferMode := MC_BUFFER_MODE#mcAborting
        );
        IF MoveCommand.Busy THEN
            CurrentStep := UINT#31;
        END_IF;
    UINT#31:
        Result := AxisObject.Release();
        IF MoveCommand.Done THEN
            CurrentStep := UINT#40;
        END_IF;
    UINT#40:
        SetPositionCommand := AxisObject.SetPosition(
            Position := REAL#0.0,
            Relative := FALSE,
            ExecutionMode := MC_EXECUTION_MODE#mcImmediately
        );
        IF SetPositionCommand.Done THEN
            CurrentStep := UINT#50;
        END_IF;
    UINT#50:
        MoveCommand := AxisObject.MoveAbsolute(
            Position := REAL#75.0,
            Velocity := REAL#15.0,
            Acceleration := REAL#8.0,
            Deceleration := REAL#8.0,
            Jerk := REAL#1.0,
            Direction := MC_DIRECTION#mcPositiveDirection,
            BufferMode := MC_BUFFER_MODE#mcAborting
        );
        IF MoveCommand.Busy THEN
            CurrentStep := UINT#51;
        END_IF;
    UINT#51:
        Result := AxisObject.Release();
        IF MoveCommand.Done THEN
            PartsPlaced := PartsPlaced + UDINT#1;
            CurrentStep := UINT#30;
        END_IF;
    UINT#900:
    ELSE
        LastError := WORD#16#0500;
        CurrentStep := UINT#900;
END_CASE;
END_PROGRAM
