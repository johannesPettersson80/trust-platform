PROGRAM Main
VAR
    Feeder : MC_OopAxis;
    AxisObject : itfAxis;
    Constants : MC_Constants;
    PowerCommand : itfCommand;
    VelocityCommand : itfContinuousAxisCommand;
    Initialized : BOOL;
    ConstantsLoaded : BOOL;
    CurrentStep : UINT;
    FeedWindows : UDINT;
    ParameterValue : REAL;
    LastError : MC_ERROR;
    Result : MC_ERROR;
END_VAR

IF NOT ConstantsLoaded THEN
    Constants();
    ConstantsLoaded := TRUE;
END_IF;

IF NOT Initialized THEN
    Result := Feeder.Bind(AxisId := UDINT#3501, InternalIndex := UINT#5);
    AxisObject := Feeder;
    CurrentStep := UINT#10;
    LastError := Result;
    Initialized := TRUE;
END_IF;

Result := Feeder.Refresh();

CASE CurrentStep OF
    UINT#10:
        Result := AxisObject.WriteParameter(
            ParameterNumber := Constants.PN_MaxVelocityAppl,
            Value := REAL#60.0
        );
        IF Result = WORD#16#0000 THEN
            CurrentStep := UINT#20;
        ELSE
            LastError := Result;
            CurrentStep := UINT#900;
        END_IF;
    UINT#20:
        PowerCommand := AxisObject.Power(Enable := TRUE, EnablePositive := TRUE, EnableNegative := TRUE);
        IF PowerCommand.Done THEN
            CurrentStep := UINT#30;
        END_IF;
    UINT#30:
        VelocityCommand := AxisObject.MoveVelocity(
            Velocity := REAL#22.0,
            Acceleration := REAL#5.0,
            Deceleration := REAL#5.0,
            Jerk := REAL#1.0,
            BufferMode := MC_BUFFER_MODE#mcAborting
        );
        IF VelocityCommand.InVelocity THEN
            CurrentStep := UINT#40;
        END_IF;
    UINT#40:
        Result := AxisObject.ReadParameter(
            ParameterNumber := Constants.PN_MaxVelocityAppl,
            Value => ParameterValue
        );
        IF Result = WORD#16#0000 THEN
            FeedWindows := FeedWindows + UDINT#1;
            CurrentStep := UINT#30;
        ELSE
            LastError := Result;
            CurrentStep := UINT#900;
        END_IF;
    UINT#900:
    ELSE
        LastError := WORD#16#0500;
        CurrentStep := UINT#900;
END_CASE;
END_PROGRAM
