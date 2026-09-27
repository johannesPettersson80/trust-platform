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
(* D is a signed controller contribution. Positive measured motion therefore
   produces negative braking torque. *)
Lesson37DerivativeTorque := -DerivativeGain
    * (CurrentPosition - PreviousPosition) / ElapsedSeconds;
END_FUNCTION

FUNCTION_BLOCK Lesson37DiscretePID
VAR
    Initialized : BOOL := FALSE;
    PreviousPosition : LREAL := 0.0;
END_VAR
VAR_OUTPUT
    ProportionalTorque : LREAL := 0.0;
    IntegralTorque : LREAL := 0.0;
    TrialIntegralTorque : LREAL := 0.0;
    DerivativeTorque : LREAL := 0.0;
    RequestedTorque : LREAL := 0.0;
    AppliedTorque : LREAL := 0.0;
    PreviousPositionOutput : LREAL := 0.0;
END_VAR

METHOD PUBLIC Update : LREAL
VAR_INPUT
    Setpoint : LREAL;
    FilteredPosition : LREAL;
    ElapsedSeconds : LREAL;
END_VAR
VAR
    Error : LREAL;
    TrackingCorrection : LREAL;
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
TrialIntegralTorque := IntegralTorque;
DerivativeTorque := Lesson37DerivativeTorque(
    DerivativeGain := LREAL#7.0,
    PreviousPosition := PreviousPosition,
    CurrentPosition := FilteredPosition,
    ElapsedSeconds := ElapsedSeconds);
ProportionalTorque := LREAL#16.0 * Error;
RequestedTorque := ProportionalTorque + TrialIntegralTorque + DerivativeTorque;
AppliedTorque := MAX(MIN(RequestedTorque, LREAL#10.0), LREAL#-10.0);
TrackingCorrection := (ElapsedSeconds / LREAL#0.2)
    * (AppliedTorque - RequestedTorque);
IntegralTorque := TrialIntegralTorque + TrackingCorrection;
Update := AppliedTorque;
PreviousPositionOutput := PreviousPosition;
PreviousPosition := FilteredPosition;
END_METHOD
END_FUNCTION_BLOCK
