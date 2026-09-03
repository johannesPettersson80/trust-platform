USING Control;

PROGRAM Main
VAR
    FeedbackController : Control.PID;
    StaticController : Control.PID;
    DynamicController : Control.PID;
    Parameters : Control.PIDParameters;
    InitialState : Control.PIDInitialState;
    FeedbackInput : Control.PIDInput;
    StaticInput : Control.PIDInput;
    DynamicInput : Control.PIDInput;
    DisturbanceFilter : Control.LowPass;
    FilterParameters : Control.LowPassParameters;
    Initialized : BOOL := FALSE;
    ExperimentEnabled : BOOL := FALSE;
    SafetyAbort : BOOL := FALSE;
    InputsValid : BOOL := TRUE;
    ExperimentElapsed : LREAL := 0.0;
    SpeedValid : BOOL := TRUE;
    Setpoint : LREAL := 120.0;
    ActualLineSpeed : LREAL := 1.0;
    MeasuredLineSpeed : LREAL := 1.0;
    DynamicFeedforwardScale : LREAL := 1.0;
    DisturbanceDeviation : LREAL := 0.0;
    FilteredDisturbance : LREAL := 0.0;
    StaticFeedforward : LREAL := 0.0;
    DynamicFeedforward : LREAL := 0.0;
    FeedbackTemperature : LREAL := 120.0;
    StaticTemperature : LREAL := 120.0;
    DynamicTemperature : LREAL := 120.0;
    FeedbackHeaterRequest : LREAL := 70.0;
    FeedbackSpeedInput : LREAL := 1.0;
    StaticHeaterRequest : LREAL := 70.0;
    StaticSpeedInput : LREAL := 1.0;
    DynamicHeaterRequest : LREAL := 70.0;
    DynamicSpeedInput : LREAL := 1.0;
    AutomaticMode : BOOL := TRUE;
END_VAR

