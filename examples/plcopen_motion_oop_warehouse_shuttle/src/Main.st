PROGRAM Main
VAR
    Shuttle : MC_OopAxis;
    AxisObject : itfAxis;
    PowerCommand : itfCommand;
    HomeCommand : itfAxisCommand;
    MoveCommand : itfAxisCommand;
    Initialized : BOOL;
    CurrentStep : UINT;
    CompletedCycles : UDINT;
    LastError : MC_ERROR;
    Result : MC_ERROR;
END_VAR

IF NOT Initialized THEN
    Result := Shuttle.Bind(AxisId := UDINT#3101, InternalIndex := UINT#1);
    AxisObject := Shuttle;
    CurrentStep := UINT#10;
    LastError := Result;
    Initialized := TRUE;
END_IF;

IF CurrentStep <> UINT#900 THEN
    Result := Shuttle.Refresh();
END_IF;

CASE CurrentStep OF
    UINT#10:
        PowerCommand := AxisObject.Power(Enable := TRUE, EnablePositive := TRUE, EnableNegative := TRUE);
        IF PowerCommand.Error THEN
            LastError := PowerCommand.ErrorId;
            CurrentStep := UINT#900;
        ELSIF PowerCommand.Done THEN
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
            Position := REAL#125.0,
            Velocity := REAL#40.0,
            Acceleration := REAL#10.0,
            Deceleration := REAL#10.0,
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
        MoveCommand := AxisObject.MoveAbsolute(
            Position := REAL#0.0,
            Velocity := REAL#45.0,
            Acceleration := REAL#10.0,
            Deceleration := REAL#10.0,
            Jerk := REAL#1.0,
            Direction := MC_DIRECTION#mcNegativeDirection,
            BufferMode := MC_BUFFER_MODE#mcAborting
        );
        IF MoveCommand.Busy THEN
            CurrentStep := UINT#41;
        END_IF;
    UINT#41:
        Result := AxisObject.Release();
        IF MoveCommand.Done THEN
            CompletedCycles := CompletedCycles + UDINT#1;
            CurrentStep := UINT#30;
        END_IF;
    UINT#900:
    ELSE
        LastError := WORD#16#0500;
        CurrentStep := UINT#900;
END_CASE;
END_PROGRAM
