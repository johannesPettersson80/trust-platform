USING Control;

(* ServoCandidate composes filtering and PID control for the flexible indexing table. It reports a requested torque; Main retains responsibility for measurement quality, operating authority, and the applied actuator limit. *)
FUNCTION_BLOCK ServoCandidate
VAR
    Controller : Control.PID;
    MeasurementFilter : Control.LowPass;
    ControllerInput : Control.PIDInput;
    Configured : BOOL := FALSE;
END_VAR
VAR_OUTPUT
    RawPosition : LREAL := 0.0;
    FilteredPosition : LREAL := 0.0;
    RequestedTorque : LREAL := 0.0;
    AppliedTorque : LREAL := 0.0;
END_VAR

METHOD PUBLIC Configure
VAR_INPUT
    ProportionalGain : LREAL;
    IntegralGain : LREAL;
    DerivativeGain : LREAL;
    FilterTimeConstant : LREAL;
    InitialPosition : LREAL;
END_VAR
VAR
    Parameters : Control.PIDParameters;
    InitialState : Control.PIDInitialState;
    FilterParameters : Control.LowPassParameters;
END_VAR

FilterParameters.TimeConstant := FilterTimeConstant;
FilterParameters.Order := FilterOrder#FirstOrder;
MeasurementFilter.Configure(Parameters := FilterParameters);
MeasurementFilter.Initialize(Input := InitialPosition);

Parameters.ProportionalGain := ProportionalGain;
Parameters.IntegralGain := IntegralGain;
Parameters.DerivativeGain := DerivativeGain;
Parameters.ProportionalSetpointWeight := LREAL#1.0;
(* Derivative action uses measured motion, not a setpoint step. *)
Parameters.DerivativeSetpointWeight := LREAL#0.0;
Parameters.Bias := LREAL#0.0;
Parameters.OutputMinimum := LREAL#-10.0;
Parameters.OutputMaximum := LREAL#10.0;
Parameters.RateMinimum := LREAL#-200.0;
Parameters.RateMaximum := LREAL#200.0;
Parameters.TrackingTime := LREAL#0.2;
Parameters.AntiWindup := PIDAntiWindup#Combined;
Controller.Configure(Parameters := Parameters);

InitialState.Setpoint := InitialPosition;
InitialState.ProcessValue := InitialPosition;
InitialState.NominalOutput := LREAL#0.0;
InitialState.SaturatedOutput := LREAL#0.0;
Controller.Initialize(State := InitialState);
RawPosition := InitialPosition;
FilteredPosition := InitialPosition;
Configured := TRUE;
END_METHOD

METHOD PUBLIC Update : LREAL
VAR_INPUT
    Setpoint : LREAL;
    MeasuredPosition : LREAL;
    AutomaticMode : BOOL;
    QualityGood : BOOL;
    ManualTorque : LREAL;
    CycleTime : LREAL;
END_VAR

IF NOT Configured THEN
    Update := LREAL#0.0;
ELSE
    RawPosition := MeasuredPosition;
    FilteredPosition := MeasurementFilter.Update(
        Input := MeasuredPosition,
        CycleTime := CycleTime
    );
    ControllerInput.Setpoint := Setpoint;
    ControllerInput.ProcessValue := FilteredPosition;
    ControllerInput.FeedForward := LREAL#0.0;
    ControllerInput.TrackingOutput := ManualTorque;
    ControllerInput.CycleTime := CycleTime;
    IF AutomaticMode AND QualityGood THEN
        ControllerInput.Mode := PIDMode#Auto;
        ControllerInput.ManualOutput := ManualTorque;
    ELSE
        ControllerInput.Mode := PIDMode#Manual;
        IF QualityGood THEN
            ControllerInput.ManualOutput := ManualTorque;
        ELSE
            ControllerInput.ManualOutput := LREAL#0.0;
        END_IF;
    END_IF;
    RequestedTorque := Controller.Update(Input := ControllerInput);
    AppliedTorque := LIMIT(LREAL#-10.0, RequestedTorque, LREAL#10.0);
    Update := AppliedTorque;
END_IF;
END_METHOD
END_FUNCTION_BLOCK
