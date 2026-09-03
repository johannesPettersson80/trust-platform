USING Control;

PROGRAM Main
VAR
    Controller : Control.PID;
    Predictor : Control.SmithPredictorFOPDT;
    Parameters : Control.PIDParameters;
    InitialState : Control.PIDInitialState;
    Input : Control.PIDInput;
    PredictorParameters : Control.SmithPredictorParameters;
    PredictorState : Control.SmithPredictorInitialState;
    PredictorInput : Control.SmithPredictorInput;
    PredictorResult : Control.SmithPredictorOutput;
    Initialized : BOOL := FALSE;
    ExperimentEnabled : BOOL := FALSE;
    SafetyAbort : BOOL := FALSE;
    InputsValid : BOOL := TRUE;
    ExperimentElapsed : LREAL := 0.0;
    AutoMode : BOOL := TRUE;
    PredictorEnabled : BOOL := TRUE;
    PredictorActive : BOOL := FALSE;
    PredictorFallback : BOOL := FALSE;
    ModelValid : BOOL := FALSE;
    TemperatureQualityValid : BOOL := TRUE;
    PredictorHistoryComplete : BOOL := FALSE;
    Setpoint : LREAL := 20.0;
    ProcessValue : LREAL := 20.0;
    ManipulatedValue : LREAL := 0.0;
    PredictedProcessValue : LREAL := 20.0;
    DelayFreeModel : LREAL := 20.0;
    DelayedModel : LREAL := 20.0;
    ModelCorrection : LREAL := 0.0;
    FeedbackProportionalGain : LREAL := 0.46875;
    FeedbackIntegralGain : LREAL := 0.0390625;
    SmithProportionalGain : LREAL := 1.875;
    SmithIntegralGain : LREAL := 0.15625;
    SelectedProportionalGain : LREAL := 0.46875;
    SelectedIntegralGain : LREAL := 0.0390625;
END_VAR

IF NOT Initialized THEN
    (* Ordinary feedback and Smith prediction are separate restored designs.
       Their gains are selected once at initialization, never jumped live.
       Do not overwrite mapped mode, predictor, quality, or setpoint values here:
       Control Studio must be able to establish the first-scan experiment. *)
    IF PredictorEnabled THEN
        SelectedProportionalGain := SmithProportionalGain;
        SelectedIntegralGain := SmithIntegralGain;
    ELSE
        SelectedProportionalGain := FeedbackProportionalGain;
        SelectedIntegralGain := FeedbackIntegralGain;
    END_IF;
    Parameters.ProportionalGain := SelectedProportionalGain;
    Parameters.IntegralGain := SelectedIntegralGain;
    Parameters.DerivativeGain := LREAL#0.0;
    Parameters.ProportionalSetpointWeight := LREAL#1.0;
    Parameters.DerivativeSetpointWeight := LREAL#0.0;
    Parameters.Bias := LREAL#0.0;
    Parameters.OutputMinimum := LREAL#0.0;
    Parameters.OutputMaximum := LREAL#100.0;
    Parameters.RateMinimum := LREAL#-10.0;
    Parameters.RateMaximum := LREAL#10.0;
    Parameters.TrackingTime := LREAL#2.0;
    Parameters.AntiWindup := PIDAntiWindup#Combined;
    InitialState.Setpoint := Setpoint;
    InitialState.ProcessValue := ProcessValue;
    InitialState.NominalOutput := LREAL#0.0;
    InitialState.SaturatedOutput := LREAL#0.0;
    Controller.Configure(Parameters := Parameters);
    Controller.Initialize(State := InitialState);

    PredictorParameters.ProcessGain := LREAL#0.8;
    PredictorParameters.TimeConstant := LREAL#12.0;
    PredictorParameters.DeadTime := LREAL#16.0;
    PredictorParameters.OperatingInput := LREAL#0.0;
    PredictorParameters.OperatingOutput := LREAL#20.0;
    PredictorParameters.MaximumCorrection := LREAL#5.0;
    Predictor.Configure(Parameters := PredictorParameters);
    PredictorState.ModelOutput := ProcessValue;
    PredictorState.ProcessValue := ProcessValue;
    Predictor.Initialize(State := PredictorState);
    Initialized := TRUE;
