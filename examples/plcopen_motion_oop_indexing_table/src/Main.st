PROGRAM Main
VAR
    TableAxis : MC_OopAxis;
    AxisObject : itfAxis;
    PowerCommand : itfCommand;
    MoveCommand : itfAxisCommand;
    Initialized : BOOL;
    CurrentStep : UINT;
    StationIndex : UINT;
    CompletedIndexes : UDINT;
    TargetPosition : REAL;
    LastError : MC_ERROR;
    Result : MC_ERROR;
END_VAR

IF NOT Initialized THEN
    Result := TableAxis.Bind(AxisId := UDINT#3401, InternalIndex := UINT#4);
    AxisObject := TableAxis;
    CurrentStep := UINT#10;
    StationIndex := UINT#0;
    LastError := Result;
    Initialized := TRUE;
END_IF;

Result := TableAxis.Refresh();

CASE CurrentStep OF
    UINT#10:
        PowerCommand := AxisObject.Power(Enable := TRUE, EnablePositive := TRUE, EnableNegative := TRUE);
        IF PowerCommand.Done THEN
            CurrentStep := UINT#20;
        END_IF;
    UINT#20:
        TargetPosition := REAL#90.0 * UINT_TO_REAL(StationIndex + UINT#1);
        MoveCommand := AxisObject.MoveAbsolute(
            Position := TargetPosition,
            Velocity := REAL#30.0,
            Acceleration := REAL#10.0,
            Deceleration := REAL#10.0,
            Jerk := REAL#1.0,
            Direction := MC_DIRECTION#mcPositiveDirection,
            BufferMode := MC_BUFFER_MODE#mcAborting
        );
        IF MoveCommand.Busy THEN
            CurrentStep := UINT#21;
        END_IF;
    UINT#21:
        Result := AxisObject.Release();
        IF MoveCommand.Done THEN
            StationIndex := StationIndex + UINT#1;
            IF StationIndex >= UINT#4 THEN
                StationIndex := UINT#0;
            END_IF;
            CompletedIndexes := CompletedIndexes + UDINT#1;
            CurrentStep := UINT#20;
        END_IF;
    UINT#900:
    ELSE
        LastError := WORD#16#0500;
        CurrentStep := UINT#900;
END_CASE;
END_PROGRAM
