USING Control;

(* OvenController compares ordinary PI, measured-load feedforward, and Smith prediction for the delayed conveyor oven. Its model-dependent result is a requested heater command; Main owns operating authority and final actuation. *)
FUNCTION_BLOCK OvenController
VAR
    TemperatureController : Control.PID;
    Predictor : Control.SmithPredictorFOPDT;
    ControllerParameters : Control.PIDParameters;
    InitialControllerState : Control.PIDInitialState;
    ControllerInput : Control.PIDInput;
    PredictorParameters : Control.SmithPredictorParameters;
    InitialPredictorState : Control.SmithPredictorInitialState;
    PredictorInput : Control.SmithPredictorInput;
    PredictorResult : Control.SmithPredictorOutput;
    FeedforwardEnabled : BOOL := FALSE;
    PredictionEnabled : BOOL := FALSE;
    LastAppliedHeaterPower : LREAL := 0.0;
    Configured : BOOL := FALSE;
END_VAR
VAR_OUTPUT
    TemperatureSetpoint : LREAL := 20.0;
    MeasuredTemperature : LREAL := 20.0;
    PredictedTemperature : LREAL := 20.0;
    DelayFreeModel : LREAL := 20.0;
    DelayedModel : LREAL := 20.0;
    ModelCorrection : LREAL := 0.0;
    UpstreamCompensation : LREAL := 0.0;
    RequestedHeaterPower : LREAL := 0.0;
    AppliedHeaterPower : LREAL := 0.0;
    HistoryComplete : BOOL := FALSE;
    PredictionActive : BOOL := FALSE;
    PredictionFallback : BOOL := FALSE;
    TrackingActive : BOOL := TRUE;
    FaultActive : BOOL := FALSE;
END_VAR

METHOD PUBLIC Configure
VAR_INPUT
    UseFeedforward : BOOL;
    UsePrediction : BOOL;
    InitialTemperature : LREAL;
END_VAR

FeedforwardEnabled := UseFeedforward;
PredictionEnabled := UsePrediction;
IF PredictionEnabled THEN
    ControllerParameters.ProportionalGain := LREAL#2.5;
    ControllerParameters.IntegralGain := LREAL#0.2083333;
ELSE
    ControllerParameters.ProportionalGain := LREAL#0.46875;
    ControllerParameters.IntegralGain := LREAL#0.0390625;
END_IF;
ControllerParameters.DerivativeGain := LREAL#0.0;
ControllerParameters.ProportionalSetpointWeight := LREAL#1.0;
ControllerParameters.DerivativeSetpointWeight := LREAL#0.0;
ControllerParameters.Bias := LREAL#0.0;
ControllerParameters.OutputMinimum := LREAL#0.0;
ControllerParameters.OutputMaximum := LREAL#100.0;
ControllerParameters.RateMinimum := LREAL#-10.0;
ControllerParameters.RateMaximum := LREAL#10.0;
ControllerParameters.TrackingTime := LREAL#2.0;
ControllerParameters.AntiWindup := PIDAntiWindup#Combined;
TemperatureController.Configure(Parameters := ControllerParameters);
InitialControllerState.Setpoint := InitialTemperature;
InitialControllerState.ProcessValue := InitialTemperature;
InitialControllerState.NominalOutput := LREAL#0.0;
InitialControllerState.SaturatedOutput := LREAL#0.0;
TemperatureController.Initialize(State := InitialControllerState);

PredictorParameters.ProcessGain := LREAL#0.8;
PredictorParameters.TimeConstant := LREAL#12.0;
PredictorParameters.DeadTime := LREAL#16.0;
PredictorParameters.OperatingInput := LREAL#0.0;
PredictorParameters.OperatingOutput := LREAL#20.0;
PredictorParameters.MaximumCorrection := LREAL#5.0;
Predictor.Configure(Parameters := PredictorParameters);
InitialPredictorState.ModelOutput := InitialTemperature;
InitialPredictorState.ProcessValue := InitialTemperature;
Predictor.Initialize(State := InitialPredictorState);

TemperatureSetpoint := InitialTemperature;
MeasuredTemperature := InitialTemperature;
PredictedTemperature := InitialTemperature;
Configured := TRUE;
END_METHOD

METHOD PUBLIC Update : BOOL
VAR_INPUT
    OperatingMode : DINT;
    RequestedTemperature : LREAL;
    TemperatureMeasurement : LREAL;
    UpstreamTemperatureDeviation : LREAL;
    UpstreamQualityGood : BOOL;
    TemperatureQualityGood : BOOL;
    ModelValid : BOOL;
    HeaterAvailable : BOOL;
    ManualHeaterPower : LREAL;
    CycleTime : LREAL;
END_VAR
VAR
    PredictionEvidenceValid : BOOL;
END_VAR

IF NOT Configured THEN
    Update := FALSE;