IF NOT Initialized THEN
    Setpoint := LREAL#120.0;
    ActualLineSpeed := LREAL#1.0;
    Parameters.ProportionalGain := LREAL#0.8;
    Parameters.IntegralGain := LREAL#0.08;
    Parameters.DerivativeGain := LREAL#0.0;
    Parameters.ProportionalSetpointWeight := LREAL#1.0;
    Parameters.DerivativeSetpointWeight := LREAL#0.0;
    Parameters.Bias := LREAL#70.0;
    Parameters.OutputMinimum := LREAL#0.0;
    Parameters.OutputMaximum := LREAL#100.0;
    Parameters.RateMinimum := LREAL#-1000.0;
    Parameters.RateMaximum := LREAL#1000.0;
    Parameters.TrackingTime := LREAL#2.0;
    Parameters.AntiWindup := PIDAntiWindup#Combined;

    InitialState.Setpoint := Setpoint;
    InitialState.ProcessValue := Setpoint;
    InitialState.FeedForward := LREAL#0.0;
    InitialState.NominalOutput := LREAL#70.0;
    InitialState.SaturatedOutput := LREAL#70.0;
    FeedbackController.Configure(Parameters := Parameters);
    StaticController.Configure(Parameters := Parameters);
    DynamicController.Configure(Parameters := Parameters);
    FeedbackController.Initialize(State := InitialState);
    StaticController.Initialize(State := InitialState);
    DynamicController.Initialize(State := InitialState);

    (* The disturbance-path filter belongs to the feedforward model. The
       measured oven temperature remains unfiltered and visible. *)
    FilterParameters.TimeConstant := LREAL#4.0;
    FilterParameters.Order := FilterOrder#FirstOrder;
    DisturbanceFilter.Configure(Parameters := FilterParameters);
    DisturbanceFilter.Initialize(Input := LREAL#0.0);
    Initialized := TRUE;
END_IF;

IF ExperimentEnabled AND (NOT SafetyAbort) AND InputsValid THEN
    ExperimentElapsed := ExperimentElapsed + LREAL#0.1;
    IF ExperimentElapsed >= LREAL#30.0 THEN
        ActualLineSpeed := LREAL#2.0;
    ELSE
        ActualLineSpeed := LREAL#1.0;
    END_IF;
ELSE
    ExperimentElapsed := LREAL#0.0;
    ActualLineSpeed := LREAL#1.0;
END_IF;
MeasuredLineSpeed := ActualLineSpeed;

FeedbackSpeedInput := ActualLineSpeed;
StaticSpeedInput := ActualLineSpeed;
DynamicSpeedInput := ActualLineSpeed;

IF SpeedValid THEN
    DisturbanceDeviation := MeasuredLineSpeed - LREAL#1.0;
    StaticFeedforward := LREAL#10.0 * DisturbanceDeviation;
ELSE
    (* Invalid disturbance data removes feedforward authority while the three
       ordinary feedback controllers continue to operate. *)
    DisturbanceDeviation := LREAL#0.0;
    StaticFeedforward := LREAL#0.0;
END_IF;

IF SpeedValid THEN
    DynamicFeedforward := DynamicFeedforwardScale
        * (LREAL#25.0 * DisturbanceDeviation - LREAL#15.0 * FilteredDisturbance);
ELSE
    DynamicFeedforward := LREAL#0.0;
END_IF;
(* Use the retained filter state for this command, then advance it for the next
   controller sample. This preserves the predicted 25-point first action. *)
FilteredDisturbance := DisturbanceFilter.Update(
    Input := DisturbanceDeviation,
    CycleTime := LREAL#0.1
);

FeedbackInput.Setpoint := Setpoint;
FeedbackInput.ProcessValue := FeedbackTemperature;
FeedbackInput.FeedForward := LREAL#0.0;
FeedbackInput.Mode := PIDMode#Auto;
FeedbackInput.CycleTime := LREAL#0.1;
FeedbackHeaterRequest := FeedbackController.Update(Input := FeedbackInput);

StaticInput.Setpoint := Setpoint;
StaticInput.ProcessValue := StaticTemperature;
StaticInput.FeedForward := StaticFeedforward;
StaticInput.Mode := PIDMode#Auto;
StaticInput.CycleTime := LREAL#0.1;
StaticHeaterRequest := StaticController.Update(Input := StaticInput);

DynamicInput.Setpoint := Setpoint;
DynamicInput.ProcessValue := DynamicTemperature;
DynamicInput.FeedForward := DynamicFeedforward;
DynamicInput.Mode := PIDMode#Auto;
DynamicInput.CycleTime := LREAL#0.1;
DynamicHeaterRequest := DynamicController.Update(Input := DynamicInput);
AutomaticMode := TRUE;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.SpeedValid AT %MX136.0 : BOOL;
    P1.AutomaticMode AT %MX136.1 : BOOL;
    P1.ExperimentEnabled AT %MX136.2 : BOOL;
    P1.SafetyAbort AT %MX136.3 : BOOL;
    P1.InputsValid AT %MX136.4 : BOOL;
    P1.Setpoint AT %ML88 : LREAL;
    P1.ActualLineSpeed AT %ML96 : LREAL;
    P1.MeasuredLineSpeed AT %ML104 : LREAL;
    P1.DynamicFeedforwardScale AT %ML112 : LREAL;
    P1.StaticFeedforward AT %ML120 : LREAL;
    P1.DynamicFeedforward AT %ML128 : LREAL;
    P1.FeedbackTemperature AT %IL0 : LREAL;
    P1.StaticTemperature AT %IL8 : LREAL;
    P1.DynamicTemperature AT %IL16 : LREAL;
    P1.FeedbackHeaterRequest AT %QL0 : LREAL;
    P1.FeedbackSpeedInput AT %QL8 : LREAL;
    P1.StaticHeaterRequest AT %QL16 : LREAL;
    P1.StaticSpeedInput AT %QL24 : LREAL;
    P1.DynamicHeaterRequest AT %QL32 : LREAL;
    P1.DynamicSpeedInput AT %QL40 : LREAL;
END_VAR
END_CONFIGURATION
