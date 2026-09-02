USING Control;

PROGRAM Main
VAR
    FeedbackController : Control.PID;
    RatioController : Control.PID;
    Parameters : Control.PIDParameters;
    FeedbackState : Control.PIDInitialState;
    RatioState : Control.PIDInitialState;
    FeedbackInput : Control.PIDInput;
    RatioInput : Control.PIDInput;
    Initialized : BOOL := FALSE;
    WildFlowValid : BOOL := TRUE;
    RatioAuthority : BOOL := TRUE;
    RatioSetpoint : LREAL := 0.30;
    ActualWildFlow : LREAL := 100.0;
    MeasuredWildFlow : LREAL := 100.0;
    FeedbackRatio : LREAL := 0.30;
    RatioControlledRatio : LREAL := 0.30;
    RatioDemand : LREAL := 30.0;
    RatioTrim : LREAL := 0.0;
    FeedbackAdditiveRequest : LREAL := 30.0;
    FeedbackWildFlowInput : LREAL := 100.0;
    RatioAdditiveRequest : LREAL := 30.0;
    RatioWildFlowInput : LREAL := 100.0;
    AutomaticMode : BOOL := TRUE;
END_VAR

RatioAuthority := WildFlowValid
    AND MeasuredWildFlow >= LREAL#10.0
    AND MeasuredWildFlow <= LREAL#160.0;
IF RatioAuthority THEN
    RatioDemand := RatioSetpoint * MeasuredWildFlow;
ELSE
    RatioDemand := LREAL#0.0;
END_IF;

IF NOT Initialized THEN
    RatioSetpoint := LREAL#0.30;
    ActualWildFlow := LREAL#100.0;
    Parameters.ProportionalGain := LREAL#50.0;
    Parameters.IntegralGain := LREAL#10.0;
    Parameters.DerivativeGain := LREAL#0.0;
    Parameters.ProportionalSetpointWeight := LREAL#1.0;
    Parameters.DerivativeSetpointWeight := LREAL#0.0;
    Parameters.Bias := LREAL#0.0;
    Parameters.OutputMinimum := LREAL#0.0;
    Parameters.OutputMaximum := LREAL#60.0;
    Parameters.RateMinimum := LREAL#-1000.0;
    Parameters.RateMaximum := LREAL#1000.0;
    Parameters.TrackingTime := LREAL#1.0;
    Parameters.AntiWindup := PIDAntiWindup#Combined;

    FeedbackState.Setpoint := RatioSetpoint;
    FeedbackState.ProcessValue := FeedbackRatio;
    FeedbackState.FeedForward := LREAL#0.0;
    FeedbackState.NominalOutput := LREAL#30.0;
    FeedbackState.SaturatedOutput := LREAL#30.0;
    RatioState.Setpoint := RatioSetpoint;
    RatioState.ProcessValue := RatioControlledRatio;
    RatioState.FeedForward := RatioDemand;
    RatioState.NominalOutput := LREAL#30.0;
    RatioState.SaturatedOutput := LREAL#30.0;
    FeedbackController.Configure(Parameters := Parameters);
    RatioController.Configure(Parameters := Parameters);
    FeedbackController.Initialize(State := FeedbackState);
    RatioController.Initialize(State := RatioState);
    Initialized := TRUE;
END_IF;

FeedbackWildFlowInput := ActualWildFlow;
RatioWildFlowInput := ActualWildFlow;

FeedbackInput.Setpoint := RatioSetpoint;
FeedbackInput.ProcessValue := FeedbackRatio;
FeedbackInput.FeedForward := LREAL#0.0;
FeedbackInput.Mode := PIDMode#Auto;
FeedbackInput.CycleTime := LREAL#0.1;
FeedbackAdditiveRequest := FeedbackController.Update(Input := FeedbackInput);

RatioInput.Setpoint := RatioSetpoint;
RatioInput.ProcessValue := RatioControlledRatio;
RatioInput.CycleTime := LREAL#0.1;
IF RatioAuthority THEN
    RatioInput.FeedForward := RatioDemand;
    RatioInput.Mode := PIDMode#Auto;
ELSE
    (* Near zero flow, a reported ratio is physically ill-conditioned. Track
       the reviewed safe request instead of integrating analyser noise. *)
    RatioInput.Setpoint := RatioControlledRatio;
    RatioInput.FeedForward := LREAL#0.0;
    RatioInput.Mode := PIDMode#Track;
    RatioInput.TrackingOutput := LREAL#0.0;
END_IF;
RatioAdditiveRequest := RatioController.Update(Input := RatioInput);
RatioTrim := RatioAdditiveRequest - RatioDemand;
AutomaticMode := TRUE;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.WildFlowValid AT %MX88.2 : BOOL;
    P1.RatioAuthority AT %MX88.3 : BOOL;
    P1.AutomaticMode AT %MX160.0 : BOOL;
    P1.RatioSetpoint AT %ML120 : LREAL;
    P1.ActualWildFlow AT %ML128 : LREAL;
    P1.MeasuredWildFlow AT %ML136 : LREAL;
    P1.RatioDemand AT %ML144 : LREAL;
    P1.RatioTrim AT %ML152 : LREAL;
    P1.FeedbackRatio AT %IL48 : LREAL;
    P1.RatioControlledRatio AT %IL56 : LREAL;
    P1.FeedbackAdditiveRequest AT %QL48 : LREAL;
    P1.FeedbackWildFlowInput AT %QL56 : LREAL;
    P1.RatioAdditiveRequest AT %QL64 : LREAL;
    P1.RatioWildFlowInput AT %QL72 : LREAL;
END_VAR
END_CONFIGURATION