ELSE
    TemperatureSetpoint := RequestedTemperature;
    MeasuredTemperature := TemperatureMeasurement;

    PredictorInput.ManipulatedValue := LastAppliedHeaterPower;
    PredictorInput.ProcessValue := TemperatureMeasurement;
    PredictorInput.CycleTime := CycleTime;
    PredictorInput.Enabled := PredictionEnabled;
    PredictorResult := Predictor.Update(Input := PredictorInput);
    DelayFreeModel := PredictorResult.DelayFreeModel;
    DelayedModel := PredictorResult.DelayedModel;
    ModelCorrection := PredictorResult.Correction;
    HistoryComplete := PredictorResult.HistoryComplete;
    PredictionEvidenceValid := ModelValid AND TemperatureQualityGood
        AND HistoryComplete
        AND (TemperatureMeasurement >= LREAL#15.0)
        AND (TemperatureMeasurement <= LREAL#60.0)
        AND (ABS(ModelCorrection) < LREAL#5.0 - LREAL#1.0e-9);
    PredictionActive := PredictionEnabled AND PredictorResult.Active
        AND PredictionEvidenceValid;
    PredictionFallback := PredictionEnabled AND (NOT PredictionActive);
    IF PredictionActive THEN
        PredictedTemperature := PredictorResult.PredictedProcessValue;
    ELSE
        (* Prediction never outranks invalid evidence. Measured downstream
           temperature owns fallback while the physical delay remains. *)
        PredictedTemperature := TemperatureMeasurement;
    END_IF;

    UpstreamCompensation := LREAL#0.0;
    IF FeedforwardEnabled AND UpstreamQualityGood THEN
        (* The qualified upstream signal moves one transport delay before its
           load reaches the product; feedback trim remains active. *)
        UpstreamCompensation := LREAL#-1.25 * UpstreamTemperatureDeviation;
    END_IF;

    FaultActive := (NOT TemperatureQualityGood) OR (NOT HeaterAvailable)
        OR (CycleTime <= LREAL#0.0)
        OR (TemperatureMeasurement < LREAL#15.0)
        OR (TemperatureMeasurement > LREAL#60.0);
    ControllerInput.Setpoint := RequestedTemperature;
    ControllerInput.ProcessValue := PredictedTemperature;
    ControllerInput.FeedForward := UpstreamCompensation;
    ControllerInput.ManualOutput := LastAppliedHeaterPower;
    ControllerInput.TrackingOutput := LastAppliedHeaterPower;
    ControllerInput.CycleTime := CycleTime;
    TrackingActive := FALSE;

    IF FaultActive OR (OperatingMode = DINT#0) THEN
        ControllerInput.Setpoint := PredictedTemperature;
        ControllerInput.FeedForward := LREAL#0.0;
        ControllerInput.Mode := PIDMode#Track;
        ControllerInput.TrackingOutput := LREAL#0.0;
        TemperatureController.Update(Input := ControllerInput);
        RequestedHeaterPower := LREAL#0.0;
        AppliedHeaterPower := LREAL#0.0;
        TrackingActive := TRUE;
    ELSIF OperatingMode = DINT#2 THEN
        ControllerInput.Setpoint := PredictedTemperature;
        ControllerInput.FeedForward := LREAL#0.0;
        ControllerInput.Mode := PIDMode#Track;
        ControllerInput.TrackingOutput := LastAppliedHeaterPower;
        TemperatureController.Update(Input := ControllerInput);
        RequestedHeaterPower := LIMIT(LREAL#0.0, ManualHeaterPower, LREAL#100.0);
        AppliedHeaterPower := LIMIT(
            LastAppliedHeaterPower - LREAL#1.0,
            RequestedHeaterPower,
            LastAppliedHeaterPower + LREAL#1.0
        );
        TrackingActive := TRUE;
    ELSIF OperatingMode = DINT#3 THEN
        ControllerInput.Setpoint := PredictedTemperature;
        ControllerInput.FeedForward := LREAL#0.0;
        ControllerInput.Mode := PIDMode#Track;
        ControllerInput.TrackingOutput := LastAppliedHeaterPower;
        TemperatureController.Update(Input := ControllerInput);
        RequestedHeaterPower := LREAL#0.0;
        AppliedHeaterPower := MAX(LREAL#0.0, LastAppliedHeaterPower - LREAL#1.0);
        TrackingActive := TRUE;
    ELSE
        ControllerInput.Mode := PIDMode#Auto;
        RequestedHeaterPower := TemperatureController.Update(Input := ControllerInput);
        AppliedHeaterPower := RequestedHeaterPower;
    END_IF;

    LastAppliedHeaterPower := AppliedHeaterPower;
    Update := NOT FaultActive;
END_IF;
END_METHOD
END_FUNCTION_BLOCK