END_IF;

(* Control Studio grants only authority. The project owns the temperature
   request and restores the operating point on every exit. *)
IF ExperimentEnabled AND InputsValid AND NOT SafetyAbort THEN
    ExperimentElapsed := ExperimentElapsed + LREAL#0.1;
    IF ExperimentElapsed < LREAL#10.0 THEN
        Setpoint := LREAL#20.0;
    ELSE
        Setpoint := LREAL#35.0;
    END_IF;
ELSE
    ExperimentElapsed := LREAL#0.0;
    Setpoint := LREAL#20.0;
END_IF;

PredictorInput.ManipulatedValue := ManipulatedValue;
PredictorInput.ProcessValue := ProcessValue;
PredictorInput.CycleTime := LREAL#0.1;
PredictorInput.Enabled := PredictorEnabled;
PredictorResult := Predictor.Update(Input := PredictorInput);
DelayFreeModel := PredictorResult.DelayFreeModel;
DelayedModel := PredictorResult.DelayedModel;
ModelCorrection := PredictorResult.Correction;
PredictorHistoryComplete := PredictorResult.HistoryComplete;
ModelValid := TemperatureQualityValid
    AND PredictorHistoryComplete
    AND (ProcessValue >= LREAL#15.0) AND (ProcessValue <= LREAL#60.0)
    AND (ManipulatedValue >= LREAL#0.0) AND (ManipulatedValue <= LREAL#100.0)
    AND (ABS(ModelCorrection)
        < PredictorParameters.MaximumCorrection - LREAL#1.0e-9);
PredictorActive := PredictorResult.Active AND ModelValid;
PredictorFallback := PredictorEnabled AND NOT PredictorActive;
IF PredictorActive THEN
    PredictedProcessValue := PredictorResult.PredictedProcessValue;
ELSE
    (* A bounded internal model never outranks invalid or unqualified measured
       data. Ordinary measured feedback deterministically owns fallback. *)
    PredictedProcessValue := ProcessValue;
END_IF;

Input.Setpoint := Setpoint;
Input.ProcessValue := PredictedProcessValue;
IF AutoMode THEN
    Input.Mode := PIDMode#Auto;
ELSE
    Input.Mode := PIDMode#Manual;
    Input.ManualOutput := LREAL#0.0;
END_IF;
Input.CycleTime := LREAL#0.1;
ManipulatedValue := Controller.Update(Input := Input);
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.AutoMode AT %MX90.0 : BOOL;
    P1.PredictorEnabled AT %MX90.1 : BOOL;
    P1.PredictorActive AT %MX90.2 : BOOL;
    P1.PredictorFallback AT %MX90.3 : BOOL;
    P1.ModelValid AT %MX90.4 : BOOL;
    P1.TemperatureQualityValid AT %MX90.5 : BOOL;
    P1.PredictorHistoryComplete AT %MX90.6 : BOOL;
    P1.ExperimentEnabled AT %MX90.7 : BOOL;
    P1.SafetyAbort AT %MX91.0 : BOOL;
    P1.InputsValid AT %MX91.1 : BOOL;
    P1.Setpoint AT %ML272 : LREAL;
    P1.PredictedProcessValue AT %ML280 : LREAL;
    P1.DelayFreeModel AT %ML288 : LREAL;
    P1.DelayedModel AT %ML296 : LREAL;
    P1.ModelCorrection AT %ML304 : LREAL;
    P1.SelectedProportionalGain AT %ML312 : LREAL;
    P1.SelectedIntegralGain AT %ML320 : LREAL;
    P1.ProcessValue AT %IL160 : LREAL;
    P1.ManipulatedValue AT %QL176 : LREAL;
END_VAR
END_CONFIGURATION
