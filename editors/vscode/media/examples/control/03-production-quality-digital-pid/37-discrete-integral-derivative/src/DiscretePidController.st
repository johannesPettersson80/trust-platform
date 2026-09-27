FUNCTION Lesson37IntegralTorque : LREAL
VAR_INPUT
    PreviousIntegralTorque : LREAL;
    IntegralGain : LREAL;
    Error : LREAL;
    ElapsedSeconds : LREAL;
END_VAR
Lesson37IntegralTorque := PreviousIntegralTorque
    + IntegralGain * Error * ElapsedSeconds;
END_FUNCTION

FUNCTION Lesson37DerivativeTorque : LREAL
VAR_INPUT
    DerivativeGain : LREAL;
    PreviousPosition : LREAL;
    CurrentPosition : LREAL;
    ElapsedSeconds : LREAL;
END_VAR
Lesson37DerivativeTorque := DerivativeGain
    * (CurrentPosition - PreviousPosition) / ElapsedSeconds;
END_FUNCTION

FUNCTION_BLOCK Lesson37DiscretePID
VAR
    Initialized : BOOL := FALSE;
    PreviousPosition : LREAL := 0.0;
    IntegralTorque : LREAL := 0.0;
END_VAR

METHOD PUBLIC Update : LREAL
VAR_INPUT
    Setpoint : LREAL;
    FilteredPosition : LREAL;
    ElapsedSeconds : LREAL;
END_VAR
VAR
    Error : LREAL;
    DerivativeTorque : LREAL;
END_VAR
IF NOT Initialized THEN
    PreviousPosition := FilteredPosition;
    Initialized := TRUE;
END_IF;
Error := Setpoint - FilteredPosition;
IntegralTorque := Lesson37IntegralTorque(
    PreviousIntegralTorque := IntegralTorque,
    IntegralGain := LREAL#4.0,
    Error := Error,
    ElapsedSeconds := ElapsedSeconds);
DerivativeTorque := Lesson37DerivativeTorque(
    DerivativeGain := LREAL#7.0,
    PreviousPosition := PreviousPosition,
    CurrentPosition := FilteredPosition,
    ElapsedSeconds := ElapsedSeconds);
Update := LREAL#16.0 * Error + IntegralTorque - DerivativeTorque;
Update := MAX(MIN(Update, LREAL#10.0), LREAL#-10.0);
PreviousPosition := FilteredPosition;
END_METHOD
END_FUNCTION_BLOCK
